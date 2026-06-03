pub mod models;
pub mod path_gen;
pub mod rar_repairer;
pub mod rar_scanner;
pub mod zip_repairer;
pub mod zip_scanner;

use std::fs;
use std::io::BufWriter;
use std::path::Path;

use models::{DamageReport, RepairError, RepairedEntry, ScanResult};
use path_gen::generate_repair_path;
use rar_repairer::RarRepairer;
use rar_scanner::RarScanner;
use zip_repairer::ZipRepairer;
use zip_scanner::ZipScanner;

/// ZIP magic bytes: `PK` (0x50, 0x4B)
const ZIP_MAGIC: [u8; 2] = [0x50, 0x4B];

/// RAR magic bytes: `Rar!` (0x52, 0x61, 0x72, 0x21)
const RAR_MAGIC: [u8; 4] = [0x52, 0x61, 0x72, 0x21];

/// Maximum bytes to scan for RAR marker when not at file start
const FORMAT_SCAN_LIMIT: usize = 1024;

/// Detected archive format
#[derive(Debug, Clone, Copy, PartialEq)]
enum ArchiveFormat {
    Zip,
    Rar,
}

/// The main entry point for archive repair operations.
/// Detects archive format and routes to the appropriate scanner/repairer.
pub struct RepairEngine;

impl RepairEngine {
    /// Diagnose an archive file — returns damage report without modifying anything.
    pub fn diagnose(archive_path: &Path) -> Result<DamageReport, RepairError> {
        let data = Self::read_file(archive_path)?;

        match Self::detect_format(&data) {
            Some(ArchiveFormat::Zip) => Ok(ZipScanner::diagnose(&data)),
            Some(ArchiveFormat::Rar) => Ok(RarScanner::diagnose(&data)),
            None => Err(RepairError::NotAnArchive),
        }
    }

    /// Repair an archive — writes repaired copy to output_path.
    /// Calls progress_fn(current_step, total_steps, entry_name) for each entry processed.
    pub fn repair<F>(
        archive_path: &Path,
        output_path: &Path,
        progress_fn: F,
    ) -> Result<ScanResult, RepairError>
    where
        F: Fn(u32, u32, &str),
    {
        let data = Self::read_file(archive_path)?;

        let format = Self::detect_format(&data)
            .ok_or(RepairError::NotAnArchive)?;

        let mut result = match format {
            ArchiveFormat::Zip => Self::repair_zip(&data, output_path, &progress_fn)?,
            ArchiveFormat::Rar => Self::repair_rar(&data, output_path, &progress_fn)?,
        };

        // Set the repaired_path in the result
        result.repaired_path = Some(output_path.to_string_lossy().to_string());

        // Check result status and return appropriate error/success
        if !result.recovered_entries.is_empty() && !result.failed_entries.is_empty() {
            // Partial repair: some entries recovered, some failed
            Err(RepairError::PartialRepair(result))
        } else if result.recovered_entries.is_empty() {
            // No entries recovered at all
            Err(RepairError::NotRepairable(
                "No entries could be recovered from the archive".to_string(),
            ))
        } else {
            // Full success
            Ok(result)
        }
    }

    /// Convenience method: auto-generates the output path and calls repair.
    pub fn repair_auto<F>(
        archive_path: &Path,
        progress_fn: F,
    ) -> Result<ScanResult, RepairError>
    where
        F: Fn(u32, u32, &str),
    {
        let output_path = generate_repair_path(archive_path);
        Self::repair(archive_path, &output_path, progress_fn)
    }

    // --- Private helpers ---

    /// Read a file into memory, returning RepairError::IoError on failure.
    fn read_file(path: &Path) -> Result<Vec<u8>, RepairError> {
        fs::read(path).map_err(|e| {
            RepairError::IoError(format!("Failed to read '{}': {}", path.display(), e))
        })
    }

    /// Detect archive format by checking magic bytes.
    /// - Starts with `PK` (0x50, 0x4B) → ZIP
    /// - Starts with `Rar!` (0x52, 0x61, 0x72, 0x21) → RAR
    /// - Also checks within first 1024 bytes for RAR marker (garbage prefix case)
    fn detect_format(data: &[u8]) -> Option<ArchiveFormat> {
        if data.len() < 2 {
            return None;
        }

        // Check ZIP magic at start
        if data[0] == ZIP_MAGIC[0] && data[1] == ZIP_MAGIC[1] {
            return Some(ArchiveFormat::Zip);
        }

        // Check RAR magic at start
        if data.len() >= 4
            && data[0] == RAR_MAGIC[0]
            && data[1] == RAR_MAGIC[1]
            && data[2] == RAR_MAGIC[2]
            && data[3] == RAR_MAGIC[3]
        {
            return Some(ArchiveFormat::Rar);
        }

        // Scan first 1024 bytes for RAR marker (handles garbage prefix)
        let scan_limit = data.len().min(FORMAT_SCAN_LIMIT);
        for i in 1..scan_limit.saturating_sub(3) {
            if data[i] == RAR_MAGIC[0]
                && data[i + 1] == RAR_MAGIC[1]
                && data[i + 2] == RAR_MAGIC[2]
                && data[i + 3] == RAR_MAGIC[3]
            {
                return Some(ArchiveFormat::Rar);
            }
        }

        None
    }

    /// Perform ZIP repair: scan LFH signatures, reconstruct CD, write repaired archive.
    fn repair_zip<F>(
        data: &[u8],
        output_path: &Path,
        progress_fn: &F,
    ) -> Result<ScanResult, RepairError>
    where
        F: Fn(u32, u32, &str),
    {
        // Scan for LFH signatures
        let lfh_offsets = ZipScanner::scan_lfh_signatures(data);

        if lfh_offsets.is_empty() {
            return Err(RepairError::NotRepairable(
                "No Local File Headers found in ZIP archive".to_string(),
            ));
        }

        // Reconstruct CD from LFH offsets
        let cd_entries = ZipRepairer::reconstruct_cd(data, &lfh_offsets);

        if cd_entries.is_empty() {
            return Err(RepairError::NotRepairable(
                "Could not reconstruct any Central Directory entries".to_string(),
            ));
        }

        // Build RepairedEntry list from the CD entries
        let repaired_entries: Vec<RepairedEntry> = cd_entries
            .iter()
            .filter_map(|cd| {
                // Parse the LFH at the referenced offset to get data_offset
                let lfh = ZipScanner::parse_lfh(data, cd.local_header_offset as u64)?;
                Some(RepairedEntry {
                    filename: cd.filename.clone(),
                    compression_method: cd.compression_method,
                    crc32: cd.crc32,
                    compressed_size: cd.compressed_size,
                    uncompressed_size: cd.uncompressed_size,
                    data_offset: lfh.data_offset,
                    last_mod_time: cd.last_mod_time,
                    last_mod_date: cd.last_mod_date,
                })
            })
            .collect();

        if repaired_entries.is_empty() {
            return Err(RepairError::NotRepairable(
                "Could not build any repairable entries from ZIP archive".to_string(),
            ));
        }

        // Create output file
        let output_file = fs::File::create(output_path).map_err(|e| {
            RepairError::IoError(format!(
                "Failed to create output file '{}': {}",
                output_path.display(),
                e
            ))
        })?;
        let mut writer = BufWriter::new(output_file);

        // Write repaired ZIP
        ZipRepairer::write_repaired(data, &repaired_entries, &mut writer, progress_fn)
    }

    /// Perform RAR repair: detect version, scan headers, write repaired archive.
    fn repair_rar<F>(
        data: &[u8],
        output_path: &Path,
        progress_fn: &F,
    ) -> Result<ScanResult, RepairError>
    where
        F: Fn(u32, u32, &str),
    {
        // Detect RAR version
        let version = RarScanner::check_marker(data).ok_or_else(|| {
            RepairError::NotRepairable("RAR marker not found in archive data".to_string())
        })?;

        // Scan headers
        let headers = RarScanner::scan_headers(data, version);

        if headers.is_empty() {
            return Err(RepairError::NotRepairable(
                "No headers found in RAR archive".to_string(),
            ));
        }

        // Create output file
        let output_file = fs::File::create(output_path).map_err(|e| {
            RepairError::IoError(format!(
                "Failed to create output file '{}': {}",
                output_path.display(),
                e
            ))
        })?;
        let mut writer = BufWriter::new(output_file);

        // Write repaired RAR
        RarRepairer::write_repaired(data, &headers, &mut writer, progress_fn)
    }
}
