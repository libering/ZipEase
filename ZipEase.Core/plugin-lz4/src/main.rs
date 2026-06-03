use std::io::{self, BufRead, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};

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
        let original_name = archive_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extracted_file")
            .to_string();

        match req.action.as_str() {
            "list" => {
                let entries = vec![PluginEntry {
                    name: original_name,
                    is_dir: false,
                    size: -1, // Unknown uncompressed size for single-file lz4
                }];

                let response = serde_json::json!({
                    "status": "ok",
                    "entries": entries
                });

                if let Ok(json_str) = serde_json::to_string(&response) {
                    println!("{}", json_str);
                    let _ = io::stdout().flush();
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

                // Use safe_join to prevent path traversal
                let dest_path = match zipease_extract::extract::safe_join(output_path, &original_name) {
                    Ok(p) => p,
                    Err(e) => {
                        send_error(&format!("Path safety violation: {:?}", e));
                        return;
                    }
                };

                // Ensure parent directory exists
                if let Some(parent) = dest_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                let input_file = match std::fs::File::open(archive_path) {
                    Ok(f) => f,
                    Err(e) => {
                        send_error(&format!("Failed to open archive: {}", e));
                        return;
                    }
                };

                let mut output_file = match std::fs::File::create(&dest_path) {
                    Ok(f) => f,
                    Err(e) => {
                        send_error(&format!("Failed to create destination file: {}", e));
                        return;
                    }
                };

                let mut decoder = lz4_flex::frame::FrameDecoder::new(input_file);
                match io::copy(&mut decoder, &mut output_file) {
                    Ok(_) => {
                        // Report 100% progress
                        let progress_msg = serde_json::json!({
                            "status": "progress",
                            "pct": 100,
                            "file": original_name
                        });
                        if let Ok(json_str) = serde_json::to_string(&progress_msg) {
                            println!("{}", json_str);
                            let _ = io::stdout().flush();
                        }

                        // Complete
                        let response = serde_json::json!({
                            "status": "done",
                            "count": 1
                        });
                        if let Ok(json_str) = serde_json::to_string(&response) {
                            println!("{}", json_str);
                            let _ = io::stdout().flush();
                        }
                    }
                    Err(e) => {
                        send_error(&format!("LZ4 decompression failed: {}", e));
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_list_action_parsing() {
        let archive_path = Path::new("C:/path/to/archive.lz4");
        let original_name = archive_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extracted_file");
        assert_eq!(original_name, "archive");
    }

    #[test]
    fn test_lz4_compression_decompression() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("source.txt");
        let archive_path = dir.path().join("source.lz4");
        let dest_dir = dir.path().join("out");

        let content = b"Hello, ZipEase LZ4 test!";
        {
            let mut f = File::create(&input_path).unwrap();
            f.write_all(content).unwrap();
        }

        {
            let f_in = File::open(&input_path).unwrap();
            let f_out = File::create(&archive_path).unwrap();
            let mut encoder = lz4_flex::frame::FrameEncoder::new(f_out);
            let mut reader = f_in;
            std::io::copy(&mut reader, &mut encoder).unwrap();
            encoder.finish().unwrap();
        }

        let dest_path = zipease_extract::extract::safe_join(&dest_dir, "source").unwrap();
        std::fs::create_dir_all(dest_path.parent().unwrap()).unwrap();
        
        let f_archive = File::open(&archive_path).unwrap();
        let mut f_dest = File::create(&dest_path).unwrap();
        let mut decoder = lz4_flex::frame::FrameDecoder::new(f_archive);
        std::io::copy(&mut decoder, &mut f_dest).unwrap();

        let decompressed = std::fs::read(&dest_path).unwrap();
        assert_eq!(decompressed, content);
    }
}

