mod common;
mod repl;
mod run;

use clap::Parser;
use run::Backend;

/// Zirc programming language runner and REPL.
///
/// The bytecode VM is the canonical backend (default); the
/// tree-walking interpreter is available with `--backend interp`.
#[derive(Parser, Debug)]
#[command(name = "zirc", version, about = "Run Zirc programs or start the REPL")]
struct Args {
    /// Execution backend: `vm` (canonical, default) or `interp`.
    /// Falls back to the `ZIRC_BACKEND` environment variable.
    #[arg(short, long, value_name = "BACKEND")]
    backend: Option<String>,

    /// Compile to bytecode and print the disassembly instead of running.
    #[arg(short = 'd', long = "disassemble")]
    disassemble: bool,

    /// Path to a `.zirc` program to run. Without a path, the REPL starts.
    path: Option<String>,
}

fn main() {
    let args = Args::parse();

    let backend = match run::resolve_backend(args.backend.as_deref()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}", e.msg);
            std::process::exit(2);
        }
    };

    let Some(path_str) = args.path else {
        repl::start_repl_with_backend(backend);
        return;
    };

    let path = std::path::PathBuf::from(&path_str);
    if !path.exists() {
        eprintln!("error: File not found: {}", path_str);
        std::process::exit(1);
    }

    if args.disassemble {
        match run::disassemble_file(&path) {
            Ok(text) => print!("{text}"),
            Err(e) => {
                run::render_error("Compile error", "", &e);
                std::process::exit(1);
            }
        }
        return;
    }

    if let Err(e) = run::run_file(&path, backend) {
        let source = std::fs::read_to_string(&path).unwrap_or_default();
        let kind = match backend {
            Backend::Vm => "VM error",
            Backend::Interp => "Runtime error",
        };
        // Classify phase errors by kind for a precise label.
        let kind = match e.kind {
            zirc_syntax::error::ErrorKind::Lex => "Lex error",
            zirc_syntax::error::ErrorKind::Parse => "Parse error",
            zirc_syntax::error::ErrorKind::Compile => "Compile error",
            _ => kind,
        };
        run::render_error(kind, &source, &e);
        std::process::exit(1);
    }
}
