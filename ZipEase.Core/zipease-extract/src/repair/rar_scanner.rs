use super::models::{DamageEntry, DamageReport, RarHeaderInfo, RarVersion};

/// RAR4 marker block: `Rar!\x1A\x07\x00` (7 bytes)
const RAR4_MARKER: &[u8] = b"Rar!\x1a\x07\x00";
/// RAR5 marker block: `Rar!\x1A\x07\x01\x00` (8 bytes)
const RAR5_MARKER: &[u8] = b"Rar!\x1a\x07\x01\x00";

/// RAR4 header types
const RAR4_ARCHIVE_HEADER: u8 = 0x73;
const RAR4_FILE_HEADER: u8 = 0x74;

/// RAR5 header types
// Retained: RAR5 protocol constant for future RAR5 archive-header validation support
#[allow(dead_code)]
const RAR5_ARCHIVE_HEADER: u8 = 1;
const RAR5_FILE_HEADER: u8 = 2;

/// Maximum bytes to scan for marker at file start
const MARKER_SCAN_LIMIT: usize = 1024;

pub struct RarScanner;

impl RarScanner {
    /// Check for RAR marker block at file start.
    /// Returns the RAR version (4 or 5) if found.
    /// Also scans first ~1024 bytes for the marker in case there's garbage before it.
    pub fn check_marker(data: &[u8]) -> Option<RarVersion> {
        let scan_limit = data.len().min(MARKER_SCAN_LIMIT);
        let scan_data = &data[..scan_limit];

        // Scan through the first 1024 bytes looking for either marker
        for i in 0..scan_data.len() {
            // Check RAR5 first (longer marker, more specific)
            if i + RAR5_MARKER.len() <= scan_data.len()
                && &scan_data[i..i + RAR5_MARKER.len()] == RAR5_MARKER
            {
                return Some(RarVersion::Rar5);
            }
            // Check RAR4
            if i + RAR4_MARKER.len() <= scan_data.len()
                && &scan_data[i..i + RAR4_MARKER.len()] == RAR4_MARKER
            {
                // Make sure it's not actually a RAR5 marker (RAR4 is a prefix of RAR5)
                if i + RAR5_MARKER.len() <= scan_data.len()
                    && &scan_data[i..i + RAR5_MARKER.len()] == RAR5_MARKER
                {
                    return Some(RarVersion::Rar5);
                }
                return Some(RarVersion::Rar4);
            }
        }

        None
    }

    /// Scan for RAR file header signatures throughout the file.
    /// Parses headers sequentially starting after the marker block.
    pub fn scan_headers(data: &[u8], version: RarVersion) -> Vec<RarHeaderInfo> {
        // Find the marker position first
        let marker_offset = Self::find_marker_offset(data, version);
        let start_offset = match marker_offset {
            Some(offset) => match version {
                RarVersion::Rar4 => offset + RAR4_MARKER.len(),
                RarVersion::Rar5 => offset + RAR5_MARKER.len(),
            },
            None => return Vec::new(),
        };

        match version {
            RarVersion::Rar4 => Self::scan_rar4_headers(data, start_offset),
            RarVersion::Rar5 => Self::scan_rar5_headers(data, start_offset),
        }
    }

    /// Validate header CRC for a RAR header at the given offset.
    ///
    /// For RAR4: CRC32 of header bytes from type field (offset+2) to end of header
    /// (offset + header_size), then compare lower 16 bits to stored header_crc.
    ///
    /// For RAR5: CRC32 of header bytes after the 4-byte CRC field (offset+4) to
    /// end of header (offset + header_size), compare full 32-bit CRC to the value
    /// stored at offset..offset+4 in the file.
    pub fn validate_header_crc(data: &[u8], header: &RarHeaderInfo) -> bool {
        let offset = header.offset as usize;
        let header_size = header.header_size as usize;

        // Ensure we have enough data
        if offset + header_size > data.len() {
            return false;
        }

        // Determine version by checking if this header looks like RAR5
        // RAR5 headers have a 4-byte CRC at the start; RAR4 headers have a 2-byte CRC.
        // We can distinguish by checking the marker or by using the header structure.
        // Since RAR5 header_size includes the 4-byte CRC + vint size + header data,
        // and RAR4 header_size includes the 2-byte CRC + type + flags + size fields,
        // we detect version by checking if a valid RAR5 CRC32 is at the offset.
        //
        // A simpler approach: check if the data starts with a RAR5 or RAR4 marker.
        let version = Self::check_marker(data);

        match version {
            Some(RarVersion::Rar5) => {
                // RAR5: CRC32 of everything after the 4-byte CRC field
                // The CRC covers header_size bytes starting at offset+4
                // (header_size in our model is total from offset to end of header data)
                if offset + 4 > data.len() {
                    return false;
                }
                let crc_start = offset + 4;
                let crc_end = offset + header_size;
                if crc_end > data.len() || crc_start >= crc_end {
                    return false;
                }

                let computed_crc = crc32fast::hash(&data[crc_start..crc_end]);
                let stored_crc = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                computed_crc == stored_crc
            }
            Some(RarVersion::Rar4) | None => {
                // RAR4: CRC32 of header bytes from type field (offset+2) to end of header
                // Then compare lower 16 bits to stored header_crc
                let crc_start = offset + 2;
                let crc_end = offset + header_size;
                if crc_end > data.len() || crc_start >= crc_end {
                    return false;
                }

                let computed_crc = crc32fast::hash(&data[crc_start..crc_end]);
                let stored_crc = header.header_crc;
                (computed_crc & 0xFFFF) as u16 == stored_crc
            }
        }
    }

    /// Diagnose a RAR archive — produces a DamageReport.
    ///
    /// Steps:
    /// 1. Check for RAR marker to detect version
    /// 2. If no marker found, return DamageReport with repairable=false and "missing_marker" damage
    /// 3. Scan headers and validate CRC for each
    /// 4. Count total file headers, corrupted headers, valid headers
    /// 5. Detect multi-volume (RAR4 archive header flags & 0x0001)
    /// 6. Build and return DamageReport
    pub fn diagnose(data: &[u8]) -> DamageReport {
        // Step 1: Detect version
        let version = match Self::check_marker(data) {
            Some(v) => v,
            None => {
                // No marker found — report as missing_marker
                return DamageReport {
                    format: "rar".to_string(),
                    total_entries: 0,
                    valid_entries: 0,
                    corrupted_entries: 0,
                    unrecoverable_entries: 0,
                    damages: vec![DamageEntry {
                        damage_type: "missing_marker".to_string(),
                        offset: 0,
                        entry_name: None,
                        description: "RAR marker block not found at file start".to_string(),
                    }],
                    repairable: false,
                };
            }
        };

        // Step 2: Scan all headers
        let mut headers = Self::scan_headers(data, version);

        // Step 3: Validate CRC for each header
        for header in headers.iter_mut() {
            header.crc_valid = Self::validate_header_crc(data, header);
        }

        // Step 4: Detect multi-volume (RAR4 archive header flags & 0x0001)
        let mut _is_multi_volume = false;
        for header in &headers {
            if version == RarVersion::Rar4 && header.header_type == RAR4_ARCHIVE_HEADER {
                let offset = header.offset as usize;
                // RAR4 flags are at offset+3..offset+5
                if offset + 5 <= data.len() {
                    let flags = u16::from_le_bytes([data[offset + 3], data[offset + 4]]);
                    if flags & 0x0001 != 0 {
                        _is_multi_volume = true;
                    }
                }
            }
        }

        // Step 5: Count file headers only (not archive headers)
        let file_header_type = match version {
            RarVersion::Rar4 => RAR4_FILE_HEADER,
            RarVersion::Rar5 => RAR5_FILE_HEADER,
        };

        let file_headers: Vec<&RarHeaderInfo> = headers
            .iter()
            .filter(|h| h.header_type == file_header_type)
            .collect();

        let total_entries = file_headers.len() as u32;
        let corrupted_entries = file_headers.iter().filter(|h| !h.crc_valid).count() as u32;
        let valid_entries = total_entries - corrupted_entries;

        // Step 6: Build damage entries for corrupted headers
        let damages: Vec<DamageEntry> = headers
            .iter()
            .filter(|h| !h.crc_valid)
            .map(|h| DamageEntry {
                damage_type: "corrupted_header".to_string(),
                offset: h.offset,
                entry_name: h.filename.clone(),
                description: format!(
                    "Header CRC mismatch at offset 0x{:X} (type: 0x{:02X})",
                    h.offset, h.header_type
                ),
            })
            .collect();

        // Step 7: Build DamageReport
        DamageReport {
            format: "rar".to_string(),
            total_entries,
            valid_entries,
            corrupted_entries,
            unrecoverable_entries: 0, // RAR repair can fix CRC issues
            damages,
            repairable: !headers.is_empty(),
        }
    }

    // --- Private helpers ---

    /// Find the byte offset of the RAR marker in data.
    fn find_marker_offset(data: &[u8], version: RarVersion) -> Option<usize> {
        let marker = match version {
            RarVersion::Rar4 => RAR4_MARKER,
            RarVersion::Rar5 => RAR5_MARKER,
        };
        let scan_limit = data.len().min(MARKER_SCAN_LIMIT);
        let scan_data = &data[..scan_limit];

        (0..scan_data.len()).find(|&i| i + marker.len() <= scan_data.len() && &scan_data[i..i + marker.len()] == marker)
    }

    /// Parse RAR4 headers sequentially from the given offset.
    ///
    /// RAR4 header structure:
    /// - 2 bytes: header CRC (CRC of header from type field to end)
    /// - 1 byte: header type
    /// - 2 bytes: flags
    /// - 2 bytes: header size (total including CRC and type fields)
    /// - If flags & 0x8000: 4 bytes additional data size after header
    /// - For file headers (type 0x74): filename is embedded in header data
    fn scan_rar4_headers(data: &[u8], start: usize) -> Vec<RarHeaderInfo> {
        let mut headers = Vec::new();
        let mut offset = start;

        loop {
            // Minimum RAR4 header is 7 bytes: 2 CRC + 1 type + 2 flags + 2 size
            if offset + 7 > data.len() {
                break;
            }

            let header_crc = u16::from_le_bytes([data[offset], data[offset + 1]]);
            let header_type = data[offset + 2];
            let flags = u16::from_le_bytes([data[offset + 3], data[offset + 4]]);
            let header_size = u16::from_le_bytes([data[offset + 5], data[offset + 6]]);

            // Validate header size - must be at least 7 and not exceed remaining data
            if header_size < 7 || (offset + header_size as usize) > data.len() {
                break;
            }

            // Check for additional data size (high bit of flags)
            let data_size = if flags & 0x8000 != 0 {
                if offset + 7 + 4 > data.len() {
                    break;
                }
                u32::from_le_bytes([
                    data[offset + 7],
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                ]) as u64
            } else {
                0
            };

            // Extract filename for file headers (type 0x74)
            let filename = if header_type == RAR4_FILE_HEADER {
                Self::extract_rar4_filename(data, offset, header_size)
            } else {
                None
            };

            headers.push(RarHeaderInfo {
                offset: offset as u64,
                header_type,
                header_size,
                header_crc,
                filename,
                data_size,
                crc_valid: true, // Will be validated in task 6.2
            });

            // Advance past header + data
            let next_offset = offset + header_size as usize + data_size as usize;
            if next_offset <= offset {
                // Prevent infinite loop on zero-size advancement
                break;
            }
            offset = next_offset;
        }

        headers
    }

    /// Extract filename from a RAR4 file header.
    ///
    /// RAR4 file header layout (after the common 7-byte header):
    /// Offset from header start:
    ///   +7:  4 bytes - compressed size (low 32 bits)
    ///   +11: 4 bytes - uncompressed size (low 32 bits)
    ///   +15: 1 byte  - host OS
    ///   +16: 4 bytes - file CRC
    ///   +20: 4 bytes - date/time
    ///   +24: 1 byte  - unpack version
    ///   +25: 1 byte  - method
    ///   +26: 2 bytes - filename length
    ///   +28: 4 bytes - file attributes
    ///   +32: filename bytes (length from offset +26)
    fn extract_rar4_filename(data: &[u8], header_offset: usize, header_size: u16) -> Option<String> {
        // File header needs at least 32 bytes before filename
        let name_len_offset = header_offset + 26;
        if name_len_offset + 2 > data.len() {
            return None;
        }

        let name_len =
            u16::from_le_bytes([data[name_len_offset], data[name_len_offset + 1]]) as usize;

        if name_len == 0 {
            return None;
        }

        let name_offset = header_offset + 32;
        let name_end = name_offset + name_len;

        // Ensure filename is within header bounds
        if name_end > header_offset + header_size as usize || name_end > data.len() {
            return None;
        }

        let name_bytes = &data[name_offset..name_end];
        // RAR4 filenames can be in various encodings, try UTF-8 first
        String::from_utf8(name_bytes.to_vec()).ok().or_else(|| {
            // Fall back to lossy conversion
            Some(String::from_utf8_lossy(name_bytes).into_owned())
        })
    }

    /// Parse RAR5 headers sequentially from the given offset.
    ///
    /// RAR5 header structure:
    /// - 4 bytes: header CRC32
    /// - vint: header size (size of header data after this field)
    /// - vint: header type
    /// - vint: header flags
    /// - (optional extra fields depending on type)
    /// - For file headers (type 2): contains filename
    fn scan_rar5_headers(data: &[u8], start: usize) -> Vec<RarHeaderInfo> {
        let mut headers = Vec::new();
        let mut offset = start;

        loop {
            // Minimum RAR5 header: 4 bytes CRC + at least 1 byte vint size + 1 byte vint type
            if offset + 6 > data.len() {
                break;
            }

            let header_crc32 = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            // Store lower 16 bits of CRC32 in the header_crc field (model constraint)
            let header_crc = header_crc32 as u16;

            let mut pos = offset + 4;

            // Read header size (vint)
            let (header_size_val, bytes_read) = match Self::read_vint(data, pos) {
                Some(v) => v,
                None => break,
            };
            pos += bytes_read;

            if header_size_val == 0 || header_size_val > u64::from(u16::MAX) * 4 {
                break;
            }

            // The header data starts at `pos` and is `header_size_val` bytes long
            let header_data_start = pos;
            let header_data_end = header_data_start + header_size_val as usize;

            if header_data_end > data.len() {
                break;
            }

            // Read header type (vint) from within header data
            let (header_type_val, type_bytes_read) = match Self::read_vint(data, pos) {
                Some(v) => v,
                None => break,
            };
            pos += type_bytes_read;

            // Read header flags (vint)
            let (header_flags, flags_bytes_read) = match Self::read_vint(data, pos) {
                Some(v) => v,
                None => break,
            };
            pos += flags_bytes_read;

            // Check if there's extra data after the header
            let data_size = if header_flags & 0x0002 != 0 {
                // Has data area - read data size from header
                // Extra area might come first if flag 0x0001 is set
                let mut extra_pos = pos;
                if header_flags & 0x0001 != 0 {
                    // Skip extra area size vint
                    if let Some((extra_size, extra_bytes)) = Self::read_vint(data, extra_pos) {
                        extra_pos += extra_bytes + extra_size as usize;
                    }
                }
                // Read data size
                match Self::read_vint(data, extra_pos) {
                    Some((ds, _)) => ds,
                    None => 0,
                }
            } else {
                0
            };

            // Extract filename for file headers (type 2)
            let filename = if header_type_val == RAR5_FILE_HEADER as u64 {
                Self::extract_rar5_filename(data, pos, header_data_end, header_flags)
            } else {
                None
            };

            // Total header size from start of CRC to end of header data
            let total_header_size = (header_data_end - offset) as u16;

            headers.push(RarHeaderInfo {
                offset: offset as u64,
                header_type: header_type_val as u8,
                header_size: total_header_size,
                header_crc,
                filename,
                data_size,
                crc_valid: true, // Will be validated in task 6.2
            });

            // Advance past header + data area
            let next_offset = header_data_end + data_size as usize;
            if next_offset <= offset {
                break;
            }
            offset = next_offset;
        }

        headers
    }

    /// Extract filename from a RAR5 file header.
    ///
    /// RAR5 file header data (after type and flags):
    /// - vint: file flags
    /// - vint: unpacked size
    /// - vint: attributes
    /// - 4 bytes: mtime (if file flags & 0x0002)
    /// - 4 bytes: data CRC32 (if file flags & 0x0004)
    /// - vint: compression info
    /// - vint: host OS
    /// - vint: name length
    /// - name bytes
    fn extract_rar5_filename(
        data: &[u8],
        pos: usize,
        header_end: usize,
        _header_flags: u64,
    ) -> Option<String> {
        let mut cur = pos;

        // Read file flags (vint)
        let (file_flags, bytes_read) = Self::read_vint(data, cur)?;
        cur += bytes_read;

        // Read unpacked size (vint)
        let (_, bytes_read) = Self::read_vint(data, cur)?;
        cur += bytes_read;

        // Read attributes (vint)
        let (_, bytes_read) = Self::read_vint(data, cur)?;
        cur += bytes_read;

        // mtime (4 bytes) if file flags & 0x0002
        if file_flags & 0x0002 != 0 {
            if cur + 4 > header_end {
                return None;
            }
            cur += 4;
        }

        // data CRC32 (4 bytes) if file flags & 0x0004
        if file_flags & 0x0004 != 0 {
            if cur + 4 > header_end {
                return None;
            }
            cur += 4;
        }

        // Compression info (vint)
        let (_, bytes_read) = Self::read_vint(data, cur)?;
        cur += bytes_read;

        // Host OS (vint)
        let (_, bytes_read) = Self::read_vint(data, cur)?;
        cur += bytes_read;

        // Name length (vint)
        let (name_len, bytes_read) = Self::read_vint(data, cur)?;
        cur += bytes_read;

        if name_len == 0 || cur + name_len as usize > header_end {
            return None;
        }

        let name_bytes = &data[cur..cur + name_len as usize];
        // RAR5 filenames are always UTF-8
        String::from_utf8(name_bytes.to_vec()).ok()
    }

    /// Read a RAR5 variable-length integer (vint).
    /// Returns (value, bytes_consumed) or None if data is insufficient.
    ///
    /// vint encoding: each byte contributes 7 bits of data.
    /// The high bit indicates continuation (1 = more bytes follow, 0 = last byte).
    /// Bytes are in little-endian order (least significant 7 bits first).
    fn read_vint(data: &[u8], offset: usize) -> Option<(u64, usize)> {
        let mut value: u64 = 0;
        let mut shift: u32 = 0;
        let mut pos = offset;

        loop {
            if pos >= data.len() {
                return None;
            }

            let byte = data[pos];
            value |= ((byte & 0x7F) as u64) << shift;
            pos += 1;

            if byte & 0x80 == 0 {
                // Last byte
                break;
            }

            shift += 7;
            if shift >= 64 {
                // Overflow protection
                return None;
            }
        }

        Some((value, pos - offset))
    }
}
