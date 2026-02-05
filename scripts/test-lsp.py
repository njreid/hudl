#!/usr/bin/env python3
"""
Test the hudl-lsp binary directly with proper LSP protocol messages.
"""

import json
import os
import subprocess
import sys

LSP_PATH = os.path.expanduser("~/bin/hudl-lsp")

def send_message(proc, method, params=None, msg_id=None):
    """Send a JSON-RPC message to the LSP."""
    message = {
        "jsonrpc": "2.0",
        "method": method,
    }
    if msg_id is not None:
        message["id"] = msg_id
    if params is not None:
        message["params"] = params

    content = json.dumps(message)
    full_message = f"Content-Length: {len(content)}\r\n\r\n{content}"

    print(f">>> Sending: {method}", file=sys.stderr)
    proc.stdin.write(full_message.encode())
    proc.stdin.flush()

def read_message(proc):
    """Read a JSON-RPC message from the LSP."""
    # Read headers
    headers = {}
    while True:
        line = proc.stdout.readline().decode()
        if line == "\r\n" or line == "\n":
            break
        if ":" in line:
            key, value = line.split(":", 1)
            headers[key.strip()] = value.strip()

    content_length = int(headers.get("Content-Length", 0))
    if content_length == 0:
        return None

    content = proc.stdout.read(content_length).decode()
    return json.loads(content)

def main():
    print(f"Testing LSP: {LSP_PATH}", file=sys.stderr)

    proc = subprocess.Popen(
        [LSP_PATH],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    try:
        # 1. Initialize
        send_message(proc, "initialize", {
            "processId": os.getpid(),
            "rootUri": "file:///tmp/test",
            "capabilities": {}
        }, msg_id=1)

        response = read_message(proc)
        print(f"<<< Initialize response: {json.dumps(response, indent=2)}", file=sys.stderr)

        if response and response.get("error"):
            print(f"ERROR: {response['error']}", file=sys.stderr)
            return 1

        # 2. Initialized notification
        send_message(proc, "initialized", {})
        print("<<< (initialized notification sent)", file=sys.stderr)

        # 3. Open a document
        send_message(proc, "textDocument/didOpen", {
            "textDocument": {
                "uri": "file:///tmp/test.hudl",
                "languageId": "hudl",
                "version": 1,
                "text": 'el { div "hello" }'
            }
        })
        print("<<< (didOpen notification sent)", file=sys.stderr)

        # Give server time to process
        import time
        time.sleep(0.5)

        # 4. Request formatting
        send_message(proc, "textDocument/formatting", {
            "textDocument": {"uri": "file:///tmp/test.hudl"},
            "options": {"tabSize": 4, "insertSpaces": True}
        }, msg_id=2)

        # Read any notifications (diagnostics) then the response
        while True:
            response = read_message(proc)
            if response is None:
                break
            print(f"<<< Response: {json.dumps(response, indent=2)}", file=sys.stderr)
            if response.get("id") == 2:
                break

        # 5. Shutdown
        send_message(proc, "shutdown", None, msg_id=3)
        response = read_message(proc)
        print(f"<<< Shutdown response: {json.dumps(response, indent=2)}", file=sys.stderr)

        # 6. Exit
        send_message(proc, "exit", None)

        print("\nSUCCESS: LSP responded correctly", file=sys.stderr)
        return 0

    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1
    finally:
        proc.terminate()
        proc.wait()

if __name__ == "__main__":
    sys.exit(main())
