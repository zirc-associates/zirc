use owo_colors::OwoColorize;
use zirc_syntax::error::{Error, ErrorKind};

/// Show a contextual hint for an error.
///
/// Matching is done primarily on the structured [`ErrorKind`]; the
/// message text is only consulted for finer-grained hints inside a
/// kind (e.g. which token was unexpected).
pub fn provide_error_suggestions(err: &Error) {
    let err_msg = err.msg.as_str();
    match err.kind {
        ErrorKind::Undefined => {
            if err_msg.contains("variable") {
                eprintln!(
                    "{}",
                    "💡 Help: Did you forget to declare this variable with 'let'?".yellow()
                );
                eprintln!("    {}", "Example: let my_var = 42".bright_black());
                return;
            }
            if err_msg.contains("function") {
                eprintln!(
                    "{}",
                    "💡 Help: Check if the function name is spelled correctly or if it's defined."
                        .yellow()
                );
                eprintln!("    {}", "Available built-ins: show, showf, len, abs, min, max, pow, sqrt, upper, lower, trim, split, join, int, str, type".bright_black());
                eprintln!(
                    "    {}",
                    "Example: fun my_func(x): x * 2 end".bright_black()
                );
                return;
            }
        }
        ErrorKind::Type => {
            eprintln!(
                "{}",
                "💡 Help: Make sure the value matches the declared type annotation.".yellow()
            );
            eprintln!(
                "    {}",
                "Zirc has types: int, string, bool, list, unit".bright_black()
            );
            eprintln!("    {}", "Example: let x: int = 42".bright_black());
            return;
        }
        ErrorKind::Io => {
            eprintln!("{}", "💡 Help: File operation failed.".yellow());
            eprintln!(
                "    {}",
                "Check if the file exists and you have permission to access it".bright_black()
            );
            return;
        }
        ErrorKind::Vm => {
            if err_msg.contains("division by zero") {
                eprintln!("{}", "💡 Help: You cannot divide by zero.".yellow());
                eprintln!(
                    "    {}",
                    "Check if the divisor is zero before the operation.".bright_black()
                );
                return;
            }
            if err_msg.contains("stack underflow") || err_msg.contains("stack overflow") {
                eprintln!(
                    "{}",
                    "💡 Help: Internal VM error - this might be a compiler bug.".yellow()
                );
                eprintln!(
                    "    {}",
                    "Try using the interpreter backend: zirc --backend interp file.zirc"
                        .bright_black()
                );
                return;
            }
        }
        ErrorKind::Lex | ErrorKind::Parse | ErrorKind::Compile => {}
        ErrorKind::Runtime | ErrorKind::General => {}
    }
    provide_suggestions_for_message(err_msg);
}

fn provide_suggestions_for_message(err_msg: &str) {
    // Variable-related errors
    if err_msg.contains("Undefined variable") {
        eprintln!(
            "{}",
            "💡 Help: Did you forget to declare this variable with 'let'?".yellow()
        );
        eprintln!("    {}", "Example: let my_var = 42".bright_black());
    }
    // Function-related errors
    else if err_msg.contains("Undefined function") {
        eprintln!(
            "{}",
            "💡 Help: Check if the function name is spelled correctly or if it's defined.".yellow()
        );
        eprintln!("    {}", "Available built-ins: show, showf, len, abs, min, max, pow, sqrt, upper, lower, trim, split, join, int, str, type".bright_black());
        eprintln!(
            "    {}",
            "Example: fun my_func(x): x * 2 end".bright_black()
        );

        // Suggest common typos
        if err_msg.contains("'show'") {
            eprintln!("    {}", "Did you mean: show() or showf()?".cyan());
        } else if err_msg.contains("'print'") {
            eprintln!(
                "    {}",
                "Did you mean: show() (Zirc uses 'show', not 'print')?".cyan()
            );
        } else if err_msg.contains("'len'") {
            eprintln!(
                "    {}",
                "Make sure you're calling it as: len(my_list) or len(my_string)".cyan()
            );
        }
    }
    // Type-related errors
    else if err_msg.contains("Type mismatch") {
        eprintln!(
            "{}",
            "💡 Help: Make sure the value matches the declared type annotation.".yellow()
        );
        eprintln!(
            "    {}",
            "Zirc has types: int, string, bool, list, unit".bright_black()
        );
        eprintln!("    {}", "Example: let x: int = 42".bright_black());
    }
    // Arithmetic errors
    else if err_msg.contains("Cannot add") {
        eprintln!(
            "{}",
            "💡 Help: Addition works with compatible types:".yellow()
        );
        eprintln!("    {}", "• Numbers: 5 + 3 = 8".bright_black());
        eprintln!(
            "    {}",
            "• Strings: \"hello\" + \" world\" = \"hello world\"".bright_black()
        );
        eprintln!(
            "    {}",
            "• Lists: [1, 2] + [3, 4] = [1, 2, 3, 4]".bright_black()
        );
    } else if err_msg.contains("Cannot subtract")
        || err_msg.contains("Cannot multiply")
        || err_msg.contains("Cannot divide")
    {
        eprintln!(
            "{}",
            "💡 Help: Arithmetic operations work only with numbers.".yellow()
        );
        eprintln!("    {}", "Example: 10 - 3, 4 * 5, 15 / 3".bright_black());
    } else if err_msg.contains("division by zero") {
        eprintln!("{}", "💡 Help: You cannot divide by zero.".yellow());
        eprintln!(
            "    {}",
            "Check if the divisor is zero before the operation.".bright_black()
        );
    }
    // Index errors
    else if err_msg.contains("index out of bounds") {
        eprintln!("{}", "💡 Help: Index is outside the valid range.".yellow());
        eprintln!("    {}", "• Lists and strings are 0-indexed".bright_black());
        eprintln!(
            "    {}",
            "• Use len() to check size: if i < len(my_list): my_list[i] end".bright_black()
        );
    }
    // Syntax errors
    else if err_msg.contains("Unexpected token") {
        eprintln!("{}", "💡 Help: Syntax error detected.".yellow());
        if err_msg.contains("'end'") {
            eprintln!(
                "    {}",
                "Did you forget an 'end' keyword for a function or if statement?".bright_black()
            );
        } else if err_msg.contains("'('") {
            eprintln!("    {}", "Check if parentheses are balanced".bright_black());
        } else if err_msg.contains("'='") {
            eprintln!(
                "    {}",
                "Use '==' for comparison, '=' for assignment".bright_black()
            );
        }
    } else if err_msg.contains("Expected") {
        eprintln!("{}", "💡 Help: Missing required syntax element.".yellow());
        if err_msg.contains("'end'") {
            eprintln!(
                "    {}",
                "Every 'fun' and 'if' needs a matching 'end'".bright_black()
            );
            eprintln!(
                "    {}",
                "Example: fun test(): showf(\"hello\") end".bright_black()
            );
        } else if err_msg.contains("identifier") {
            eprintln!(
                "    {}",
                "Expected a variable or function name".bright_black()
            );
        }
    }
    // Control flow errors
    else if err_msg.contains("'break' outside of loop") {
        eprintln!(
            "{}",
            "💡 Help: 'break' can only be used inside while or for loops.".yellow()
        );
        eprintln!(
            "    {}",
            "Example: while condition: if done: break end end".bright_black()
        );
    } else if err_msg.contains("'continue' outside of loop") {
        eprintln!(
            "{}",
            "💡 Help: 'continue' can only be used inside while or for loops.".yellow()
        );
        eprintln!(
            "    {}",
            "Example: for i in 0..10: if i == 5: continue end end".bright_black()
        );
    }
    // Function call errors
    else if err_msg.contains("expected") && err_msg.contains("args") {
        eprintln!(
            "{}",
            "💡 Help: Function called with wrong number of arguments.".yellow()
        );
        eprintln!(
            "    {}",
            "Check the function signature and provide the correct number of arguments"
                .bright_black()
        );
    }
    // File-related errors
    else if err_msg.contains("Failed to read file") {
        eprintln!("{}", "💡 Help: File operation failed.".yellow());
        eprintln!(
            "    {}",
            "Check if the file exists and you have permission to read it".bright_black()
        );
    } else if err_msg.contains("Failed to write file") {
        eprintln!("{}", "💡 Help: File write operation failed.".yellow());
        eprintln!(
            "    {}",
            "Check if you have permission to write to that location".bright_black()
        );
    }
    // Built-in function specific errors
    else if err_msg.contains("showf missing") {
        eprintln!(
            "{}",
            "💡 Help: Format string needs more arguments.".yellow()
        );
        eprintln!(
            "    {}",
            "Use %d for numbers, %s for strings: showf(\"Number: %d\", 42)".bright_black()
        );
    } else if err_msg.contains("sqrt() argument cannot be negative") {
        eprintln!(
            "{}",
            "💡 Help: Square root of negative numbers is not supported.".yellow()
        );
        eprintln!(
            "    {}",
            "Use abs() first if needed: sqrt(abs(x))".bright_black()
        );
    } else if err_msg.contains("pow() exponent cannot be negative") {
        eprintln!(
            "{}",
            "💡 Help: Negative exponents are not supported in pow().".yellow()
        );
        eprintln!(
            "    {}",
            "Use only non-negative integers: pow(2, 3) = 8".bright_black()
        );
    }
    // General parsing errors
    else if err_msg.contains("Unterminated string") {
        eprintln!("{}", "💡 Help: String is missing closing quote.".yellow());
        eprintln!(
            "    {}",
            "Make sure every \" has a matching closing \"".bright_black()
        );
    } else if err_msg.contains("Invalid number") {
        eprintln!("{}", "💡 Help: Number format is not recognized.".yellow());
        eprintln!("    {}", "Use integers like: 42, 100, -5".bright_black());
    }
    // Stack/memory errors
    else if err_msg.contains("stack underflow") || err_msg.contains("stack overflow") {
        eprintln!(
            "{}",
            "💡 Help: Internal VM error - this might be a compiler bug.".yellow()
        );
        eprintln!(
            "    {}",
            "Try using the interpreter backend: zirc --backend interp file.zirc".bright_black()
        );
    }
}
