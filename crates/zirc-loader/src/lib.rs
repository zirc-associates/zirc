//! Zirc module loader: expands `import` declarations.
//!
//! [`lex_and_parse_with_modules`] lexes, parses and expands a
//! program's imports so the engines never see `Item::Import`.
//! Modules resolve as `<name>.zirc` relative to the importing
//! file, then via `$ZIRC_PATH`. Expansion inlines the module's
//! items and registers its functions under `<module>.<fn>`
//! (and `<alias>.<fn>` when aliased). Cycles and missing modules
//! are errors; diamond imports are included only once.

use std::path::Path;

use zirc_lexer::Lexer;
use zirc_parser::Parser;
use zirc_syntax::ast::{Function, Item, ItemKind, Program};
use zirc_syntax::error::{Error, ErrorKind, Result};

/// Lex and parse source text into a program.
pub fn lex_and_parse(source: &str) -> Result<Program> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

/// Lex, parse and expand `import`s in source text.
///
/// `base_dir` is the directory of the file the source came from
/// (or the current directory for REPL input); imported modules are
/// resolved as `<name>.zirc` relative to it, then via each directory
/// listed in the `ZIRC_PATH` environment variable (path-separator
/// separated), which is how the shipped `stdlib/` is found.
///
/// Expansion inlines the imported module's items at the import
/// point and registers its functions under qualified names
/// `<module>.<fn>` (and `<alias>.<fn>` when aliased), so they can be
/// called as `math.gcd(...)`. Import cycles and missing modules are
/// errors; diamond imports are included only once.
pub fn lex_and_parse_with_modules(source: &str, base_dir: &Path) -> Result<Program> {
    let program = lex_and_parse(source)?;
    let mut visited: Vec<std::path::PathBuf> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = Vec::new();
    expand_program(program, base_dir, &mut visited, &mut stack)
}

/// Resolve `<module>.zirc` against `base_dir` and `ZIRC_PATH` dirs.
fn resolve_module(module: &str, base_dir: &Path) -> Option<std::path::PathBuf> {
    let file_name = format!("{module}.zirc");
    let mut dirs: Vec<std::path::PathBuf> = vec![base_dir.to_path_buf()];
    if let Ok(zirc_path) = std::env::var("ZIRC_PATH") {
        for dir in std::env::split_paths(&zirc_path) {
            dirs.push(dir);
        }
    }
    dirs.iter()
        .map(|d| d.join(&file_name))
        .find(|candidate| candidate.is_file())
}

fn expand_program(
    program: Program,
    base_dir: &Path,
    visited: &mut Vec<std::path::PathBuf>,
    stack: &mut Vec<std::path::PathBuf>,
) -> Result<Program> {
    let mut out: Vec<Item> = Vec::new();
    for item in program.items {
        let span = item.span;
        match item.kind {
            ItemKind::Import { module, alias } => {
                let path = resolve_module(&module, base_dir).ok_or_else(|| {
                    Error::of_kind(
                        ErrorKind::Io,
                        format!(
                            "Module '{}' not found (looked for {}.zirc in {} and ZIRC_PATH)",
                            module,
                            module,
                            base_dir.display()
                        ),
                    )
                })?;
                let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| {
                    if path.is_absolute() {
                        path.clone()
                    } else {
                        std::env::current_dir()
                            .map(|cwd| cwd.join(&path))
                            .unwrap_or_else(|_| path.clone())
                    }
                });
                if stack.contains(&canonical) {
                    return Err(Error::of_kind(
                        ErrorKind::General,
                        format!("Import cycle detected involving module '{}'", module),
                    ));
                }
                if visited.contains(&canonical) {
                    continue; // diamond import: already included once
                }
                stack.push(canonical.clone());
                let src = std::fs::read_to_string(&path).map_err(|e| {
                    Error::of_kind(
                        ErrorKind::Io,
                        format!("Failed to read module {}: {}", path.display(), e),
                    )
                })?;
                let module_program = lex_and_parse(&src)?;
                let module_dir = path.parent().unwrap_or(base_dir).to_path_buf();
                let expanded = expand_program(module_program, &module_dir, visited, stack)?;
                stack.pop();
                visited.push(canonical);
                // Qualified name: the module's own declaration wins,
                // otherwise the import name. `import x as f` also
                // registers functions under `f.<fn>`.
                let module_name = expanded
                    .items
                    .iter()
                    .find_map(|i| match &i.kind {
                        ItemKind::Module(name) => Some(name.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| module.clone());
                for mod_item in expanded.items {
                    let mspan = mod_item.span;
                    match mod_item.kind {
                        ItemKind::Function(f) => {
                            out.push(Item {
                                span: mspan,
                                kind: ItemKind::Function(f.clone()),
                            });
                            out.push(Item {
                                span: mspan,
                                kind: ItemKind::Function(Function {
                                    name: format!("{}.{}", module_name, f.name),
                                    ..f.clone()
                                }),
                            });
                            if let Some(alias_name) = &alias {
                                out.push(Item {
                                    span: mspan,
                                    kind: ItemKind::Function(Function {
                                        name: format!("{}.{}", alias_name, f.name),
                                        ..f
                                    }),
                                });
                            }
                        }
                        ItemKind::Module(_) => {}
                        other => out.push(Item {
                            span: mspan,
                            kind: other,
                        }),
                    }
                }
            }
            other => out.push(Item { span, kind: other }),
        }
    }
    Ok(Program { items: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_names(p: &Program) -> Vec<String> {
        p.items
            .iter()
            .filter_map(|i| match &i.kind {
                ItemKind::Function(f) => Some(f.name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn expands_import_with_qualified_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("math.zirc"),
            "module \"math\"\nfun gcd(a, b): a end\n",
        )
        .unwrap();
        let program = lex_and_parse_with_modules("import math\n", dir.path()).unwrap();
        let names = function_names(&program);
        assert!(names.contains(&"gcd".to_string()), "got: {names:?}");
        assert!(names.contains(&"math.gcd".to_string()), "got: {names:?}");
    }

    #[test]
    fn alias_registers_alias_qualified_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("math.zirc"), "fun gcd(a, b): a end\n").unwrap();
        let program = lex_and_parse_with_modules("import math as m\n", dir.path()).unwrap();
        let names = function_names(&program);
        assert!(names.contains(&"m.gcd".to_string()), "got: {names:?}");
    }

    #[test]
    fn missing_module_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = lex_and_parse_with_modules("import nope_xyz\n", dir.path()).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Io);
    }

    #[test]
    fn import_cycle_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.zirc"), "import b\n").unwrap();
        std::fs::write(dir.path().join("b.zirc"), "import a\n").unwrap();
        let err = lex_and_parse_with_modules("import a\n", dir.path()).unwrap_err();
        assert!(err.msg.contains("cycle"), "got: {}", err.msg);
    }

    #[test]
    fn diamond_import_included_once() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("c.zirc"), "fun f(): 1 end\n").unwrap();
        std::fs::write(dir.path().join("a.zirc"), "import c\nfun a(): 1 end\n").unwrap();
        std::fs::write(dir.path().join("b.zirc"), "import c\nfun b(): 1 end\n").unwrap();
        let program = lex_and_parse_with_modules("import a\nimport b\n", dir.path()).unwrap();
        let names = function_names(&program);
        // `f` comes from c.zirc once (unqualified), plus qualified copies.
        assert_eq!(names.iter().filter(|n| *n == "f").count(), 1);
    }
}
