//! End-to-end LSP protocol tests: speak JSON-RPC with the server
//! over stdio exactly as an editor would.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};

struct Client {
    child: Child,
}

impl Client {
    fn new() -> Self {
        Self {
            child: Command::new(env!("CARGO_BIN_EXE_zirc-lsp"))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        }
    }

    fn send(&mut self, msg: serde_json::Value) {
        let body = serde_json::to_vec(&msg).unwrap();
        let stdin = self.child.stdin.as_mut().unwrap();
        write!(stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        stdin.write_all(&body).unwrap();
        stdin.flush().unwrap();
    }

    fn recv(&mut self) -> serde_json::Value {
        let stdout = self.child.stdout.as_mut().unwrap();
        let mut headers = Vec::new();
        let mut buf = [0u8; 1];
        // Read until the blank line ending the headers.
        while !headers.ends_with(b"\r\n\r\n") {
            stdout.read_exact(&mut buf).unwrap();
            headers.push(buf[0]);
        }
        let header = String::from_utf8(headers).unwrap();
        let len: usize = header
            .lines()
            .find_map(|l| {
                l.strip_prefix("Content-Length:")
                    .map(|v| v.trim().parse().unwrap())
            })
            .unwrap();
        let mut body = vec![0u8; len];
        stdout.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn request(&mut self, id: u64, method: &str, params: serde_json::Value) -> serde_json::Value {
        self.send(serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        }));
        self.recv()
    }

    fn notify(&mut self, method: &str, params: serde_json::Value) {
        self.send(serde_json::json!({
            "jsonrpc": "2.0", "method": method, "params": params
        }));
    }
}

const URI: &str = "file:///test.zirc";

#[test]
fn initialize_hover_completion_and_diagnostics() {
    let mut client = Client::new();

    let init = client.request(1, "initialize", serde_json::json!({}));
    let caps = &init["result"]["capabilities"];
    assert!(caps.get("hoverProvider").is_some());
    assert!(caps.get("completionProvider").is_some());
    client.notify("initialized", serde_json::json!({}));

    let text = "fun fact(n): n end\nshowf(\"%d\", fact(5))\n";
    client.notify(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {"uri": URI, "languageId": "zirc", "version": 1, "text": text}
        }),
    );
    let diag = client.recv();
    assert_eq!(diag["params"]["uri"], URI);
    assert_eq!(diag["params"]["diagnostics"].as_array().unwrap().len(), 0);

    // Hover over `showf` (line 1, character 2) resolves the builtin.
    let hover = client.request(
        2,
        "textDocument/hover",
        serde_json::json!({"textDocument": {"uri": URI}, "position": {"line": 1, "character": 2}}),
    );
    assert!(
        hover["result"]["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("showf"),
        "hover: {hover}"
    );

    // Completion offers builtins and file symbols.
    let completion = client.request(
        3,
        "textDocument/completion",
        serde_json::json!({"textDocument": {"uri": URI}, "position": {"line": 0, "character": 0}}),
    );
    let labels: Vec<&str> = completion["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap())
        .collect();
    assert!(labels.contains(&"showf"), "completion: {labels:?}");
    assert!(labels.contains(&"fact"), "completion: {labels:?}");

    // A parse error is published as a diagnostic on open.
    client.notify(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {"uri": "file:///bad.zirc", "languageId": "zirc", "version": 1, "text": "fun x(\n"}
        }),
    );
    let bad_diag = client.recv();
    assert_eq!(
        bad_diag["params"]["diagnostics"].as_array().unwrap().len(),
        1,
        "bad_diag: {bad_diag}"
    );

    // Semantic (lint) diagnostics: undefined variable is a warning.
    client.notify(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {"uri": "file:///lint.zirc", "languageId": "zirc", "version": 1, "text": "let x = y\n"}
        }),
    );
    let lint_diag = client.recv();
    let diags = lint_diag["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diags.len(), 1, "lint_diag: {lint_diag}");
    assert_eq!(diags[0]["severity"], 2);
    assert!(diags[0]["message"].as_str().unwrap().contains('y'));

    // Formatting returns a whole-document edit for unformatted code.
    client.notify(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {"uri": "file:///fmt.zirc", "languageId": "zirc", "version": 1, "text": "let x=1+2\n"}
        }),
    );
    let _ = client.recv();
    let fmt = client.request(
        10,
        "textDocument/formatting",
        serde_json::json!({"textDocument": {"uri": "file:///fmt.zirc"}, "options": {"tabSize": 2, "insertSpaces": true}}),
    );
    let edits = fmt["result"].as_array().unwrap();
    assert_eq!(edits.len(), 1, "formatting: {fmt}");
    assert!(edits[0]["newText"]
        .as_str()
        .unwrap()
        .contains("let x = 1 + 2"));

    let shutdown = client.request(4, "shutdown", serde_json::Value::Null);
    assert!(shutdown["error"].is_null(), "shutdown: {shutdown}");
    client.notify("exit", serde_json::json!({}));
    let status = client.child.wait().unwrap();
    assert!(status.success());
}
