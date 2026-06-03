use std::io::{self, Write};

use super::models::{RarHeaderInfo, RarVersion, RepairError, ScanResult};

/// RAR4 marker block: `Rar!\x1A\x07\x00` (7 bytes)
const RAR4_MARKER: &[u8] = b"Rar!\x1a\x07\x00";
/// RAR5 marker block: `Rar!\x1A\x07\x01\x00` (8 bytes)
const RAR5_MARKER: &[u8] = b"Rar!\x1a\x07\x01\x00";

/// RAR4 header types use values like 0x73 (archive header) and 0x74 (file header).
/// RAR5 header types use values like 1 (archive header) and 2 (file header).
/// We use this threshold to distinguish: RAR4 types are >= 0x70, RAR5 types are small integers.
const RAR4_TYPE_THRESHOLD: u8 = 0x70;

pub struct RarRepairer;

impl RarRepairer {
    /// Prepend the correct RAR marker block for the detected version.
    ///
    /// - For Rar4: writes `b"Rar!\x1a\x07\x00"` (7 bytes)
    /// - For Rar5: writes `b"Rar!\x1a\x07\x01\x00"` (8 bytes)
    pub fn prepend_marker<W: Write>(writer: &mut W, version: RarVersion) -> io::Result<()> {
        let marker = match version {
            RarVersion::Rar4 => RAR4_MARKER,
            RarVersion::Rar5 => RAR5_MARKER,
        };
        writer.write_all(marker)
    }

    /// Recalculate and fix header CRC for a RAR header.
    ///
    /// Determines the RAR version from the header_type field:
    /// - RAR4 headers have type values >= 0x70 (e.g., 0x73 archive, 0x74 file)
    /// - RAR5 headers have small type values (e.g., 1 archive, 2 file)
    ///
    /// For RAR4:
    ///   - Compute CRC32 of header bytes from type field (offset+2) to end of header (offset+header_size)
    ///   - Take lower 16 bits of the CRC32
    ///   - Write the 16-bit CRC back to data[offset..offset+2] in little-endian
    ///
    /// For RAR5:
    ///   - Compute CRC32 of header bytes after the 4-byte CRC field (offset+4 to offset+header_size)
    ///   - Write the full 32-bit CRC back to data[offset..offset+4] in little-endian
    pub fn fix_header_crc(data: &mut [u8], header: &RarHeaderInfo) {
        let offset = header.offset as usize;
        let header_size = header.header_size as usize;

        // Ensure we have enough data to work with
        if offset + header_size > data.len() {
            return;
        }

        if Self::is_rar4_header(header) {
            // RAR4: CRC32 of bytes from offset+2 (type field) to offset+header_size
            let crc_start = offset + 2;
            let crc_end = offset + header_size;
            if crc_start >= crc_end {
                return;
            }

            let computed_crc = crc32fast::hash(&data[crc_start..crc_end]);
            let crc16 = (computed_crc & 0xFFFF) as u16;
            let crc_bytes = crc16.to_le_bytes();
            data[offset] = crc_bytes[0];
            data[offset + 1] = crc_bytes[1];
        } else {
            // RAR5: CRC32 of bytes from offset+4 to offset+header_size
            let crc_start = offset + 4;
            let crc_end = offset + header_size;
            if crc_start >= crc_end || offset + 4 > data.len() {
                return;
            }

            let computed_crc = crc32fast::hash(&data[crc_start..crc_end]);
            let crc_bytes = computed_crc.to_le_bytes();
            data[offset] = crc_bytes[0];
            data[offset + 1] = crc_bytes[1];
            data[offset + 2] = crc_bytes[2];
            data[offset + 3] = crc_bytes[3];
        }
    }

    /// Reconstruct a minimal valid Archive Header with default flags.
    ///
    /// For RAR4:
    ///   - 2 bytes: header CRC (lower 16 bits of CRC32 of bytes from type to end)
    ///   - 1 byte: type = 0x73
    ///   - 2 bytes: flags = 0x0000
    ///   - 2 bytes: header size = 13
    ///   - 6 bytes: reserved (zeros)
    ///
    ///   Total = 13 bytes
    ///
    /// For RAR5:
    ///   - 4 bytes: CRC32 (of header data after this field: size vint + type vint + flags vint)
    ///   - 1 byte: header_size vint = 2 (type + flags = 2 bytes)
    ///   - 1 byte: header_type vint = 1 (archive header)
    ///   - 1 byte: header_flags vint = 0
    ///
    ///   Total = 7 bytes
    pub fn reconstruct_archive_header(version: RarVersion) -> Vec<u8> {
        match version {
            RarVersion::Rar4 => {
                // Build the header body (everything after the 2-byte CRC field)
                // type(1) + flags(2) + size(2) + reserved(6) = 11 bytes
                let mut body = Vec::with_capacity(11);
                body.push(0x73); // header type: archive header
                body.extend_from_slice(&0x0000u16.to_le_bytes()); // flags: no special flags
                body.extend_from_slice(&13u16.to_le_bytes()); // header size: 13 total
                body.extend_from_slice(&[0u8; 6]); // reserved bytes

                // Compute CRC32 of body, take lower 16 bits
                let crc = crc32fast::hash(&body);
                let crc16 = (crc & 0xFFFF) as u16;

                // Assemble full header: CRC(2) + body(11) = 13 bytes
                let mut header = Vec::with_capacity(13);
                header.extend_from_slice(&crc16.to_le_bytes());
                header.extend_from_slice(&body);
                header
            }
            RarVersion::Rar5 => {
                // Header data (after CRC32 and size vint):
                //   type vint = 1 (1 byte)
                //   flags vint = 0 (1 byte)
                // header_size = 2 (covers type + flags)
                let header_data: &[u8] = &[
                    2, // header_size vint: 2 bytes of data follow
                    1, // header_type vint: 1 = archive header
                    0, // header_flags vint: 0 = no flags
                ];

                // Compute CRC32 of header_data (size vint + type vint + flags vint)
                let crc = crc32fast::hash(header_data);

                // Assemble full header: CRC32(4) + header_data(3) = 7 bytes
                let mut header = Vec::with_capacity(7);
                header.extend_from_slice(&crc.to_le_bytes());
                header.extend_from_slice(header_data);
                header
            }
        }
    }

    /// Write a repaired RAR archive.
    ///
    /// Steps:
    /// 1. Determine version from headers (RAR4 types >= 0x70, RAR5 types are small)
    /// 2. Write marker using `prepend_marker()`
    /// 3. Check if first header is an archive header — if not, write a reconstructed one
    /// 4. Make a mutable copy of the data for CRC fixing
    /// 5. For each header:
    ///    - Fix its CRC using `fix_header_crc()` on the mutable copy
    ///    - Write the header bytes (from offset to offset+header_size)
    ///    - Write the data area (header_size bytes after the header, for data_size bytes)
    ///    - Call progress_fn for file headers
    /// 6. Return ScanResult with recovered_entries and failed_entries
    pub fn write_repaired<W: Write, F>(
        data: &[u8],
        headers: &[RarHeaderInfo],
        writer: &mut W,
        progress_fn: F,
    ) -> Result<ScanResult, RepairError>
    where
        F: Fn(u32, u32, &str),
    {
        if headers.is_empty() {
            return Err(RepairError::NotRepairable(
                "No headers found in RAR archive".to_string(),
            ));
        }

        // Step 1: Determine version from headers
        let version = if headers.iter().any(Self::is_rar4_header) {
            RarVersion::Rar4
        } else {
            RarVersion::Rar5
        };

        let archive_header_type = match version {
            RarVersion::Rar4 => 0x73u8,
            RarVersion::Rar5 => 1u8,
        };
        let file_header_type = match version {
            RarVersion::Rar4 => 0x74u8,
            RarVersion::Rar5 => 2u8,
        };

        // Count total file headers for progress reporting
        let total_file_headers = headers
            .iter()
            .filter(|h| h.header_type == file_header_type)
            .count() as u32;

        // Step 2: Write marker
        Self::prepend_marker(writer, version).map_err(|e| RepairError::IoError(e.to_string()))?;

        // Step 3: Check if first header is an archive header — if not, write a reconstructed one
        let has_archive_header = headers
            .first()
            .map(|h| h.header_type == archive_header_type)
            .unwrap_or(false);

        if !has_archive_header {
            let archive_header = Self::reconstruct_archive_header(version);
            writer
                .write_all(&archive_header)
                .map_err(|e| RepairError::IoError(e.to_string()))?;
        }

        // Step 4: Make a mutable copy of the data for CRC fixing
        let mut fixed_data = data.to_vec();

        // Step 5: Process each header
        let mut recovered_entries: Vec<String> = Vec::new();
        let mut failed_entries: Vec<String> = Vec::new();
        let mut current_step: u32 = 0;

        for header in headers {
            let offset = header.offset as usize;
            let header_size = header.header_size as usize;

            // Fix header CRC on the mutable copy
            Self::fix_header_crc(&mut fixed_data, header);

            // Validate bounds for header bytes
            if offset + header_size > fixed_data.len() {
                let entry_name = header
                    .filename
                    .clone()
                    .unwrap_or_else(|| format!("entry@0x{:X}", header.offset));
                failed_entries.push(entry_name);
                continue;
            }

            // Write the header bytes from the fixed copy
            writer
                .write_all(&fixed_data[offset..offset + header_size])
                .map_err(|e| RepairError::IoError(e.to_string()))?;

            // Write the data area (data_size bytes after the header)
            if header.data_size > 0 {
                let data_start = offset + header_size;
                let data_end = data_start + header.data_size as usize;

                if data_end > fixed_data.len() {
                    // Data area is out of bounds — mark as failed
                    let entry_name = header
                        .filename
                        .clone()
                        .unwrap_or_else(|| format!("entry@0x{:X}", header.offset));
                    failed_entries.push(entry_name);
                    // Write whatever data we can
                    let available_end = fixed_data.len().min(data_end);
                    if data_start < available_end {
                        writer
                            .write_all(&fixed_data[data_start..available_end])
                            .map_err(|e| RepairError::IoError(e.to_string()))?;
                    }
                } else {
                    writer
                        .write_all(&fixed_data[data_start..data_end])
                        .map_err(|e| RepairError::IoError(e.to_string()))?;
                }
            }

            // Call progress_fn for file headers and track recovered entries
            if header.header_type == file_header_type {
                current_step += 1;
                let entry_name = header
                    .filename
                    .clone()
                    .unwrap_or_else(|| format!("entry@0x{:X}", header.offset));
                progress_fn(current_step, total_file_headers, &entry_name);

                // If this entry wasn't already marked as failed, it's recovered
                if !failed_entries.contains(&entry_name) {
                    recovered_entries.push(entry_name);
                }
            }
        }

        let success = failed_entries.is_empty() && !recovered_entries.is_empty();

        Ok(ScanResult {
            success,
            recovered_entries,
            failed_entries,
            repaired_path: None, // Caller sets this
        })
    }

    /// Determine if a header is RAR4 based on its header_type value.
    /// RAR4 types are >= 0x70 (e.g., 0x73 for archive header, 0x74 for file header).
    /// RAR5 types are small integers (e.g., 1 for archive header, 2 for file header).
    fn is_rar4_header(header: &RarHeaderInfo) -> bool {
        header.header_type >= RAR4_TYPE_THRESHOLD
    }
}
