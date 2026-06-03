use super::models::{CdEntry, DamageEntry, DamageReport, EocdRecord, LocalFileHeader};

pub struct ZipScanner;

impl ZipScanner {
    /// Scan raw bytes for Local File Header signatures (PK\x03\x04).
    /// Returns byte offsets of all found signatures, sorted in ascending order.
    pub fn scan_lfh_signatures(data: &[u8]) -> Vec<u64> {
        const LFH_SIGNATURE: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

        let mut offsets = Vec::new();

        if data.len() < LFH_SIGNATURE.len() {
            return offsets;
        }

        for i in 0..=(data.len() - LFH_SIGNATURE.len()) {
            if data[i..i + 4] == LFH_SIGNATURE {
                offsets.push(i as u64);
            }
        }

        offsets
    }

    /// Parse a Local File Header at the given offset.
    /// Returns None if the data at offset is not a valid LFH structure.
    pub fn parse_lfh(data: &[u8], offset: u64) -> Option<LocalFileHeader> {
        const LFH_SIGNATURE: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
        const LFH_FIXED_SIZE: usize = 30;

        let offset_usize = offset as usize;

        // Check bounds: need at least 30 bytes for the fixed header
        if offset_usize + LFH_FIXED_SIZE > data.len() {
            return None;
        }

        // Verify signature
        if data[offset_usize..offset_usize + 4] != LFH_SIGNATURE {
            return None;
        }

        let h = &data[offset_usize..];

        let version_needed = u16::from_le_bytes([h[4], h[5]]);
        let flags = u16::from_le_bytes([h[6], h[7]]);
        let compression_method = u16::from_le_bytes([h[8], h[9]]);
        let last_mod_time = u16::from_le_bytes([h[10], h[11]]);
        let last_mod_date = u16::from_le_bytes([h[12], h[13]]);
        let crc32 = u32::from_le_bytes([h[14], h[15], h[16], h[17]]);
        let compressed_size = u32::from_le_bytes([h[18], h[19], h[20], h[21]]);
        let uncompressed_size = u32::from_le_bytes([h[22], h[23], h[24], h[25]]);
        let filename_length = u16::from_le_bytes([h[26], h[27]]) as usize;
        let extra_field_length = u16::from_le_bytes([h[28], h[29]]) as usize;

        // Check bounds for variable-length fields
        let variable_end = offset_usize + LFH_FIXED_SIZE + filename_length + extra_field_length;
        if variable_end > data.len() {
            return None;
        }

        let filename_start = offset_usize + LFH_FIXED_SIZE;
        let filename = data[filename_start..filename_start + filename_length].to_vec();

        let extra_start = filename_start + filename_length;
        let extra_field = data[extra_start..extra_start + extra_field_length].to_vec();

        let data_offset = variable_end as u64;

        Some(LocalFileHeader {
            offset,
            version_needed,
            flags,
            compression_method,
            last_mod_time,
            last_mod_date,
            crc32,
            compressed_size,
            uncompressed_size,
            filename,
            extra_field,
            data_offset,
        })
    }

    /// Locate and parse the End of Central Directory record.
    /// Scans backwards from end of file (per ZIP spec).
    pub fn find_eocd(data: &[u8]) -> Option<(u64, EocdRecord)> {
        const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];
        const EOCD_FIXED_SIZE: usize = 22;
        const MAX_COMMENT_SIZE: usize = 65535;

        if data.len() < EOCD_FIXED_SIZE {
            return None;
        }

        // EOCD can be at most 22 + 65535 bytes from the end
        let search_start = if data.len() > EOCD_FIXED_SIZE + MAX_COMMENT_SIZE {
            data.len() - EOCD_FIXED_SIZE - MAX_COMMENT_SIZE
        } else {
            0
        };

        // Scan backwards from end looking for EOCD signature
        let mut pos = data.len() - EOCD_FIXED_SIZE;
        loop {
            if data[pos..pos + 4] == EOCD_SIGNATURE {
                // Verify that the comment length field is consistent
                let comment_length =
                    u16::from_le_bytes([data[pos + 20], data[pos + 21]]) as usize;
                if pos + EOCD_FIXED_SIZE + comment_length == data.len() {
                    let disk_number = u16::from_le_bytes([data[pos + 4], data[pos + 5]]);
                    let cd_start_disk = u16::from_le_bytes([data[pos + 6], data[pos + 7]]);
                    let cd_entries_on_disk =
                        u16::from_le_bytes([data[pos + 8], data[pos + 9]]);
                    let cd_entries_total =
                        u16::from_le_bytes([data[pos + 10], data[pos + 11]]);
                    let cd_size = u32::from_le_bytes([
                        data[pos + 12],
                        data[pos + 13],
                        data[pos + 14],
                        data[pos + 15],
                    ]);
                    let cd_offset = u32::from_le_bytes([
                        data[pos + 16],
                        data[pos + 17],
                        data[pos + 18],
                        data[pos + 19],
                    ]);

                    let comment = if comment_length > 0 {
                        data[pos + EOCD_FIXED_SIZE..pos + EOCD_FIXED_SIZE + comment_length]
                            .to_vec()
                    } else {
                        Vec::new()
                    };

                    return Some((
                        pos as u64,
                        EocdRecord {
                            disk_number,
                            cd_start_disk,
                            cd_entries_on_disk,
                            cd_entries_total,
                            cd_size,
                            cd_offset,
                            comment,
                        },
                    ));
                }
            }

            if pos == search_start {
                break;
            }
            pos -= 1;
        }

        None
    }

    /// Parse Central Directory entries starting at the given offset.
    /// Parses `count` entries; skips corrupted entries (bad signature or out of bounds).
    pub fn parse_central_directory(data: &[u8], offset: u64, count: u16) -> Vec<CdEntry> {
        const CD_SIGNATURE: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
        const CD_FIXED_SIZE: usize = 46;

        let mut entries = Vec::new();
        let mut pos = offset as usize;

        for _ in 0..count {
            // Check bounds for fixed header
            if pos + CD_FIXED_SIZE > data.len() {
                break;
            }

            // Verify CD signature
            if data[pos..pos + 4] != CD_SIGNATURE {
                // Corrupted entry — skip and try to find next CD signature
                // Advance by 1 byte and scan for next signature within remaining data
                let mut found_next = false;
                for scan_pos in (pos + 1)..data.len().saturating_sub(CD_FIXED_SIZE - 1) {
                    if data[scan_pos..scan_pos + 4] == CD_SIGNATURE {
                        pos = scan_pos;
                        found_next = true;
                        break;
                    }
                }
                if !found_next {
                    break;
                }
                // Re-verify after scanning
                if data[pos..pos + 4] != CD_SIGNATURE {
                    break;
                }
            }

            let h = &data[pos..];

            let version_made_by = u16::from_le_bytes([h[4], h[5]]);
            let version_needed = u16::from_le_bytes([h[6], h[7]]);
            let flags = u16::from_le_bytes([h[8], h[9]]);
            let compression_method = u16::from_le_bytes([h[10], h[11]]);
            let last_mod_time = u16::from_le_bytes([h[12], h[13]]);
            let last_mod_date = u16::from_le_bytes([h[14], h[15]]);
            let crc32 = u32::from_le_bytes([h[16], h[17], h[18], h[19]]);
            let compressed_size = u32::from_le_bytes([h[20], h[21], h[22], h[23]]);
            let uncompressed_size = u32::from_le_bytes([h[24], h[25], h[26], h[27]]);
            let filename_length = u16::from_le_bytes([h[28], h[29]]) as usize;
            let extra_field_length = u16::from_le_bytes([h[30], h[31]]) as usize;
            let comment_length = u16::from_le_bytes([h[32], h[33]]) as usize;
            let disk_number_start = u16::from_le_bytes([h[34], h[35]]);
            let internal_attrs = u16::from_le_bytes([h[36], h[37]]);
            let external_attrs = u32::from_le_bytes([h[38], h[39], h[40], h[41]]);
            let local_header_offset = u32::from_le_bytes([h[42], h[43], h[44], h[45]]);

            // Check bounds for variable-length fields
            let variable_end =
                pos + CD_FIXED_SIZE + filename_length + extra_field_length + comment_length;
            if variable_end > data.len() {
                // Corrupted entry — out of bounds, skip it
                break;
            }

            let filename_start = pos + CD_FIXED_SIZE;
            let filename = data[filename_start..filename_start + filename_length].to_vec();

            let extra_start = filename_start + filename_length;
            let extra_field = data[extra_start..extra_start + extra_field_length].to_vec();

            let comment_start = extra_start + extra_field_length;
            let comment = data[comment_start..comment_start + comment_length].to_vec();

            entries.push(CdEntry {
                version_made_by,
                version_needed,
                flags,
                compression_method,
                last_mod_time,
                last_mod_date,
                crc32,
                compressed_size,
                uncompressed_size,
                filename,
                extra_field,
                comment,
                disk_number_start,
                internal_attrs,
                external_attrs,
                local_header_offset,
            });

            pos = variable_end;
        }

        entries
    }

    /// Diagnose a ZIP archive — produces a DamageReport.
    pub fn diagnose(data: &[u8]) -> DamageReport {
        // If data is too short or has no LFH signatures, return not repairable
        if data.len() < 4 {
            return DamageReport {
                format: "zip".to_string(),
                total_entries: 0,
                valid_entries: 0,
                corrupted_entries: 0,
                unrecoverable_entries: 0,
                damages: Vec::new(),
                repairable: false,
            };
        }

        let lfh_offsets = Self::scan_lfh_signatures(data);

        if lfh_offsets.is_empty() {
            return DamageReport {
                format: "zip".to_string(),
                total_entries: 0,
                valid_entries: 0,
                corrupted_entries: 0,
                unrecoverable_entries: 0,
                damages: Vec::new(),
                repairable: false,
            };
        }

        let total_entries = lfh_offsets.len() as u32;
        let mut damages: Vec<DamageEntry> = Vec::new();
        let mut valid_entries: u32 = 0;
        let mut corrupted_entries: u32 = 0;
        let mut unrecoverable_entries: u32 = 0;

        // Try to find EOCD
        let eocd_result = Self::find_eocd(data);

        // Parse CD entries if EOCD is found
        let cd_entries = if let Some((_eocd_offset, ref eocd)) = eocd_result {
            Self::parse_central_directory(data, eocd.cd_offset as u64, eocd.cd_entries_total)
        } else {
            Vec::new()
        };

        // Detect missing EOCD
        if eocd_result.is_none() {
            damages.push(DamageEntry {
                damage_type: "missing_eocd".to_string(),
                offset: 0,
                entry_name: None,
                description: "End of Central Directory record is missing or corrupted"
                    .to_string(),
            });
        }

        // Detect corrupted CD (CD entry count doesn't match LFH count)
        if let Some((_, ref eocd)) = eocd_result {
            if (cd_entries.len() as u16) != eocd.cd_entries_total
                || (cd_entries.len() as u32) != total_entries
            {
                damages.push(DamageEntry {
                    damage_type: "corrupted_cd".to_string(),
                    offset: eocd.cd_offset as u64,
                    entry_name: None,
                    description: format!(
                        "Central Directory entry count mismatch: CD has {} entries, but {} LFH signatures found",
                        cd_entries.len(),
                        total_entries
                    ),
                });
            }
        }

        // Build a set of LFH offsets referenced by CD entries
        let cd_referenced_offsets: Vec<u32> = cd_entries
            .iter()
            .map(|cd| cd.local_header_offset)
            .collect();

        // Classify each LFH entry
        for &lfh_offset in &lfh_offsets {
            let lfh = Self::parse_lfh(data, lfh_offset);

            match lfh {
                Some(ref header) => {
                    let is_referenced = cd_referenced_offsets
                        .iter()
                        .any(|&cd_off| cd_off as u64 == lfh_offset);

                    let has_invalid_crc = header.crc32 == 0;

                    // Determine entry name for reporting
                    let entry_name = String::from_utf8(header.filename.clone())
                        .ok()
                        .filter(|s| !s.is_empty());

                    if !is_referenced && eocd_result.is_some() {
                        // Misaligned LFH — not referenced by any CD entry
                        damages.push(DamageEntry {
                            damage_type: "misaligned_lfh".to_string(),
                            offset: lfh_offset,
                            entry_name: entry_name.clone(),
                            description: format!(
                                "Local File Header at offset {lfh_offset} is not referenced by any Central Directory entry"
                            ),
                        });
                        corrupted_entries += 1;
                    } else if has_invalid_crc {
                        // Invalid CRC — potentially corrupted data
                        damages.push(DamageEntry {
                            damage_type: "invalid_crc".to_string(),
                            offset: lfh_offset,
                            entry_name: entry_name.clone(),
                            description: format!(
                                "Local File Header at offset {lfh_offset} has CRC-32 == 0 (potentially corrupted)"
                            ),
                        });
                        corrupted_entries += 1;
                    } else {
                        valid_entries += 1;
                    }
                }
                None => {
                    // LFH signature found but couldn't parse — unrecoverable
                    unrecoverable_entries += 1;
                }
            }
        }

        // Ensure invariant: total_entries == valid_entries + corrupted_entries + unrecoverable_entries
        debug_assert_eq!(
            total_entries,
            valid_entries + corrupted_entries + unrecoverable_entries
        );

        let repairable = valid_entries > 0 || corrupted_entries > 0;

        DamageReport {
            format: "zip".to_string(),
            total_entries,
            valid_entries,
            corrupted_entries,
            unrecoverable_entries,
            damages,
            repairable,
        }
    }
}
