//! `zirc-lint` binary: run static checks over `.zirc` files.
//!
//! Usage: `zirc-lint <file.zirc> [...]`. Exits 1 when warnings are
//! found (like most linters), 0 when clean.

use clap::Parser;
use std::path::PathBuf;

use zirc_lexer::Lexer;
use zirc_parser::Parser as ZircParser;

#[derive(Parser, Debug)]
#[command(
    name = "zirc-lint",
    version,
    about = "Static analysis for Zirc programs"
)]
struct Args {
    /// `.zirc` files to check.
    files: Vec<PathBuf>,
}

fn main() {
    let args = Args::parse();
    if args.files.is_empty() {
        eprintln!("Usage: zirc-lint <file.zirc> [...]");
        std::process::exit(2);
    }
    let mut total = 0;
    for path in &args.files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: failed to read: {}", path.display(), e);
                total += 1;
                continue;
            }
        };
        let mut lexer = Lexer::new(&source);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{}: lex error: {}", path.display(), e);
                total += 1;
                continue;
            }
        };
        let mut parser = ZircParser::new(tokens);
        let program = match parser.parse_program() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}: parse error: {}", path.display(), e);
                total += 1;
                continue;
            }
        };
        let warnings = zirc_lint::lint_program(&program);
        for w in &warnings {
            eprintln!("{}: [{}] {}", path.display(), w.rule, w.message);
        }
        total += warnings.len();
    }
    if total > 0 {
        eprintln!("{} warning(s) found", total);
        std::process::exit(1);
    }
}
