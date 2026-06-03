use serde::{Deserialize, Serialize};

/// Diagnosis report describing the state of a damaged archive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DamageReport {
    /// Archive format: "zip" or "rar"
    pub format: String,
    /// Total entries detected in the archive
    pub total_entries: u32,
    /// Entries with valid, intact headers
    pub valid_entries: u32,
    /// Entries with corrupted but potentially repairable headers
    pub corrupted_entries: u32,
    /// Entries that cannot be recovered
    pub unrecoverable_entries: u32,
    /// Detailed damage descriptions
    pub damages: Vec<DamageEntry>,
    /// Whether the archive is repairable (at least partially)
    pub repairable: bool,
}

/// A single damage finding within an archive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DamageEntry {
    /// Type of damage: "missing_eocd", "corrupted_cd", "misaligned_lfh",
    /// "invalid_crc", "corrupted_header", "missing_marker", etc.
    pub damage_type: String,
    /// Byte offset in the archive where damage was detected
    pub offset: u64,
    /// Entry name if recoverable, None otherwise
    pub entry_name: Option<String>,
    /// Human-readable description of the damage
    pub description: String,
}

/// Result of a repair operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanResult {
    /// Whether the repair was fully successful
    pub success: bool,
    /// Names of entries that were successfully recovered
    pub recovered_entries: Vec<String>,
    /// Names/descriptions of entries that could not be recovered
    pub failed_entries: Vec<String>,
    /// Path to the repaired archive file (None if repair failed entirely)
    pub repaired_path: Option<String>,
}

/// Errors that can occur during repair operations.
#[derive(Debug)]
pub enum RepairError {
    /// File cannot be read (IO error, permissions, zero bytes)
    IoError(String),
    /// File is not a recognized archive format
    NotAnArchive,
    /// Archive is too damaged to repair at all
    NotRepairable(String),
    /// Repair partially succeeded (some entries unrecoverable)
    PartialRepair(ScanResult),
}

impl RepairError {
    pub fn to_ffi_code(&self) -> i32 {
        match self {
            RepairError::IoError(_) => -1,
            RepairError::NotAnArchive => 0x2006_u32 as i32,
            RepairError::NotRepairable(_) => 0x2006_u32 as i32,
            RepairError::PartialRepair(_) => 0x2007_u32 as i32,
        }
    }
}

/// Parsed Local File Header (30 bytes fixed + variable)
#[derive(Debug, Clone)]
pub struct LocalFileHeader {
    pub offset: u64,
    pub version_needed: u16,
    pub flags: u16,
    pub compression_method: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub filename: Vec<u8>,
    pub extra_field: Vec<u8>,
    /// Offset where compressed data begins (after header + filename + extra)
    pub data_offset: u64,
}

/// Parsed Central Directory entry
#[derive(Debug, Clone)]
pub struct CdEntry {
    pub version_made_by: u16,
    pub version_needed: u16,
    pub flags: u16,
    pub compression_method: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub filename: Vec<u8>,
    pub extra_field: Vec<u8>,
    pub comment: Vec<u8>,
    pub disk_number_start: u16,
    pub internal_attrs: u16,
    pub external_attrs: u32,
    pub local_header_offset: u32,
}

/// End of Central Directory record
#[derive(Debug, Clone)]
pub struct EocdRecord {
    pub disk_number: u16,
    pub cd_start_disk: u16,
    pub cd_entries_on_disk: u16,
    pub cd_entries_total: u16,
    pub cd_size: u32,
    pub cd_offset: u32,
    pub comment: Vec<u8>,
}

/// RAR archive version
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RarVersion {
    Rar4, // Rar!\x1A\x07\x00
    Rar5, // Rar!\x1A\x07\x01\x00
}

/// RAR header information
#[derive(Debug, Clone)]
pub struct RarHeaderInfo {
    pub offset: u64,
    pub header_type: u8,
    pub header_size: u16,
    pub header_crc: u16,
    pub filename: Option<String>,
    pub data_size: u64,
    pub crc_valid: bool,
}

/// Helper struct for the write phase of repair
#[derive(Debug, Clone)]
pub struct RepairedEntry {
    pub filename: Vec<u8>,
    pub compression_method: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub data_offset: u64,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
}
