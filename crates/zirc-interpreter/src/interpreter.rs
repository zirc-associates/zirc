//! Main interpreter engine and builtins.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};

use crate::env::Env;
use crate::flow::Flow;
use crate::value::Value;
use zirc_bytecode::builtins::{eval_pure, format_showf};
use zirc_bytecode::Builtin;
use zirc_syntax::ast::*;
use zirc_syntax::error::{error, Error, ErrorKind, Result};

/// Wrap a checked integer operation: overflow is an error, never a panic.
fn checked_int(v: Option<i64>) -> Result<Value> {
    v.map(Value::Int)
        .ok_or_else(|| Error::of_kind(ErrorKind::Runtime, "integer overflow"))
}

/// Numeric equality with int/float promotion (`1 == 1.0` is true).
fn values_equal(x: &Value, y: &Value) -> bool {
    match (x.as_f64(), y.as_f64()) {
        (Some(a), Some(b)) if !matches!(x, Value::Int(_)) || !matches!(y, Value::Int(_)) => a == b,
        _ => x == y,
    }
}

/// Compare two numeric operands (int/float promoted to float).
fn numeric_cmp(
    interp: &mut Interpreter,
    env: &mut Env<'_>,
    a: &Expr,
    b: &Expr,
    op: impl Fn(f64, f64) -> bool,
    op_name: &str,
) -> Result<Value> {
    let (x, y) = (interp.eval_expr(env, a)?, interp.eval_expr(env, b)?);
    match (x.as_f64(), y.as_f64()) {
        (Some(p), Some(q)) => Ok(Value::Bool(op(p, q))),
        _ => error(format!("{} expects numbers", op_name)),
    }
}

#[derive(Default, Debug, Clone)]
pub struct MemoryStats {
    /// Number of string values allocated during execution
    pub strings_allocated: usize,
    /// Total bytes allocated for string storage
    pub bytes_allocated: usize,
}

/// A closure definition in the interpreter's closure table.
/// `captures` are the free-variable names, in the same order as the
/// captured values stored in the `Value::Closure`.
struct ClosureDef {
    params: Vec<Param>,
    capture_names: Vec<String>,
    return_type: Option<Type>,
    body: Vec<Stmt>,
}

pub struct Interpreter {
    /// Global function definitions available to all scopes
    functions: HashMap<String, Function>,
    /// Closure bodies; `Value::Closure.func` indexes into this table.
    closures: Vec<ClosureDef>,
    /// Cache from closure definition site to table index, so
    /// evaluating the same closure literal in a loop does not grow
    /// the table (one entry per lexical site, like the VM).
    closure_cache: HashMap<String, usize>,
    /// Current call depth, for the recursion limit.
    depth: usize,
    /// Memory usage tracking for observability
    mem: MemoryStats,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            closures: Vec::new(),
            closure_cache: HashMap::new(),
            depth: 0,
            mem: MemoryStats::default(),
        }
    }

    pub fn memory_stats(&self) -> MemoryStats {
        self.mem.clone()
    }

    pub fn reset(&mut self) {
        self.functions.clear();
        self.closures.clear();
        self.closure_cache.clear();
        self.mem = MemoryStats::default();
    }

    pub fn function_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.functions.keys().cloned().collect();
        v.sort();
        v
    }

    pub fn run(&mut self, program: Program) -> Result<()> {
        let mut env = Env::new_root();
        let _ = self.run_with_env(program, &mut env)?;
        Ok(())
    }

    pub fn run_with_env(&mut self, program: Program, env: &mut Env<'_>) -> Result<Option<Value>> {
        for item in &program.items {
            match &item.kind {
                ItemKind::Function(f) => {
                    self.functions.insert(f.name.clone(), f.clone());
                }
                ItemKind::Import { module, .. } => {
                    return error(format!(
                        "import {} requires the module loader (use the zirc CLI)",
                        module
                    ));
                }
                _ => {}
            }
        }
        let mut last: Option<Value> = None;
        for item in program.items {
            if let ItemKind::Stmt(s) = item.kind {
                match self.exec_stmt(env, &s)? {
                    Flow::Continue(v) => last = Some(v),
                    Flow::Return(_) => return error("'return' outside of function"),
                    Flow::Break => return error("'break' outside of loop"),
                    Flow::ContinueLoop => return error("'continue' outside of loop"),
                }
            }
        }
        Ok(last)
    }

    fn exec_block(&mut self, env: &mut Env<'_>, body: &[Stmt]) -> Result<Flow> {
        // `let` bindings created inside a block do not leak out of it.
        // Assignments to bindings that already existed keep their effect.
        let snapshot = env.enter_scope();
        let result = self.exec_block_inner(env, body);
        env.restore_scope(&snapshot);
        result
    }

    fn exec_block_inner(&mut self, env: &mut Env<'_>, body: &[Stmt]) -> Result<Flow> {
        let mut last = Value::Unit;
        for s in body {
            match self.exec_stmt(env, s)? {
                Flow::Continue(v) => {
                    last = v;
                }
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Break => return Ok(Flow::Break),
                Flow::ContinueLoop => return Ok(Flow::ContinueLoop),
            }
        }
        Ok(Flow::Continue(last))
    }

    fn exec_stmt(&mut self, env: &mut Env<'_>, stmt: &Stmt) -> Result<Flow> {
        // Attach the statement's span to any unlocated error, so
        // runtime failures point at the offending line.
        let span = stmt.span;
        self.exec_stmt_inner(env, stmt)
            .map_err(|e| e.with_span_if_missing(span))
    }

    fn exec_stmt_inner(&mut self, env: &mut Env<'_>, stmt: &Stmt) -> Result<Flow> {
        match &stmt.kind {
            StmtKind::Comment(_) => Ok(Flow::Continue(Value::Unit)),
            StmtKind::Let { name, ty, expr } => {
                let v = self.eval_expr(env, expr)?;
                if let Some(t) = ty {
                    Interpreter::check_type(&v, t)?;
                }
                env.define(name.clone(), v, ty.clone());
                Ok(Flow::Continue(Value::Unit))
            }
            StmtKind::Assign { name, expr } => {
                let v = self.eval_expr(env, expr)?;
                env.assign(name, v)?;
                Ok(Flow::Continue(Value::Unit))
            }
            StmtKind::Return(opt) => {
                let v = match opt {
                    Some(e) => self.eval_expr(env, e)?,
                    None => Value::Unit,
                };
                Ok(Flow::Return(v))
            }
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                let c = self.eval_expr(env, cond)?;
                match c {
                    Value::Bool(true) => self.exec_block(env, then_body),
                    Value::Bool(false) => self.exec_block(env, else_body),
                    other => error(format!("if condition must be bool, got {:?}", other)),
                }
            }
            StmtKind::While { cond, body } => {
                loop {
                    let c = self.eval_expr(env, cond)?;
                    let go = match c {
                        Value::Bool(b) => b,
                        other => {
                            return error(format!("while condition must be bool, got {:?}", other));
                        }
                    };
                    if !go {
                        break;
                    }
                    match self.exec_block(env, body)? {
                        Flow::Continue(_) => {}
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Break => break,
                        Flow::ContinueLoop => continue,
                    }
                }
                Ok(Flow::Continue(Value::Unit))
            }
            StmtKind::For {
                var,
                start,
                end,
                body,
            } => {
                let s = self.eval_expr(env, start)?;
                let e = self.eval_expr(env, end)?;
                let (mut i, e) = match (s, e) {
                    (Value::Int(a), Value::Int(b)) => (a, b),
                    (a, b) => {
                        return error(format!("for bounds must be ints, got {:?} and {:?}", a, b));
                    }
                };
                // The loop variable is scoped to the loop: restore any
                // previous binding (or remove it) when the loop ends.
                let previous = env.get(var);
                while i < e {
                    if env.get(var).is_some() {
                        env.assign(var, Value::Int(i))?;
                    } else {
                        env.define(var.clone(), Value::Int(i), Some(Type::Int));
                    }
                    match self.exec_block(env, body)? {
                        Flow::Continue(_) => {}
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Break => break,
                        Flow::ContinueLoop => {
                            i += 1;
                            continue;
                        }
                    }
                    i += 1;
                }
                match previous {
                    Some(binding) => env.define(var.clone(), binding.value, binding.ty),
                    None => env.remove(var),
                }
                Ok(Flow::Continue(Value::Unit))
            }
            StmtKind::Break => Ok(Flow::Break),
            StmtKind::Continue => Ok(Flow::ContinueLoop),
            StmtKind::ExprStmt(e) => {
                let v = self.eval_expr(env, e)?;
                Ok(Flow::Continue(v))
            }
        }
    }

    fn eval_expr(&mut self, env: &mut Env<'_>, expr: &Expr) -> Result<Value> {
        match expr {
            Expr::LiteralInt(n) => Ok(Value::Int(*n)),
            Expr::LiteralFloat(v) => Ok(Value::Float(*v)),
            Expr::LiteralString(s) => {
                self.mem.strings_allocated += 1;
                self.mem.bytes_allocated += s.len();
                Ok(Value::Str(s.clone()))
            }
            Expr::LiteralBool(b) => Ok(Value::Bool(*b)),
            Expr::Ident(name) => match env.get(name) {
                Some(b) => Ok(b.value),
                None => zirc_syntax::error::error(format!("Undefined variable '{}'", name)),
            },
            Expr::Member(base, field) => {
                // Resolve qualified names like a.b (and nested) as a single global identifier.
                fn qualify(e: &Expr, tail: &str) -> Option<String> {
                    match e {
                        Expr::Ident(n) => Some(format!("{}.{}", n, tail)),
                        Expr::Member(b, f) => qualify(b, &format!("{}.{}", f, tail)),
                        _ => None,
                    }
                }
                let qualified = qualify(base, field).ok_or_else(|| {
                    format!(
                        "member access only supported on identifiers/modules, got {:?}",
                        base
                    )
                })?;
                match env.get(&qualified) {
                    Some(b) => Ok(b.value),
                    None => {
                        zirc_syntax::error::error(format!("Undefined variable '{}'", qualified))
                    }
                }
            }
            Expr::BinaryAdd(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) {
                (Value::Int(x), Value::Int(y)) => checked_int(x.checked_add(y)),
                (Value::Str(x), Value::Str(y)) => {
                    let r = format!("{}{}", x, y);
                    self.mem.strings_allocated += 1;
                    self.mem.bytes_allocated += r.len();
                    Ok(Value::Str(r))
                }
                (Value::List(mut x), Value::List(y)) => {
                    x.extend(y);
                    Ok(Value::List(x))
                }
                (x, y) if x.as_f64().is_some() && y.as_f64().is_some() => {
                    Ok(Value::Float(x.as_f64().unwrap() + y.as_f64().unwrap()))
                }
                (x, y) => error(format!("Cannot add {:?} and {:?}", x, y)),
            },
            Expr::BinarySub(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) {
                (Value::Int(x), Value::Int(y)) => checked_int(x.checked_sub(y)),
                (x, y) if x.as_f64().is_some() && y.as_f64().is_some() => {
                    Ok(Value::Float(x.as_f64().unwrap() - y.as_f64().unwrap()))
                }
                (x, y) => error(format!("Cannot subtract {:?} and {:?}", x, y)),
            },
            Expr::BinaryMul(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) {
                (Value::Int(x), Value::Int(y)) => checked_int(x.checked_mul(y)),
                (x, y) if x.as_f64().is_some() && y.as_f64().is_some() => {
                    Ok(Value::Float(x.as_f64().unwrap() * y.as_f64().unwrap()))
                }
                (x, y) => error(format!("Cannot multiply {:?} and {:?}", x, y)),
            },
            Expr::BinaryDiv(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) {
                (Value::Int(_), Value::Int(0)) => error("division by zero"),
                (Value::Int(x), Value::Int(y)) => checked_int(x.checked_div(y)),
                (x, y) if x.as_f64().is_some() && y.as_f64().is_some() => {
                    let divisor = y.as_f64().unwrap();
                    if divisor == 0.0 {
                        error("division by zero")
                    } else {
                        Ok(Value::Float(x.as_f64().unwrap() / divisor))
                    }
                }
                (x, y) => error(format!("Cannot divide {:?} and {:?}", x, y)),
            },
            Expr::Eq(a, b) => {
                let (x, y) = (self.eval_expr(env, a)?, self.eval_expr(env, b)?);
                Ok(Value::Bool(values_equal(&x, &y)))
            }
            Expr::Ne(a, b) => {
                let (x, y) = (self.eval_expr(env, a)?, self.eval_expr(env, b)?);
                Ok(Value::Bool(!values_equal(&x, &y)))
            }
            Expr::LogicalAnd(a, b) => match self.eval_expr(env, a)? {
                Value::Bool(false) => Ok(Value::Bool(false)),
                Value::Bool(true) => match self.eval_expr(env, b)? {
                    Value::Bool(bb) => Ok(Value::Bool(bb)),
                    other => error(format!("&& expects bool, got {:?}", other)),
                },
                other => error(format!("&& expects bool, got {:?}", other)),
            },
            Expr::LogicalOr(a, b) => match self.eval_expr(env, a)? {
                Value::Bool(true) => Ok(Value::Bool(true)),
                Value::Bool(false) => match self.eval_expr(env, b)? {
                    Value::Bool(bb) => Ok(Value::Bool(bb)),
                    other => error(format!("|| expects bool, got {:?}", other)),
                },
                other => error(format!("|| expects bool, got {:?}", other)),
            },
            Expr::LogicalNot(e) => match self.eval_expr(env, e)? {
                Value::Bool(b) => Ok(Value::Bool(!b)),
                other => error(format!("! expects bool, got {:?}", other)),
            },
            Expr::Neg(e) => match self.eval_expr(env, e)? {
                Value::Int(n) => checked_int(n.checked_neg()),
                Value::Float(v) => Ok(Value::Float(-v)),
                other => error(format!("- expects number, got {:?}", other)),
            },
            Expr::Lt(a, b) => numeric_cmp(self, env, a, b, |x, y| x < y, "<"),
            Expr::Le(a, b) => numeric_cmp(self, env, a, b, |x, y| x <= y, "<="),
            Expr::Gt(a, b) => numeric_cmp(self, env, a, b, |x, y| x > y, ">"),
            Expr::Ge(a, b) => numeric_cmp(self, env, a, b, |x, y| x >= y, ">="),
            Expr::List(elems) => {
                let mut v = Vec::with_capacity(elems.len());
                for e in elems {
                    v.push(self.eval_expr(env, e)?);
                }
                Ok(Value::List(v))
            }
            Expr::Index(base, idx) => {
                let b = self.eval_expr(env, base)?;
                let i = self.eval_expr(env, idx)?;
                let ix = match i {
                    Value::Int(n) => n,
                    other => return error(format!("index expects int, got {:?}", other)),
                };
                match b {
                    Value::List(items) => {
                        if ix < 0 || (ix as usize) >= items.len() {
                            return error("index out of bounds");
                        }
                        Ok(items[ix as usize].clone())
                    }
                    Value::Str(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        if ix < 0 || (ix as usize) >= chars.len() {
                            return error("index out of bounds");
                        }
                        let ch = chars[ix as usize];
                        let ss = ch.to_string();
                        self.mem.strings_allocated += 1;
                        self.mem.bytes_allocated += ss.len();
                        Ok(Value::Str(ss))
                    }
                    other => error(format!("indexing not supported for {:?}", other)),
                }
            }
            Expr::Call { name, args } => {
                // Builtins: resolve through the single registry in
                // `zirc-bytecode`, shared with the compiler and the VM.
                if let Some(bi) = Builtin::from_name(name) {
                    return self.call_builtin(env, bi, args);
                }
                // Named function first, then a closure stored in a
                // variable of the same name.
                if let Some(func) = self.functions.get(name).cloned() {
                    let mut evaluated_args = Vec::with_capacity(args.len());
                    for a in args.iter() {
                        evaluated_args.push(self.eval_expr(env, a)?);
                    }
                    return self.exec_function(env, &func, evaluated_args);
                }
                match env.get(name) {
                    Some(binding) => match binding.value {
                        Value::Closure {
                            func: idx,
                            captures,
                        } => {
                            let mut evaluated_args = Vec::with_capacity(args.len());
                            for a in args.iter() {
                                evaluated_args.push(self.eval_expr(env, a)?);
                            }
                            self.call_closure(env, idx, captures, evaluated_args)
                        }
                        other => error(format!("'{}' is not callable (got {:?})", name, other)),
                    },
                    None => error(format!("Undefined function '{}'", name)),
                }
            }
            Expr::CallValue(callee, args) => {
                let value = self.eval_expr(env, callee)?;
                match value {
                    Value::Closure {
                        func: idx,
                        captures,
                    } => {
                        let mut evaluated_args = Vec::with_capacity(args.len());
                        for a in args.iter() {
                            evaluated_args.push(self.eval_expr(env, a)?);
                        }
                        self.call_closure(env, idx, captures, evaluated_args)
                    }
                    other => error(format!(
                        "attempted to call a non-closure value: {:?}",
                        other
                    )),
                }
            }
            Expr::Closure(f) => {
                // Capture free variables by value, in free-var order.
                let param_names: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
                let free = zirc_syntax::analysis::free_vars(&param_names, &f.body);
                let mut captures = Vec::with_capacity(free.len());
                for name in &free {
                    match env.get(name) {
                        Some(binding) => captures.push(binding.value),
                        None => {
                            return error(format!(
                                "Undefined variable '{}' (captured by a closure; captured variables must be defined before the closure is created)",
                                name
                            ));
                        }
                    }
                }
                // Reuse the table entry for this lexical site.
                let key = format!("{:?}", (&f.params, &f.return_type, &f.body));
                let idx = if let Some(&existing) = self.closure_cache.get(&key) {
                    existing
                } else {
                    let def = ClosureDef {
                        params: f.params.clone(),
                        capture_names: free,
                        return_type: f.return_type.clone(),
                        body: f.body.clone(),
                    };
                    self.closures.push(def);
                    let idx = self.closures.len() - 1;
                    self.closure_cache.insert(key, idx);
                    idx
                };
                Ok(Value::Closure {
                    func: idx,
                    captures,
                })
            }
        }
    }

    /// Guard a function call with the recursion limit (parity with
    /// the VM's frame limit: deep recursion is an error, never a
    /// host stack overflow).
    fn enter_call(&mut self) -> Result<()> {
        const MAX_DEPTH: usize = 1000;
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            return error("call stack overflow (too many nested calls)");
        }
        Ok(())
    }

    /// Execute a named function with already-evaluated arguments.
    fn exec_function(
        &mut self,
        env: &mut Env<'_>,
        func: &Function,
        evaluated_args: Vec<Value>,
    ) -> Result<Value> {
        self.enter_call()?;
        let result = self.exec_function_inner(env, func, evaluated_args);
        self.depth -= 1;
        result
    }

    fn exec_function_inner(
        &mut self,
        env: &mut Env<'_>,
        func: &Function,
        evaluated_args: Vec<Value>,
    ) -> Result<Value> {
        if func.params.len() != evaluated_args.len() {
            return error(format!(
                "Function '{}' expected {} args, got {}",
                func.name,
                func.params.len(),
                evaluated_args.len()
            ));
        }
        let mut child = env.child();
        for (p, v) in func.params.iter().zip(evaluated_args.into_iter()) {
            if let Some(t) = &p.ty {
                Interpreter::check_type(&v, t)?;
            }
            child.define(p.name.clone(), v, p.ty.clone());
        }
        let mut inner = child;
        let flow = self.exec_block(&mut inner, &func.body)?;
        let ret_val = match flow {
            Flow::Continue(v) => v, // implicit last value
            Flow::Return(v) => v,
            Flow::Break => return error("'break' outside of loop"),
            Flow::ContinueLoop => return error("'continue' outside of loop"),
        };
        if let Some(expected) = func.return_type.clone() {
            Interpreter::check_type(&ret_val, &expected)?;
        }
        Ok(ret_val)
    }

    /// Call a closure by table index with captured values bound
    /// first, then arguments.
    fn call_closure(
        &mut self,
        env: &mut Env<'_>,
        idx: usize,
        captures: Vec<Value>,
        args: Vec<Value>,
    ) -> Result<Value> {
        self.enter_call()?;
        let result = self.call_closure_inner(env, idx, captures, args);
        self.depth -= 1;
        result
    }

    fn call_closure_inner(
        &mut self,
        env: &mut Env<'_>,
        idx: usize,
        captures: Vec<Value>,
        args: Vec<Value>,
    ) -> Result<Value> {
        // Clone the definition out of the table so the mutable
        // borrow of `self` does not conflict with execution.
        let def = self
            .closures
            .get(idx)
            .ok_or_else(|| format!("invalid closure index {}", idx))
            .map(|d| {
                (
                    d.params.clone(),
                    d.capture_names.clone(),
                    d.return_type.clone(),
                    d.body.clone(),
                )
            })?;
        let (params, capture_names, return_type, body) = def;
        if params.len() != args.len() {
            return error(format!(
                "Function expected {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let mut child = env.child();
        for (name, v) in capture_names.iter().zip(captures) {
            child.define(name.clone(), v, None);
        }
        for (p, v) in params.iter().zip(args) {
            if let Some(t) = &p.ty {
                Interpreter::check_type(&v, t)?;
            }
            child.define(p.name.clone(), v, p.ty.clone());
        }
        let mut inner = child;
        let flow = self.exec_block(&mut inner, &body)?;
        let ret_val = match flow {
            Flow::Continue(v) => v,
            Flow::Return(v) => v,
            Flow::Break => return error("'break' outside of loop"),
            Flow::ContinueLoop => return error("'continue' outside of loop"),
        };
        if let Some(expected) = &return_type {
            Interpreter::check_type(&ret_val, expected)?;
        }
        Ok(ret_val)
    }

    pub(crate) fn check_type(val: &Value, ty: &Type) -> Result<()> {
        let ok = matches!(
            (val, ty),
            (Value::Int(_), Type::Int)
                | (Value::Float(_), Type::Float)
                | (Value::Str(_), Type::String)
                | (Value::Bool(_), Type::Bool)
                | (Value::List(_), Type::List)
                | (Value::Unit, Type::Unit)
        );
        if ok {
            Ok(())
        } else {
            Err(Error::of_kind(
                ErrorKind::Type,
                format!(
                    "Type mismatch: value {:?} does not match type {:?}",
                    val, ty
                ),
            ))
        }
    }

    fn call_showf(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.is_empty() {
            return error("showf requires at least a format string");
        }
        let fmt = match self.eval_expr(env, &args[0])? {
            Value::Str(s) => s,
            _ => return error("showf first argument must be a string"),
        };
        let mut values = Vec::with_capacity(args.len().saturating_sub(1));
        for a in &args[1..] {
            values.push(self.eval_expr(env, a)?);
        }
        let out = format_showf(&fmt, &values)?;
        if std::env::var("ZIRC_BENCH_SILENT").is_err() {
            println!("{}", out);
        }
        Ok(Value::Unit)
    }

    /// Dispatch a builtin resolved through the shared registry.
    /// Pure builtins are evaluated by `zirc-bytecode` (shared with the
    /// VM); side-effecting ones keep their engine-specific handling.
    fn call_builtin(&mut self, env: &mut Env<'_>, bi: Builtin, args: &[Expr]) -> Result<Value> {
        match bi {
            Builtin::Show => self.call_show(env, args),
            Builtin::ShowF => self.call_showf(env, args),
            Builtin::Prompt => self.call_prompt(env, args),
            Builtin::Rf => self.call_rf(env, args),
            Builtin::Wf => self.call_wf(env, args),
            Builtin::Push => self.call_push(env, args),
            Builtin::Pop => self.call_pop(env, args),
            pure => self.call_pure(env, pure, args),
        }
    }

    /// Evaluate arguments, then run the shared pure implementation.
    fn call_pure(&mut self, env: &mut Env<'_>, bi: Builtin, args: &[Expr]) -> Result<Value> {
        let mut values = Vec::with_capacity(args.len());
        for a in args {
            values.push(self.eval_expr(env, a)?);
        }
        let result = eval_pure(bi, &values)
            .unwrap_or_else(|| error(format!("{}() is not a pure builtin", bi.name())))?;
        if let Value::Str(s) = &result {
            self.mem.strings_allocated += 1;
            self.mem.bytes_allocated += s.len();
        }
        Ok(result)
    }

    /// Simple show function - prints a single value
    fn call_show(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 {
            return error("show() expects exactly 1 argument");
        }
        let val = self.eval_expr(env, &args[0])?;
        if std::env::var("ZIRC_BENCH_SILENT").is_err() {
            println!("{}", val);
        }
        Ok(Value::Unit)
    }

    /// Prompt function - reads a line from stdin and returns as string
    fn call_prompt(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() > 1 {
            return error("prompt() expects 0 or 1 arguments");
        }
        let silent = std::env::var("ZIRC_BENCH_SILENT").is_ok();
        // Optional prompt string
        if args.len() == 1 {
            let prompt = self.eval_expr(env, &args[0])?;
            match prompt {
                Value::Str(s) => {
                    if !silent {
                        print!("{}", s);
                        io::stdout()
                            .flush()
                            .map_err(|e| format!("IO error: {}", e))?;
                    }
                }
                other => return error(format!("prompt() prompt must be string, got {:?}", other)),
            }
        }
        let input = if silent {
            std::env::var("ZIRC_BENCH_PROMPT_REPLY").unwrap_or_default()
        } else {
            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| format!("IO error: {}", e))?;
            // Remove trailing newline
            if input.ends_with('\n') {
                input.pop();
                if input.ends_with('\r') {
                    input.pop();
                }
            }
            input
        };
        self.mem.strings_allocated += 1;
        self.mem.bytes_allocated += input.len();
        Ok(Value::Str(input))
    }

    /// Read file function - reads entire file content as string
    fn call_rf(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 {
            return error("rf() expects exactly 1 argument");
        }
        let path = match self.eval_expr(env, &args[0])? {
            Value::Str(s) => s,
            other => return error(format!("rf() path must be string, got {:?}", other)),
        };
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;
        self.mem.strings_allocated += 1;
        self.mem.bytes_allocated += content.len();
        Ok(Value::Str(content))
    }

    /// Write file function - writes string content to file
    fn call_wf(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 2 {
            return error("wf() expects exactly 2 arguments: path and content");
        }
        let path = match self.eval_expr(env, &args[0])? {
            Value::Str(s) => s,
            other => return error(format!("wf() path must be string, got {:?}", other)),
        };
        let content = match self.eval_expr(env, &args[1])? {
            Value::Str(s) => s,
            other => return error(format!("wf() content must be string, got {:?}", other)),
        };
        fs::write(&path, &content)
            .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;
        Ok(Value::Unit)
    }

    /// Push function - adds element to end of list (mutates the list)
    fn call_push(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 2 {
            return error("push() expects exactly 2 arguments: list_variable and value");
        }
        let var_name = match &args[0] {
            Expr::Ident(name) => name,
            _ => return error("push() first argument must be a variable name"),
        };
        let current = env
            .get(var_name)
            .ok_or_else(|| format!("Undefined variable '{}'", var_name))?;
        let mut list = match current.value {
            Value::List(items) => items,
            other => return error(format!("push() expects list variable, got {:?}", other)),
        };
        let value = self.eval_expr(env, &args[1])?;
        list.push(value);
        env.assign(var_name, Value::List(list))?;
        Ok(Value::Unit)
    }

    /// Pop function - removes and returns last element from list
    fn call_pop(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 {
            return error("pop() expects exactly 1 argument: list_variable");
        }
        let var_name = match &args[0] {
            Expr::Ident(name) => name,
            _ => return error("pop() first argument must be a variable name"),
        };
        let current = env
            .get(var_name)
            .ok_or_else(|| format!("Undefined variable '{}'", var_name))?;
        let mut list = match current.value {
            Value::List(items) => items,
            other => return error(format!("pop() expects list variable, got {:?}", other)),
        };
        let popped = list.pop().ok_or("Cannot pop from empty list")?;
        env.assign(var_name, Value::List(list))?;
        Ok(popped)
    }
}
