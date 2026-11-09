//! Function builder and local management for codegen.

use std::collections::HashMap;

use zirc_bytecode::{Function as BcFunction, Instruction as BC, VarRef};
use zirc_syntax::ast::*;
use zirc_syntax::error::{error, Result};

use crate::compiler::{builtin_of, Compiler};

pub(crate) struct FuncBuilder {
    name: String,
    arity: usize,
    code: Vec<BC>,
    /// Per-instruction debug info: (line, col) of the statement
    /// that emitted each instruction.
    debug: Vec<Option<(usize, usize)>>,
    /// Span of the statement currently being emitted.
    current_span: zirc_syntax::error::Span,
    locals: Locals,
    // loop stack
    loop_stack: Vec<LoopCtx>,
    // when true, compile idents/lets/assigns to globals if not local
    global_mode: bool,
    // Declared return type, checked before every `Return`.
    return_type: Option<Type>,
}

impl FuncBuilder {
    pub(crate) fn new(name: String, arity: usize, global_mode: bool) -> Self {
        // Locals start at 0; params will occupy slots [0..arity)
        Self {
            name,
            arity,
            code: Vec::new(),
            debug: Vec::new(),
            current_span: zirc_syntax::error::Span::default(),
            locals: Locals::new(0),
            loop_stack: Vec::new(),
            global_mode,
            return_type: None,
        }
    }

    pub(crate) fn with_return_type(mut self, ty: Option<Type>) -> Self {
        self.return_type = ty;
        self
    }

    pub(crate) fn finish(self) -> BcFunction {
        BcFunction {
            name: self.name,
            arity: self.arity,
            local_count: self.locals.max_alloc as usize,
            code: self.code,
            debug: self.debug,
        }
    }

    pub(crate) fn emit(&mut self, i: BC) -> usize {
        self.debug
            .push(Some((self.current_span.line, self.current_span.col)));
        self.code.push(i);
        self.code.len() - 1
    }
    fn here(&self) -> usize {
        self.code.len()
    }

    fn patch_to_here(&mut self, at: usize) -> Result<()> {
        let tgt = self.here();
        match &mut self.code[at] {
            BC::Jump(ref mut x) | BC::JumpIfFalse(ref mut x) | BC::JumpIfTrue(ref mut x) => {
                *x = tgt;
                Ok(())
            }
            other => error(format!("cannot patch at {:?}", other)),
        }
    }

    pub(crate) fn declare_param(&mut self, name: String) -> Result<()> {
        self.locals.declare(name)?;
        Ok(())
    }

    /// Declare a parameter with an optional type annotation, to be
    /// checked on entry by [`FuncBuilder::emit_param_check`].
    pub(crate) fn declare_param_typed(&mut self, name: String, ty: Option<Type>) -> Result<()> {
        self.locals.declare_typed(name, ty)?;
        Ok(())
    }

    /// Emit a runtime type check for the most recently declared
    /// parameter (its slot is the last declared one). Args arrive
    /// in their local slots, so checking is `LoadLocal, CheckType,
    /// Pop` per annotated param.
    pub(crate) fn emit_param_check(&mut self) {
        // The slot is `next - 1`; find its type via the types map
        // using the name at that slot — recover it by scanning.
        let slot = self.locals.next.saturating_sub(1);
        let ty: Option<Type> =
            self.locals.types.iter().find_map(|(name, ty)| {
                (self.locals.resolve(name) == Some(slot)).then(|| ty.clone())
            });
        if let Some(ty) = ty {
            self.emit(BC::LoadLocal(slot));
            self.emit(BC::CheckType(ty));
            self.emit(BC::Pop);
        }
    }

    fn declare_var(&mut self, name: String) -> Result<u16> {
        self.locals.declare(name)
    }

    fn resolve_var(&self, name: &str) -> Result<u16> {
        self.locals
            .resolve(name)
            .ok_or_else(|| zirc_syntax::error::Error::new(format!("Undefined variable '{}'", name)))
    }

    /// Compile `push(var, value)` / `pop(var)` where `var` must be a
    /// plain identifier naming a local or global list binding.
    fn emit_push_pop(&mut self, c: &mut Compiler, name: &str, args: &[Expr]) -> Result<()> {
        let expected = if name == "push" { 2 } else { 1 };
        if args.len() != expected {
            return error(format!(
                "{}() expects exactly {} argument(s)",
                name, expected
            ));
        }
        let var_name = match &args[0] {
            Expr::Ident(n) => n.clone(),
            _ => return error(format!("{}() first argument must be a variable name", name)),
        };
        let var_ref = if let Some(slot) = self.locals.resolve(&var_name) {
            VarRef::Local(slot)
        } else if self.global_mode {
            VarRef::Global(var_name)
        } else {
            return error(format!("Undefined variable '{}'", var_name));
        };
        if name == "push" {
            self.emit_expr(c, &args[1])?;
            self.emit(BC::PushToVar(var_ref));
        } else {
            self.emit(BC::PopFromVar(var_ref));
        }
        Ok(())
    }

    /// Emit a function body with an implicit return of the last
    /// expression (matching the interpreter's `exec_block`, which
    /// returns the last statement's value). A trailing `ExprStmt`
    /// is emitted without its usual `Pop` so its value is returned.
    pub(crate) fn emit_function_body(&mut self, c: &mut Compiler, body: &[Stmt]) -> Result<()> {
        if body.is_empty() {
            self.emit(BC::PushUnit);
            self.emit(BC::Return);
            return Ok(());
        }
        for s in &body[..body.len() - 1] {
            self.emit_stmt(c, s)?;
        }
        match &body[body.len() - 1].kind {
            StmtKind::ExprStmt(e) => {
                self.emit_expr(c, e)?;
                if let Some(ty) = &self.return_type {
                    self.emit(BC::CheckType(ty.clone()));
                }
                self.emit(BC::Return);
            }
            _ => {
                self.emit_stmt(c, &body[body.len() - 1])?;
                self.emit(BC::PushUnit);
                if let Some(ty) = &self.return_type {
                    self.emit(BC::CheckType(ty.clone()));
                }
                self.emit(BC::Return);
            }
        }
        Ok(())
    }

    pub(crate) fn emit_stmt(&mut self, c: &mut Compiler, s: &Stmt) -> Result<()> {
        let span = s.span;
        let result = self.emit_stmt_inner(c, s);
        result.map_err(|e| e.with_span_if_missing(span))
    }

    fn emit_stmt_inner(&mut self, c: &mut Compiler, s: &Stmt) -> Result<()> {
        self.current_span = s.span;
        match &s.kind {
            StmtKind::Comment(_) => Ok(()),
            StmtKind::Let { name, ty, expr } => {
                self.emit_expr(c, expr)?;
                if let Some(ty) = ty {
                    self.emit(BC::CheckType(ty.clone()));
                }
                if self.global_mode {
                    self.emit(BC::StoreGlobal(name.clone()));
                } else {
                    let slot = self.locals.declare_typed(name.clone(), ty.clone())?;
                    self.emit(BC::StoreLocal(slot));
                }
                Ok(())
            }
            StmtKind::Assign { name, expr } => {
                self.emit_expr(c, expr)?;
                if let Ok(slot) = self.resolve_var(name) {
                    // Re-check against the declared type, if any.
                    // CheckType pops and pushes the value back, so
                    // the stack is unchanged for the store below.
                    if let Some(ty) = self.locals.ty_of(name) {
                        self.emit(BC::CheckType(ty));
                    }
                    self.emit(BC::StoreLocal(slot));
                } else if self.global_mode {
                    self.emit(BC::StoreGlobal(name.clone()));
                } else {
                    return error(format!("Undefined variable '{}'", name));
                }
                Ok(())
            }
            StmtKind::Return(opt) => {
                if let Some(e) = opt {
                    self.emit_expr(c, e)?;
                } else {
                    self.emit(BC::PushUnit);
                }
                if let Some(ty) = &self.return_type {
                    self.emit(BC::CheckType(ty.clone()));
                }
                self.emit(BC::Return);
                Ok(())
            }
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                self.emit_expr(c, cond)?;
                let jf_at = self.emit(BC::JumpIfFalse(0));
                self.locals.push_scope();
                for s in then_body {
                    self.emit_stmt(c, s)?;
                }
                self.locals.pop_scope();
                let jend_at = self.emit(BC::Jump(0));
                self.patch_to_here(jf_at)?; // else starts here
                self.locals.push_scope();
                for s in else_body {
                    self.emit_stmt(c, s)?;
                }
                self.locals.pop_scope();
                self.patch_to_here(jend_at)?;
                Ok(())
            }
            StmtKind::While { cond, body } => {
                let loop_start = self.here();
                self.emit_expr(c, cond)?;
                let jf_at = self.emit(BC::JumpIfFalse(0));
                self.loop_stack.push(LoopCtx::new());
                self.locals.push_scope();
                for s in body {
                    self.emit_stmt(c, s)?;
                }
                self.locals.pop_scope();
                // continue target is loop_start
                let ctx = self.loop_stack.pop().ok_or_else(|| {
                    zirc_syntax::error::Error::new("internal: loop context missing")
                })?;
                // patch continues -> loop_start
                for at in ctx.continues {
                    self.code[at] = BC::Jump(loop_start);
                }
                // jump back to start
                self.emit(BC::Jump(loop_start));
                // end label
                self.patch_to_here(jf_at)?;
                // patch breaks -> end
                let end = self.here();
                for at in ctx.breaks {
                    self.code[at] = BC::Jump(end);
                }
                Ok(())
            }
            StmtKind::For {
                var,
                start,
                end,
                body,
            } => {
                // The loop variable (and body bindings) are scoped to
                // the loop. In global mode they still live in globals,
                // but in function mode they are locals in a new scope.
                self.locals.push_scope();
                // If in global mode, use globals for the loop var; otherwise, use a local.
                let end_slot = self.locals.alloc_temp();
                self.emit_expr(c, end)?;
                self.emit(BC::StoreLocal(end_slot));
                if self.global_mode {
                    self.emit_expr(c, start)?;
                    self.emit(BC::StoreGlobal(var.clone()));
                } else {
                    let i_slot = self.declare_var(var.clone())?;
                    self.emit_expr(c, start)?;
                    self.emit(BC::StoreLocal(i_slot));
                }
                let loop_start = self.here();
                if self.global_mode {
                    self.emit(BC::LoadGlobal(var.clone()));
                } else {
                    let slot = self.resolve_var(var)?;
                    self.emit(BC::LoadLocal(slot));
                }
                self.emit(BC::LoadLocal(end_slot));
                self.emit(BC::Lt);
                let jf_at = self.emit(BC::JumpIfFalse(0));
                self.loop_stack.push(LoopCtx::new());
                self.locals.push_scope();
                for s in body {
                    self.emit_stmt(c, s)?;
                }
                self.locals.pop_scope();
                // continue target: increment
                let incr_ip = self.here();
                {
                    let ctx = self.loop_stack.last_mut().ok_or_else(|| {
                        zirc_syntax::error::Error::new("internal: loop context missing")
                    })?;
                    ctx.continue_target = Some(incr_ip);
                }
                if self.global_mode {
                    self.emit(BC::LoadGlobal(var.clone()));
                    self.emit(BC::PushInt(1));
                    self.emit(BC::Add);
                    self.emit(BC::StoreGlobal(var.clone()));
                } else {
                    let slot = self.resolve_var(var)?;
                    self.emit(BC::LoadLocal(slot));
                    self.emit(BC::PushInt(1));
                    self.emit(BC::Add);
                    self.emit(BC::StoreLocal(slot));
                }
                self.emit(BC::Jump(loop_start));
                // end label
                self.patch_to_here(jf_at)?;
                let ctx = self.loop_stack.pop().ok_or_else(|| {
                    zirc_syntax::error::Error::new("internal: loop context missing")
                })?;
                let end_ip = self.here();
                for at in ctx.breaks {
                    self.code[at] = BC::Jump(end_ip);
                }
                let cont_ip = ctx.continue_target.unwrap_or(loop_start);
                for at in ctx.continues {
                    self.code[at] = BC::Jump(cont_ip);
                }
                self.locals.pop_scope();
                Ok(())
            }
            StmtKind::Break => {
                let at = self.emit(BC::Jump(0));
                if let Some(ctx) = self.loop_stack.last_mut() {
                    ctx.breaks.push(at);
                    Ok(())
                } else {
                    error("'break' outside of loop")
                }
            }
            StmtKind::Continue => {
                let at = self.emit(BC::Jump(0));
                if let Some(ctx) = self.loop_stack.last_mut() {
                    ctx.continues.push(at);
                    Ok(())
                } else {
                    error("'continue' outside of loop")
                }
            }
            StmtKind::ExprStmt(e) => {
                self.emit_expr(c, e)?;
                self.emit(BC::Pop);
                Ok(())
            }
        }
    }

    fn emit_expr(&mut self, c: &mut Compiler, e: &Expr) -> Result<()> {
        match e {
            Expr::LiteralInt(n) => {
                self.emit(BC::PushInt(*n));
                Ok(())
            }
            Expr::LiteralFloat(v) => {
                self.emit(BC::PushFloat(*v));
                Ok(())
            }
            Expr::LiteralString(s) => {
                self.emit(BC::PushStr(s.clone()));
                Ok(())
            }
            Expr::LiteralBool(b) => {
                self.emit(BC::PushBool(*b));
                Ok(())
            }
            Expr::Ident(name) => {
                if let Ok(slot) = self.resolve_var(name) {
                    self.emit(BC::LoadLocal(slot));
                } else if self.global_mode {
                    self.emit(BC::LoadGlobal(name.clone()));
                } else {
                    return error(format!("Undefined variable '{}'", name));
                }
                Ok(())
            }
            Expr::Member(base, field) => {
                // Treat module access a.b (and nested) as a single global name "a.b[.c...]".
                fn qualify(e: &Expr, tail: &str) -> Option<String> {
                    match e {
                        Expr::Ident(n) => Some(format!("{}.{}", n, tail)),
                        Expr::Member(b, f) => qualify(b, &format!("{}.{}", f, tail)),
                        _ => None,
                    }
                }
                let q = qualify(base, field).ok_or_else(|| {
                    zirc_syntax::error::Error::new(format!(
                        "member access only supported on identifiers/modules, got {:?}",
                        base
                    ))
                })?;
                // Allow loading globals by qualified name regardless of local/global mode.
                self.emit(BC::LoadGlobal(q));
                Ok(())
            }
            Expr::BinaryAdd(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Add);
                Ok(())
            }
            Expr::BinarySub(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Sub);
                Ok(())
            }
            Expr::BinaryMul(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Mul);
                Ok(())
            }
            Expr::BinaryDiv(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Div);
                Ok(())
            }
            Expr::Eq(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Eq);
                Ok(())
            }
            Expr::Ne(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Ne);
                Ok(())
            }
            Expr::Lt(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Lt);
                Ok(())
            }
            Expr::Le(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Le);
                Ok(())
            }
            Expr::Gt(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Gt);
                Ok(())
            }
            Expr::Ge(a, b) => {
                self.emit_expr(c, a)?;
                self.emit_expr(c, b)?;
                self.emit(BC::Ge);
                Ok(())
            }
            Expr::LogicalNot(a) => {
                self.emit_expr(c, a)?;
                self.emit(BC::Not);
                Ok(())
            }
            Expr::Neg(a) => {
                self.emit_expr(c, a)?;
                self.emit(BC::Neg);
                Ok(())
            }
            Expr::LogicalAnd(a, b) => {
                // short-circuit: if a is false, skip b
                self.emit_expr(c, a)?;
                let jf = self.emit(BC::JumpIfFalse(0));
                // if true, evaluate b and leave its value on stack
                self.emit_expr(c, b)?;
                // else-branch: push false
                let jend = self.emit(BC::Jump(0));
                self.patch_to_here(jf)?;
                self.emit(BC::PushBool(false));
                self.patch_to_here(jend)?;
                Ok(())
            }
            Expr::LogicalOr(a, b) => {
                // short-circuit: if a is true, skip b
                self.emit_expr(c, a)?;
                let jt = self.emit(BC::JumpIfTrue(0));
                // if false, evaluate b
                self.emit_expr(c, b)?;
                let jend = self.emit(BC::Jump(0));
                self.patch_to_here(jt)?;
                self.emit(BC::PushBool(true));
                self.patch_to_here(jend)?;
                Ok(())
            }
            Expr::Call { name, args } => {
                // `push(list_var, value)` / `pop(list_var)` mutate a
                // variable binding, so they compile to dedicated
                // instructions that know which binding to write back —
                // a generic builtin call only has a stack copy.
                if name == "push" || name == "pop" {
                    return self.emit_push_pop(c, name, args);
                }
                // builtins
                if let Some(bi) = builtin_of(name) {
                    for a in args {
                        self.emit_expr(c, a)?;
                    }
                    self.emit(BC::BuiltinCall(bi, args.len()));
                    return Ok(());
                }
                if let Some(&fi) = c.func_indices.get(name) {
                    for a in args {
                        self.emit_expr(c, a)?;
                    }
                    self.emit(BC::Call(fi, args.len()));
                    return Ok(());
                }
                // Closure stored in a variable: load it, then call
                // indirectly. Undefined names fail at runtime on the
                // load (global mode) or here (function mode).
                if let Ok(slot) = self.resolve_var(name) {
                    self.emit(BC::LoadLocal(slot));
                } else if self.global_mode {
                    self.emit(BC::LoadGlobal(name.clone()));
                } else {
                    return error(format!("Undefined function '{}'", name));
                }
                for a in args {
                    self.emit_expr(c, a)?;
                }
                self.emit(BC::CallIndirect(args.len()));
                Ok(())
            }
            Expr::CallValue(callee, args) => {
                self.emit_expr(c, callee)?;
                for a in args {
                    self.emit_expr(c, a)?;
                }
                self.emit(BC::CallIndirect(args.len()));
                Ok(())
            }
            Expr::Closure(f) => {
                // Compile the body as a synthetic function whose
                // locals are [params..., captures...], then emit
                // capture loads + MakeClosure at this site.
                let param_names: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
                let free = zirc_syntax::analysis::free_vars(&param_names, &f.body);
                let fi = c.compile_closure(f, free.clone())?;
                for name in &free {
                    if let Ok(slot) = self.resolve_var(name) {
                        self.emit(BC::LoadLocal(slot));
                    } else if self.global_mode {
                        self.emit(BC::LoadGlobal(name.clone()));
                    } else {
                        return error(format!("Undefined variable '{}'", name));
                    }
                }
                self.emit(BC::MakeClosure(fi, free.len()));
                Ok(())
            }
            Expr::List(elems) => {
                for a in elems {
                    self.emit_expr(c, a)?;
                }
                self.emit(BC::MakeList(elems.len()));
                Ok(())
            }
            Expr::Index(base, idx) => {
                self.emit_expr(c, base)?;
                self.emit_expr(c, idx)?;
                self.emit(BC::Index);
                Ok(())
            }
        }
    }
}

struct Locals {
    scopes: Vec<HashMap<String, u16>>, // name -> slot
    types: HashMap<String, Type>, // declared type per name (by slot owner), for CheckType on assign
    next: u16,
    max_alloc: u16,
}

impl Locals {
    fn new(start: u16) -> Self {
        Self {
            scopes: vec![HashMap::new()],
            types: HashMap::new(),
            next: start,
            max_alloc: start,
        }
    }
    fn declare(&mut self, name: String) -> Result<u16> {
        self.declare_typed(name, None)
    }
    fn declare_typed(&mut self, name: String, ty: Option<Type>) -> Result<u16> {
        if let Some(ty) = &ty {
            self.types.insert(name.clone(), ty.clone());
        }
        let scope = self
            .scopes
            .last_mut()
            .ok_or_else(|| zirc_syntax::error::Error::new("internal: no scope"))?;
        if scope.contains_key(&name) {
            return error(format!("Variable '{}' already defined in scope", name));
        }
        let idx = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| zirc_syntax::error::Error::new("too many locals"))?;
        scope.insert(name, idx);
        if idx + 1 > self.max_alloc {
            self.max_alloc = idx + 1;
        }
        Ok(idx)
    }
    fn resolve(&self, name: &str) -> Option<u16> {
        for scope in self.scopes.iter().rev() {
            if let Some(&i) = scope.get(name) {
                return Some(i);
            }
        }
        None
    }
    fn ty_of(&self, name: &str) -> Option<Type> {
        self.types.get(name).cloned()
    }
    fn alloc_temp(&mut self) -> u16 {
        let idx = self.next;
        self.next = self.next.saturating_add(1);
        if idx + 1 > self.max_alloc {
            self.max_alloc = idx + 1;
        }
        idx
    }
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }
}

struct LoopCtx {
    breaks: Vec<usize>,
    continues: Vec<usize>,
    continue_target: Option<usize>,
}
impl LoopCtx {
    fn new() -> Self {
        Self {
            breaks: Vec::new(),
            continues: Vec::new(),
            continue_target: None,
        }
    }
}
