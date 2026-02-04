#!/usr/bin/env python3
"""
Debug wrapper for hudl-lsp.
Logs all LSP traffic to /tmp/hudl-lsp-debug.log
"""

import os
import sys
import subprocess
import threading
import datetime
import io

LOG_FILE = "/tmp/hudl-lsp-debug.log"
LSP_PATH = os.path.expanduser("~/bin/hudl-lsp")

log_lock = threading.Lock()

def log(direction: str, data: bytes):
    with log_lock:
        with open(LOG_FILE, "a") as f:
            timestamp = datetime.datetime.now().isoformat()
            f.write(f"\n=== {timestamp} {direction} ({len(data)} bytes) ===\n")
            try:
                f.write(data.decode("utf-8", errors="replace"))
            except:
                f.write(f"<binary: {data.hex()}>")
            f.write("\n")
            f.flush()

def read_lsp_message(stream):
    """Read a complete LSP message from a stream. Returns (header, content) or (None, None) on EOF."""
    # Read headers line by line until we hit \r\n\r\n
    headers = b""
    while True:
        byte = stream.read(1)
        if not byte:
            return None, None
        headers += byte
        if headers.endswith(b"\r\n\r\n"):
            break

    # Parse Content-Length
    content_length = 0
    for line in headers.decode("utf-8", errors="replace").split("\r\n"):
        if line.lower().startswith("content-length:"):
            content_length = int(line.split(":", 1)[1].strip())
            break

    if content_length == 0:
        return headers, b""

    # Read exact content length
    content = b""
    remaining = content_length
    while remaining > 0:
        chunk = stream.read(remaining)
        if not chunk:
            return None, None
        content += chunk
        remaining -= len(chunk)

    return headers, content

def forward_stdin(proc):
    """Forward stdin to the LSP process, logging along the way."""
    try:
        while True:
            header, content = read_lsp_message(sys.stdin.buffer)
            if header is None:
                break

            message = header + content
            log("EDITOR -> LSP", message)

            proc.stdin.write(message)
            proc.stdin.flush()
    except Exception as e:
        log("ERROR", f"stdin forward error: {e}".encode())

def forward_stdout(proc):
    """Forward LSP stdout to the editor, logging along the way."""
    try:
        while True:
            header, content = read_lsp_message(proc.stdout)
            if header is None:
                break

            message = header + content
            log("LSP -> EDITOR", message)

            sys.stdout.buffer.write(message)
            sys.stdout.buffer.flush()
    except Exception as e:
        log("ERROR", f"stdout forward error: {e}".encode())

def main():
    import signal

    with open(LOG_FILE, "a") as f:
        f.write(f"\n\n{'='*60}\n")
        f.write(f"Session started: {datetime.datetime.now().isoformat()}\n")
        f.write(f"LSP: {LSP_PATH}\n")
        f.write(f"{'='*60}\n")

    proc = subprocess.Popen(
        [LSP_PATH],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    # Handle signals to ensure clean shutdown
    def signal_handler(signum, frame):
        log("SIGNAL", f"Received signal {signum}, terminating LSP".encode())
        proc.terminate()
        sys.exit(0)

    signal.signal(signal.SIGTERM, signal_handler)
    signal.signal(signal.SIGINT, signal_handler)

    # Forward stderr to log
    def log_stderr():
        try:
            for line in proc.stderr:
                log("LSP STDERR", line)
        except:
            pass

    stdin_thread = threading.Thread(target=forward_stdin, args=(proc,), daemon=True)
    stdout_thread = threading.Thread(target=forward_stdout, args=(proc,), daemon=True)
    stderr_thread = threading.Thread(target=log_stderr, daemon=True)

    stdin_thread.start()
    stdout_thread.start()
    stderr_thread.start()

    try:
        proc.wait()
    except:
        proc.terminate()

    log("INFO", b"LSP process exited")

if __name__ == "__main__":
    main()
