use std::io::{Read, Write};

use super::models::{CdEntry, RepairError, RepairedEntry, ScanResult};
use super::zip_scanner::ZipScanner;

pub struct ZipRepairer;

impl ZipRepairer {
    /// Reconstruct Central Directory from discovered Local File Headers.
    /// Returns the rebuilt CD entries and the byte ranges of file data.
    pub fn reconstruct_cd(data: &[u8], lfh_offsets: &[u64]) -> Vec<CdEntry> {
        let mut cd_entries = Vec::new();

        for &offset in lfh_offsets {
            // Parse the LFH at this offset; skip if unparseable
            let lfh = match ZipScanner::parse_lfh(data, offset) {
                Some(h) => h,
                None => continue,
            };

            // Determine compressed size: use LFH value if non-zero, otherwise infer
            let compressed_size = if lfh.compressed_size == 0 {
                // Infer from scanning forward for next boundary
                let inferred = Self::infer_data_size(data, lfh.data_offset);
                // Cap to u32::MAX since CdEntry uses u32
                if inferred > u32::MAX as u64 {
                    u32::MAX
                } else {
                    inferred as u32
                }
            } else {
                lfh.compressed_size
            };

            cd_entries.push(CdEntry {
                version_made_by: 20,
                version_needed: lfh.version_needed,
                flags: lfh.flags,
                compression_method: lfh.compression_method,
                last_mod_time: lfh.last_mod_time,
                last_mod_date: lfh.last_mod_date,
                crc32: lfh.crc32,
                compressed_size,
                uncompressed_size: lfh.uncompressed_size,
                filename: lfh.filename,
                extra_field: lfh.extra_field,
                comment: Vec::new(),
                disk_number_start: 0,
                internal_attrs: 0,
                external_attrs: 0,
                local_header_offset: offset as u32,
            });
        }

        cd_entries
    }

    /// Infer compressed data size when the size field is invalid.
    /// Scans forward for next LFH signature, CD signature, EOCD signature, or EOF.
    pub fn infer_data_size(data: &[u8], data_start: u64) -> u64 {
        let start = data_start as usize;

        // If data_start is beyond the buffer, size is 0
        if start >= data.len() {
            return 0;
        }

        // Scan forward for the next boundary signature
        let search_region = &data[start..];

        // Look for PK\x03\x04 (LFH), PK\x01\x02 (CD), PK\x05\x06 (EOCD)
        for i in 0..search_region.len().saturating_sub(3) {
            if search_region[i] == 0x50 && search_region[i + 1] == 0x4B {
                let sig_type = search_region[i + 2];
                let sig_sub = search_region[i + 3];
                // LFH: PK\x03\x04
                // CD:  PK\x01\x02
                // EOCD: PK\x05\x06
                if (sig_type == 0x03 && sig_sub == 0x04)
                    || (sig_type == 0x01 && sig_sub == 0x02)
                    || (sig_type == 0x05 && sig_sub == 0x06)
                {
                    return i as u64;
                }
            }
        }

        // No boundary found — data extends to EOF
        (data.len() - start) as u64
    }

    /// Recalculate CRC-32 for an entry's decompressed data.
    /// - method 0 (STORED): data is uncompressed, compute CRC-32 directly
    /// - method 8 (DEFLATED): decompress using flate2, then compute CRC-32
    /// - Other methods: return None (unsupported compression)
    pub fn recalculate_crc32(compressed_data: &[u8], method: u16) -> Option<u32> {
        match method {
            0 => {
                // STORED: data is already uncompressed
                Some(crc32fast::hash(compressed_data))
            }
            8 => {
                // DEFLATED: decompress first, then compute CRC-32
                let mut decoder =
                    flate2::read::DeflateDecoder::new(compressed_data);
                let mut decompressed = Vec::new();
                match decoder.read_to_end(&mut decompressed) {
                    Ok(_) => Some(crc32fast::hash(&decompressed)),
                    Err(_) => None,
                }
            }
            _ => {
                // Unsupported compression method
                None
            }
        }
    }

    /// Write a repaired ZIP archive to the output writer.
    /// Copies valid file data, writes reconstructed CD and EOCD.
    /// Invokes progress_fn(current_step, total_steps, entry_name) for each entry processed.
    /// Returns ScanResult with recovered_entries and failed_entries.
    pub fn write_repaired<W: Write, F>(
        data: &[u8],
        entries: &[RepairedEntry],
        writer: &mut W,
        progress_fn: F,
    ) -> Result<ScanResult, RepairError>
    where
        F: Fn(u32, u32, &str),
    {
        let total_steps = entries.len() as u32;
        let mut recovered_entries: Vec<String> = Vec::new();
        let mut failed_entries: Vec<String> = Vec::new();
        // Track the output offset of each LFH for CD construction
        let mut lfh_output_offsets: Vec<u32> = Vec::new();
        let mut current_output_offset: u32 = 0;

        // Phase 1: Write Local File Headers + compressed data for each entry
        for (idx, entry) in entries.iter().enumerate() {
            let entry_name = String::from_utf8_lossy(&entry.filename).to_string();

            // Invoke progress callback
            progress_fn((idx as u32) + 1, total_steps, &entry_name);

            // Validate that we can read the compressed data from the source
            let data_start = entry.data_offset as usize;
            let data_end = data_start + entry.compressed_size as usize;

            if data_end > data.len() {
                // Can't read this entry's data — mark as failed
                failed_entries.push(entry_name);
                lfh_output_offsets.push(0); // placeholder
                continue;
            }

            let compressed_data = &data[data_start..data_end];

            // Record the output offset where this LFH starts
            lfh_output_offsets.push(current_output_offset);

            // Write Local File Header (30 bytes fixed + filename)
            let filename_len = entry.filename.len() as u16;
            let extra_field_len: u16 = 0;

            // LFH signature: PK\x03\x04
            let mut lfh_buf: Vec<u8> = Vec::with_capacity(30 + entry.filename.len());
            lfh_buf.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]); // signature
            lfh_buf.extend_from_slice(&20u16.to_le_bytes()); // version needed (2.0)
            lfh_buf.extend_from_slice(&0u16.to_le_bytes()); // flags
            lfh_buf.extend_from_slice(&entry.compression_method.to_le_bytes());
            lfh_buf.extend_from_slice(&entry.last_mod_time.to_le_bytes());
            lfh_buf.extend_from_slice(&entry.last_mod_date.to_le_bytes());
            lfh_buf.extend_from_slice(&entry.crc32.to_le_bytes());
            lfh_buf.extend_from_slice(&entry.compressed_size.to_le_bytes());
            lfh_buf.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
            lfh_buf.extend_from_slice(&filename_len.to_le_bytes());
            lfh_buf.extend_from_slice(&extra_field_len.to_le_bytes());
            lfh_buf.extend_from_slice(&entry.filename);

            if let Err(e) = writer.write_all(&lfh_buf) {
                return Err(RepairError::IoError(format!(
                    "Failed to write LFH for '{entry_name}': {e}"
                )));
            }

            // Write compressed data
            if let Err(e) = writer.write_all(compressed_data) {
                return Err(RepairError::IoError(format!(
                    "Failed to write data for '{entry_name}': {e}"
                )));
            }

            current_output_offset += lfh_buf.len() as u32 + entry.compressed_size;
            recovered_entries.push(entry_name);
        }

        // Phase 2: Write Central Directory entries
        let cd_start_offset = current_output_offset;
        let mut cd_size: u32 = 0;
        let mut cd_entry_count: u16 = 0;

        for (idx, entry) in entries.iter().enumerate() {
            let entry_name = String::from_utf8_lossy(&entry.filename).to_string();

            // Skip entries that failed (not in recovered_entries)
            if !recovered_entries.contains(&entry_name) {
                continue;
            }

            let lfh_offset = lfh_output_offsets[idx];
            let filename_len = entry.filename.len() as u16;
            let extra_field_len: u16 = 0;
            let comment_len: u16 = 0;

            // CD entry: 46 bytes fixed + filename
            let mut cd_buf: Vec<u8> = Vec::with_capacity(46 + entry.filename.len());
            cd_buf.extend_from_slice(&[0x50, 0x4B, 0x01, 0x02]); // CD signature
            cd_buf.extend_from_slice(&20u16.to_le_bytes()); // version made by
            cd_buf.extend_from_slice(&20u16.to_le_bytes()); // version needed
            cd_buf.extend_from_slice(&0u16.to_le_bytes()); // flags
            cd_buf.extend_from_slice(&entry.compression_method.to_le_bytes());
            cd_buf.extend_from_slice(&entry.last_mod_time.to_le_bytes());
            cd_buf.extend_from_slice(&entry.last_mod_date.to_le_bytes());
            cd_buf.extend_from_slice(&entry.crc32.to_le_bytes());
            cd_buf.extend_from_slice(&entry.compressed_size.to_le_bytes());
            cd_buf.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
            cd_buf.extend_from_slice(&filename_len.to_le_bytes());
            cd_buf.extend_from_slice(&extra_field_len.to_le_bytes());
            cd_buf.extend_from_slice(&comment_len.to_le_bytes());
            cd_buf.extend_from_slice(&0u16.to_le_bytes()); // disk number start
            cd_buf.extend_from_slice(&0u16.to_le_bytes()); // internal file attributes
            cd_buf.extend_from_slice(&0u32.to_le_bytes()); // external file attributes
            cd_buf.extend_from_slice(&lfh_offset.to_le_bytes()); // relative offset of LFH
            cd_buf.extend_from_slice(&entry.filename);

            if let Err(e) = writer.write_all(&cd_buf) {
                return Err(RepairError::IoError(format!(
                    "Failed to write CD entry for '{entry_name}': {e}"
                )));
            }

            cd_size += cd_buf.len() as u32;
            cd_entry_count += 1;
        }

        // Phase 3: Write End of Central Directory record (22 bytes fixed)
        let mut eocd_buf: Vec<u8> = Vec::with_capacity(22);
        eocd_buf.extend_from_slice(&[0x50, 0x4B, 0x05, 0x06]); // EOCD signature
        eocd_buf.extend_from_slice(&0u16.to_le_bytes()); // disk number
        eocd_buf.extend_from_slice(&0u16.to_le_bytes()); // disk where CD starts
        eocd_buf.extend_from_slice(&cd_entry_count.to_le_bytes()); // entries on this disk
        eocd_buf.extend_from_slice(&cd_entry_count.to_le_bytes()); // total entries
        eocd_buf.extend_from_slice(&cd_size.to_le_bytes()); // size of CD
        eocd_buf.extend_from_slice(&cd_start_offset.to_le_bytes()); // offset of CD start
        eocd_buf.extend_from_slice(&0u16.to_le_bytes()); // comment length

        if let Err(e) = writer.write_all(&eocd_buf) {
            return Err(RepairError::IoError(format!(
                "Failed to write EOCD: {e}"
            )));
        }

        let success = failed_entries.is_empty() && !recovered_entries.is_empty();

        Ok(ScanResult {
            success,
            recovered_entries,
            failed_entries,
            repaired_path: None, // Caller sets this after writing to a file
        })
    }
}
