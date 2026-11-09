#!/bin/bash
# Examples runner script for Zirc language (Unix)
# Runs the runnable examples explicitly. Skips:
# - examples/errors/*, test_errors.zirc, test_undefined.zirc (intentional errors)
# - examples/io/* (interactive prompt / writes files)
# - examples/modules/geometry.zirc (module library, no main)

set -e

EXAMPLES=(
  examples/basic/hello.zirc
  examples/basic/conditionals.zirc
  examples/basic/loops.zirc
  examples/basic/for_loop.zirc
  examples/basic/plus.zirc
  examples/basic/test_builtins.zirc
  examples/basic/test_builtins_vm.zirc
  examples/basic/error_demo.zirc
  examples/modules/main.zirc
  examples/others/factorial.zirc
  examples/others/print-test.zirc
  examples/others/scopes.zirc
)

echo "[EXAMPLES] Running Zirc examples..."
for file in "${EXAMPLES[@]}"; do
    echo "[RUN] $file"
    cargo run -q -p zirc-cli -- "$file"
    echo
done

echo "[EXAMPLES] All examples completed!"
