//! Bytecode compiler from AST to IR.

use std::collections::HashMap;

use zirc_bytecode::{Function as BcFunction, Instruction as BC, Program as BcProgram};
use zirc_syntax::ast::*;
use zirc_syntax::error::{error, Result};

use crate::builder::FuncBuilder;

pub struct Compiler {
    pub(crate) func_indices: HashMap<String, usize>,
    pub(crate) functions: Vec<BcFunction>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            func_indices: HashMap::new(),
            functions: Vec::new(),
        }
    }

    pub fn function_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.func_indices.keys().cloned().collect();
        v.sort();
        v
    }

    pub fn compile(&mut self, program: Program) -> Result<BcProgram> {
        // Imports must be expanded by the module loader (CLI) first.
        for item in &program.items {
            if let ItemKind::Import { module, .. } = &item.kind {
                return error(format!(
                    "import {} requires the module loader (use the zirc CLI)",
                    module
                ));
            }
        }
        // First pass: collect function names to assign indices
        for item in &program.items {
            if let ItemKind::Function(f) = &item.kind {
                if self.func_indices.contains_key(&f.name) {
                    return error(format!("Duplicate function '{}'", f.name));
                }
                let idx = self.functions.len();
                self.func_indices.insert(f.name.clone(), idx);
                self.functions.push(BcFunction {
                    name: f.name.clone(),
                    arity: f.params.len(),
                    local_count: 0,
                    code: Vec::new(),
                    debug: Vec::new(),
                });
            }
        }
        // Second pass: compile functions
        for item in &program.items {
            if let ItemKind::Function(f) = &item.kind {
                let idx = *self.func_indices.get(&f.name).ok_or_else(|| {
                    zirc_syntax::error::Error::new(format!("Undefined function '{}'", f.name))
                })?;
                let compiled = self.compile_function(f)?;
                self.functions[idx] = compiled;
            }
        }
        // Compile main (top-level statements)
        let mut main_builder = FuncBuilder::new("__main".to_string(), 0, true);
        for item in program.items.into_iter() {
            if let ItemKind::Stmt(s) = item.kind {
                main_builder.emit_stmt(self, &s)?;
            }
        }
        main_builder.emit(BC::Halt);
        let main = main_builder.finish();
        // Constant folding runs on every compiled program; the VM
        // only ever executes optimized bytecode.
        Ok(zirc_bytecode::optimize::optimize_program(BcProgram {
            version: zirc_bytecode::BYTECODE_VERSION,
            functions: self.functions.clone(),
            main,
        }))
    }

    /// Compile an anonymous closure as a synthetic function.
    /// Its locals are params followed by captures (free variables,
    /// in free-var order). Returns the synthetic function's index.
    pub(crate) fn compile_closure(&mut self, f: &Function, captures: Vec<String>) -> Result<usize> {
        let name = format!("__closure_{}", self.functions.len());
        let mut b = FuncBuilder::new(name.clone(), f.params.len(), false)
            .with_return_type(f.return_type.clone());
        for p in &f.params {
            b.declare_param_typed(p.name.clone(), p.ty.clone())?;
            b.emit_param_check();
        }
        for cap in &captures {
            b.declare_param(cap.clone())?;
        }
        b.emit_function_body(self, &f.body)?;
        let compiled = b.finish();
        let idx = self.functions.len();
        self.functions.push(compiled);
        self.func_indices.insert(name, idx);
        Ok(idx)
    }

    fn compile_function(&mut self, f: &Function) -> Result<BcFunction> {
        let mut b = FuncBuilder::new(f.name.clone(), f.params.len(), false)
            .with_return_type(f.return_type.clone());
        for p in &f.params {
            b.declare_param_typed(p.name.clone(), p.ty.clone())?;
            b.emit_param_check();
        }
        b.emit_function_body(self, &f.body)?;
        Ok(b.finish())
    }
}

/// Resolve a builtin by name through the single registry in
/// `zirc-bytecode` (also used by the interpreter dispatch).
pub(crate) fn builtin_of(name: &str) -> Option<zirc_bytecode::Builtin> {
    zirc_bytecode::Builtin::from_name(name)
}
