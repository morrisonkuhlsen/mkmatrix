//! SQL(query, table1, [table2], …): a SQL `SELECT` over ranges. Each range after the query is a
//! table, `t1`, `t2`…, whose first row names its columns; the result spills with a header row.
//!
//! The dialect is a small, safe subset: `SELECT [DISTINCT] … FROM t1 [AS a] [[LEFT] JOIN t2 ON …]
//! [WHERE …] [GROUP BY …] [HAVING …] [ORDER BY … [ASC|DESC]] [LIMIT n [OFFSET m]]`, with
//! arithmetic, `||`, comparisons, `AND`/`OR`/`NOT`, `LIKE`, `IN`, `BETWEEN`, `IS [NOT] NULL`,
//! `CASE WHEN`, the aggregates COUNT/SUM/AVG/MIN/MAX and a handful of scalar functions. As in the
//! rest of the spreadsheet, text compares without regard to case and an empty cell is `NULL`.
//!
//! Input is hostile: the query is capped in length, nesting is bounded, and a join or result past
//! the cell budget is `#NUM!`. A query that doesn't parse or names an unknown column is `#VALUE!`.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use mkmatrix_core::{Array, CellError, Value, sort_compare};

use crate::util::{MAX_CELLS, R, arg, array_val, as_array, scalar};
use crate::{Arg, Ctx, FnSpec, VAR};

/// The longest query read.
const MAX_QUERY: usize = 20_000;
/// The deepest expression nesting parsed.
const MAX_DEPTH: usize = 64;
/// The most row pairs a join looks at.
const MAX_JOIN_PAIRS: usize = 5_000_000;

pub(crate) fn specs() -> Vec<FnSpec> {
    vec![f!(
        "SQL",
        2,
        VAR,
        Lookup,
        &[true, false],
        "SQL(query, table1, [table2], ...)",
        "Runs a SQL SELECT over ranges (t1, t2…) whose first row names the columns; the result spills with a header row.",
        sql
    )]
}

fn sql(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let query = scalar(arg(args, 0)?).to_text()?;
    if query.len() > MAX_QUERY {
        return Err(CellError::Value);
    }
    let mut tables = HashMap::new();
    for (i, a) in args.iter().enumerate().skip(1) {
        if let Value::Error(e) = &a.value {
            return Err(*e);
        }
        tables.insert(format!("t{i}"), Table::from_array(&as_array(&a.value)));
    }
    let stmt = Parser::new(&query)?.statement()?;
    let (header, rows) = run(&stmt, &tables)?;
    let w = header.len();
    let h = rows.len() + 1;
    if h.saturating_mul(w) > MAX_CELLS {
        return Err(CellError::Num);
    }
    let mut data: Vec<Value> = header.into_iter().map(Value::text).collect();
    for r in rows {
        data.extend(r);
    }
    array_val(h, w, data)
}

// ---------------------------------------------------------------------------------------------
// Tables

struct Table {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

impl Table {
    /// The first row names the columns (an empty heading is `c1`, `c2`…); the rest are the rows.
    fn from_array(a: &Array) -> Table {
        let columns = (0..a.cols)
            .map(|c| match a.get(0, c) {
                Some(Value::Empty) | None => format!("c{}", c + 1),
                Some(v) => v.to_text().unwrap_or_else(|_| format!("c{}", c + 1)),
            })
            .collect();
        let rows = (1..a.rows).map(|r| (0..a.cols).map(|c| a.get(r, c).cloned().unwrap_or(Value::Empty)).collect()).collect();
        Table { columns, rows }
    }
}

// ---------------------------------------------------------------------------------------------
// Lexer

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    /// A name; `quoted` when written `"…"`, `[…]` or `` `…` `` (never a keyword then).
    Ident(String, bool),
    Str(String),
    Num(f64),
    Sym(&'static str),
}

fn lex(src: &str) -> R<Vec<(Tok, usize, usize)>> {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let at = |i: usize| chars.get(i).map(|(_, c)| *c);
    let pos = |i: usize| chars.get(i).map_or(src.len(), |(p, _)| *p);
    while let Some(c) = at(i) {
        let start = pos(i);
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // Comments: `-- …` to the end of the line.
        if c == '-' && at(i + 1) == Some('-') {
            while at(i).is_some_and(|c| c != '\n') {
                i += 1;
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut s = String::new();
            while let Some(c) = at(i).filter(|c| c.is_alphanumeric() || *c == '_') {
                s.push(c);
                i += 1;
            }
            out.push((Tok::Ident(s, false), start, pos(i)));
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && at(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let mut s = String::new();
            while let Some(c) = at(i).filter(|c| c.is_ascii_digit() || *c == '.') {
                s.push(c);
                i += 1;
            }
            if matches!(at(i), Some('e' | 'E')) && at(i + 1).is_some_and(|d| d.is_ascii_digit() || d == '-' || d == '+') {
                s.push('e');
                i += 1;
                while let Some(c) = at(i).filter(|c| c.is_ascii_digit() || *c == '-' || *c == '+') {
                    s.push(c);
                    i += 1;
                }
            }
            let n: f64 = s.parse().map_err(|_| CellError::Value)?;
            out.push((Tok::Num(n), start, pos(i)));
            continue;
        }
        // Quoted text ('…') and names ("…", […], `…`); a doubled quote inside is one quote.
        if let Some(close) = match c {
            '\'' => Some('\''),
            '"' => Some('"'),
            '`' => Some('`'),
            '[' => Some(']'),
            _ => None,
        } {
            i += 1;
            let mut s = String::new();
            loop {
                match at(i) {
                    None => return Err(CellError::Value),
                    Some(x) if x == close => {
                        if close != ']' && at(i + 1) == Some(close) {
                            s.push(close);
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    }
                    Some(x) => {
                        s.push(x);
                        i += 1;
                    }
                }
            }
            let tok = if c == '\'' { Tok::Str(s) } else { Tok::Ident(s, true) };
            out.push((tok, start, pos(i)));
            continue;
        }
        let two: String = [c, at(i + 1).unwrap_or(' ')].iter().collect();
        let sym2 = match two.as_str() {
            "<=" => Some("<="),
            ">=" => Some(">="),
            "<>" => Some("<>"),
            "!=" => Some("<>"),
            "||" => Some("||"),
            "==" => Some("="),
            _ => None,
        };
        if let Some(s) = sym2 {
            i += 2;
            out.push((Tok::Sym(s), start, pos(i)));
            continue;
        }
        let sym = match c {
            ',' => ",",
            '(' => "(",
            ')' => ")",
            '.' => ".",
            '*' => "*",
            '+' => "+",
            '-' => "-",
            '/' => "/",
            '%' => "%",
            '=' => "=",
            '<' => "<",
            '>' => ">",
            ';' => ";",
            _ => return Err(CellError::Value),
        };
        i += 1;
        out.push((Tok::Sym(sym), start, pos(i)));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Syntax

#[derive(Clone, Copy, Debug, PartialEq)]
enum Bin {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Debug)]
enum Expr {
    Col(Option<String>, String),
    Lit(Value),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(Bin, Box<Expr>, Box<Expr>),
    /// A function call; `star` for `COUNT(*)`.
    Call {
        name: String,
        args: Vec<Expr>,
        distinct: bool,
        star: bool,
    },
    In(Box<Expr>, Vec<Expr>, bool),
    Between(Box<Expr>, Box<Expr>, Box<Expr>, bool),
    Like(Box<Expr>, Box<Expr>, bool),
    IsNull(Box<Expr>, bool),
    /// `CASE [subject] WHEN … THEN … [ELSE …] END`.
    Case(Option<Box<Expr>>, Vec<(Expr, Expr)>, Option<Box<Expr>>),
}

const AGGREGATES: [&str; 5] = ["COUNT", "SUM", "AVG", "MIN", "MAX"];

impl Expr {
    fn has_aggregate(&self) -> bool {
        match self {
            Expr::Call { name, args, .. } => AGGREGATES.contains(&name.as_str()) || args.iter().any(Expr::has_aggregate),
            Expr::Neg(e) | Expr::Not(e) | Expr::IsNull(e, _) => e.has_aggregate(),
            Expr::Bin(_, a, b) | Expr::Like(a, b, _) => a.has_aggregate() || b.has_aggregate(),
            Expr::In(e, l, _) => e.has_aggregate() || l.iter().any(Expr::has_aggregate),
            Expr::Between(a, b, c, _) => a.has_aggregate() || b.has_aggregate() || c.has_aggregate(),
            Expr::Case(s, arms, e) => {
                s.as_ref().is_some_and(|s| s.has_aggregate())
                    || arms.iter().any(|(w, t)| w.has_aggregate() || t.has_aggregate())
                    || e.as_ref().is_some_and(|e| e.has_aggregate())
            }
            Expr::Col(..) | Expr::Lit(_) => false,
        }
    }
}

enum Item {
    /// `*` or `t.*`.
    Star(Option<String>),
    Expr {
        expr: Expr,
        alias: Option<String>,
        text: String,
    },
}

struct Join {
    left: bool,
    table: String,
    alias: String,
    on: Option<Expr>,
}

struct Select {
    distinct: bool,
    items: Vec<Item>,
    from: (String, String),
    joins: Vec<Join>,
    filter: Option<Expr>,
    group: Vec<Expr>,
    having: Option<Expr>,
    order: Vec<(Expr, bool)>,
    limit: Option<usize>,
    offset: usize,
}

const KEYWORDS: [&str; 30] = [
    "SELECT", "DISTINCT", "FROM", "WHERE", "GROUP", "BY", "HAVING", "ORDER", "ASC", "DESC", "LIMIT", "OFFSET", "AS", "AND", "OR", "NOT", "IN",
    "BETWEEN", "LIKE", "IS", "NULL", "JOIN", "LEFT", "INNER", "OUTER", "ON", "CASE", "WHEN", "THEN", "END",
];

struct Parser<'a> {
    src: &'a str,
    toks: Vec<(Tok, usize, usize)>,
    i: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> R<Parser<'a>> {
        Ok(Parser { src, toks: lex(src)?, i: 0, depth: 0 })
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i).map(|(t, ..)| t)
    }

    /// Whether the next token is the keyword `kw` (an unquoted name, any case).
    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s, false)) if s.eq_ignore_ascii_case(kw))
    }

    fn kw(&mut self, kw: &str) -> bool {
        let yes = self.is_kw(kw);
        if yes {
            self.i += 1;
        }
        yes
    }

    fn expect_kw(&mut self, kw: &str) -> R<()> {
        if self.kw(kw) { Ok(()) } else { Err(CellError::Value) }
    }

    fn sym(&mut self, s: &str) -> bool {
        let yes = matches!(self.peek(), Some(Tok::Sym(x)) if *x == s);
        if yes {
            self.i += 1;
        }
        yes
    }

    fn expect_sym(&mut self, s: &str) -> R<()> {
        if self.sym(s) { Ok(()) } else { Err(CellError::Value) }
    }

    /// A name: quoted, or unquoted and not a keyword.
    fn name(&mut self) -> R<String> {
        match self.peek().cloned() {
            Some(Tok::Ident(s, quoted)) if quoted || !KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(&s)) => {
                self.i += 1;
                Ok(s)
            }
            _ => Err(CellError::Value),
        }
    }

    fn statement(mut self) -> R<Select> {
        self.expect_kw("SELECT")?;
        let distinct = self.kw("DISTINCT");
        let mut items = vec![self.item()?];
        while self.sym(",") {
            items.push(self.item()?);
        }
        self.expect_kw("FROM")?;
        let from = self.table_ref()?;
        let mut joins = Vec::new();
        loop {
            let left = self.kw("LEFT");
            if left {
                self.kw("OUTER");
            } else {
                self.kw("INNER");
            }
            if !self.kw("JOIN") {
                if left {
                    return Err(CellError::Value);
                }
                // `FROM t1, t2` is a cross join.
                if self.sym(",") {
                    let (table, alias) = self.table_ref()?;
                    joins.push(Join { left: false, table, alias, on: None });
                    continue;
                }
                break;
            }
            let (table, alias) = self.table_ref()?;
            let on = if self.kw("ON") { Some(self.expr()?) } else { None };
            joins.push(Join { left, table, alias, on });
        }
        let filter = if self.kw("WHERE") { Some(self.expr()?) } else { None };
        let mut group = Vec::new();
        if self.kw("GROUP") {
            self.expect_kw("BY")?;
            group.push(self.expr()?);
            while self.sym(",") {
                group.push(self.expr()?);
            }
        }
        let having = if self.kw("HAVING") { Some(self.expr()?) } else { None };
        let mut order = Vec::new();
        if self.kw("ORDER") {
            self.expect_kw("BY")?;
            loop {
                let e = self.expr()?;
                let desc = if self.kw("DESC") {
                    true
                } else {
                    self.kw("ASC");
                    false
                };
                order.push((e, desc));
                if !self.sym(",") {
                    break;
                }
            }
        }
        let mut limit = None;
        let mut offset = 0;
        if self.kw("LIMIT") {
            limit = Some(self.count()?);
            if self.kw("OFFSET") {
                offset = self.count()?;
            }
        }
        self.sym(";");
        if self.i != self.toks.len() {
            return Err(CellError::Value);
        }
        Ok(Select { distinct, items, from, joins, filter, group, having, order, limit, offset })
    }

    fn count(&mut self) -> R<usize> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) if n >= 0.0 && n.fract() == 0.0 && n < 1e15 => {
                self.i += 1;
                Ok(n as usize)
            }
            _ => Err(CellError::Value),
        }
    }

    fn table_ref(&mut self) -> R<(String, String)> {
        let table = self.name()?.to_lowercase();
        let alias = if self.kw("AS") {
            self.name()?.to_lowercase()
        } else {
            match self.peek() {
                Some(Tok::Ident(s, q)) if *q || !KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(s)) => self.name()?.to_lowercase(),
                _ => table.clone(),
            }
        };
        Ok((table, alias))
    }

    fn item(&mut self) -> R<Item> {
        if self.sym("*") {
            return Ok(Item::Star(None));
        }
        // `t.*`
        if let (Some(Tok::Ident(t, _)), Some((Tok::Sym("."), ..)), Some((Tok::Sym("*"), ..))) =
            (self.peek().cloned(), self.toks.get(self.i + 1), self.toks.get(self.i + 2))
        {
            self.i += 3;
            return Ok(Item::Star(Some(t.to_lowercase())));
        }
        let start = self.toks.get(self.i).map_or(0, |(_, s, _)| *s);
        let expr = self.expr()?;
        let end = self.toks.get(self.i.wrapping_sub(1)).map_or(start, |(_, _, e)| *e);
        let text = self.src.get(start..end).unwrap_or("").trim().to_string();
        // `AS name`, or a bare name straight after the expression.
        let alias = if self.kw("AS") || matches!(self.peek(), Some(Tok::Ident(s, q)) if *q || !KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(s))) {
            Some(self.name()?)
        } else {
            None
        };
        Ok(Item::Expr { expr, alias, text })
    }

    fn expr(&mut self) -> R<Expr> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(CellError::Value);
        }
        let r = self.or();
        self.depth -= 1;
        r
    }

    fn or(&mut self) -> R<Expr> {
        let mut e = self.and()?;
        while self.kw("OR") {
            e = Expr::Bin(Bin::Or, Box::new(e), Box::new(self.and()?));
        }
        Ok(e)
    }

    fn and(&mut self) -> R<Expr> {
        let mut e = self.not()?;
        while self.kw("AND") {
            e = Expr::Bin(Bin::And, Box::new(e), Box::new(self.not()?));
        }
        Ok(e)
    }

    fn not(&mut self) -> R<Expr> {
        if self.kw("NOT") {
            self.depth += 1;
            if self.depth > MAX_DEPTH {
                return Err(CellError::Value);
            }
            let e = self.not();
            self.depth -= 1;
            return Ok(Expr::Not(Box::new(e?)));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> R<Expr> {
        let e = self.additive()?;
        if self.kw("IS") {
            let not = self.kw("NOT");
            self.expect_kw("NULL")?;
            return Ok(Expr::IsNull(Box::new(e), not));
        }
        let not = self.kw("NOT");
        if self.kw("IN") {
            self.expect_sym("(")?;
            let mut list = vec![self.expr()?];
            while self.sym(",") {
                list.push(self.expr()?);
            }
            self.expect_sym(")")?;
            return Ok(Expr::In(Box::new(e), list, not));
        }
        if self.kw("BETWEEN") {
            let lo = self.additive()?;
            self.expect_kw("AND")?;
            let hi = self.additive()?;
            return Ok(Expr::Between(Box::new(e), Box::new(lo), Box::new(hi), not));
        }
        if self.kw("LIKE") {
            let p = self.additive()?;
            return Ok(Expr::Like(Box::new(e), Box::new(p), not));
        }
        if not {
            return Err(CellError::Value);
        }
        let op = match self.peek() {
            Some(Tok::Sym("=")) => Bin::Eq,
            Some(Tok::Sym("<>")) => Bin::Ne,
            Some(Tok::Sym("<")) => Bin::Lt,
            Some(Tok::Sym("<=")) => Bin::Le,
            Some(Tok::Sym(">")) => Bin::Gt,
            Some(Tok::Sym(">=")) => Bin::Ge,
            _ => return Ok(e),
        };
        self.i += 1;
        Ok(Expr::Bin(op, Box::new(e), Box::new(self.additive()?)))
    }

    fn additive(&mut self) -> R<Expr> {
        let mut e = self.term()?;
        loop {
            let op = if self.sym("+") {
                Bin::Add
            } else if self.sym("-") {
                Bin::Sub
            } else if self.sym("||") {
                Bin::Concat
            } else {
                return Ok(e);
            };
            e = Expr::Bin(op, Box::new(e), Box::new(self.term()?));
        }
    }

    fn term(&mut self) -> R<Expr> {
        let mut e = self.unary()?;
        loop {
            let op = if self.sym("*") {
                Bin::Mul
            } else if self.sym("/") {
                Bin::Div
            } else if self.sym("%") {
                Bin::Mod
            } else {
                return Ok(e);
            };
            e = Expr::Bin(op, Box::new(e), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> R<Expr> {
        if self.sym("-") {
            self.depth += 1;
            if self.depth > MAX_DEPTH {
                return Err(CellError::Value);
            }
            let e = self.unary();
            self.depth -= 1;
            return Ok(Expr::Neg(Box::new(e?)));
        }
        if self.sym("+") {
            return self.unary();
        }
        self.primary()
    }

    fn primary(&mut self) -> R<Expr> {
        let Some(tok) = self.peek().cloned() else { return Err(CellError::Value) };
        match tok {
            Tok::Num(n) => {
                self.i += 1;
                Ok(Expr::Lit(Value::number(n)))
            }
            Tok::Str(s) => {
                self.i += 1;
                Ok(Expr::Lit(Value::text(s)))
            }
            Tok::Sym("(") => {
                self.i += 1;
                let e = self.expr()?;
                self.expect_sym(")")?;
                Ok(e)
            }
            Tok::Ident(s, false) if s.eq_ignore_ascii_case("NULL") => {
                self.i += 1;
                Ok(Expr::Lit(Value::Empty))
            }
            Tok::Ident(s, false) if s.eq_ignore_ascii_case("TRUE") || s.eq_ignore_ascii_case("FALSE") => {
                self.i += 1;
                Ok(Expr::Lit(Value::Bool(s.eq_ignore_ascii_case("TRUE"))))
            }
            Tok::Ident(s, false) if s.eq_ignore_ascii_case("CASE") => {
                self.i += 1;
                self.case()
            }
            Tok::Ident(s, quoted) => {
                // A function call.
                if !quoted && matches!(self.toks.get(self.i + 1), Some((Tok::Sym("("), ..))) {
                    self.i += 2;
                    let name = s.to_uppercase();
                    if self.sym("*") {
                        self.expect_sym(")")?;
                        return Ok(Expr::Call { name, args: Vec::new(), distinct: false, star: true });
                    }
                    let distinct = self.kw("DISTINCT");
                    let mut args = Vec::new();
                    if !self.sym(")") {
                        args.push(self.expr()?);
                        while self.sym(",") {
                            args.push(self.expr()?);
                        }
                        self.expect_sym(")")?;
                    }
                    return Ok(Expr::Call { name, args, distinct, star: false });
                }
                let first = self.name()?;
                if self.sym(".") {
                    let col = self.name()?;
                    return Ok(Expr::Col(Some(first.to_lowercase()), col));
                }
                Ok(Expr::Col(None, first))
            }
            Tok::Sym(_) => Err(CellError::Value),
        }
    }

    fn case(&mut self) -> R<Expr> {
        let subject = if self.is_kw("WHEN") { None } else { Some(Box::new(self.expr()?)) };
        let mut arms = Vec::new();
        while self.kw("WHEN") {
            let w = self.expr()?;
            self.expect_kw("THEN")?;
            arms.push((w, self.expr()?));
        }
        if arms.is_empty() {
            return Err(CellError::Value);
        }
        let otherwise = if self.kw("ELSE") { Some(Box::new(self.expr()?)) } else { None };
        self.expect_kw("END")?;
        Ok(Expr::Case(subject, arms, otherwise))
    }
}

// ---------------------------------------------------------------------------------------------
// Execution

/// The joined rows' columns: (table alias, column name) in order.
struct Schema {
    cols: Vec<(String, String)>,
}

impl Schema {
    /// The column `name` (of `table` when given), any case. Unknown or ambiguous is `#VALUE!`.
    fn find(&self, table: Option<&str>, name: &str) -> R<usize> {
        let mut hits = self.cols.iter().enumerate().filter(|(_, (t, c))| table.is_none_or(|x| x == t) && c.eq_ignore_ascii_case(name));
        let first = hits.next().map(|(i, _)| i).ok_or(CellError::Value)?;
        // The same name in two tables needs its table named.
        if table.is_none() && hits.next().is_some() {
            return Err(CellError::Value);
        }
        Ok(first)
    }
}

/// Where an expression is evaluated: one row, or a group of rows (for aggregates).
struct Scope<'a> {
    schema: &'a Schema,
    row: &'a [Value],
    group: Option<&'a [&'a Vec<Value>]>,
}

fn run(stmt: &Select, tables: &HashMap<String, Table>) -> R<(Vec<String>, Vec<Vec<Value>>)> {
    // FROM and the joins, nested-loop.
    let base = tables.get(&stmt.from.0).ok_or(CellError::Value)?;
    let mut schema = Schema { cols: base.columns.iter().map(|c| (stmt.from.1.clone(), c.clone())).collect() };
    let mut rows: Vec<Vec<Value>> = base.rows.clone();
    for j in &stmt.joins {
        let right = tables.get(&j.table).ok_or(CellError::Value)?;
        let mut joined_schema = Schema { cols: schema.cols.clone() };
        joined_schema.cols.extend(right.columns.iter().map(|c| (j.alias.clone(), c.clone())));
        if rows.len().saturating_mul(right.rows.len().max(1)) > MAX_JOIN_PAIRS {
            return Err(CellError::Num);
        }
        let mut out = Vec::new();
        for l in &rows {
            let mut matched = false;
            for r in &right.rows {
                let mut row = l.clone();
                row.extend(r.iter().cloned());
                let keep = match &j.on {
                    Some(on) => truthy(&eval(on, &Scope { schema: &joined_schema, row: &row, group: None })?),
                    None => true,
                };
                if keep {
                    matched = true;
                    out.push(row);
                    if out.len().saturating_mul(joined_schema.cols.len()) > MAX_CELLS {
                        return Err(CellError::Num);
                    }
                }
            }
            if j.left && !matched {
                let mut row = l.clone();
                row.extend(std::iter::repeat_n(Value::Empty, right.columns.len()));
                out.push(row);
            }
        }
        rows = out;
        schema = joined_schema;
    }

    // WHERE.
    if let Some(f) = &stmt.filter {
        let mut kept = Vec::with_capacity(rows.len());
        for r in rows {
            if truthy(&eval(f, &Scope { schema: &schema, row: &r, group: None })?) {
                kept.push(r);
            }
        }
        rows = kept;
    }

    // The output columns.
    let mut outputs: Vec<(String, Expr)> = Vec::new();
    for item in &stmt.items {
        match item {
            Item::Star(t) => {
                let mut any = false;
                for (i, (table, name)) in schema.cols.iter().enumerate() {
                    if t.as_ref().is_none_or(|x| x == table) {
                        any = true;
                        outputs.push((name.clone(), Expr::Col(Some(table.clone()), format!("\u{0}{i}"))));
                    }
                }
                if !any {
                    return Err(CellError::Value);
                }
            }
            Item::Expr { expr, alias, text } => {
                // A bare column is headed by its name as the table writes it.
                let heading = match (alias, expr) {
                    (Some(a), _) => a.clone(),
                    (None, Expr::Col(t, name)) => {
                        schema.find(t.as_deref(), name).ok().and_then(|i| schema.cols.get(i)).map_or_else(|| text.clone(), |(_, c)| c.clone())
                    }
                    (None, _) => text.clone(),
                };
                outputs.push((heading, expr.clone()));
            }
        }
    }
    let header: Vec<String> = outputs.iter().map(|(h, _)| h.clone()).collect();

    let grouped = !stmt.group.is_empty() || stmt.having.as_ref().is_some_and(Expr::has_aggregate) || outputs.iter().any(|(_, e)| e.has_aggregate());
    // Each output row with the scope its ORDER BY keys are read in.
    let mut result: Vec<(Vec<Value>, Vec<Value>)> = Vec::new();
    let order_keys = |scope: &Scope, out: &[Value]| -> R<Vec<Value>> { stmt.order.iter().map(|(e, _)| order_key(e, scope, out, &header)).collect() };
    if grouped {
        let mut groups: Vec<(Vec<Value>, Vec<&Vec<Value>>)> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for r in &rows {
            let scope = Scope { schema: &schema, row: r, group: None };
            let key: Vec<Value> = stmt.group.iter().map(|g| eval(g, &scope)).collect::<R<_>>()?;
            let k = key_of(&key);
            match index.get(&k) {
                Some(&i) => {
                    if let Some(g) = groups.get_mut(i) {
                        g.1.push(r);
                    }
                }
                None => {
                    index.insert(k, groups.len());
                    groups.push((key, vec![r]));
                }
            }
        }
        // Aggregates over no rows (and no GROUP BY) still give one row: COUNT(*) is 0.
        let empty_row = vec![Value::Empty; schema.cols.len()];
        if groups.is_empty() && stmt.group.is_empty() {
            groups.push((Vec::new(), Vec::new()));
        }
        for (_, members) in &groups {
            let first = members.first().map_or(empty_row.as_slice(), |r| r.as_slice());
            let scope = Scope { schema: &schema, row: first, group: Some(members) };
            if let Some(h) = &stmt.having
                && !truthy(&eval(h, &scope)?)
            {
                continue;
            }
            let out: Vec<Value> = outputs.iter().map(|(_, e)| eval(e, &scope)).collect::<R<_>>()?;
            let keys = order_keys(&scope, &out)?;
            result.push((out, keys));
        }
    } else {
        for r in &rows {
            let scope = Scope { schema: &schema, row: r, group: None };
            let out: Vec<Value> = outputs.iter().map(|(_, e)| eval(e, &scope)).collect::<R<_>>()?;
            let keys = order_keys(&scope, &out)?;
            result.push((out, keys));
            if result.len().saturating_mul(header.len()) > MAX_CELLS {
                return Err(CellError::Num);
            }
        }
    }

    if stmt.distinct {
        let mut seen = HashSet::new();
        result.retain(|(out, _)| seen.insert(key_of(out)));
    }
    if !stmt.order.is_empty() {
        // Blanks sort last whichever the direction, as in Excel's Sort.
        result.sort_by(|(_, a), (_, b)| {
            for ((x, y), (_, desc)) in a.iter().zip(b).zip(&stmt.order) {
                match (x.is_empty(), y.is_empty()) {
                    (true, true) => continue,
                    (true, false) => return Ordering::Greater,
                    (false, true) => return Ordering::Less,
                    _ => {}
                }
                let o = sort_compare(x, y);
                if o != Ordering::Equal {
                    return if *desc { o.reverse() } else { o };
                }
            }
            Ordering::Equal
        });
    }
    let rows: Vec<Vec<Value>> = result.into_iter().map(|(out, _)| out).skip(stmt.offset).take(stmt.limit.unwrap_or(usize::MAX)).collect();
    Ok((header, rows))
}

/// An ORDER BY key: an output column's position (`ORDER BY 2`) or alias, or an expression.
fn order_key(e: &Expr, scope: &Scope, out: &[Value], header: &[String]) -> R<Value> {
    match e {
        Expr::Lit(Value::Number(n)) if n.fract() == 0.0 && *n >= 1.0 => out.get(*n as usize - 1).cloned().ok_or(CellError::Value),
        Expr::Col(None, name) if scope.schema.find(None, name).is_err() => {
            header.iter().position(|h| h.eq_ignore_ascii_case(name)).and_then(|i| out.get(i)).cloned().ok_or(CellError::Value)
        }
        _ => eval(e, scope),
    }
}

/// A key that groups and dedupes values as SQL does: numbers by value, text without case.
fn key_of(vals: &[Value]) -> String {
    let mut k = String::new();
    for v in vals {
        match v {
            Value::Empty => k.push_str("\u{1}n"),
            Value::Number(n) => k.push_str(&format!("\u{1}#{n}")),
            Value::Text(t) => k.push_str(&format!("\u{1}t{}", t.to_lowercase())),
            Value::Bool(b) => k.push_str(&format!("\u{1}b{b}")),
            Value::Error(e) => k.push_str(&format!("\u{1}e{e:?}")),
            Value::Array(_) => k.push_str("\u{1}a"),
        }
    }
    k
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => *n != 0.0,
        _ => false,
    }
}

/// `Some(bool)` for a truth value, `None` for NULL (and anything else).
fn tri(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => Some(*n != 0.0),
        _ => None,
    }
}

fn num(v: &Value) -> R<Option<f64>> {
    match v {
        Value::Empty => Ok(None),
        Value::Number(n) => Ok(Some(*n)),
        Value::Bool(b) => Ok(Some(if *b { 1.0 } else { 0.0 })),
        // `inf` and `NaN` parse in Rust but aren't numbers in a sheet.
        Value::Text(t) => t.trim().parse::<f64>().ok().filter(|n| n.is_finite()).map(Some).ok_or(CellError::Value),
        Value::Error(e) => Err(*e),
        Value::Array(_) => Err(CellError::Value),
    }
}

/// SQL equality and order between two values: numbers by value, text without case; `None` when
/// either is NULL.
fn compare(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Empty, _) | (_, Value::Empty) => None,
        (Value::Number(x), Value::Number(y)) => x.partial_cmp(y),
        (Value::Text(x), Value::Text(y)) => Some(x.to_lowercase().cmp(&y.to_lowercase())),
        (Value::Number(x), Value::Text(t)) => t.trim().parse::<f64>().ok().and_then(|y| x.partial_cmp(&y)).or(Some(Ordering::Less)),
        (Value::Text(t), Value::Number(y)) => t.trim().parse::<f64>().ok().and_then(|x| x.partial_cmp(y)).or(Some(Ordering::Greater)),
        _ => Some(sort_compare(a, b)),
    }
}

fn number(n: f64) -> Value {
    if n.is_finite() { Value::number(n) } else { Value::Error(CellError::Num) }
}

fn eval(e: &Expr, s: &Scope) -> R<Value> {
    Ok(match e {
        Expr::Lit(v) => v.clone(),
        Expr::Col(table, name) => {
            // `*` expands to positional columns (`\0i`).
            let i = match name.strip_prefix('\u{0}').and_then(|i| i.parse::<usize>().ok()) {
                Some(i) => i,
                None => s.schema.find(table.as_deref(), name)?,
            };
            s.row.get(i).cloned().unwrap_or(Value::Empty)
        }
        Expr::Neg(x) => match num(&eval(x, s)?)? {
            Some(n) => number(-n),
            None => Value::Empty,
        },
        Expr::Not(x) => match tri(&eval(x, s)?) {
            Some(b) => Value::Bool(!b),
            None => Value::Empty,
        },
        Expr::Bin(op, a, b) => binary(*op, a, b, s)?,
        Expr::IsNull(x, not) => Value::Bool(eval(x, s)?.is_empty() != *not),
        Expr::In(x, list, not) => {
            let v = eval(x, s)?;
            if v.is_empty() {
                return Ok(Value::Empty);
            }
            let mut found = false;
            for item in list {
                if compare(&v, &eval(item, s)?) == Some(Ordering::Equal) {
                    found = true;
                    break;
                }
            }
            Value::Bool(found != *not)
        }
        Expr::Between(x, lo, hi, not) => {
            let v = eval(x, s)?;
            match (compare(&v, &eval(lo, s)?), compare(&v, &eval(hi, s)?)) {
                (Some(a), Some(b)) => Value::Bool((a != Ordering::Less && b != Ordering::Greater) != *not),
                _ => Value::Empty,
            }
        }
        Expr::Like(x, p, not) => {
            let v = eval(x, s)?;
            let p = eval(p, s)?;
            if v.is_empty() || p.is_empty() {
                return Ok(Value::Empty);
            }
            Value::Bool(like(&v.to_text()?, &p.to_text()?) != *not)
        }
        Expr::Case(subject, arms, otherwise) => {
            let subject = subject.as_ref().map(|x| eval(x, s)).transpose()?;
            for (w, t) in arms {
                let w = eval(w, s)?;
                let hit = match &subject {
                    Some(v) => compare(v, &w) == Some(Ordering::Equal),
                    None => truthy(&w),
                };
                if hit {
                    return eval(t, s);
                }
            }
            match otherwise {
                Some(o) => eval(o, s)?,
                None => Value::Empty,
            }
        }
        Expr::Call { name, args, distinct, star } => call(name, args, *distinct, *star, s)?,
    })
}

fn binary(op: Bin, a: &Expr, b: &Expr, s: &Scope) -> R<Value> {
    // AND / OR with three-valued logic: false AND NULL is false, true OR NULL is true.
    if matches!(op, Bin::And | Bin::Or) {
        let x = tri(&eval(a, s)?);
        let y = tri(&eval(b, s)?);
        return Ok(match (op, x, y) {
            (Bin::And, Some(false), _) | (Bin::And, _, Some(false)) => Value::Bool(false),
            (Bin::And, Some(true), Some(true)) => Value::Bool(true),
            (Bin::Or, Some(true), _) | (Bin::Or, _, Some(true)) => Value::Bool(true),
            (Bin::Or, Some(false), Some(false)) => Value::Bool(false),
            _ => Value::Empty,
        });
    }
    let x = eval(a, s)?;
    let y = eval(b, s)?;
    if let Value::Error(e) = x {
        return Ok(Value::Error(e));
    }
    if let Value::Error(e) = y {
        return Ok(Value::Error(e));
    }
    if op == Bin::Concat {
        if x.is_empty() || y.is_empty() {
            return Ok(Value::Empty);
        }
        return Ok(Value::text(format!("{}{}", x.to_text()?, y.to_text()?)));
    }
    if matches!(op, Bin::Eq | Bin::Ne | Bin::Lt | Bin::Le | Bin::Gt | Bin::Ge) {
        let Some(o) = compare(&x, &y) else { return Ok(Value::Empty) };
        return Ok(Value::Bool(match op {
            Bin::Eq => o == Ordering::Equal,
            Bin::Ne => o != Ordering::Equal,
            Bin::Lt => o == Ordering::Less,
            Bin::Le => o != Ordering::Greater,
            Bin::Gt => o == Ordering::Greater,
            _ => o != Ordering::Less,
        }));
    }
    let (Some(x), Some(y)) = (num(&x)?, num(&y)?) else { return Ok(Value::Empty) };
    Ok(match op {
        Bin::Add => number(x + y),
        Bin::Sub => number(x - y),
        Bin::Mul => number(x * y),
        Bin::Div if y == 0.0 => Value::Error(CellError::Div0),
        Bin::Div => number(x / y),
        Bin::Mod if y == 0.0 => Value::Error(CellError::Div0),
        _ => number(x % y),
    })
}

/// `LIKE`: `%` any run, `_` one character, without regard to case.
fn like(text: &str, pattern: &str) -> bool {
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    // Iterative wildcard match with backtracking to the last `%`: linear in practice, never
    // recursive, so a hostile pattern can't blow the stack.
    let (mut ti, mut pi) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        match p.get(pi) {
            Some('%') => {
                star = Some((pi, ti));
                pi += 1;
            }
            Some(c) if *c == '_' || Some(c) == t.get(ti) => {
                ti += 1;
                pi += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    pi = sp + 1;
                    ti = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    while p.get(pi) == Some(&'%') {
        pi += 1;
    }
    pi == p.len()
}

fn call(name: &str, args: &[Expr], distinct: bool, star: bool, s: &Scope) -> R<Value> {
    if AGGREGATES.contains(&name) {
        let Some(group) = s.group else { return Err(CellError::Value) };
        if star {
            return if name == "COUNT" { Ok(Value::number(group.len() as f64)) } else { Err(CellError::Value) };
        }
        let [arg] = args else { return Err(CellError::Value) };
        let mut values = Vec::with_capacity(group.len());
        for row in group {
            let v = eval(arg, &Scope { schema: s.schema, row, group: None })?;
            if !v.is_empty() {
                values.push(v);
            }
        }
        if distinct {
            let mut seen = HashSet::new();
            values.retain(|v| seen.insert(key_of(std::slice::from_ref(v))));
        }
        return Ok(match name {
            "COUNT" => Value::number(values.len() as f64),
            "SUM" | "AVG" => {
                let mut total = 0.0;
                let mut n = 0usize;
                for v in &values {
                    if let Value::Error(e) = v {
                        return Ok(Value::Error(*e));
                    }
                    // Text that isn't a number is skipped, as SUM skips it in a range.
                    if let Ok(Some(x)) = num(v) {
                        total += x;
                        n += 1;
                    }
                }
                match (name, n) {
                    (_, 0) => Value::Empty,
                    ("AVG", n) => number(total / n as f64),
                    _ => number(total),
                }
            }
            "MIN" => values.into_iter().min_by(|a, b| compare(a, b).unwrap_or(Ordering::Equal)).unwrap_or(Value::Empty),
            _ => values.into_iter().max_by(|a, b| compare(a, b).unwrap_or(Ordering::Equal)).unwrap_or(Value::Empty),
        });
    }
    let vals: Vec<Value> = args.iter().map(|a| eval(a, s)).collect::<R<_>>()?;
    let text_of = |i: usize| -> R<Option<String>> {
        match vals.get(i) {
            None | Some(Value::Empty) => Ok(None),
            Some(v) => v.to_text().map(Some),
        }
    };
    let num_of = |i: usize| -> R<Option<f64>> { vals.get(i).map_or(Ok(None), num) };
    Ok(match (name, vals.len()) {
        ("UPPER", 1) => text_of(0)?.map_or(Value::Empty, |t| Value::text(t.to_uppercase())),
        ("LOWER", 1) => text_of(0)?.map_or(Value::Empty, |t| Value::text(t.to_lowercase())),
        ("TRIM", 1) => text_of(0)?.map_or(Value::Empty, |t| Value::text(t.trim())),
        ("LENGTH" | "LEN", 1) => text_of(0)?.map_or(Value::Empty, |t| Value::number(t.chars().count() as f64)),
        ("ABS", 1) => num_of(0)?.map_or(Value::Empty, |n| number(n.abs())),
        ("ROUND", 1 | 2) => {
            let digits = num_of(1)?.unwrap_or(0.0).clamp(-15.0, 15.0).trunc() as i32;
            num_of(0)?.map_or(Value::Empty, |n| {
                let f = 10f64.powi(digits);
                number((n * f).round() / f)
            })
        }
        ("SUBSTR" | "SUBSTRING", 2 | 3) => {
            let Some(t) = text_of(0)? else { return Ok(Value::Empty) };
            let from = num_of(1)?.unwrap_or(1.0);
            let start = if from.is_finite() { from.clamp(1.0, 1e9) as usize } else { 1 }.saturating_sub(1);
            let len = num_of(2)?.map_or(usize::MAX, |n| n.clamp(0.0, 1e9) as usize);
            Value::text(t.chars().skip(start).take(len).collect::<String>())
        }
        ("REPLACE", 3) => match (text_of(0)?, text_of(1)?, text_of(2)?) {
            (Some(t), Some(from), to) if !from.is_empty() => {
                // Bounded growth: the result can't pass the length of a cell's text.
                let out = t.replace(&from, &to.unwrap_or_default());
                if out.chars().count() > 32_767 {
                    return Err(CellError::Value);
                }
                Value::text(out)
            }
            (Some(t), ..) => Value::text(t),
            _ => Value::Empty,
        },
        ("COALESCE" | "IFNULL", n) if n >= 1 => vals.iter().find(|v| !v.is_empty()).cloned().unwrap_or(Value::Empty),
        ("NULLIF", 2) => match (vals.first(), vals.get(1)) {
            (Some(a), Some(b)) if compare(a, b) == Some(Ordering::Equal) => Value::Empty,
            (Some(a), _) => a.clone(),
            _ => Value::Empty,
        },
        _ => return Err(CellError::Value),
    })
}

#[cfg(test)]
mod tests {
    use mkmatrix_core::{CellError, Value};

    use crate::util::testutil::*;

    fn sales() -> Value {
        arr(vec![
            vec![tv("Region"), tv("Product"), tv("Total")],
            vec![tv("North"), tv("Pens"), nv(10.0)],
            vec![tv("South"), tv("Pens"), nv(30.0)],
            vec![tv("North"), tv("Ink"), nv(5.0)],
            vec![tv("East"), tv("Ink"), Value::Empty],
            vec![tv("south"), tv("Paper"), nv(20.0)],
        ])
    }

    fn q(query: &str) -> Vec<Vec<Value>> {
        rows_of(&ev("SQL", vec![t(query), rf(sales())]))
    }

    #[test]
    fn select_where_order() {
        let r = q("SELECT Region, Total FROM t1 WHERE Total >= 10 ORDER BY Total DESC");
        assert_eq!(r, vec![vec![tv("Region"), tv("Total")], vec![tv("South"), nv(30.0)], vec![tv("south"), nv(20.0)], vec![tv("North"), nv(10.0)]]);
        // Every column, and no rows but the header.
        assert_eq!(q("SELECT * FROM t1 WHERE Total > 100"), vec![vec![tv("Region"), tv("Product"), tv("Total")]]);
        assert_eq!(q("select product from t1 where region = 'north' and product like 'p%'"), vec![vec![tv("Product")], vec![tv("Pens")]]);
    }

    #[test]
    fn group_by_aggregates_without_case() {
        let r = q("SELECT Region, SUM(Total) AS Sum, COUNT(*) AS n FROM t1 GROUP BY Region ORDER BY Sum DESC");
        assert_eq!(
            r,
            vec![
                vec![tv("Region"), tv("Sum"), tv("n")],
                vec![tv("South"), nv(50.0), nv(2.0)],
                vec![tv("North"), nv(15.0), nv(2.0)],
                vec![tv("East"), Value::Empty, nv(1.0)],
            ]
        );
        assert_eq!(q("SELECT COUNT(Total), AVG(Total), MAX(Region) FROM t1")[1], vec![nv(4.0), nv(16.25), tv("south")]);
        assert_eq!(
            q("SELECT Product FROM t1 GROUP BY Product HAVING COUNT(*) > 1 ORDER BY 1"),
            vec![vec![tv("Product")], vec![tv("Ink")], vec![tv("Pens")]]
        );
        assert_eq!(q("SELECT COUNT(DISTINCT Region) FROM t1")[1], vec![nv(3.0)]);
    }

    #[test]
    fn joins_distinct_limit_and_case() {
        let prices = arr(vec![vec![tv("Product"), tv("Price")], vec![tv("Pens"), nv(2.0)], vec![tv("Ink"), nv(7.0)]]);
        let r = rows_of(&ev(
            "SQL",
            vec![
                t("SELECT s.Product, p.Price FROM t1 s LEFT JOIN t2 p ON s.Product = p.Product WHERE s.Region = 'south' ORDER BY s.Product"),
                rf(sales()),
                rf(prices),
            ],
        ));
        assert_eq!(r, vec![vec![tv("Product"), tv("Price")], vec![tv("Paper"), Value::Empty], vec![tv("Pens"), nv(2.0)]]);
        assert_eq!(
            q("SELECT DISTINCT Product FROM t1 ORDER BY Product LIMIT 2 OFFSET 1"),
            vec![vec![tv("Product")], vec![tv("Paper")], vec![tv("Pens")]]
        );
        let r = q("SELECT CASE WHEN Total >= 20 THEN 'big' WHEN Total IS NULL THEN '?' ELSE 'small' END AS size FROM t1");
        assert_eq!(r.iter().skip(1).map(|r| r[0].clone()).collect::<Vec<_>>(), vec![tv("small"), tv("big"), tv("small"), tv("?"), tv("big")]);
        assert_eq!(q("SELECT UPPER(Region) || '-' || Product AS k, ROUND(Total / 3, 1) FROM t1 LIMIT 1")[1], vec![tv("NORTH-Pens"), nv(3.3)]);
    }

    #[test]
    fn bad_queries_are_value_errors() {
        for bad in [
            "",
            "DELETE FROM t1",
            "SELECT Nope FROM t1",
            "SELECT Region FROM t9",
            "SELECT Region FROM t1 WHERE",
            "SELECT 'unterminated FROM t1",
            "SELECT SUM(Total, Total) FROM t1",
            "SELECT Region FROM t1 extra junk",
        ] {
            is_err(ev("SQL", vec![t(bad), rf(sales())]), CellError::Value);
        }
        // Hostile nesting is refused, not a stack overflow.
        let deep = format!("SELECT {}1{} FROM t1", "(".repeat(5_000), ")".repeat(5_000));
        is_err(ev("SQL", vec![t(&deep), rf(sales())]), CellError::Value);
        let negs = format!("SELECT {}1 FROM t1", "-".repeat(5_000));
        is_err(ev("SQL", vec![t(&negs), rf(sales())]), CellError::Value);
        // An ambiguous column needs its table.
        is_err(ev("SQL", vec![t("SELECT Region FROM t1 a JOIN t2 b ON a.Total = b.Total"), rf(sales()), rf(sales())]), CellError::Value);
    }

    #[test]
    fn hostile_numbers_are_errors_not_panics() {
        let odd = arr(vec![vec![tv("s"), tv("n")], vec![tv("abc"), tv("NaN")], vec![tv("abc"), tv("inf")]]);
        // Text that reads as NaN or infinity isn't a number: #VALUE!, never a panic.
        is_err(ev("SQL", vec![t("SELECT SUBSTR(s, n) FROM t1"), rf(odd.clone())]), CellError::Value);
        is_err(ev("SQL", vec![t("SELECT n + 1 FROM t1"), rf(odd)]), CellError::Value);
    }

    #[test]
    fn like_wildcards() {
        assert!(super::like("Paper", "p%r"));
        assert!(super::like("Pens", "_ens"));
        assert!(!super::like("Pens", "_ns"));
        assert!(super::like("anything", "%"));
        assert!(!super::like("aaaaaaaaaaaaaaaaaaaaaaaaaaaaab", "%a%a%a%a%a%a%a%c"));
    }
}
