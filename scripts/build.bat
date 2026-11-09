@echo off
REM Build script for Zirc language (Windows)

echo [BUILD] Building Zirc workspace...
cargo build --workspace --release
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Build failed!
    exit /b 1
)

echo [BUILD] Build completed successfully!
echo [INFO] Binaries available at: target/release/zirc.exe, target/release/zirc-repl.exe, target/release/zirc-lint.exe, target/release/zirc-lsp.exe, target/release/zirc-fmt.exe
