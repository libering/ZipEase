#!/usr/bin/env python3
import sys
import json
import os
import subprocess

def send_error(msg):
    print(json.dumps({"status": "error", "message": msg}), flush=True)

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

        plugin_dir = os.path.dirname(os.path.abspath(__file__))
        arj_path = os.path.join(plugin_dir, "tools", "arj.exe")

        if not os.path.exists(arj_path):
            send_error(f"arj.exe not found at {arj_path}. Please download arj.exe and place it in the tools folder.")
            return

        if action == "list":
            # arj l <archive>
            res = subprocess.run([arj_path, "v", archive_path], capture_output=True, text=True)
            if res.returncode != 0:
                send_error(f"arj listed with error: {res.stderr or res.stdout}")
                return
            
            entries = []
            lines = res.stdout.splitlines()
            parse_start = False
            for l in lines:
                if "--------------------" in l:
                    parse_start = not parse_start
                    continue
                if parse_start:
                    parts = l.strip().split()
                    if len(parts) >= 2:
                        file_name = parts[0].replace("\\", "/")
                        size_str = parts[1]
                        try:
                            size = int(size_str)
                        except ValueError:
                            size = -1
                        is_dir = file_name.endswith("/")
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
            
            # arj x -y <archive>
            res = subprocess.run(
                [arj_path, "x", "-y", os.path.abspath(archive_path)],
                cwd=output_dir,
                capture_output=True,
                text=True
            )
            
            if res.returncode != 0:
                send_error(f"arj extraction failed: {res.stderr or res.stdout}")
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
