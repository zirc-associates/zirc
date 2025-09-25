//! Main interpreter engine and builtins.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};

use crate::env::Env;
use crate::flow::Flow;
use crate::value::Value;
use zirc_syntax::ast::*;
use zirc_syntax::error::{Result, error};

#[derive(Default, Debug, Clone)]
pub struct MemoryStats {
    /// Number of string values allocated during execution
    pub strings_allocated: usize,
    /// Total bytes allocated for string storage
    pub bytes_allocated: usize,
}

pub struct Interpreter {
    /// Global function definitions available to all scopes
    functions: HashMap<String, Function>,
    /// Memory usage tracking for observability
    mem: MemoryStats,
}

impl Default for Interpreter {
    fn default() -> Self { Self::new() }
}

impl Interpreter {
    pub fn new() -> Self {
        Self { functions: HashMap::new(), mem: MemoryStats::default() }
    }

    pub fn memory_stats(&self) -> MemoryStats { self.mem.clone() }

    pub fn reset(&mut self) {
        self.functions.clear();
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
            if let Item::Function(f) = item {
                self.functions.insert(f.name.clone(), f.clone());
            }
        }
        let mut last: Option<Value> = None;
        for item in program.items {
            if let Item::Stmt(s) = item {
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
        let mut last = Value::Unit;
        for s in body {
            match self.exec_stmt(env, s)? {
                Flow::Continue(v) => { last = v; }
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Break => return Ok(Flow::Break),
                Flow::ContinueLoop => return Ok(Flow::ContinueLoop),
            }
        }
        Ok(Flow::Continue(last))
    }

    fn exec_stmt(&mut self, env: &mut Env<'_>, stmt: &Stmt) -> Result<Flow> {
        match stmt {
            Stmt::Let { name, ty, expr } => {
                let v = self.eval_expr(env, expr)?;
                if let Some(t) = ty { Interpreter::check_type(&v, t)?; }
                env.define(name.clone(), v, ty.clone());
                Ok(Flow::Continue(Value::Unit))
            }
            Stmt::Assign { name, expr } => {
                let v = self.eval_expr(env, expr)?;
                env.assign(name, v)?;
                Ok(Flow::Continue(Value::Unit))
            }
            Stmt::Return(opt) => {
                let v = match opt { Some(e) => self.eval_expr(env, e)?, None => Value::Unit };
                Ok(Flow::Return(v))
            }
            Stmt::If { cond, then_body, else_body } => {
                let c = self.eval_expr(env, cond)?;
                match c {
                    Value::Bool(true) => self.exec_block(env, then_body),
                    Value::Bool(false) => self.exec_block(env, else_body),
                    other => error(format!("if condition must be bool, got {:?}", other)),
                }
            }
            Stmt::While { cond, body } => {
                loop {
                    let c = self.eval_expr(env, cond)?;
                    let go = match c { Value::Bool(b) => b, other => { return error(format!("while condition must be bool, got {:?}", other)); } };
                    if !go { break; }
                    match self.exec_block(env, body)? {
                        Flow::Continue(_) => {}
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Break => break,
                        Flow::ContinueLoop => continue,
                    }
                }
                Ok(Flow::Continue(Value::Unit))
            }
            Stmt::For { var, start, end, body } => {
                let s = self.eval_expr(env, start)?;
                let e = self.eval_expr(env, end)?;
                let (mut i, e) = match (s, e) {
                    (Value::Int(a), Value::Int(b)) => (a, b),
                    (a, b) => { return error(format!("for bounds must be ints, got {:?} and {:?}", a, b)); }
                };
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
                        Flow::ContinueLoop => { i += 1; continue; }
                    }
                    i += 1;
                }
                Ok(Flow::Continue(Value::Unit))
            }
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::ContinueLoop),
            Stmt::ExprStmt(e) => {
                let v = self.eval_expr(env, e)?;
                Ok(Flow::Continue(v))
            }
        }
    }

    fn eval_expr(&mut self, env: &mut Env<'_>, expr: &Expr) -> Result<Value> {
        match expr {
            Expr::LiteralInt(n) => Ok(Value::Int(*n)),
            Expr::LiteralString(s) => { self.mem.strings_allocated += 1; self.mem.bytes_allocated += s.len(); Ok(Value::Str(s.clone())) }
            Expr::LiteralBool(b) => Ok(Value::Bool(*b)),
            Expr::Ident(name) => match env.get(name) { Some(b) => Ok(b.value), None => zirc_syntax::error::error(format!("Undefined variable '{}'", name)) },
            Expr::Member(base, field) => {
                // Resolve qualified names like a.b (and nested) as a single global identifier.
                fn qualify(e: &Expr, tail: &str) -> Option<String> {
                    match e {
                        Expr::Ident(n) => Some(format!("{}.{}", n, tail)),
                        Expr::Member(b, f) => qualify(b, &format!("{}.{}", f, tail)),
                        _ => None,
                    }
                }
                let qualified = qualify(base, field).ok_or_else(|| format!("member access only supported on identifiers/modules, got {:?}", base))?;
                match env.get(&qualified) {
                    Some(b) => Ok(b.value),
                    None => zirc_syntax::error::error(format!("Undefined variable '{}'", qualified)),
                }
            }
            Expr::BinaryAdd(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) {
                (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x + y)),
                (Value::Str(x), Value::Str(y)) => { let r = format!("{}{}", x, y); self.mem.strings_allocated += 1; self.mem.bytes_allocated += r.len(); Ok(Value::Str(r)) }
                (Value::List(mut x), Value::List(y)) => { x.extend(y); Ok(Value::List(x)) }
                (x, y) => error(format!("Cannot add {:?} and {:?}", x, y)),
            },
            Expr::BinarySub(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) {
                (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x - y)),
                (x, y) => error(format!("Cannot subtract {:?} and {:?}", x, y)),
            },
            Expr::BinaryMul(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) {
                (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x * y)),
                (x, y) => error(format!("Cannot multiply {:?} and {:?}", x, y)),
            },
            Expr::BinaryDiv(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) {
                (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x / y)),
                (x, y) => error(format!("Cannot divide {:?} and {:?}", x, y)),
            },
            Expr::Eq(a, b) => Ok(Value::Bool(self.eval_expr(env, a)? == self.eval_expr(env, b)?)),
            Expr::Ne(a, b) => Ok(Value::Bool(self.eval_expr(env, a)? != self.eval_expr(env, b)?)),
            Expr::LogicalAnd(a, b) => match self.eval_expr(env, a)? {
                Value::Bool(false) => Ok(Value::Bool(false)),
                Value::Bool(true) => match self.eval_expr(env, b)? { Value::Bool(bb) => Ok(Value::Bool(bb)), other => error(format!("&& expects bool, got {:?}", other)) },
                other => error(format!("&& expects bool, got {:?}", other)),
            },
            Expr::LogicalOr(a, b) => match self.eval_expr(env, a)? {
                Value::Bool(true) => Ok(Value::Bool(true)),
                Value::Bool(false) => match self.eval_expr(env, b)? { Value::Bool(bb) => Ok(Value::Bool(bb)), other => error(format!("|| expects bool, got {:?}", other)) },
                other => error(format!("|| expects bool, got {:?}", other)),
            },
            Expr::LogicalNot(e) => match self.eval_expr(env, e)? { Value::Bool(b) => Ok(Value::Bool(!b)), other => error(format!("! expects bool, got {:?}", other)) },
            Expr::Lt(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) { (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x < y)), _ => error("< expects ints") },
            Expr::Le(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) { (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x <= y)), _ => error("<= expects ints") },
            Expr::Gt(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) { (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x > y)), _ => error("> expects ints") },
            Expr::Ge(a, b) => match (self.eval_expr(env, a)?, self.eval_expr(env, b)?) { (Value::Int(x), Value::Int(y)) => Ok(Value::Bool(x >= y)), _ => error(">= expects ints") },
            Expr::List(elems) => { let mut v = Vec::with_capacity(elems.len()); for e in elems { v.push(self.eval_expr(env, e)?); } Ok(Value::List(v)) }
            Expr::Index(base, idx) => {
                let b = self.eval_expr(env, base)?;
                let i = self.eval_expr(env, idx)?;
                let ix = match i { Value::Int(n) => n, other => return error(format!("index expects int, got {:?}", other)) };
                match b {
                    Value::List(items) => {
                        if ix < 0 || (ix as usize) >= items.len() { return error("index out of bounds"); }
                        Ok(items[ix as usize].clone())
                    }
                    Value::Str(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        if ix < 0 || (ix as usize) >= chars.len() { return error("index out of bounds"); }
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
                // builtins
                match name.as_str() {
                    "showf" => return self.call_showf(env, args),
                    "show" => return self.call_show(env, args),
                    "prompt" => return self.call_prompt(env, args),
                    "rf" => return self.call_rf(env, args),
                    "wf" => return self.call_wf(env, args),
                    "len" => return self.call_len(env, args),
                    "push" => return self.call_push(env, args),
                    "pop" => return self.call_pop(env, args),
                    "slice" => return self.call_slice(env, args),
                    // Mathematical functions
                    "abs" => return self.call_abs(env, args),
                    "min" => return self.call_min(env, args),
                    "max" => return self.call_max(env, args),
                    "pow" => return self.call_pow(env, args),
                    "sqrt" => return self.call_sqrt(env, args),
                    "hex" => return self.call_hex(env, args),
                    "bin" => return self.call_bin(env, args),
                    // String functions
                    "upper" => return self.call_upper(env, args),
                    "lower" => return self.call_lower(env, args),
                    "trim" => return self.call_trim(env, args),
                    "split" => return self.call_split(env, args),
                    "join" => return self.call_join(env, args),
                    // Type conversion
                    "int" => return self.call_int(env, args),
                    "str" => return self.call_str(env, args),
                    // Utility functions
                    "type" => return self.call_type(env, args),
                    _ => {}
                }
                let func = self
                    .functions
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("Undefined function '{}'", name))?;
                if func.params.len() != args.len() {
                    return error(format!("Function '{}' expected {} args, got {}", name, func.params.len(), args.len()));
                }
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args.iter() { evaluated_args.push(self.eval_expr(env, a)?); }
                let mut child = env.child();
                for (p, v) in func.params.iter().zip(evaluated_args.into_iter()) {
                    if let Some(t) = &p.ty { Interpreter::check_type(&v, t)?; }
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
                if let Some(expected) = func.return_type.clone() { Interpreter::check_type(&ret_val, &expected)?; }
                Ok(ret_val)
            }
        }
    }

    pub(crate) fn check_type(val: &Value, ty: &Type) -> Result<()> {
        let ok = matches!((val, ty),
            (Value::Int(_), Type::Int)
            | (Value::Str(_), Type::String)
            | (Value::Bool(_), Type::Bool)
            | (Value::Unit, Type::Unit)
        );
        if ok { Ok(()) } else { error(format!("Type mismatch: value {:?} does not match type {:?}", val, ty)) }
    }

    fn call_showf(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.is_empty() { return error("showf requires at least a format string"); }
        let fmt = match self.eval_expr(env, &args[0])? { Value::Str(s) => s, _ => return error("showf first argument must be a string") };
        let mut out = String::new();
        let mut arg_i = 1usize;
        let mut chars = fmt.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                match chars.next() {
                    Some('d') => {
                        if arg_i >= args.len() { return error("showf missing %d argument"); }
                        match self.eval_expr(env, &args[arg_i])? { Value::Int(n) => out.push_str(&n.to_string()), other => return error(format!("%d expects int, got {:?}", other)) }
                        arg_i += 1;
                    }
                    Some('s') => {
                        if arg_i >= args.len() { return error("showf missing %s argument"); }
                        match self.eval_expr(env, &args[arg_i])? {
                            Value::Str(s) => out.push_str(&s),
                            Value::Bool(b) => out.push_str(if b { "true" } else { "false" }),
                            Value::List(items) => out.push_str(&format!("{}", Value::List(items))),
                            other => { return error(format!("%s expects string/bool/list, got {:?}", other)); }
                        }
                        arg_i += 1;
                    }
                    Some('%') => out.push('%'),
                    Some(other) => { return error(format!("Unsupported format specifier %{}", other)); }
                    None => return error("Dangling % at end of format string"),
                }
            } else {
                out.push(c);
            }
        }
        if std::env::var("ZIRC_BENCH_SILENT").is_err() { println!("{}", out); }
        Ok(Value::Unit)
    }

    /// Simple show function - prints a single value
    fn call_show(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("show() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        if std::env::var("ZIRC_BENCH_SILENT").is_err() { println!("{}", val); }
        Ok(Value::Unit)
    }

    /// Prompt function - reads a line from stdin and returns as string
    fn call_prompt(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() > 1 { return error("prompt() expects 0 or 1 arguments"); }
        let silent = std::env::var("ZIRC_BENCH_SILENT").is_ok();
        // Optional prompt string
        if args.len() == 1 {
            let prompt = self.eval_expr(env, &args[0])?;
            match prompt {
                Value::Str(s) => {
                    if !silent { print!("{}", s); io::stdout().flush().map_err(|e| format!("IO error: {}", e))?; }
                }
                other => return error(format!("prompt() prompt must be string, got {:?}", other)),
            }
        }
        let input = if silent {
            std::env::var("ZIRC_BENCH_PROMPT_REPLY").unwrap_or_default()
        } else {
            let mut input = String::new();
            io::stdin().read_line(&mut input).map_err(|e| format!("IO error: {}", e))?;
            // Remove trailing newline
            if input.ends_with('\n') { input.pop(); if input.ends_with('\r') { input.pop(); } }
            input
        };
        self.mem.strings_allocated += 1;
        self.mem.bytes_allocated += input.len();
        Ok(Value::Str(input))
    }

    /// Read file function - reads entire file content as string
    fn call_rf(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("rf() expects exactly 1 argument"); }
        let path = match self.eval_expr(env, &args[0])? { Value::Str(s) => s, other => return error(format!("rf() path must be string, got {:?}", other)) };
        let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file '{}': {}", path, e))?;
        self.mem.strings_allocated += 1;
        self.mem.bytes_allocated += content.len();
        Ok(Value::Str(content))
    }

    /// Write file function - writes string content to file
    fn call_wf(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 2 { return error("wf() expects exactly 2 arguments: path and content"); }
        let path = match self.eval_expr(env, &args[0])? { Value::Str(s) => s, other => return error(format!("wf() path must be string, got {:?}", other)) };
        let content = match self.eval_expr(env, &args[1])? { Value::Str(s) => s, other => return error(format!("wf() content must be string, got {:?}", other)) };
        fs::write(&path, &content).map_err(|e| format!("Failed to write file '{}': {}", path, e))?;
        Ok(Value::Unit)
    }

    /// Length function - returns length of string or list
    fn call_len(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("len() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        match val {
            Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
            Value::List(items) => Ok(Value::Int(items.len() as i64)),
            other => error(format!("len() expects string or list, got {:?}", other)),
        }
    }

    /// Push function - adds element to end of list (mutates the list)
    fn call_push(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 2 { return error("push() expects exactly 2 arguments: list_variable and value"); }
        
        // First argument must be an identifier (variable name)
        let var_name = match &args[0] {
            Expr::Ident(name) => name,
            _ => return error("push() first argument must be a variable name"),
        };
        
        // Get the current value and ensure it's a list
        let current = env.get(var_name)
            .ok_or_else(|| format!("Undefined variable '{}'", var_name))?;
        
        let mut list = match current.value {
            Value::List(items) => items,
            other => return error(format!("push() expects list variable, got {:?}", other)),
        };
        
        // Evaluate the value to push
        let value = self.eval_expr(env, &args[1])?;
        
        // Add the value to the list
        list.push(value);
        
        // Update the variable
        env.assign(var_name, Value::List(list))?;
        
        Ok(Value::Unit)
    }

    /// Pop function - removes and returns last element from list
    fn call_pop(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("pop() expects exactly 1 argument: list_variable"); }
        
        // First argument must be an identifier (variable name)
        let var_name = match &args[0] {
            Expr::Ident(name) => name,
            _ => return error("pop() first argument must be a variable name"),
        };
        
        // Get the current value and ensure it's a list
        let current = env.get(var_name)
            .ok_or_else(|| format!("Undefined variable '{}'", var_name))?;
        
        let mut list = match current.value {
            Value::List(items) => items,
            other => return error(format!("pop() expects list variable, got {:?}", other)),
        };
        
        // Pop the last element
        let popped = list.pop().ok_or_else(|| "Cannot pop from empty list")?;
        
        // Update the variable
        env.assign(var_name, Value::List(list))?;
        
        Ok(popped)
    }

    /// Slice function - returns a portion of a string or list
    fn call_slice(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 3 { return error("slice() expects exactly 3 arguments: collection, start, end"); }
        
        let collection = self.eval_expr(env, &args[0])?;
        let start = match self.eval_expr(env, &args[1])? {
            Value::Int(n) => n,
            other => return error(format!("slice() start index must be int, got {:?}", other)),
        };
        let end = match self.eval_expr(env, &args[2])? {
            Value::Int(n) => n,
            other => return error(format!("slice() end index must be int, got {:?}", other)),
        };
        
        if start < 0 { return error("slice() start index cannot be negative"); }
        if end < start { return error("slice() end index must be >= start index"); }
        
        match collection {
            Value::Str(s) => {
                let chars: Vec<char> = s.chars().collect();
                let start_idx = start as usize;
                let end_idx = (end as usize).min(chars.len());
                
                if start_idx >= chars.len() {
                    let result = String::new();
                    self.mem.strings_allocated += 1;
                    return Ok(Value::Str(result));
                }
                
                let slice: String = chars[start_idx..end_idx].iter().collect();
                self.mem.strings_allocated += 1;
                self.mem.bytes_allocated += slice.len();
                Ok(Value::Str(slice))
            },
            Value::List(items) => {
                let start_idx = start as usize;
                let end_idx = (end as usize).min(items.len());
                
                if start_idx >= items.len() {
                    return Ok(Value::List(Vec::new()));
                }
                
                Ok(Value::List(items[start_idx..end_idx].to_vec()))
            },
            other => error(format!("slice() expects string or list, got {:?}", other)),
        }
    }

    // Mathematical functions
    
    /// Absolute value function
    fn call_abs(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("abs() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        match val {
            Value::Int(n) => Ok(Value::Int(n.abs())),
            other => error(format!("abs() expects int, got {:?}", other)),
        }
    }
    
    /// Minimum of two values
    fn call_min(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 2 { return error("min() expects exactly 2 arguments"); }
        let a = self.eval_expr(env, &args[0])?;
        let b = self.eval_expr(env, &args[1])?;
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x.min(y))),
            _ => error("min() expects two ints"),
        }
    }
    
    /// Maximum of two values
    fn call_max(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 2 { return error("max() expects exactly 2 arguments"); }
        let a = self.eval_expr(env, &args[0])?;
        let b = self.eval_expr(env, &args[1])?;
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x.max(y))),
            _ => error("max() expects two ints"),
        }
    }
    
    /// Power function (base^exp)
    fn call_pow(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 2 { return error("pow() expects exactly 2 arguments: base and exponent"); }
        let base = self.eval_expr(env, &args[0])?;
        let exp = self.eval_expr(env, &args[1])?;
        match (base, exp) {
            (Value::Int(b), Value::Int(e)) => {
                if e < 0 { return error("pow() exponent cannot be negative"); }
                let result = (b as f64).powi(e as i32) as i64;
                Ok(Value::Int(result))
            },
            _ => error("pow() expects two ints"),
        }
    }
    
    /// Square root function
    fn call_sqrt(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("sqrt() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        match val {
            Value::Int(n) => {
                if n < 0 { return error("sqrt() argument cannot be negative"); }
                let result = (n as f64).sqrt() as i64;
                Ok(Value::Int(result))
            },
            other => error(format!("sqrt() expects int, got {:?}", other)),
        }
    }

    /// Hexadecimal function converts integer to hex string
    fn call_hex(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("hex() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        match val {
            Value::Int(n) => {
                let result = format!("0x{:x}", n);
                self.mem.strings_allocated += 1;
                self.mem.bytes_allocated += result.len();
                Ok(Value::Str(result))
            },
            other => error(format!("hex() expects int, got {:?}", other)),
        }
    }

    /// Binary function converts integer to binary string
    fn call_bin(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("bin() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        match val {
            Value::Int(n) => {
                let result = format!("0b{:b}", n);
                self.mem.strings_allocated += 1;
                self.mem.bytes_allocated += result.len();
                Ok(Value::Str(result))
            },
            other => error(format!("bin() expects int, got {:?}", other)),
        }
    }
    
    // String functions
    
    /// Convert string to uppercase
    fn call_upper(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("upper() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        match val {
            Value::Str(s) => {
                let result = s.to_uppercase();
                self.mem.strings_allocated += 1;
                self.mem.bytes_allocated += result.len();
                Ok(Value::Str(result))
            },
            other => error(format!("upper() expects string, got {:?}", other)),
        }
    }
    
    /// Convert string to lowercase
    fn call_lower(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("lower() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        match val {
            Value::Str(s) => {
                let result = s.to_lowercase();
                self.mem.strings_allocated += 1;
                self.mem.bytes_allocated += result.len();
                Ok(Value::Str(result))
            },
            other => error(format!("lower() expects string, got {:?}", other)),
        }
    }
    
    /// Trim whitespace from string
    fn call_trim(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("trim() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        match val {
            Value::Str(s) => {
                let result = s.trim().to_string();
                self.mem.strings_allocated += 1;
                self.mem.bytes_allocated += result.len();
                Ok(Value::Str(result))
            },
            other => error(format!("trim() expects string, got {:?}", other)),
        }
    }
    
    /// Split string by delimiter
    fn call_split(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 2 { return error("split() expects exactly 2 arguments: string and delimiter"); }
        let text = self.eval_expr(env, &args[0])?;
        let delimiter = self.eval_expr(env, &args[1])?;
        match (text, delimiter) {
            (Value::Str(s), Value::Str(delim)) => {
                let parts: Vec<Value> = s.split(&delim)
                    .map(|part| {
                        self.mem.strings_allocated += 1;
                        self.mem.bytes_allocated += part.len();
                        Value::Str(part.to_string())
                    })
                    .collect();
                Ok(Value::List(parts))
            },
            _ => error("split() expects two strings"),
        }
    }
    
    /// Join list of strings with separator
    fn call_join(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 2 { return error("join() expects exactly 2 arguments: list and separator"); }
        let list = self.eval_expr(env, &args[0])?;
        let separator = self.eval_expr(env, &args[1])?;
        match (list, separator) {
            (Value::List(items), Value::Str(sep)) => {
                let strings: Result<Vec<String>> = items.into_iter()
                    .map(|item| match item {
                        Value::Str(s) => Ok(s),
                        other => error(format!("join() list must contain only strings, got {:?}", other)),
                    })
                    .collect();
                let result = strings?.join(&sep);
                self.mem.strings_allocated += 1;
                self.mem.bytes_allocated += result.len();
                Ok(Value::Str(result))
            },
            _ => error("join() expects list and string"),
        }
    }
    
    // Type conversion functions
    
    /// Convert value to integer
    fn call_int(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("int() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        match val {
            Value::Int(n) => Ok(Value::Int(n)),
            Value::Str(s) => {
                match s.parse::<i64>() {
                    Ok(n) => Ok(Value::Int(n)),
                    Err(_) => error(format!("Cannot convert '{}' to int", s)),
                }
            },
            Value::Bool(true) => Ok(Value::Int(1)),
            Value::Bool(false) => Ok(Value::Int(0)),
            other => error(format!("Cannot convert {:?} to int", other)),
        }
    }
    
    /// Convert value to string
    fn call_str(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("str() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        let result = match val {
            Value::Str(s) => s,
            Value::Int(n) => n.to_string(),
            Value::Bool(b) => if b { "true".to_string() } else { "false".to_string() },
            Value::List(items) => format!("{}", Value::List(items)),
            Value::Unit => "<unit>".to_string(),
        };
        self.mem.strings_allocated += 1;
        self.mem.bytes_allocated += result.len();
        Ok(Value::Str(result))
    }
    
    // Utility functions
    
    /// Get type of value as string
    fn call_type(&mut self, env: &mut Env<'_>, args: &[Expr]) -> Result<Value> {
        if args.len() != 1 { return error("type() expects exactly 1 argument"); }
        let val = self.eval_expr(env, &args[0])?;
        let type_name = match val {
            Value::Int(_) => "int",
            Value::Str(_) => "string",
            Value::Bool(_) => "bool",
            Value::List(_) => "list",
            Value::Unit => "unit",
        };
        self.mem.strings_allocated += 1;
        self.mem.bytes_allocated += type_name.len();
        Ok(Value::Str(type_name.to_string()))
    }
}

