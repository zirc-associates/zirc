#!/bin/bash
# REPL launcher script for Zirc language (Unix)

echo "[REPL] Starting Zirc REPL (VM backend, canonical)..."
cargo run -q -p zirc-cli --bin zirc-repl
