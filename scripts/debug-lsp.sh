#!/usr/bin/env bash
# Debug wrapper for hudl-lsp
# Logs all stdin/stdout traffic to /tmp/hudl-lsp-debug.log

LOG="/tmp/hudl-lsp-debug.log"
LSP="${HOME}/bin/hudl-lsp"

echo "=== Session started at $(date) ===" >> "$LOG"

# Use tee to log both directions
# stdin -> lsp, log to file
# lsp stdout -> editor, log to file
exec > >(tee -a "${LOG}.out") 2>> "$LOG"
exec < <(tee -a "${LOG}.in")

exec "$LSP" "$@"
