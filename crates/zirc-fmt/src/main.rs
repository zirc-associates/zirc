//! `zirc-fmt`: formatter for Zirc source files.
//!
//! Parses a `.zirc` file into the AST and prints it back in canonical
//! form. Usage: `zirc-fmt [--check|--write] <file.zirc>`. `~` line
//! comments are preserved (see `zirc-fmt` library docs).

use std::env;
use std::fs;
use std::path::PathBuf;

use zirc_fmt::format_program;
use zirc_lexer::Lexer;
use zirc_parser::Parser;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("Usage: zirc-fmt [--check|--write] <file.zirc>");
        std::process::exit(2);
    }
    let mut check = false;
    let mut write = false;
    let mut file = None;
    while let Some(a) = args.first().cloned() {
        if a == "--check" {
            check = true;
            args.remove(0);
        } else if a == "--write" {
            write = true;
            args.remove(0);
        } else {
            file = Some(PathBuf::from(a));
            args.remove(0);
            break;
        }
    }
    let file = file.expect("file required");
    let src = fs::read_to_string(&file).expect("read file");
    let mut lexer = Lexer::new(&src);
    let tokens = lexer.tokenize().unwrap_or_else(|e| {
        eprintln!("Lex error: {}", e);
        std::process::exit(1)
    });
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap_or_else(|e| {
        eprintln!("Parse error: {}", e);
        std::process::exit(1)
    });

    let formatted = format_program(&program);

    if check {
        if normalize_newlines(&formatted) != normalize_newlines(&src) {
            eprintln!("{}: not formatted", file.display());
            std::process::exit(1);
        } else {
            println!("{}: ok", file.display());
        }
    } else if write {
        fs::write(&file, formatted).expect("write");
    } else {
        print!("{}", formatted);
    }
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}
