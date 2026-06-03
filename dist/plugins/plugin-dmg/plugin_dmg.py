#!/usr/bin/env python3
import sys
import json
import os
import subprocess
import shutil

def send_error(msg):
    print(json.dumps({"status": "error", "message": msg}), flush=True)

def find_7z():
    plugin_dir = os.path.dirname(os.path.abspath(__file__))
    
    # 1. Check plugin local tools/7z.exe
    local_7z = os.path.join(plugin_dir, "tools", "7z.exe")
    if os.path.exists(local_7z):
        return local_7z
        
    # 2. Check system PATH for 7z
    system_7z = shutil.which("7z")
    if system_7z:
        return system_7z
        
    # 3. Check system PATH for 7za
    system_7za = shutil.which("7za")
    if system_7za:
        return system_7za
        
    return None

def main():
    try:
        line = sys.stdin.readline()
        if not line:
            return
        
        req = json.loads(line.strip())
        action = req.get("action")
        archive_path = req.get("path")
        
        if not archive_path:
            send_error("Archive path is required")
            return

        cmd_7z = find_7z()
        if not cmd_7z:
            send_error("7z or 7za executable not found. Please place 7z.exe in tools folder or install it in system PATH.")
            return

        if action == "list":
            # Run "7z l archive"
            res = subprocess.run([cmd_7z, "l", archive_path], capture_output=True, text=True)
            if res.returncode != 0:
                send_error(f"7z listing failed: {res.stderr or res.stdout}")
                return
            
            entries = []
            lines = res.stdout.splitlines()
            parse_start = False
            for l in lines:
                if l.startswith("-------------------"):
                    parse_start = not parse_start
                    continue
                if parse_start:
                    # Parse 7z columns: Date Time Attr Size Compressed Name
                    # Example: 2026-05-31 17:00:00 ....D          0            0  folder_name
                    # Example: 2026-05-31 17:00:00 ....         123          100  file.txt
                    parts = l.strip().split(None, 5)
                    if len(parts) >= 6:
                        attr = parts[2]
                        size_str = parts[3]
                        file_name = parts[5].replace("\\", "/")
                        
                        is_dir = 'D' in attr
                        try:
                            size = int(size_str)
                        except ValueError:
                            size = -1
                            
                        entries.append({
                            "name": file_name,
                            "is_dir": is_dir,
                            "size": size
                        })
            
            print(json.dumps({"status": "ok", "entries": entries}), flush=True)

        elif action == "extract":
            output_dir = req.get("output")
            if not output_dir:
                send_error("Output directory is required for extract action")
                return
            
            os.makedirs(output_dir, exist_ok=True)
            
            # Run "7z x -y -o{output_dir} {archive}"
            # For 7z, -o prefix must not have space
            res = subprocess.run(
                [cmd_7z, "x", "-y", f"-o{output_dir}", archive_path],
                capture_output=True,
                text=True
            )
            
            if res.returncode != 0:
                send_error(f"7z extraction failed: {res.stderr or res.stdout}")
                return
            
            # Report 100% progress
            print(json.dumps({"status": "progress", "pct": 100, "file": os.path.basename(archive_path)}), flush=True)
            
            # Done
            print(json.dumps({"status": "done", "count": 1}), flush=True)

        else:
            send_error(f"Unknown action: {action}")

    except Exception as e:
        send_error(str(e))

if __name__ == "__main__":
    main()
