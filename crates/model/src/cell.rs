//! Cells.

use std::borrow::Cow;
use std::sync::Arc;

use mkmatrix_core::{RangeRef, Value};
use mkmatrix_formula::Expr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::style::StyleId;

/// A parsed formula. Its text is the canonical formula text without the leading `=`.
///
/// Its tree is either its own (`expr`) or shared with the formulas filled from the same one
/// (`shared`): a column of `=B1*C1`, `=B2*C2`… keeps one tree and each cell's offset from it,
/// instead of a text and a tree per cell, and a shared formula's text is printed when asked for.
/// Either way [`Formula::expr`] gives this cell's tree, and equality compares the text.
#[derive(Clone, Debug)]
pub struct Formula {
    /// The text, if it is kept: always for a formula with a tree of its own, and for a shared one
    /// only when it came with it (read from a file). A filled formula keeps none.
    own_text: Box<str>,
    pub expr: Option<Arc<Expr>>,
    pub shared: Option<Shared>,
    /// Legacy array formula (Ctrl+Shift+Enter) covering this range; the formula lives in the
    /// top-left cell.
    pub array: Option<RangeRef>,
    /// Read from a file that marks this formula as a dynamic-array formula (cell metadata `cm`).
    /// The writer keeps the mark, so a formula Excel evaluates as an array is never saved as a
    /// legacy formula (which Excel evaluates with implicit intersection). Formulas entered or
    /// rewritten in MK Matrix start without it, and the writer decides from their shape.
    pub dynamic: bool,
}

/// A tree shared by formulas filled from one: the first formula's tree, and how far this cell's
/// references move from it (relative parts only, as filling moves them).
#[derive(Clone, Debug)]
pub struct Shared {
    pub tree: Arc<Expr>,
    pub rows: i32,
    pub cols: i32,
}

/// How a formula is saved: `{"text":"A1*2","array":null}`, and `"dynamic":true` when marked.
#[derive(Serialize)]
struct SavedRef<'a> {
    text: Cow<'a, str>,
    array: Option<RangeRef>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    dynamic: bool,
}

#[derive(Deserialize)]
struct Saved {
    text: String,
    array: Option<RangeRef>,
    #[serde(default)]
    dynamic: bool,
}

impl Serialize for Formula {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SavedRef { text: self.text(), array: self.array, dynamic: self.dynamic }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Formula {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Formula, D::Error> {
        let saved = Saved::deserialize(deserializer)?;
        Ok(Formula { own_text: saved.text.into(), expr: None, shared: None, array: saved.array, dynamic: saved.dynamic })
    }
}

impl PartialEq for Formula {
    /// The same formula: the same text, array range and dynamic mark (the text is printed from
    /// the tree, so the tree adds nothing, however it is stored).
    fn eq(&self, other: &Formula) -> bool {
        if self.array != other.array || self.dynamic != other.dynamic {
            return false;
        }
        self.same_text(other)
    }
}

/// Equality is on the text, which is reflexive; it also lets `Arc<Formula>` compare by pointer first.
impl Eq for Formula {}

impl Formula {
    /// Parses `text` (with or without `=`). Unparseable formulas keep their text and evaluate to
    /// `#NAME?`.
    pub fn new(text: &str) -> Formula {
        let body = text.strip_prefix('=').unwrap_or(text);
        match mkmatrix_formula::parse(body) {
            Ok(e) => Formula::from_expr(e),
            Err(_) => Formula { own_text: body.into(), expr: None, shared: None, array: None, dynamic: false },
        }
    }
    pub fn from_expr(e: Expr) -> Formula {
        Formula { own_text: mkmatrix_formula::print(&e).into(), expr: Some(Arc::new(e)), shared: None, array: None, dynamic: false }
    }
    /// This formula keeping `text`, which must be what it would print: for a reader that already
    /// holds the text, so that it isn't printed again or thrown away.
    pub fn keeping_text(self, text: String) -> Formula {
        Formula { own_text: text.into(), ..self }
    }
    /// This formula with the array range and dynamic mark given.
    pub fn marked(self, array: Option<RangeRef>, dynamic: bool) -> Formula {
        Formula { array, dynamic, ..self }
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
        let (rows, cols) = (i32::try_from(rows).ok()?, i32::try_from(cols).ok()?);
        Some(Formula {
            own_text: Box::default(),
            expr: None,
            shared: Some(Shared { tree: Arc::clone(tree), rows, cols }),
            array: None,
            dynamic: false,
        })
    }
    /// The formula text, printed from the shared tree when none is kept: a filled formula keeps
    /// no text, so this costs a print for those. Don't call it in a loop over a sheet when the
    /// tree would do.
    pub fn text(&self) -> Cow<'_, str> {
        match (&self.expr, &self.shared) {
            (None, Some(s)) if self.own_text.is_empty() => Cow::Owned(mkmatrix_formula::print(&mkmatrix_formula::adjust::shift_relative(
                (*s.tree).clone(),
                i64::from(s.rows),
                i64::from(s.cols),
            ))),
            _ => Cow::Borrowed(&self.own_text),
        }
    }
    /// Whether two formulas have the same text. The same tree moved the same way prints the same,
    /// so that case is answered without printing.
    pub fn same_text(&self, other: &Formula) -> bool {
        if let (None, None, Some(a), Some(b)) = (&self.expr, &other.expr, &self.shared, &other.shared)
            && Arc::ptr_eq(&a.tree, &b.tree)
            && (a.rows, a.cols) == (b.rows, b.cols)
        {
            return true;
        }
        self.text() == other.text()
    }
    /// Bytes of text this formula keeps (none for a filled one).
    pub fn text_bytes(&self) -> usize {
        self.own_text.len()
    }
    /// The parsed expression, re-parsing after deserialization.
    pub fn expr(&self) -> Option<Expr> {
        match (&self.expr, &self.shared) {
            (Some(e), _) => Some((**e).clone()),
            (None, Some(s)) => Some(mkmatrix_formula::adjust::shift_relative((*s.tree).clone(), i64::from(s.rows), i64::from(s.cols))),
            (None, None) => mkmatrix_formula::parse(&self.own_text).ok(),
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
    /// The tree this formula keeps and how far its relative references move, without touching
    /// the tree's reference count (threads share it); `None` when it has to be parsed first.
    pub fn tree(&self) -> Option<(&Arc<Expr>, i64, i64)> {
        match (&self.expr, &self.shared) {
            (Some(e), _) => Some((e, 0, 0)),
            (None, Some(s)) => Some((&s.tree, i64::from(s.rows), i64::from(s.cols))),
            (None, None) => None,
        }
    }
    /// What to evaluate: a tree and how far its relative references move (0, 0 for a formula's
    /// own tree). A shared tree isn't copied.
    pub fn eval_tree(&self) -> Option<(Arc<Expr>, i64, i64)> {
        match (&self.expr, &self.shared) {
            (Some(e), _) => Some((Arc::clone(e), 0, 0)),
            (None, Some(s)) => Some((Arc::clone(&s.tree), i64::from(s.rows), i64::from(s.cols))),
            (None, None) => mkmatrix_formula::parse(&self.own_text).ok().map(|e| (Arc::new(e), 0, 0)),
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
    /// Holds a value or a formula (a cell with only a style does not).
    pub fn has_value(&self) -> bool {
        !self.value.is_empty() || self.formula.is_some()
    }
    /// No value, no formula and the default style: storing it is pointless.
    pub fn is_blank(&self) -> bool {
        self.value.is_empty() && self.formula.is_none() && self.style == StyleId::DEFAULT
    }
    /// What the formula bar shows.
    pub fn input_text(&self) -> String {
        if let Some(f) = &self.formula {
            return format!("={}", f.text());
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
            Some(f) if f.array.is_some() => format!("{{={}}}", f.text()),
            _ => self.input_text(),
        }
    }
}
