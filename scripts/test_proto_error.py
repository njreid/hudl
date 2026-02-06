#!/usr/bin/env python3
import json
import os
import subprocess
import sys
import time

LSP_PATH = "./target/debug/hudl-lsp"

def send_message(proc, method, params=None, msg_id=None):
    message = {"jsonrpc": "2.0", "method": method}
    if msg_id is not None: message["id"] = msg_id
    if params is not None: message["params"] = params
    content = json.dumps(message)
    # Using separate parts to avoid issues with escaped characters in write_file
    header = "Content-Length: " + str(len(content)) + "\r\n\r\n"
    proc.stdin.write(header.encode())
    proc.stdin.write(content.encode())
    proc.stdin.flush()

def read_message(proc):
    headers = {}
    while True:
        line = proc.stdout.readline().decode()
        if not line or line == "\r\n" or line == "\n": break
        if ":" in line:
            key, value = line.split(":", 1)
            headers[key.strip()] = value.strip()
    
    content_length = int(headers.get("Content-Length", 0))
    if content_length == 0: return None
    return json.loads(proc.stdout.read(content_length).decode())

def main():
    print("Building LSP...")
    subprocess.run(["cargo", "build", "--manifest-path", "lsp/Cargo.toml"], check=True)

    proc = subprocess.Popen(["./lsp/target/debug/hudl-lsp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=sys.stderr)
    
    # 1. Initialize
    print("Initializing...")
    send_message(proc, "initialize", {"rootUri": "file://" + os.getcwd()}, msg_id=1)
    init_res = read_message(proc)
    print(f"Init response: {init_res}")
    send_message(proc, "initialized", {})

    # 2. Open document with proto error
    error_content = """/**
message ErrorData {
    string name = 1;
    invalid field syntax here;
}
*/
el { div `name` }
"""
    print("Opening document with proto error...")
    send_message(proc, "textDocument/didOpen", {
        "textDocument": {
            "uri": "file:///tmp/error.hudl",
            "languageId": "hudl",
            "version": 1,
            "text": error_content
        }
    })

    # 3. Wait for diagnostics
    print("Waiting for diagnostics...")
    found = False
    for _ in range(20):
        msg = read_message(proc)
        if msg and msg.get("method") == "textDocument/publishDiagnostics":
            print(json.dumps(msg, indent=2))
            diags = msg["params"]["diagnostics"]
            if any("Proto error: Syntax error on line 4" in d["message"] for d in diags):
                print("\nSUCCESS: Found expected proto syntax error diagnostic!")
                found = True
                break
        time.sleep(0.1)

    proc.terminate()
    if found:
        return 0
    else:
        print("\nFAILURE: Did not find expected proto syntax error diagnostic")
        return 1

if __name__ == "__main__":
    sys.exit(main())