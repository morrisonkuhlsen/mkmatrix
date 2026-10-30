//! Cells.

use std::sync::Arc;

use mkmatrix_core::{RangeRef, Value};
use mkmatrix_formula::Expr;
use serde::{Deserialize, Serialize};

use crate::style::StyleId;

/// A parsed formula. `text` is the canonical formula text without the leading `=`.
///
/// Its tree is either its own (`expr`) or shared with the formulas filled from the same one
/// (`shared`): a column of `=B1*C1`, `=B2*C2`… keeps one tree and each cell's offset from it,
/// instead of a tree per cell. Either way [`Formula::expr`] gives this cell's tree, and equality
/// compares the text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Formula {
    pub text: String,
    #[serde(skip)]
    pub expr: Option<Arc<Expr>>,
    #[serde(skip)]
    pub shared: Option<Shared>,
    /// Legacy array formula (Ctrl+Shift+Enter) covering this range; the formula lives in the
    /// top-left cell.
    pub array: Option<RangeRef>,
    /// Read from a file that marks this formula as a dynamic-array formula (cell metadata `cm`).
    /// The writer keeps the mark, so a formula Excel evaluates as an array is never saved as a
    /// legacy formula (which Excel evaluates with implicit intersection). Formulas entered or
    /// rewritten in MK Matrix start without it, and the writer decides from their shape.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dynamic: bool,
}

/// A tree shared by formulas filled from one: the first formula's tree, and how far this cell's
/// references move from it (relative parts only, as filling moves them).
#[derive(Clone, Debug)]
pub struct Shared {
    pub tree: Arc<Expr>,
    pub rows: i64,
    pub cols: i64,
}

impl PartialEq for Formula {
    /// The same formula: the same text, array range and dynamic mark (the text is printed from
    /// the tree, so the tree adds nothing, however it is stored).
    fn eq(&self, other: &Formula) -> bool {
        self.text == other.text && self.array == other.array && self.dynamic == other.dynamic
    }
}

impl Formula {
    /// Parses `text` (with or without `=`). Unparseable formulas keep their text and evaluate to
    /// `#NAME?`.
    pub fn new(text: &str) -> Formula {
        let body = text.strip_prefix('=').unwrap_or(text);
        match mkmatrix_formula::parse(body) {
            Ok(e) => Formula::from_expr(e),
            Err(_) => Formula { text: body.to_string(), expr: None, shared: None, array: None, dynamic: false },
        }
    }
    pub fn from_expr(e: Expr) -> Formula {
        Formula { text: mkmatrix_formula::print(&e), expr: Some(Arc::new(e)), shared: None, array: None, dynamic: false }
    }
    /// `tree` (the formula filled from) with its relative references moved by `(rows, cols)`,
    /// sharing `tree` instead of keeping a tree of its own. `None` when a reference would leave
    /// the sheet: that formula has `#REF!` in place of it and gets a tree of its own.
    pub fn shared(tree: &Arc<Expr>, rows: i64, cols: i64) -> Option<Formula> {
        let mut leaves = false;
        tree.walk(&mut |e| {
            if let Expr::Ref(r) = e {
                leaves |= mkmatrix_formula::adjust::shift_ref(r, rows, cols).is_none();
            }
        });
        if leaves {
            return None;
        }
        let moved = mkmatrix_formula::adjust::shift_relative((**tree).clone(), rows, cols);
        Some(Formula {
            text: mkmatrix_formula::print(&moved),
            expr: None,
            shared: Some(Shared { tree: Arc::clone(tree), rows, cols }),
            array: None,
            dynamic: false,
        })
    }
    /// The parsed expression, re-parsing after deserialization.
    pub fn expr(&self) -> Option<Expr> {
        match (&self.expr, &self.shared) {
            (Some(e), _) => Some((**e).clone()),
            (None, Some(s)) => Some(mkmatrix_formula::adjust::shift_relative((*s.tree).clone(), s.rows, s.cols)),
            (None, None) => mkmatrix_formula::parse(&self.text).ok(),
        }
    }
    /// The parsed expression without copying it when the formula has its own (re-parses after
    /// deserialization; builds it for a shared one).
    pub fn expr_arc(&self) -> Option<Arc<Expr>> {
        match (&self.expr, &self.shared) {
            (Some(e), _) => Some(Arc::clone(e)),
            _ => self.expr().map(Arc::new),
        }
    }
    /// What to evaluate: a tree and how far its relative references move (0, 0 for a formula's
    /// own tree). A shared tree isn't copied.
    pub fn eval_tree(&self) -> Option<(Arc<Expr>, i64, i64)> {
        match (&self.expr, &self.shared) {
            (Some(e), _) => Some((Arc::clone(e), 0, 0)),
            (None, Some(s)) => Some((Arc::clone(&s.tree), s.rows, s.cols)),
            (None, None) => mkmatrix_formula::parse(&self.text).ok().map(|e| (Arc::new(e), 0, 0)),
        }
    }
}

/// A static picture stored as cell content, independent of floating drawing anchors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellPicture {
    /// Encoded PNG/JPEG bytes, base64 in JSON. Sheet entries share the payload through Arc.
    #[serde(with = "crate::b64")]
    pub data: Vec<u8>,
    pub mime: String,
    pub alt: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    /// Constant value, or the cached result of the formula.
    pub value: Value,
    pub formula: Option<Arc<Formula>>,
    pub style: StyleId,
}

impl Cell {
    pub fn value(v: Value) -> Cell {
        Cell { value: v, formula: None, style: StyleId::DEFAULT }
    }
    pub fn formula(f: Formula) -> Cell {
        Cell { value: Value::Empty, formula: Some(Arc::new(f)), style: StyleId::DEFAULT }
    }
    /// No value, no formula and the default style: storing it is pointless.
    pub fn is_blank(&self) -> bool {
        self.value.is_empty() && self.formula.is_none() && self.style == StyleId::DEFAULT
    }
    /// What the formula bar shows.
    pub fn input_text(&self) -> String {
        if let Some(f) = &self.formula {
            return format!("={}", f.text);
        }
        match &self.value {
            Value::Text(t) => {
                // Text that would parse as something else is shown with its apostrophe by Excel
                // only when entered that way; we show it plainly.
                t.to_string()
            }
            v => v.display(),
        }
    }
    /// What the formula bar shows when not editing: [`input_text`](Self::input_text), with a
    /// legacy array formula in braces (`{=A1:A2*2}`).
    pub fn bar_text(&self) -> String {
        match &self.formula {
            Some(f) if f.array.is_some() => format!("{{={}}}", f.text),
            _ => self.input_text(),
        }
    }
}
