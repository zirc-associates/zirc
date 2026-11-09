//! Interactive REPL for the Zirc language.
//!
//! A single loop holds both backend sessions (VM canonical,
//! interpreter reference) and evaluates through the shared
//! pipeline in [`crate::run`]; `:backend` switches between them
//! at runtime, `:dis` shows the bytecode of the buffered input.
//!
//! Input is syntax-highlighted: after each line is read on a TTY,
//! the echoed line is re-rendered with ANSI colors (keywords,
//! strings, numbers, comments). Highlighting is a no-op for
//! piped (non-TTY) input.

use std::io::{self, IsTerminal, Write};

#[cfg(windows)]
use windows::Win32::System::Console::{GetConsoleCP, SetConsoleOutputCP};

use owo_colors::OwoColorize;
use zirc_interpreter::{Env, Interpreter, MemoryStats, Value};
use zirc_lexer::Lexer;
use zirc_syntax::token::TokenKind;

use zirc_compiler::Compiler;
use zirc_vm::Vm;

use crate::run::{self, Backend};

pub fn start_repl_with_backend(backend: Backend) {
    // Banner and prompts only make sense on an interactive TTY;
    // piped input gets clean, results-only output.
    if io::stdin().is_terminal() {
        let version = env!("CARGO_PKG_VERSION");
        println!(
            "{}",
            format!(
                "Zirc v{} REPL (backend: {}). Type :help for help, :quit to exit.",
                version,
                backend.name()
            )
            .bold()
            .bright_black()
        );
        println!();
    }
    repl_loop(backend);
}

/// Set the Windows console to UTF-8 so REPL output renders correctly.
/// No-op on other platforms.
fn init_console() {
    #[cfg(windows)]
    {
        let utf8_cp: u32 = 65001;
        let result = unsafe { SetConsoleOutputCP(utf8_cp) };
        if !result.as_bool() {
            let err_code = unsafe { GetConsoleCP() };
            eprintln!(
                "{}",
                format!(
                    "Failed to set console code page to UTF-8. Error code: {}",
                    err_code
                )
                .red()
            );
        }
    }
}

/// Syntax-highlight one line of Zirc source (for the TTY echo).
fn highlight(line: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '~' {
            let text: String = chars[i..].iter().collect();
            out.push_str(&text.dimmed().to_string());
            break;
        }
        if c == '"' {
            let mut j = i + 1;
            let mut s = String::from("\"");
            while j < chars.len() && chars[j] != '"' {
                if chars[j] == '\\' && j + 1 < chars.len() {
                    s.push(chars[j]);
                    s.push(chars[j + 1]);
                    j += 2;
                } else {
                    s.push(chars[j]);
                    j += 1;
                }
            }
            if j < chars.len() {
                s.push('"');
                j += 1;
            }
            out.push_str(&s.green().to_string());
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            let mut s = String::new();
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                s.push(chars[j]);
                j += 1;
            }
            out.push_str(&s.cyan().to_string());
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let mut j = i;
            let mut s = String::new();
            while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                s.push(chars[j]);
                j += 1;
            }
            let kw = matches!(
                s.as_str(),
                "fun"
                    | "end"
                    | "if"
                    | "else"
                    | "let"
                    | "while"
                    | "for"
                    | "in"
                    | "return"
                    | "break"
                    | "continue"
                    | "module"
                    | "import"
                    | "as"
                    | "true"
                    | "false"
            );
            if kw {
                out.push_str(&s.magenta().bold().to_string());
            } else if zirc_bytecode::Builtin::from_name(&s).is_some() {
                out.push_str(&s.yellow().to_string());
            } else {
                out.push_str(&s);
            }
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Re-render the just-typed line in highlight colors (TTY only).
///
/// In cooked mode the terminal has already echoed the line and
/// moved to the next row. Move back up, clear that row, redraw
/// `prompt + highlighted line`, and finish with a newline so the
/// next prompt/result starts on a fresh line (never glued to the
/// input). Long lines that may have wrapped are left as typed:
/// moving up one row would only clear their last visual row.
fn rerender_line(prompt: &str, line: &str) {
    if !io::stdin().is_terminal() {
        return;
    }
    let text = line.trim_end();
    // prompt is ANSI-colored; its visible length is short. Skip
    // re-rendering lines long enough to risk wrapping.
    if text.chars().count() > 100 {
        return;
    }
    print!("\u{1b}[1A\u{1b}[2K{}{}", prompt, highlight(text));
    println!();
    let _ = io::stdout().flush();
}

fn repl_loop(mut backend: Backend) {
    init_console();
    let tty = io::stdin().is_terminal();
    let mut interpreter = Interpreter::new();
    let mut env = Env::new_root();
    let mut compiler = Compiler::new();
    let mut vm = Vm::new();
    let mut buffer = String::new();
    // Last evaluated input, kept so `:dis` can show its bytecode
    // after it has already been cleared from the buffer.
    let mut last_input = String::new();

    loop {
        let prompt = if buffer.is_empty() {
            match backend {
                Backend::Vm => "zirc(vm)> ".cyan().to_string(),
                Backend::Interp => "> ".white().to_string(),
            }
        } else {
            "... > ".bright_white().to_string()
        };
        if tty {
            print!("{}", prompt);
            let _ = io::stdout().flush();
        }

        let mut line = String::new();
        let n = match io::stdin().read_line(&mut line) {
            Ok(n) => n,
            Err(_) => {
                if tty {
                    println!("<input error>");
                }
                break;
            }
        };
        if n == 0 {
            if tty {
                println!("Goodbye.");
            }
            break;
        }
        rerender_line(&prompt, &line);
        let trimmed = line.trim_end().to_string();

        // Commands work even mid-buffer for the ones that make
        // sense there (:quit/:reset discard the buffer, :dis
        // inspects it); the rest only when the buffer is empty.
        if trimmed.starts_with(':') {
            match trimmed.as_str() {
                ":quit" | ":q" | ":exit" => {
                    if tty {
                        println!("Goodbye.");
                    }
                    break;
                }
                ":reset" => {
                    interpreter.reset();
                    env = Env::new_root();
                    compiler = Compiler::new();
                    vm = Vm::new();
                    buffer.clear();
                    println!("{}", "State reset.".green());
                    continue;
                }
                ":dis" => {
                    let target = if buffer.is_empty() {
                        &last_input
                    } else {
                        &buffer
                    };
                    let buffer = target.clone();
                    if buffer.is_empty() {
                        println!("{}", "Nothing to disassemble: type code first.".red());
                    } else {
                        let cwd = std::env::current_dir()
                            .unwrap_or_else(|_| std::path::PathBuf::from("."));
                        match run::lex_and_parse_with_modules(&buffer, &cwd) {
                            Ok(program) => match Compiler::new().compile(program) {
                                Ok(bprog) => print!("{}", zirc_bytecode::disassemble(&bprog)),
                                Err(e) => run::render_error("Compile error", &buffer, &e),
                            },
                            Err(e) => run::render_error("Parse error", &buffer, &e),
                        }
                    }
                    continue;
                }
                _ if buffer.is_empty() => match trimmed.as_str() {
                    ":help" | ":h" => {
                        print_help();
                    }
                    ":vars" => match backend {
                        Backend::Interp => print_vars_interp(&env),
                        Backend::Vm => print_vars_vm(&vm),
                    },
                    ":funcs" => match backend {
                        Backend::Interp => print_funcs_interp(&interpreter),
                        Backend::Vm => print_funcs_vm(&compiler),
                    },
                    ":mem" => match backend {
                        Backend::Interp => print_mem(&interpreter),
                        Backend::Vm => println!("{}", "<no memory stats in VM>".dimmed()),
                    },
                    ":backend" => println!("backend: {}", backend.name()),
                    ":backend vm" => {
                        backend = Backend::Vm;
                        println!("{}", "Backend: vm".green());
                    }
                    ":backend interp" | ":backend interpreter" => {
                        backend = Backend::Interp;
                        println!("{}", "Backend: interp".green());
                    }
                    _ => {
                        println!("{}", "Unknown command. Type :help.".red());
                    }
                },
                _ => {
                    println!("{}", "Unknown command. Type :help.".red());
                }
            }
            continue;
        }

        buffer.push_str(&line);
        if !is_complete(&buffer) {
            continue;
        }

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        match run::lex_and_parse_with_modules(&buffer, &cwd) {
            Ok(program) => match backend {
                Backend::Interp => match run::eval_interp(&mut interpreter, &mut env, program) {
                    Ok(last) => {
                        if let Some(val) = last {
                            if val != Value::Unit {
                                println!("{}", format!("{}", val).bright_blue());
                            }
                        }
                    }
                    Err(e) => run::render_error("Runtime error", &buffer, &e),
                },
                Backend::Vm => match run::eval_vm(&mut compiler, &mut vm, program) {
                    Ok(last) => {
                        if let Some(val) = last {
                            if val != Value::Unit {
                                println!("{}", format!("{}", val).bright_blue());
                            }
                        }
                    }
                    Err(e) => run::render_error("VM error", &buffer, &e),
                },
            },
            Err(e) => run::render_error("Parse error", &buffer, &e),
        }
        last_input = buffer.clone();
        buffer.clear();
    }
}

fn print_help() {
    println!("{}", "Commands:".bold());
    println!("  {}  Show this help", ":help".yellow());
    println!("  {}  Exit the REPL", ":quit".yellow());
    println!();
    println!("  {}  List top-level variables", ":vars".yellow());
    println!("  {}  List defined functions", ":funcs".yellow());
    println!("  {}  Memory stats (interpreter only)", ":mem".yellow());
    println!("  {}  Clear state (env/functions/mem)", ":reset".yellow());
    println!();
    println!("  {}  Show/switch backend (vm|interp)", ":backend".yellow());
    println!("  {}  Show bytecode of the last input", ":dis".yellow());
    println!();
    println!(
        "Type code to evaluate. Use 'fun...end' and 'if...end'. Multi-line input is supported."
    );
}

fn print_vars_interp(env: &Env) {
    let mut vars = env.vars_snapshot();
    vars.sort_by(|a, b| a.0.cmp(&b.0));
    if vars.is_empty() {
        println!("{}", "<no vars>".dimmed());
        return;
    }
    for (k, v) in vars {
        println!("{} = {}", k.yellow(), format!("{}", v).bright_blue());
    }
}

fn print_funcs_interp(interp: &Interpreter) {
    let names = interp.function_names();
    if names.is_empty() {
        println!("{}", "<no functions>".dimmed());
        return;
    }
    for n in names {
        println!("{}", n.yellow());
    }
}

fn print_vars_vm(vm: &Vm) {
    let vars = vm.globals_snapshot();
    if vars.is_empty() {
        println!("{}", "<no vars>".dimmed());
        return;
    }
    for (k, v) in vars {
        println!("{} = {}", k.yellow(), format!("{}", v).bright_blue());
    }
}

fn print_funcs_vm(compiler: &Compiler) {
    let names = compiler.function_names();
    if names.is_empty() {
        println!("{}", "<no functions>".dimmed());
        return;
    }
    for n in names {
        println!("{}", n.yellow());
    }
}

fn print_mem(interp: &Interpreter) {
    let MemoryStats {
        strings_allocated,
        bytes_allocated,
    } = interp.memory_stats();
    println!("{}: {}", "strings".yellow(), strings_allocated);
    println!("{}: {} bytes", "bytes".yellow(), bytes_allocated);
}

fn is_complete(input: &str) -> bool {
    let mut lexer = Lexer::new(input);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        // Only an unterminated string can be fixed by more input;
        // any other lex error will never complete, so evaluate it
        // (and show the error) instead of waiting forever.
        Err(e) => return !e.msg.contains("Unterminated"),
    };
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut starts = 0i32; // fun/if/while/for openers, each needs `end`
    let mut ends = 0i32;
    for tk in tokens.iter() {
        match &tk.kind {
            TokenKind::LParen => paren += 1,
            TokenKind::RParen => paren -= 1,
            TokenKind::LBracket => bracket += 1,
            TokenKind::RBracket => bracket -= 1,
            TokenKind::Fun | TokenKind::If | TokenKind::While | TokenKind::For => starts += 1,
            TokenKind::End => ends += 1,
            _ => {}
        }
    }
    // Balanced -> complete. Over-closed (extra `end`/`)`/`]`) can
    // never balance with more input -> complete too, so the error
    // is shown instead of waiting forever.
    paren < 0 || bracket < 0 || starts < ends || (paren == 0 && bracket == 0 && starts == ends)
}

#[cfg(test)]
mod tests {
    use super::highlight;

    #[test]
    fn highlight_colors_keywords_numbers_strings() {
        // ANSI codes are emitted by owo-colors; just check the
        // plain text survives highlighting unchanged in content.
        let out = highlight("let x = 42 ~ hi");
        assert!(out.contains("let"), "got: {out}");
        assert!(out.contains("42"), "got: {out}");
        assert!(out.contains("hi"), "got: {out}");
    }
}
