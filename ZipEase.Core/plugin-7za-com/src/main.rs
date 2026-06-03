use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use serde::{Deserialize, Serialize};
use zipease_extract::extract::ExtractionBackend;
use zipease_extract::extract::sevenzadll::SevenZaDllBackendWithClsid;

#[derive(Deserialize, Debug)]
struct PluginRequest {
    action: String,
    path: String,
    output: Option<String>,
    #[allow(dead_code)]
    password: Option<String>,
}

#[derive(Serialize)]
struct PluginEntry {
    name: String,
    is_dir: bool,
    size: i64,
}

fn main() {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();

    // Read the first line of JSON from stdin
    if let Ok(bytes_read) = handle.read_line(&mut line) {
        if bytes_read == 0 || line.trim().is_empty() {
            send_error("Empty request received");
            return;
        }

        let req: PluginRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                send_error(&format!("Invalid JSON request: {}", e));
                return;
            }
        };

        let archive_path = Path::new(&req.path);
        let ext = archive_path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        let clsid = match ext.as_str() {
            "xz" => Some(zipease_extract::extract::sevenzadll::CLSID_XZ_HANDLER),
            "lzma" => Some(zipease_extract::extract::sevenzadll::CLSID_LZMA_HANDLER),
            "wim" => Some(zipease_extract::extract::sevenzadll::CLSID_WIM_HANDLER),
            "vhd" | "vhdx" => Some(zipease_extract::extract::sevenzadll::CLSID_VHD_HANDLER),
            _ => None,
        };

        let clsid = match clsid {
            Some(c) => c,
            None => {
                send_error(&format!("Unsupported archive extension: .{}", ext));
                return;
            }
        };

        let backend = SevenZaDllBackendWithClsid(clsid);

        match req.action.as_str() {
            "list" => {
                match backend.list_entries_info(archive_path) {
                    Ok(entries) => {
                        let plugin_entries: Vec<PluginEntry> = entries
                            .into_iter()
                            .map(|e| PluginEntry {
                                name: e.name,
                                is_dir: e.is_directory,
                                size: e.size,
                            })
                            .collect();

                        let response = serde_json::json!({
                            "status": "ok",
                            "entries": plugin_entries
                        });

                        if let Ok(json_str) = serde_json::to_string(&response) {
                            println!("{}", json_str);
                            let _ = io::stdout().flush();
                        }
                    }
                    Err(e) => {
                        send_error(&format!("Failed to list archive: {:?}", e));
                    }
                }
            }
            "extract" => {
                let output_str = match &req.output {
                    Some(out) => out,
                    None => {
                        send_error("Extract action requires output path");
                        return;
                    }
                };
                let output_path = Path::new(output_str);

                let total_count = AtomicUsize::new(0);

                let result = backend.extract_with_progress(
                    archive_path,
                    output_path,
                    |current, total, file| {
                        total_count.store(total, Ordering::SeqCst);
                        let pct = if total > 0 {
                            ((current as f64 / total as f64) * 100.0) as u32
                        } else {
                            0
                        };
                        let progress_msg = serde_json::json!({
                            "status": "progress",
                            "pct": pct,
                            "file": file
                        });
                        if let Ok(json_str) = serde_json::to_string(&progress_msg) {
                            println!("{}", json_str);
                            let _ = io::stdout().flush();
                        }
                    },
                );

                match result {
                    Ok(_) => {
                        let response = serde_json::json!({
                            "status": "done",
                            "count": total_count.load(Ordering::SeqCst)
                        });
                        if let Ok(json_str) = serde_json::to_string(&response) {
                            println!("{}", json_str);
                            let _ = io::stdout().flush();
                        }
                    }
                    Err(e) => {
                        send_error(&format!("Extraction failed: {:?}", e));
                    }
                }
            }
            other => {
                send_error(&format!("Unknown action: {}", other));
            }
        }
    } else {
        send_error("Failed to read from stdin");
    }
}

fn send_error(msg: &str) {
    let err_msg = serde_json::json!({
        "status": "error",
        "message": msg
    });
    if let Ok(json_str) = serde_json::to_string(&err_msg) {
        println!("{}", json_str);
        let _ = io::stdout().flush();
    }
}
