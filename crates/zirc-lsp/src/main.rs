//! `zirc-lsp`: minimal language server for Zirc.
//!
//! Speaks JSON-RPC (LSP framing: `Content-Length` headers) over
//! stdio and implements:
//! - `initialize` / `initialized` negotiation
//! - `textDocument/didOpen`, `didChange`, `didClose` with
//!   `textDocument/publishDiagnostics` (lex/parse errors, using
//!   the structured `ErrorKind`/span from `zirc-syntax`)
//! - `textDocument/hover`: builtins (single registry), keywords,
//!   functions and variables defined in the document
//! - `textDocument/completion`: builtins, keywords, functions and
//!   variables defined in the document
//! - `shutdown` / `exit`

use std::io::{BufRead, BufReader, Write};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct Message {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: &'static str,
    id: Value,
    result: Option<Value>,
    error: Option<Value>,
}

impl Response {
    fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct Notification {
    jsonrpc: &'static str,
    method: &'static str,
    params: Value,
}

fn read_message(reader: &mut impl BufRead) -> Option<Message> {
    // Read headers until blank line, extracting Content-Length.
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().ok();
            }
        }
    }
    let len = content_length?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn write_message(writer: &mut impl Write, msg: &impl Serialize) -> std::io::Result<()> {
    let body =
        serde_json::to_vec(msg).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

fn diagnose(text: &str) -> Vec<Value> {
    // Lex first, then parse: the first failure is the diagnostic.
    let mut lexer = zirc_lexer::Lexer::new(text);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => return vec![diagnostic("lex", &e.msg, e.line, e.col)],
    };
    let mut parser = zirc_parser::Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => return vec![diagnostic("parse", &e.msg, e.line, e.col)],
    };
    // Semantic (lint) diagnostics on top of a successful parse:
    // undefined variables/functions and duplicate definitions.
    zirc_lint::lint_program(&program)
        .iter()
        .map(|w| diagnostic_severity(w.rule, &w.message, w.line, w.col, 2))
        .collect()
}

fn diagnostic_severity(
    _kind: &str,
    message: &str,
    line: Option<usize>,
    col: Option<usize>,
    severity: u32,
) -> Value {
    let line0 = line.map(|l| l.saturating_sub(1) as u32).unwrap_or(0);
    let col0 = col.map(|c| c.saturating_sub(1) as u32).unwrap_or(0);
    json!({
        "range": {
            "start": {"line": line0, "character": col0},
            "end": {"line": line0, "character": col0 + 1}
        },
        "severity": severity,
        "source": "zirc-lsp",
        "message": message
    })
}

fn diagnostic(_kind: &str, message: &str, line: Option<usize>, col: Option<usize>) -> Value {
    let line0 = line.map(|l| l.saturating_sub(1) as u32).unwrap_or(0);
    let col0 = col.map(|c| c.saturating_sub(1) as u32).unwrap_or(0);
    json!({
        "range": {
            "start": {"line": line0, "character": col0},
            "end": {"line": line0, "character": col0 + 1}
        },
        "severity": 1,
        "source": "zirc-lsp",
        "message": message
    })
}

fn document_uri_and_text(params: &Value) -> Option<(String, String)> {
    let uri = params
        .get("textDocument")?
        .get("uri")?
        .as_str()?
        .to_string();
    let text = params
        .get("textDocument")?
        .get("text")?
        .as_str()
        .map(str::to_string)
        .unwrap_or_default();
    Some((uri, text))
}

fn publish_diagnostics(writer: &mut impl Write, uri: &str, text: &str) {
    let _ = write_message(
        writer,
        &Notification {
            jsonrpc: "2.0",
            method: "textDocument/publishDiagnostics",
            params: json!({
                "uri": uri,
                "diagnostics": diagnose(text)
            }),
        },
    );
}

/// Word (identifier characters) at a 0-based line/character position.
fn word_at(text: &str, line: u32, character: u32) -> Option<String> {
    let line_text = text.lines().nth(line as usize)?;
    let chars: Vec<char> = line_text.chars().collect();
    let mut i = character as usize;
    if i >= chars.len() {
        i = chars.len().saturating_sub(1);
    }
    if chars.is_empty() || !chars[i].is_alphanumeric() && chars[i] != '_' && chars[i] != '.' {
        // Try the character just before the cursor.
        if i == 0 {
            return None;
        }
        i -= 1;
    }
    let mut start = i;
    while start > 0
        && (chars[start - 1].is_alphanumeric()
            || chars[start - 1] == '_'
            || chars[start - 1] == '.')
    {
        start -= 1;
    }
    let mut end = i;
    while end < chars.len()
        && (chars[end].is_alphanumeric() || chars[end] == '_' || chars[end] == '.')
    {
        end += 1;
    }
    let word: String = chars[start..end].iter().collect();
    if word.is_empty() {
        None
    } else {
        Some(word)
    }
}

fn parse_document(text: &str) -> Option<zirc_syntax::ast::Program> {
    let mut lexer = zirc_lexer::Lexer::new(text);
    let tokens = lexer.tokenize().ok()?;
    let mut parser = zirc_parser::Parser::new(tokens);
    parser.parse_program().ok()
}

fn hover_for(text: &str, word: &str) -> Option<String> {
    // Builtin first (single registry).
    if let Some(b) = zirc_bytecode::Builtin::from_name(word) {
        return Some(format!("```zirc\n{}()\n```\nBuiltin function", b.name()));
    }
    let keyword_doc = match word {
        "fun" => "Function or closure definition: `fun name(params): body end`",
        "end" => "Closes a `fun`, `if`, `while` or `for` block",
        "if" => "Conditional: `if cond: ... else: ... end`",
        "else" => "Alternative branch of an `if`",
        "let" => "Variable binding: `let name[: type] = expr`",
        "while" => "Loop: `while cond: ... end`",
        "for" => "Range loop: `for i in start..end: ... end`",
        "in" => "Range operator in `for` loops",
        "return" => "Return a value from a function",
        "break" => "Exit the innermost loop",
        "continue" => "Skip to the next loop iteration",
        "module" => "Module declaration: `module \"name\"`",
        "import" => "Import a module: `import name [as alias]`",
        "as" => "Alias in an import: `import name as alias`",
        "true" | "false" => "Boolean literal",
        _ => "",
    };
    if !keyword_doc.is_empty() {
        return Some(keyword_doc.to_string());
    }
    let program = parse_document(text)?;
    for item in &program.items {
        if let zirc_syntax::ast::ItemKind::Function(f) = &item.kind {
            if f.name == word {
                let params: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
                return Some(format!(
                    "```zirc\nfun {}({})\n```\nFunction defined in this file",
                    f.name,
                    params.join(", ")
                ));
            }
        }
    }
    // Let-bound variable?
    fn stmt_lets(s: &zirc_syntax::ast::Stmt, word: &str) -> bool {
        match &s.kind {
            zirc_syntax::ast::StmtKind::Let { name, .. } => name == word,
            zirc_syntax::ast::StmtKind::If {
                then_body,
                else_body,
                ..
            } => then_body
                .iter()
                .chain(else_body.iter())
                .any(|s| stmt_lets(s, word)),
            zirc_syntax::ast::StmtKind::While { body, .. }
            | zirc_syntax::ast::StmtKind::For { body, .. } => {
                body.iter().any(|s| stmt_lets(s, word))
            }
            _ => false,
        }
    }
    for item in &program.items {
        if let zirc_syntax::ast::ItemKind::Stmt(s) = &item.kind {
            if stmt_lets(s, word) {
                return Some(format!("Variable `{}` defined in this file", word));
            }
        }
    }
    None
}

fn completions_for(text: &str) -> Vec<Value> {
    let mut items: Vec<Value> = Vec::new();
    for b in zirc_bytecode::Builtin::ALL {
        items.push(json!({"label": b.name(), "kind": 3, "detail": "builtin function"}));
    }
    for kw in [
        "fun", "end", "if", "else", "let", "while", "for", "in", "return", "break", "continue",
        "module", "import", "as", "true", "false",
    ] {
        items.push(json!({"label": kw, "kind": 14, "detail": "keyword"}));
    }
    if let Some(program) = parse_document(text) {
        for item in &program.items {
            match &item.kind {
                zirc_syntax::ast::ItemKind::Function(f) => {
                    items.push(json!({"label": f.name, "kind": 3, "detail": "function"}));
                }
                zirc_syntax::ast::ItemKind::Stmt(s) => {
                    if let zirc_syntax::ast::StmtKind::Let { name, .. } = &s.kind {
                        items.push(json!({"label": name, "kind": 6, "detail": "variable"}));
                    }
                }
                _ => {}
            }
        }
    }
    items
}

fn main() {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut shutdown = false;
    let mut documents: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    while let Some(msg) = read_message(&mut reader) {
        match msg.method.as_str() {
            "initialize" => {
                let id = msg.id.unwrap_or(Value::Null);
                let result = json!({
                    "capabilities": {
                        "textDocumentSync": 1,
                        "hoverProvider": true,
                        "completionProvider": {"triggerCharacters": ["."]},
                        "documentFormattingProvider": true,
                        "positionEncoding": "utf-16"
                    },
                    "serverInfo": {
                        "name": "zirc-lsp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                });
                let _ = write_message(&mut writer, &Response::ok(id, result));
            }
            "initialized" => {}
            "shutdown" => {
                shutdown = true;
                let id = msg.id.unwrap_or(Value::Null);
                let _ = write_message(&mut writer, &Response::ok(id, Value::Null));
            }
            "exit" => std::process::exit(if shutdown { 0 } else { 1 }),
            "textDocument/didOpen" => {
                if let Some(params) = &msg.params {
                    if let Some((uri, text)) = document_uri_and_text(params) {
                        documents.insert(uri.clone(), text.clone());
                        publish_diagnostics(&mut writer, &uri, &text);
                    }
                }
            }
            "textDocument/formatting" => {
                let id = msg.id.unwrap_or(Value::Null);
                let result = msg
                    .params
                    .as_ref()
                    .and_then(|params| {
                        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
                        let text = documents.get(uri)?;
                        let mut lexer = zirc_lexer::Lexer::new(text);
                        let tokens = lexer.tokenize().ok()?;
                        let mut parser = zirc_parser::Parser::new(tokens);
                        let program = parser.parse_program().ok()?;
                        let formatted = zirc_fmt::format_program(&program);
                        if &formatted == text {
                            return None;
                        }
                        let lines = text.lines().count() as u32;
                        Some(json!([{
                            "range": {
                                "start": {"line": 0, "character": 0},
                                "end": {"line": lines, "character": 0}
                            },
                            "newText": formatted
                        }]))
                    })
                    .unwrap_or(Value::Array(Vec::new()));
                let _ = write_message(&mut writer, &Response::ok(id, result));
            }
            "textDocument/hover" => {
                let id = msg.id.unwrap_or(Value::Null);
                let result = msg
                    .params
                    .as_ref()
                    .and_then(|params| {
                        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
                        let position = params.get("position")?;
                        let line = position.get("line")?.as_u64()? as u32;
                        let character = position.get("character")?.as_u64()? as u32;
                        let text = documents.get(uri)?;
                        let word = word_at(text, line, character)?;
                        let contents = hover_for(text, &word)?;
                        Some(json!({
                            "contents": {"kind": "markdown", "value": contents},
                            "range": {
                                "start": {"line": line, "character": character},
                                "end": {"line": line, "character": character + word.len() as u32}
                            }
                        }))
                    })
                    .unwrap_or(Value::Null);
                let _ = write_message(&mut writer, &Response::ok(id, result));
            }
            "textDocument/completion" => {
                let id = msg.id.unwrap_or(Value::Null);
                let items = msg
                    .params
                    .as_ref()
                    .and_then(|params| {
                        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
                        let text = documents.get(uri)?;
                        Some(completions_for(text))
                    })
                    .unwrap_or_default();
                let _ = write_message(&mut writer, &Response::ok(id, Value::Array(items)));
            }
            "textDocument/didChange" => {
                if let Some(params) = &msg.params {
                    let uri = params
                        .get("textDocument")
                        .and_then(|d| d.get("uri"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string();
                    let text = params
                        .get("contentChanges")
                        .and_then(|c| c.as_array())
                        .and_then(|changes| changes.last())
                        .and_then(|c| c.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    documents.insert(uri.clone(), text.clone());
                    publish_diagnostics(&mut writer, &uri, &text);
                }
            }
            "textDocument/didClose" => {
                if let Some(params) = &msg.params {
                    if let Some(uri) = params
                        .get("textDocument")
                        .and_then(|d| d.get("uri"))
                        .and_then(|u| u.as_str())
                    {
                        documents.remove(uri);
                    }
                }
                if let Some(params) = &msg.params {
                    if let Some(uri) = params
                        .get("textDocument")
                        .and_then(|d| d.get("uri"))
                        .and_then(|u| u.as_str())
                    {
                        let _ = write_message(
                            &mut writer,
                            &Notification {
                                jsonrpc: "2.0",
                                method: "textDocument/publishDiagnostics",
                                params: json!({"uri": uri, "diagnostics": []}),
                            },
                        );
                    }
                }
            }
            _ => {
                // Unknown methods: respond with method-not-found when
                // they expect a response, otherwise ignore.
                if let Some(id) = msg.id {
                    let _ = write_message(
                        &mut writer,
                        &Response {
                            jsonrpc: "2.0",
                            id,
                            result: None,
                            error: Some(json!({"code": -32601, "message": "method not found"})),
                        },
                    );
                }
            }
        }
    }
}
