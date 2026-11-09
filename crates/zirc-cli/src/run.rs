//! Shared execution pipeline for the Zirc CLI.
//!
//! Every entry point (file runner in `main.rs`, both REPL loops in
//! `repl.rs`) goes through the helpers here instead of repeating the
//! lex → parse → run/compile chain. The bytecode VM is the canonical
//! backend; the tree-walking interpreter is available as the
//! reference backend via `--backend interp`.

use std::path::Path;

use owo_colors::OwoColorize;
use zirc_compiler::Compiler;
use zirc_interpreter::{Env, Interpreter};
use zirc_syntax::ast::Program;
use zirc_syntax::error::{Error, ErrorKind, Result};
use zirc_vm::Vm;

pub use zirc_loader::lex_and_parse_with_modules;

/// Execution backend. The VM is canonical and therefore the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    /// Bytecode VM (canonical, default).
    #[default]
    Vm,
    /// Tree-walking interpreter (reference backend).
    Interp,
}

impl Backend {
    /// Parse a backend name: `vm` (default), `interp` or `interpreter`.
    pub fn parse(name: &str) -> Option<Backend> {
        match name {
            "vm" => Some(Backend::Vm),
            "interp" | "interpreter" => Some(Backend::Interp),
            _ => None,
        }
    }

    /// Short name used in help and error messages.
    pub fn name(self) -> &'static str {
        match self {
            Backend::Vm => "vm",
            Backend::Interp => "interp",
        }
    }
}

/// Resolve the backend from (in priority order): an explicit flag
/// value, the `ZIRC_BACKEND` environment variable, or the default.
pub fn resolve_backend(flag: Option<&str>) -> Result<Backend> {
    let raw = flag
        .map(str::to_string)
        .or_else(|| std::env::var("ZIRC_BACKEND").ok());
    match raw {
        None => Ok(Backend::default()),
        Some(name) => Backend::parse(&name).ok_or_else(|| {
            Error::of_kind(
                ErrorKind::General,
                format!("Unknown backend '{}' (expected 'vm' or 'interp')", name),
            )
        }),
    }
}
// Import expansion lives in the shared `zirc-loader` crate
// (re-exported above); `lex_and_parse`/`lex_and_parse_with_modules`
// are not duplicated here.

/// Evaluate a program with the interpreter backend, keeping the
/// caller's environment (REPL session state).
pub fn eval_interp(
    interp: &mut Interpreter,
    env: &mut Env<'_>,
    program: Program,
) -> Result<Option<zirc_interpreter::Value>> {
    interp.run_with_env(program, env)
}

/// Compile and run a program with the VM backend.
pub fn eval_vm(
    compiler: &mut Compiler,
    vm: &mut Vm,
    program: Program,
) -> Result<Option<zirc_bytecode::Value>> {
    let bprog = compiler.compile(program)?;
    vm.run(&bprog)
}

/// Evaluate source text with the requested backend (one-shot).
/// Used by the file runner; the REPL evaluates incrementally instead.
#[allow(dead_code)]
pub fn eval_source(
    backend: Backend,
    interp: &mut Interpreter,
    env: &mut Env<'_>,
    compiler: &mut Compiler,
    vm: &mut Vm,
    source: &str,
) -> Result<Option<zirc_bytecode::Value>> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let program = lex_and_parse_with_modules(source, &cwd)?;
    match backend {
        Backend::Vm => eval_vm(compiler, vm, program),
        Backend::Interp => eval_interp(interp, env, program),
    }
}

/// Read a source file and run it with the requested backend.
/// Imports are resolved relative to the file's directory.
#[allow(dead_code)]
pub fn run_file(path: &Path, backend: Backend) -> Result<()> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        Error::of_kind(
            ErrorKind::Io,
            format!("Failed to read {}: {}", path.display(), e),
        )
    })?;
    let base_dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let program = lex_and_parse_with_modules(&source, &base_dir)?;
    let mut interp = Interpreter::new();
    let mut env = Env::new_root();
    let mut compiler = Compiler::new();
    let mut vm = Vm::new();
    match backend {
        Backend::Vm => eval_vm(&mut compiler, &mut vm, program)?,
        Backend::Interp => eval_interp(&mut interp, &mut env, program)?,
    };
    Ok(())
}

/// Compile a file to bytecode and return its disassembly.
#[allow(dead_code)]
pub fn disassemble_file(path: &Path) -> Result<String> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        Error::of_kind(
            ErrorKind::Io,
            format!("Failed to read {}: {}", path.display(), e),
        )
    })?;
    let base_dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let program = lex_and_parse_with_modules(&source, &base_dir)?;
    let mut compiler = Compiler::new();
    let bprog = compiler.compile(program)?;
    Ok(zirc_bytecode::disassemble(&bprog))
}

/// Render an error with source context and a contextual hint.
/// This is the single error renderer used by `main` and the REPL.
pub fn render_error(kind: &str, source: &str, err: &Error) {
    eprintln!("{}: {}", kind.red().bold(), err.msg.red());
    if let (Some(line), Some(col)) = (err.line, err.col) {
        eprintln!("  --> line {}, column {}", line, col);
        if let Some(src_line) = source.lines().nth(line.saturating_sub(1)) {
            let line_num_str = format!("{:3} | ", line);
            eprintln!("     |");
            eprintln!("{}{}", line_num_str.bright_black(), src_line);
            let mut marker = " ".repeat(line_num_str.len());
            if col > 1 {
                marker.push_str(&" ".repeat(col - 1));
            }
            marker.push('^');
            eprintln!("{}{}", marker.red(), " error here".red());
            eprintln!("     |");
        }
    }
    crate::common::provide_error_suggestions(err);
}
