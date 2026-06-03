//! ISO 9660 extraction backend.
//!
//! Implements a minimal ISO 9660 reader (Primary Volume Descriptor + directory tree walk)
//! sufficient for listing and extracting files from standard CD/DVD images.
//! Joliet (UCS-2 filenames) is supported via the Supplementary Volume Descriptor.
//!
//! No external crate is required — ISO 9660 has a simple, well-documented binary layout.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use zipease_shared::LockError;
use super::{ExtractionBackend, ArchiveEntryInfo};

const SECTOR_SIZE: u64 = 2048;
const SYSTEM_AREA_SECTORS: u64 = 16;

pub struct IsoBackend;

// ── Low-level helpers ────────────────────────────────────────────────────────

fn read_sector(f: &mut File, lba: u32) -> io::Result<[u8; 2048]> {
    let mut buf = [0u8; 2048];
    f.seek(SeekFrom::Start(lba as u64 * SECTOR_SIZE))?;
    f.read_exact(&mut buf)?;
    Ok(buf)
}

fn le32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

// ── Volume Descriptor parsing ────────────────────────────────────────────────

struct VolumeInfo {
    root_lba: u32,
    root_size: u32,
    joliet: bool,
}

/// Scan Volume Descriptor Set for the best (Joliet preferred) PVD.
fn find_volume(f: &mut File) -> Result<VolumeInfo, LockError> {
    let mut best: Option<VolumeInfo> = None;

    for sector in SYSTEM_AREA_SECTORS.. {
        let buf = read_sector(f, sector as u32)
            .map_err(|e| LockError::ExtractionFailed(format!("ISO read sector {}: {}", sector, e)))?;

        let vd_type = buf[0];
        if vd_type == 0xFF {
            break; // Volume Descriptor Set Terminator
        }
        if &buf[1..6] != b"CD001" {
            break; // Not a valid VD
        }

        match vd_type {
            0x01 => {
                // Primary Volume Descriptor — always accept as fallback
                let root_lba = le32(&buf, 156 + 2);
                let root_size = le32(&buf, 156 + 10);
                if best.is_none() {
                    best = Some(VolumeInfo { root_lba, root_size, joliet: false });
                }
            }
            0x02 => {
                // Supplementary Volume Descriptor — check for Joliet escape sequences
                let escape = &buf[88..120];
                let is_joliet = escape.windows(3).any(|w| {
                    matches!(w, b"%/@" | b"%/C" | b"%/E")
                });
                if is_joliet {
                    let root_lba = le32(&buf, 156 + 2);
                    let root_size = le32(&buf, 156 + 10);
                    // Joliet is preferred — overwrite any previous entry
                    best = Some(VolumeInfo { root_lba, root_size, joliet: true });
                }
            }
            _ => {}
        }
    }

    best.ok_or_else(|| LockError::ExtractionFailed("No valid ISO 9660 volume descriptor found".to_string()))
}

// ── Directory record parsing ─────────────────────────────────────────────────

#[derive(Debug)]
struct DirEntry {
    name: String,
    lba: u32,
    size: u32,
    is_dir: bool,
}

fn parse_dir_record(buf: &[u8], joliet: bool) -> Option<DirEntry> {
    let rec_len = buf[0] as usize;
    if rec_len < 34 {
        return None;
    }
    let flags = buf[25];
    let is_dir = (flags & 0x02) != 0;
    let lba = le32(buf, 2);
    let size = le32(buf, 10);
    let name_len = buf[32] as usize;
    if rec_len < 33 + name_len {
        return None;
    }
    let name_bytes = &buf[33..33 + name_len];

    // Skip "." and ".." entries
    if name_bytes == b"\x00" || name_bytes == b"\x01" {
        return None;
    }

    let name = if joliet {
        // Joliet: UCS-2 Big Endian
        let chars: Vec<u16> = name_bytes
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&chars)
    } else {
        // ISO 9660: ASCII, strip version suffix ";1"
        let s = String::from_utf8_lossy(name_bytes);
        s.trim_end_matches(|c: char| c == ';' || c.is_ascii_digit())
            .trim_end_matches(';')
            .to_string()
    };

    // Strip trailing dot from directory names (ISO 9660 quirk)
    let name = name.trim_end_matches('.').to_string();
    if name.is_empty() {
        return None;
    }

    Some(DirEntry { name, lba, size, is_dir })
}

fn read_dir(f: &mut File, lba: u32, size: u32, joliet: bool) -> Result<Vec<DirEntry>, LockError> {
    let mut entries = Vec::new();
    let mut remaining = size as usize;
    let mut current_lba = lba;

    while remaining > 0 {
        let buf = read_sector(f, current_lba)
            .map_err(|e| LockError::ExtractionFailed(format!("ISO dir read: {}", e)))?;

        let mut off = 0usize;
        let to_read = remaining.min(SECTOR_SIZE as usize);

        while off < to_read {
            let rec_len = buf[off] as usize;
            if rec_len == 0 {
                // Padding — skip to next sector
                break;
            }
            if let Some(entry) = parse_dir_record(&buf[off..], joliet) {
                entries.push(entry);
            }
            off += rec_len;
        }

        remaining = remaining.saturating_sub(SECTOR_SIZE as usize);
        current_lba += 1;
    }

    Ok(entries)
}

// ── Recursive walk ───────────────────────────────────────────────────────────

fn walk_dir(
    f: &mut File,
    lba: u32,
    size: u32,
    prefix: &str,
    joliet: bool,
    out: &mut Vec<ArchiveEntryInfo>,
) -> Result<(), LockError> {
    let entries = read_dir(f, lba, size, joliet)?;
    for entry in entries {
        let full_name = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", prefix, entry.name)
        };

        if entry.is_dir {
            out.push(ArchiveEntryInfo {
                name: format!("{}/", full_name),
                is_directory: true,
                size: -1,
            });
            walk_dir(f, entry.lba, entry.size, &full_name, joliet, out)?;
        } else {
            out.push(ArchiveEntryInfo {
                name: full_name,
                is_directory: false,
                size: entry.size as i64,
            });
        }
    }
    Ok(())
}

// ── ExtractionBackend impl ───────────────────────────────────────────────────

impl ExtractionBackend for IsoBackend {
    fn extract(&self, archive_path: &Path, output_dir: &Path) -> Result<(), LockError> {
        self.extract_with_progress(archive_path, output_dir, |_, _, _| {})
    }

    fn list_entries(&self, archive_path: &Path) -> Result<Vec<String>, LockError> {
        Ok(self.list_entries_info(archive_path)?
            .into_iter()
            .filter(|e| !e.is_directory)
            .map(|e| e.name)
            .collect())
    }

    fn list_entries_info(&self, archive_path: &Path) -> Result<Vec<ArchiveEntryInfo>, LockError> {
        let mut f = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let vol = find_volume(&mut f)?;
        let mut entries = Vec::new();
        walk_dir(&mut f, vol.root_lba, vol.root_size, "", vol.joliet, &mut entries)?;
        Ok(entries)
    }

    fn extract_with_progress<F>(
        &self,
        archive_path: &Path,
        output_dir: &Path,
        progress_fn: F,
    ) -> Result<(), LockError>
    where
        F: Fn(usize, usize, &str),
    {
        let mut f = File::open(archive_path)
            .map_err(|e| LockError::PathNotFound(e.to_string()))?;
        let vol = find_volume(&mut f)?;

        let mut all_entries = Vec::new();
        walk_dir(&mut f, vol.root_lba, vol.root_size, "", vol.joliet, &mut all_entries)?;

        let files: Vec<&ArchiveEntryInfo> = all_entries.iter().filter(|e| !e.is_directory).collect();
        let total = files.len();

        // Re-walk to extract (we need lba/size per file — re-collect with metadata)
        let mut file_meta: Vec<(String, u32, u32)> = Vec::new();
        collect_file_meta(&mut f, vol.root_lba, vol.root_size, "", vol.joliet, &mut file_meta)?;

        for (idx, (name, lba, size)) in file_meta.iter().enumerate() {
            progress_fn(idx, total, name);

            let out_path = super::safe_join(output_dir, name)?;
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| LockError::Unknown(format!("mkdir: {}", e)))?;
            }

            extract_file(&mut f, *lba, *size, &out_path)?;
        }

        if total > 0 {
            progress_fn(total, total, "");
        }

        Ok(())
    }
}

fn collect_file_meta(
    f: &mut File,
    lba: u32,
    size: u32,
    prefix: &str,
    joliet: bool,
    out: &mut Vec<(String, u32, u32)>,
) -> Result<(), LockError> {
    let entries = read_dir(f, lba, size, joliet)?;
    for entry in entries {
        let full_name = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", prefix, entry.name)
        };
        if entry.is_dir {
            collect_file_meta(f, entry.lba, entry.size, &full_name, joliet, out)?;
        } else {
            out.push((full_name, entry.lba, entry.size));
        }
    }
    Ok(())
}

fn extract_file(f: &mut File, lba: u32, size: u32, out_path: &Path) -> Result<(), LockError> {
    f.seek(SeekFrom::Start(lba as u64 * SECTOR_SIZE))
        .map_err(|e| LockError::ExtractionFailed(format!("seek: {}", e)))?;

    let mut out_file = File::create(out_path)
        .map_err(|e| LockError::Unknown(format!("create '{}': {}", out_path.display(), e)))?;

    let mut remaining = size as usize;
    let mut buf = [0u8; 4096];

    while remaining > 0 {
        let to_read = remaining.min(buf.len());
        let n = f.read(&mut buf[..to_read])
            .map_err(|e| LockError::ExtractionFailed(format!("read: {}", e)))?;
        if n == 0 {
            break;
        }
        io::Write::write_all(&mut out_file, &buf[..n])
            .map_err(|e| LockError::ExtractionFailed(format!("write: {}", e)))?;
        remaining -= n;
    }

    Ok(())
}
