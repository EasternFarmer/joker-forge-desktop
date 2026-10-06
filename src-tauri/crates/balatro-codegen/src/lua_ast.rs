use std::fmt::{self, Write as _};
use serde::Serialize;
use serde_json::Value;

// ---------------------------------------------------------------------------
// AST node types
// ---------------------------------------------------------------------------

/// A top-level sequence of statements (a file / chunk).
#[derive(Debug, Clone)]
pub struct Chunk {
    pub stmts: Vec<Stmt>,
}

/// Lua statement.
#[derive(Debug, Clone)]
pub enum Stmt {
    /// `local name = expr` (expr is optional → `local name`)
    Local(String, Option<Expr>),
    /// `target = expr`  (target can be an ident, field access, index: etc.)
    Assign(Expr, Expr),
    /// Multi-target assign: `a, b = expr1, expr2`
    MultiAssign(Vec<Expr>, Vec<Expr>),
    /// `if cond then body [elseif cond then body]* [else body] end`
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        else_body: Option<Vec<Stmt>>,
    },
    /// `return expr`  (expr is optional → bare `return`)
    Return(Option<Expr>),
    /// Expression used as a statement (function calls: method calls).
    ExprStmt(Expr),
    /// `for name = start, stop [: step] do body end`
    ForRange {
        var: String,
        start: Expr,
        stop: Expr,
        step: Option<Expr>,
        body: Vec<Stmt>,
    },
    /// `for vars in exprs do body end`
    ForIn {
        vars: Vec<String>,
        iterators: Vec<Expr>,
        body: Vec<Stmt>,
    },
    /// `-- comment text`
    Comment(String),
    /// A blank line for readability.
    Blank,
    /// Raw Lua string: escape hatch for patterns that don't fit the AST.
    Raw(String),
    /// A `do ... end` block.
    DoBlock(Vec<Stmt>),
    /// Non-emitted boundary marker used to build external segment maps.
    SegmentStart(String),
    /// Non-emitted boundary marker used to build external segment maps.
    SegmentEnd(String),
}

/// Lua expression.
#[derive(Debug, Clone)]
pub enum Expr {
    /// An editable source value; renders exactly as its inner expression.
    FieldBinding(Box<Expr>, LuaFieldSource),
    /// A source block span; renders exactly as its inner expression.
    Segment(Box<Expr>, String),
    Nil,
    Bool(bool),
    Int(i64),
    Number(f64),
    Str(String),
    /// Multi-line string `[[...]]`
    LongStr(String),
    /// Variable / identifier reference.
    Ident(String),
    /// `obj.field`
    Field(Box<Expr>, String),
    /// `obj[key]`
    Index(Box<Expr>, Box<Expr>),
    /// `lhs op rhs`
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    /// `op expr`
    UnaryOp(UnaryOp, Box<Expr>),
    /// `func(args)`
    Call(Box<Expr>, Vec<Expr>),
    /// `obj:method(args)`
    MethodCall(Box<Expr>, String, Vec<Expr>),
    /// `{ entries }`
    Table(Vec<TableEntry>),
    /// `function(params) body end`
    Function {
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    /// Raw Lua expression string: escape hatch.
    Raw(String),
    /// `func { entries }`, Lua table-call syntax (single table arg: no parens).
    /// Used for `SMODS.Joker { ... }` style calls.
    TableCall(Box<Expr>, Vec<TableEntry>),
}

/// Table constructor entry.
#[derive(Debug, Clone)]
pub enum TableEntry {
    /// `key = value`
    KeyValue(String, Expr),
    /// `[expr] = value`
    IndexValue(Expr, Expr),
    /// Positional value.
    Value(Expr),
    /// `-- comment` inside a table body (used for section markers).
    Comment(String),
    /// Non-emitted boundary marker used to build external segment maps.
    SegmentStart(String),
    /// Non-emitted boundary marker used to build external segment maps.
    SegmentEnd(String),
}

#[derive(Debug, Clone)]
pub struct LuaSegment {
    pub id: String,
    pub segment_type: String,
    pub name: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone)]
pub struct LuaFieldSource {
    pub source_path: Vec<Value>,
    pub original_value: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LuaFieldBinding {
    pub source_path: Vec<Value>,
    pub value_type: String,
    pub original_value: Value,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

pub fn lua_field_binding(expr: Expr, source: LuaFieldSource) -> Expr {
    Expr::FieldBinding(Box::new(expr), source)
}

/// Mark scalar leaves using sources scoped to one compiler node.
pub fn bind_scalar_literals(
    expr: &mut Expr,
    source_for: &impl Fn(&Expr) -> Option<LuaFieldSource>,
) {
    if matches!(
        expr,
        Expr::Str(_) | Expr::Bool(_) | Expr::Int(_) | Expr::Number(_)
    ) {
        if let Some(source) = source_for(expr) {
            *expr = lua_field_binding(expr.clone(), source);
        }
        return;
    }
    match expr {
        Expr::FieldBinding(_, _) => {}
        Expr::Segment(object, _) | Expr::Field(object, _) | Expr::UnaryOp(_, object) => {
            bind_scalar_literals(object, source_for)
        }
        Expr::Index(object, index) | Expr::BinOp(object, _, index) => {
            bind_scalar_literals(object, source_for);
            bind_scalar_literals(index, source_for);
        }
        Expr::Call(function, args) | Expr::MethodCall(function, _, args) => {
            bind_scalar_literals(function, source_for);
            for arg in args {
                bind_scalar_literals(arg, source_for);
            }
        }
        Expr::Table(entries) | Expr::TableCall(_, entries) => {
            for entry in entries {
                match entry {
                    TableEntry::KeyValue(_, value) | TableEntry::Value(value) => {
                        bind_scalar_literals(value, source_for)
                    }
                    TableEntry::IndexValue(_, value) => bind_scalar_literals(value, source_for),
                    _ => {}
                }
            }
        }
        Expr::Function { body, .. } => {
            for stmt in body {
                bind_statement_literals(stmt, source_for);
            }
        }
        _ => {}
    }
}

pub fn bind_statement_literals(
    stmt: &mut Stmt,
    source_for: &impl Fn(&Expr) -> Option<LuaFieldSource>,
) {
    match stmt {
        Stmt::Local(_, Some(value)) | Stmt::Return(Some(value)) | Stmt::ExprStmt(value) => {
            bind_scalar_literals(value, source_for)
        }
        Stmt::Assign(_, value) => bind_scalar_literals(value, source_for),
        Stmt::MultiAssign(_, values) => {
            for value in values {
                bind_scalar_literals(value, source_for);
            }
        }
        Stmt::If {
            branches,
            else_body,
        } => {
            for (condition, statements) in branches {
                bind_scalar_literals(condition, source_for);
                for statement in statements {
                    bind_statement_literals(statement, source_for);
                }
            }
            if let Some(statements) = else_body {
                for statement in statements {
                    bind_statement_literals(statement, source_for);
                }
            }
        }
        // Loop bounds and constructor indices often contain generated constants;
        // counts are bound separately in config.extra using their precise source.
        Stmt::ForRange { body, .. } | Stmt::ForIn { body, .. } | Stmt::DoBlock(body) => {
            for statement in body {
                bind_statement_literals(statement, source_for);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Concat,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    Len,
}

// ---------------------------------------------------------------------------
// Builder helpers, ergonomic AST construction
// ---------------------------------------------------------------------------

pub fn lua_nil() -> Expr {
    Expr::Nil
}
pub fn lua_bool(b: bool) -> Expr {
    Expr::Bool(b)
}
pub fn lua_int(n: i64) -> Expr {
    Expr::Int(n)
}
pub fn lua_num(n: f64) -> Expr {
    Expr::Number(n)
}
pub fn lua_str(s: impl Into<String>) -> Expr {
    Expr::Str(s.into())
}
pub fn lua_long_str(s: impl Into<String>) -> Expr {
    Expr::LongStr(s.into())
}
pub fn lua_ident(s: impl Into<String>) -> Expr {
    Expr::Ident(s.into())
}
pub fn lua_raw_expr(s: impl Into<String>) -> Expr {
    Expr::Raw(s.into())
}
pub fn lua_raw_stmt(s: impl Into<String>) -> Stmt {
    Stmt::Raw(s.into())
}

/// `obj.field`
pub fn lua_field(obj: Expr, field: impl Into<String>) -> Expr {
    Expr::Field(Box::new(obj), field.into())
}

/// `obj[key]`
pub fn lua_index(obj: Expr, key: Expr) -> Expr {
    Expr::Index(Box::new(obj), Box::new(key))
}

/// `func(args...)`
pub fn lua_call(func: impl Into<String>, args: Vec<Expr>) -> Expr {
    Expr::Call(Box::new(lua_ident(func)), args)
}

/// `obj:method(args...)`
pub fn lua_method(obj: Expr, method: impl Into<String>, args: Vec<Expr>) -> Expr {
    Expr::MethodCall(Box::new(obj), method.into(), args)
}

/// `{ key = value: ... }` from pairs
pub fn lua_table(entries: Vec<(impl Into<String>, Expr)>) -> Expr {
    Expr::Table(
        entries
            .into_iter()
            .map(|(k, v)| TableEntry::KeyValue(k.into(), v))
            .collect(),
    )
}

/// `{ entry: ... }` from raw TableEntry list
pub fn lua_table_raw(entries: Vec<TableEntry>) -> Expr {
    Expr::Table(entries)
}

/// `local name = expr`
pub fn lua_local(name: impl Into<String>, val: Expr) -> Stmt {
    Stmt::Local(name.into(), Some(val))
}

/// `name = expr`
pub fn lua_assign(target: Expr, val: Expr) -> Stmt {
    Stmt::Assign(target, val)
}

/// `return expr`
pub fn lua_return(val: Expr) -> Stmt {
    Stmt::Return(Some(val))
}

/// `return`
pub fn lua_return_bare() -> Stmt {
    Stmt::Return(None)
}

/// `-- comment`
pub fn lua_comment(text: impl Into<String>) -> Stmt {
    Stmt::Comment(text.into())
}

/// if cond then body end
pub fn lua_if(cond: Expr, body: Vec<Stmt>) -> Stmt {
    Stmt::If {
        branches: vec![(cond, body)],
        else_body: None,
    }
}

/// if cond then body else else_body end
pub fn lua_if_else(cond: Expr, body: Vec<Stmt>, else_body: Vec<Stmt>) -> Stmt {
    Stmt::If {
        branches: vec![(cond, body)],
        else_body: Some(else_body),
    }
}

/// `lhs and rhs`
pub fn lua_and(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::And, Box::new(rhs))
}

/// `lhs or rhs`
pub fn lua_or(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Or, Box::new(rhs))
}

/// `not expr`
pub fn lua_not(expr: Expr) -> Expr {
    Expr::UnaryOp(UnaryOp::Not, Box::new(expr))
}

/// `#expr` (length)
pub fn lua_len(expr: Expr) -> Expr {
    Expr::UnaryOp(UnaryOp::Len, Box::new(expr))
}

/// `lhs == rhs`
pub fn lua_eq(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Eq, Box::new(rhs))
}

/// `lhs ~= rhs`
pub fn lua_neq(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Neq, Box::new(rhs))
}

/// `lhs > rhs`
pub fn lua_gt(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Gt, Box::new(rhs))
}

/// `lhs < rhs`
pub fn lua_lt(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Lt, Box::new(rhs))
}

/// `lhs >= rhs`
pub fn lua_ge(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Ge, Box::new(rhs))
}

/// `lhs <= rhs`
pub fn lua_le(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Le, Box::new(rhs))
}

/// `lhs + rhs`
pub fn lua_add(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Add, Box::new(rhs))
}

/// `lhs - rhs`
pub fn lua_sub(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Sub, Box::new(rhs))
}

/// `lhs * rhs`
pub fn lua_mul(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Mul, Box::new(rhs))
}

/// `lhs / rhs`
pub fn lua_div(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp(Box::new(lhs), BinOp::Div, Box::new(rhs))
}

/// Chain expressions with `and`. Returns `true` for empty list.
pub fn lua_and_chain(exprs: Vec<Expr>) -> Expr {
    exprs
        .into_iter()
        .reduce(lua_and)
        .unwrap_or(Expr::Bool(true))
}

/// Chain expressions with `or`. Returns `false` for empty list.
pub fn lua_or_chain(exprs: Vec<Expr>) -> Expr {
    exprs
        .into_iter()
        .reduce(lua_or)
        .unwrap_or(Expr::Bool(false))
}

/// Build a dotted path: `a.b.c.d` from segments.
pub fn lua_path(segments: &[&str]) -> Expr {
    let mut iter = segments.iter();
    let first = iter.next().expect("lua_path needs at least one segment");
    let mut expr = lua_ident(*first);
    for seg in iter {
        expr = lua_field(expr, *seg);
    }
    expr
}

/// `func { entries }`: table-call syntax.
pub fn lua_table_call(func: Expr, entries: Vec<TableEntry>) -> Expr {
    Expr::TableCall(Box::new(func), entries)
}

/// Start a non-emitted segment boundary inside table bodies.
pub fn section_begin(section_id: &str) -> TableEntry {
    TableEntry::SegmentStart(section_id.to_string())
}

/// End a non-emitted segment boundary inside table bodies.
pub fn section_end(section_id: &str) -> TableEntry {
    TableEntry::SegmentEnd(section_id.to_string())
}

/// Start a non-emitted segment boundary in statement lists.
pub fn stmt_section_begin(section_id: &str) -> Stmt {
    Stmt::SegmentStart(section_id.to_string())
}

/// End a non-emitted segment boundary in statement lists.
pub fn stmt_section_end(section_id: &str) -> Stmt {
    Stmt::SegmentEnd(section_id.to_string())
}

/// Expression as statement (for function / method calls).
pub fn lua_expr_stmt(expr: Expr) -> Stmt {
    Stmt::ExprStmt(expr)
}

// ---------------------------------------------------------------------------
// Emitter, pretty-prints AST to Lua source
// ---------------------------------------------------------------------------

pub struct Emitter {
    indent: usize,
    indent_str: String,
    buf: String,
    segments: Vec<LuaSegment>,
    field_bindings: Vec<LuaFieldBinding>,
    segment_stack: Vec<(String, usize, usize)>,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            indent: 0,
            indent_str: "    ".to_string(),
            buf: String::with_capacity(4096),
            segments: Vec::new(),
            field_bindings: Vec::new(),
            segment_stack: Vec::new(),
        }
    }

    pub fn with_indent(indent: usize) -> Self {
        Self {
            indent,
            indent_str: "    ".to_string(),
            buf: String::with_capacity(4096),
            segments: Vec::new(),
            field_bindings: Vec::new(),
            segment_stack: Vec::new(),
        }
    }

    pub fn emit_chunk(mut self, chunk: &Chunk) -> String {
        for stmt in &chunk.stmts {
            self.emit_stmt(stmt);
        }
        self.buf
    }

    pub fn emit_stmts(mut self, stmts: &[Stmt]) -> String {
        for stmt in stmts {
            self.emit_stmt(stmt);
        }
        self.buf
    }

    pub fn emit_chunk_with_segments(mut self, chunk: &Chunk) -> (String, Vec<LuaSegment>) {
        for stmt in &chunk.stmts {
            self.emit_stmt(stmt);
        }
        (self.buf, self.segments)
    }

    pub fn emit_chunk_with_field_bindings(mut self, chunk: &Chunk) -> (String, Vec<LuaSegment>, Vec<LuaFieldBinding>) {
        for stmt in &chunk.stmts {
            self.emit_stmt(stmt);
        }
        (self.buf, self.segments, self.field_bindings)
    }

    pub fn emit_expr_to_string(mut self, expr: &Expr) -> String {
        self.emit_expr(expr);
        self.buf
    }

    // -- statements ---------------------------------------------------------

    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Local(name, expr) => {
                self.write_indent();
                if let Some(e) = expr {
                    self.buf.push_str("local ");
                    self.buf.push_str(name);
                    self.buf.push_str(" = ");
                    self.emit_expr(e);
                } else {
                    self.buf.push_str("local ");
                    self.buf.push_str(name);
                }
                self.buf.push('\n');
            }
            Stmt::Assign(target, val) => {
                self.write_indent();
                self.emit_expr(target);
                self.buf.push_str(" = ");
                self.emit_expr(val);
                self.buf.push('\n');
            }
            Stmt::MultiAssign(targets, values) => {
                self.write_indent();
                for (i, t) in targets.iter().enumerate() {
                    if i > 0 {
                        self.buf.push_str(", ");
                    }
                    self.emit_expr(t);
                }
                self.buf.push_str(" = ");
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        self.buf.push_str(", ");
                    }
                    self.emit_expr(v);
                }
                self.buf.push('\n');
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (i, (cond, body)) in branches.iter().enumerate() {
                    self.write_indent();
                    if i == 0 {
                        self.buf.push_str("if ");
                    } else {
                        self.buf.push_str("elseif ");
                    }
                    self.emit_expr(cond);
                    self.buf.push_str(" then\n");
                    self.indent += 1;
                    for s in body {
                        self.emit_stmt(s);
                    }
                    self.indent -= 1;
                }
                if let Some(eb) = else_body {
                    self.write_indent();
                    self.buf.push_str("else\n");
                    self.indent += 1;
                    for s in eb {
                        self.emit_stmt(s);
                    }
                    self.indent -= 1;
                }
                self.write_indent();
                self.buf.push_str("end\n");
            }
            Stmt::Return(expr) => {
                self.write_indent();
                if let Some(e) = expr {
                    self.buf.push_str("return ");
                    self.emit_expr(e);
                } else {
                    self.buf.push_str("return");
                }
                self.buf.push('\n');
            }
            Stmt::ExprStmt(expr) => {
                self.write_indent();
                self.emit_expr(expr);
                self.buf.push('\n');
            }
            Stmt::ForRange {
                var,
                start,
                stop,
                step,
                body,
            } => {
                self.write_indent();
                self.buf.push_str("for ");
                self.buf.push_str(var);
                self.buf.push_str(" = ");
                self.emit_expr(start);
                self.buf.push_str(", ");
                self.emit_expr(stop);
                if let Some(s) = step {
                    self.buf.push_str(", ");
                    self.emit_expr(s);
                }
                self.buf.push_str(" do\n");
                self.indent += 1;
                for s in body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.write_indent();
                self.buf.push_str("end\n");
            }
            Stmt::ForIn {
                vars,
                iterators,
                body,
            } => {
                self.write_indent();
                self.buf.push_str("for ");
                self.buf.push_str(&vars.join(", "));
                self.buf.push_str(" in ");
                for (i, it) in iterators.iter().enumerate() {
                    if i > 0 {
                        self.buf.push_str(", ");
                    }
                    self.emit_expr(it);
                }
                self.buf.push_str(" do\n");
                self.indent += 1;
                for s in body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.write_indent();
                self.buf.push_str("end\n");
            }
            Stmt::Comment(text) => {
                for line in text.lines() {
                    self.write_indent();
                    self.buf.push_str("-- ");
                    self.buf.push_str(line);
                    self.buf.push('\n');
                }
            }
            Stmt::Blank => {
                self.buf.push('\n');
            }
            Stmt::Raw(s) => {
                for line in s.lines() {
                    self.write_indent();
                    self.buf.push_str(line);
                    self.buf.push('\n');
                }
            }
            Stmt::DoBlock(body) => {
                self.write_indent();
                self.buf.push_str("do\n");
                self.indent += 1;
                for s in body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.write_indent();
                self.buf.push_str("end\n");
            }
            Stmt::SegmentStart(id) => self.push_segment_start(id),
            Stmt::SegmentEnd(id) => self.push_segment_end(id),
        }
    }

    // -- expressions --------------------------------------------------------

    fn emit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Segment(expr, id) => {
                self.push_segment_start(id);
                self.emit_expr(expr);
                self.push_segment_end(id);
            }
            Expr::FieldBinding(expr, source) => {
                let (start_line, start_column) = self.current_line_col();
                self.emit_expr(expr);
                let (end_line, end_column) = self.current_line_col();
                let (value_type, original_value) = match expr.as_ref() {
                    Expr::Str(value) => ("string", serde_json::json!(value)),
                    Expr::Bool(value) => ("boolean", serde_json::json!(value)),
                    Expr::Int(value) => ("number", serde_json::json!(value)),
                    Expr::Number(value) => ("number", serde_json::json!(value)),
                    _ => ("number", source.original_value.clone()),
                };
                self.field_bindings.push(LuaFieldBinding {
                    source_path: source.source_path.clone(),
                    value_type: value_type.to_string(),
                    original_value,
                    start_line, start_column, end_line, end_column,
                });
            }
            Expr::Nil => self.buf.push_str("nil"),
            Expr::Bool(b) => self.buf.push_str(if *b { "true" } else { "false" }),
            Expr::Int(n) => {
                self.buf.push_str(&n.to_string());
            }
            Expr::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    write!(self.buf, "{}", *n as i64).unwrap();
                } else {
                    write!(self.buf, "{}", n).unwrap();
                }
            }
            Expr::Str(s) => {
                self.buf.push('\'');
                self.buf.push_str(&escape_lua_string(s));
                self.buf.push('\'');
            }
            Expr::LongStr(s) => {
                // Use bracket level that doesn't conflict with content
                let level = find_safe_bracket_level(s);
                let eq = "=".repeat(level);
                self.buf.push('[');
                self.buf.push_str(&eq);
                self.buf.push('[');
                self.buf.push_str(s);
                self.buf.push(']');
                self.buf.push_str(&eq);
                self.buf.push(']');
            }
            Expr::Ident(name) => self.buf.push_str(name),
            Expr::Field(obj, field) => {
                self.emit_expr(obj);
                self.buf.push('.');
                self.buf.push_str(field);
            }
            Expr::Index(obj, key) => {
                self.emit_expr(obj);
                self.buf.push('[');
                self.emit_expr(key);
                self.buf.push(']');
            }
            Expr::BinOp(lhs, op, rhs) => {
                let needs_parens_lhs = expr_needs_parens(lhs, *op, true);
                let needs_parens_rhs = expr_needs_parens(rhs, *op, false);

                if needs_parens_lhs {
                    self.buf.push('(');
                }
                self.emit_expr(lhs);
                if needs_parens_lhs {
                    self.buf.push(')');
                }

                self.buf.push(' ');
                self.buf.push_str(op.as_str());
                self.buf.push(' ');

                if needs_parens_rhs {
                    self.buf.push('(');
                }
                self.emit_expr(rhs);
                if needs_parens_rhs {
                    self.buf.push(')');
                }
            }
            Expr::UnaryOp(op, inner) => {
                self.buf.push_str(op.as_str());
                // Add space after `not`
                if *op == UnaryOp::Not {
                    self.buf.push(' ');
                }
                let needs_parens = matches!(unwrapped_expr(inner), Expr::BinOp(..));
                if needs_parens {
                    self.buf.push('(');
                }
                self.emit_expr(inner);
                if needs_parens {
                    self.buf.push(')');
                }
            }
            Expr::Call(func, args) => {
                // If the function is a single-arg table call like SMODS.Joker{...}
                self.emit_expr(func);
                self.buf.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.buf.push_str(", ");
                    }
                    self.emit_expr(arg);
                }
                self.buf.push(')');
            }
            Expr::MethodCall(obj, method, args) => {
                self.emit_expr(obj);
                self.buf.push(':');
                self.buf.push_str(method);
                self.buf.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.buf.push_str(", ");
                    }
                    self.emit_expr(arg);
                }
                self.buf.push(')');
            }
            Expr::Table(entries) => {
                if entries.is_empty() {
                    self.buf.push_str("{}");
                    return;
                }
                // Single simple entry → inline (skip comments for inline check)
                let non_comment: Vec<&TableEntry> = entries
                    .iter()
                    .filter(|e| {
                        !matches!(
                            e,
                            TableEntry::Comment(_)
                                | TableEntry::SegmentStart(_)
                                | TableEntry::SegmentEnd(_)
                        )
                    })
                    .collect();
                if non_comment.len() == 1 && is_simple_entry(non_comment[0]) && entries.len() == 1 {
                    self.buf.push_str("{ ");
                    self.emit_table_entry(&entries[0]);
                    self.buf.push_str(" }");
                    return;
                }
                // Multi-line table
                self.buf.push_str("{\n");
                self.indent += 1;
                for (i, entry) in entries.iter().enumerate() {
                    if is_segment_entry(entry) {
                        self.emit_table_entry(entry);
                        continue;
                    }
                    self.write_indent();
                    self.emit_table_entry(entry);
                    // No comma after comment entries
                    if !matches!(
                        entry,
                        TableEntry::Comment(_)
                            | TableEntry::SegmentStart(_)
                            | TableEntry::SegmentEnd(_)
                    ) {
                        // Add comma if the next non-comment entry exists
                        let has_next_value = entries[i + 1..].iter().any(|e| {
                            !matches!(
                                e,
                                TableEntry::Comment(_)
                                    | TableEntry::SegmentStart(_)
                                    | TableEntry::SegmentEnd(_)
                            )
                        });
                        if has_next_value {
                            self.buf.push(',');
                        }
                    }
                    self.buf.push('\n');
                }
                self.indent -= 1;
                self.write_indent();
                self.buf.push('}');
            }
            Expr::Function { params, body } => {
                self.buf.push_str("function(");
                self.buf.push_str(&params.join(", "));
                self.buf.push_str(")\n");
                self.indent += 1;
                for s in body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.write_indent();
                self.buf.push_str("end");
            }
            Expr::Raw(s) => self.buf.push_str(s),
            Expr::TableCall(func, entries) => {
                self.emit_expr(func);
                if entries.is_empty() {
                    self.buf.push_str(" {}");
                    return;
                }
                self.buf.push_str(" {\n");
                self.indent += 1;
                for (i, entry) in entries.iter().enumerate() {
                    if is_segment_entry(entry) {
                        self.emit_table_entry(entry);
                        continue;
                    }
                    self.write_indent();
                    self.emit_table_entry(entry);
                    if !matches!(
                        entry,
                        TableEntry::Comment(_)
                            | TableEntry::SegmentStart(_)
                            | TableEntry::SegmentEnd(_)
                    ) {
                        let has_next_value = entries[i + 1..].iter().any(|e| {
                            !matches!(
                                e,
                                TableEntry::Comment(_)
                                    | TableEntry::SegmentStart(_)
                                    | TableEntry::SegmentEnd(_)
                            )
                        });
                        if has_next_value {
                            self.buf.push(',');
                        }
                    }
                    self.buf.push('\n');
                }
                self.indent -= 1;
                self.write_indent();
                self.buf.push('}');
            }
        }
    }

    fn emit_table_entry(&mut self, entry: &TableEntry) {
        match entry {
            TableEntry::KeyValue(key, val) => {
                // Use ['key'] syntax if key has special chars, otherwise plain key
                if needs_bracket_key(key) {
                    self.buf.push_str("['");
                    self.buf.push_str(&escape_lua_string(key));
                    self.buf.push_str("'] = ");
                } else {
                    self.buf.push_str(key);
                    self.buf.push_str(" = ");
                }
                self.emit_expr(val);
            }
            TableEntry::IndexValue(idx, val) => {
                self.buf.push('[');
                self.emit_expr(idx);
                self.buf.push_str("] = ");
                self.emit_expr(val);
            }
            TableEntry::Value(val) => {
                self.emit_expr(val);
            }
            TableEntry::Comment(text) => {
                self.buf.push_str("-- ");
                self.buf.push_str(text);
            }
            TableEntry::SegmentStart(id) => self.push_segment_start(id),
            TableEntry::SegmentEnd(id) => self.push_segment_end(id),
        }
    }

    fn current_line_col(&self) -> (usize, usize) {
        let mut line = 1usize;
        let mut col = 1usize;
        for ch in self.buf.chars() {
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += ch.len_utf16();
            }
        }
        (line, col)
    }

    fn push_segment_start(&mut self, id: &str) {
        let (line, col) = self.current_line_col();
        self.segment_stack.push((id.to_string(), line, col));
    }

    fn push_segment_end(&mut self, id: &str) {
        let (end_line, end_col) = self.current_line_col();
        if let Some(idx) = self
            .segment_stack
            .iter()
            .rposition(|(start_id, _, _)| start_id == id)
        {
            let (_, start_line, start_col) = self.segment_stack.remove(idx);
            let (segment_type, name) = classify_segment(id);
            self.segments.push(LuaSegment {
                id: id.to_string(),
                segment_type,
                name,
                start_line,
                start_column: start_col,
                end_line,
                end_column: end_col,
            });
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.buf.push_str(&self.indent_str);
        }
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Operator display
// ---------------------------------------------------------------------------

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Pow => "^",
            BinOp::Concat => "..",
            BinOp::Eq => "==",
            BinOp::Neq => "~=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }

    fn precedence(self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 3,
            BinOp::Concat => 4,
            BinOp::Add | BinOp::Sub => 5,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 6,
            BinOp::Pow => 8,
        }
    }
}

impl UnaryOp {
    pub fn as_str(self) -> &'static str {
        match self {
            UnaryOp::Not => "not",
            UnaryOp::Neg => "-",
            UnaryOp::Len => "#",
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn expr_needs_parens(expr: &Expr, parent_op: BinOp, _is_left: bool) -> bool {
    match expr {
        Expr::FieldBinding(expr, _) | Expr::Segment(expr, _) => expr_needs_parens(expr, parent_op, _is_left),
        Expr::BinOp(_, child_op, _) => child_op.precedence() < parent_op.precedence(),
        _ => false,
    }
}

fn unwrapped_expr(expr: &Expr) -> &Expr {
    match expr {
        Expr::FieldBinding(inner, _) | Expr::Segment(inner, _) => unwrapped_expr(inner),
        _ => expr,
    }
}

fn escape_lua_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\0', "\\0")
}

fn find_safe_bracket_level(s: &str) -> usize {
    let mut level = 0;
    loop {
        let close = format!("]{}]", "=".repeat(level));
        if !s.contains(&close) {
            return level;
        }
        level += 1;
    }
}

fn needs_bracket_key(key: &str) -> bool {
    if key.is_empty() {
        return true;
    }
    let first = key.chars().next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return true;
    }
    !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_simple_entry(entry: &TableEntry) -> bool {
    fn simple(expr: &Expr) -> bool {
        match expr {
            Expr::FieldBinding(inner, _) | Expr::Segment(inner, _) => simple(inner),
            Expr::Int(_) | Expr::Number(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Ident(_) | Expr::Nil => true,
            _ => false,
        }
    }
    match entry {
        TableEntry::KeyValue(_, val) | TableEntry::Value(val) => simple(val),
        TableEntry::IndexValue(_, _)
        | TableEntry::Comment(_)
        | TableEntry::SegmentStart(_)
        | TableEntry::SegmentEnd(_) => false,
    }
}

fn is_segment_entry(entry: &TableEntry) -> bool {
    matches!(
        entry,
        TableEntry::SegmentStart(_) | TableEntry::SegmentEnd(_)
    )
}

fn classify_segment(id: &str) -> (String, String) {
    if let Some(rule_id) = id.strip_prefix("rule:") {
        ("rule".to_string(), rule_id.to_string())
    } else if let Some(rule_id) = id.strip_prefix("trigger:") {
        ("trigger".to_string(), rule_id.to_string())
    } else if let Some(condition_key) = id.strip_prefix("condition:") {
        ("condition".to_string(), condition_key.to_string())
    } else if let Some(effect_key) = id.strip_prefix("effect:") {
        ("effect".to_string(), effect_key.to_string())
    } else {
        ("section".to_string(), id.to_string())
    }
}

/// Final-pass Lua formatting for generated text.
///
/// This is intentionally conservative: it normalizes line endings/trailing
/// whitespace and applies indentation heuristics around common Lua block
/// keywords (`if/elseif/else/do/function/repeat/end/until`) and table braces.
///
/// The AST emitter already produces good formatting; this helper mainly covers
/// raw/manual Lua snippets that bypass structured emission.
pub fn format_lua_source(source: &str) -> String {
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(normalized.len() + 32);

    let mut indent: usize = 0;
    let mut last_nonblank_trimmed: Option<String> = None;
    for raw_line in normalized.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim_start();

        if trimmed.is_empty() {
            if matches!(
                last_nonblank_trimmed.as_deref(),
                Some(prev) if prev.starts_with("return {")
            ) {
                continue;
            }
            out.push('\n');
            continue;
        }

        let dedent_before = should_dedent_before(trimmed);
        if dedent_before {
            indent = indent.saturating_sub(1);
        }

        for _ in 0..indent {
            out.push_str("    ");
        }
        out.push_str(trimmed);
        out.push('\n');
        last_nonblank_trimmed = Some(trimmed.to_string());

        if should_indent_after(trimmed) {
            indent += 1;
        }
    }

    out
}

fn should_dedent_before(line: &str) -> bool {
    line == "end"
        || line.starts_with("end ")
        || line.starts_with("end,")
        || line == "else"
        || line.starts_with("elseif ")
        || line.starts_with("until ")
        || line == "}"
        || line.starts_with("},")
        || line.starts_with("})")
        || line.starts_with("}):")
}

fn should_indent_after(line: &str) -> bool {
    if line == "else" || line.starts_with("elseif ") {
        return true;
    }

    // One-line blocks should not affect indentation depth.
    if line.contains(" then ") && line.contains(" end") {
        return false;
    }

    line == "repeat"
        || line.ends_with(" then")
        || line.ends_with(" do")
        || line.ends_with("{")
        || line.starts_with("function ")
        || line.contains(" = function(")
}

// ---------------------------------------------------------------------------
// Display for Expr (convenience, uses Emitter)
// ---------------------------------------------------------------------------

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = Emitter::new().emit_expr_to_string(self);
        f.write_str(&s)
    }
}

impl fmt::Display for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = Emitter::new().emit_chunk(self);
        f.write_str(&s)
    }
}
