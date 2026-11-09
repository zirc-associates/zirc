# Security Policy

## Supported versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x: (development)   |

## Reporting a vulnerability

Please report security issues privately by opening a **private**
GitHub Security Advisory on this repository
(`zirc-industries/zirc` → Security → Report a vulnerability).

Do **not** open a public issue for a suspected vulnerability.

What to include: affected version, steps to reproduce, and the
impact you observe. Interpreted languages run arbitrary user
code by design (`rf`/`wf`/`prompt` are builtins); vulnerabilities
in scope are crashes, sandbox escapes in the VM, and memory safety
issues in the Rust crates themselves.

## What to expect

- Acknowledgement within 5 business days.
- A fix timeline communicated in the advisory thread.
- Public disclosure with credit once a fix is released.
