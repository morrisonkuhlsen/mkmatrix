//! Plain arithmetic over single cells, run without the interpreter: a filled column's `=B1*C1+D1`
//! compiled once and evaluated for each of its cells, with each cell's offset.
//!
//! A kernel only answers when the answer is certainly the interpreter's: it reads numbers and
//! blanks, and gives up (`None`) on anything else, such as text, an error, a formula not yet
//! calculated, a division by zero or a result that isn't finite. The interpreter then evaluates
//! that cell, so results are the same whichever one ran.

use std::sync::Arc;

use mkmatrix_core::{CellRef, MAX_COLS, MAX_ROWS, Value};
use mkmatrix_formula::{Anchor, BinOp, Expr, RefKind, SheetSel, UnOp};

/// Longest program compiled.
const MAX_OPS: usize = 64;
/// Most values a program holds at once.
const MAX_STACK: usize = 16;
/// Deepest tree compiled.
const MAX_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug)]
enum Op {
    Number(f64),
    Cell(Anchor),
    Neg,
    Add,
    Sub,
    Mul,
    Div,
}

/// A tree of plain arithmetic, as a program for a small stack machine.
#[derive(Clone, Debug)]
pub(super) struct Kernel {
    ops: Vec<Op>,
}

impl Kernel {
    /// `None` when the tree holds anything but numbers, single cells of the formula's own sheet,
    /// `+ - * /` and signs, or is too big.
    pub(super) fn compile(e: &Expr) -> Option<Kernel> {
        let mut ops = Vec::new();
        let mut held = 0;
        push(e, &mut ops, &mut held, 0)?;
        Some(Kernel { ops })
    }

    /// The result for a cell whose references move by `(rows, cols)`, where `read` gives a cell's
    /// number (a blank is 0), or `None` when it can't say.
    #[inline]
    pub(super) fn run(&self, rows: i64, cols: i64, mut read: impl FnMut(CellRef) -> Option<f64>) -> Option<f64> {
        let mut stack = [0.0f64; MAX_STACK];
        let mut top = 0usize;
        for op in &self.ops {
            let value = match op {
                Op::Number(n) => *n,
                Op::Cell(a) => read(moved(a, rows, cols)?)?,
                Op::Neg => {
                    let x = *stack.get(top.checked_sub(1)?)?;
                    top -= 1;
                    -x
                }
                Op::Add | Op::Sub | Op::Mul | Op::Div => {
                    let y = *stack.get(top.checked_sub(1)?)?;
                    let x = *stack.get(top.checked_sub(2)?)?;
                    top -= 2;
                    let z = match op {
                        Op::Add => x + y,
                        Op::Sub => x - y,
                        Op::Mul => x * y,
                        _ if y == 0.0 => return None,
                        _ => x / y,
                    };
                    if !z.is_finite() {
                        return None;
                    }
                    z
                }
            };
            *stack.get_mut(top)? = value;
            top += 1;
        }
        if top == 1 { stack.first().copied() } else { None }
    }
}

/// Trees met so far, with their kernels if they compile: few for a filled sheet, and cells of
/// different columns take turns in row order. Trees past the limit replace earlier ones.
#[derive(Default)]
pub(super) struct Kernels {
    known: Vec<(Arc<Expr>, Option<Kernel>)>,
    next: usize,
}

/// Most trees remembered.
const MAX_KERNELS: usize = 32;

impl Kernels {
    /// The kernel of `tree`, if it has one.
    pub(super) fn get(&mut self, tree: &Arc<Expr>) -> Option<&Kernel> {
        let i = match self.known.iter().position(|(t, _)| Arc::ptr_eq(t, tree)) {
            Some(i) => i,
            None => {
                let entry = (Arc::clone(tree), Kernel::compile(tree));
                if self.known.len() < MAX_KERNELS {
                    self.known.push(entry);
                    self.known.len() - 1
                } else {
                    let i = self.next % MAX_KERNELS;
                    self.next = self.next.wrapping_add(1);
                    *self.known.get_mut(i)? = entry;
                    i
                }
            }
        };
        self.known.get(i)?.1.as_ref()
    }
}

/// The number a kernel reads for `v`: blanks are 0, and anything else isn't its to answer.
#[inline]
pub(super) fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => Some(*n),
        Value::Empty => Some(0.0),
        _ => None,
    }
}

fn moved(a: &Anchor, rows: i64, cols: i64) -> Option<CellRef> {
    let row = if a.row_abs { i64::from(a.row) } else { i64::from(a.row) + rows };
    let col = if a.col_abs { i64::from(a.col) } else { i64::from(a.col) + cols };
    if (0..i64::from(MAX_ROWS)).contains(&row) && (0..i64::from(MAX_COLS)).contains(&col) {
        Some(CellRef::new(u32::try_from(row).ok()?, u32::try_from(col).ok()?))
    } else {
        None
    }
}

fn push_value(ops: &mut Vec<Op>, held: &mut usize, op: Op) -> Option<()> {
    *held += 1;
    ops.push(op);
    (*held <= MAX_STACK).then_some(())
}

/// Adds the program for `e`; `held` is how many values are on the stack.
fn push(e: &Expr, ops: &mut Vec<Op>, held: &mut usize, depth: usize) -> Option<()> {
    if depth > MAX_DEPTH || ops.len() >= MAX_OPS {
        return None;
    }
    match e {
        Expr::Number(n) if n.is_finite() => push_value(ops, held, Op::Number(*n)),
        Expr::Ref(r) => match (&r.sheet, &r.kind) {
            (SheetSel::Current, RefKind::Cell(a)) => push_value(ops, held, Op::Cell(*a)),
            _ => None,
        },
        Expr::Paren(x) | Expr::Unary(UnOp::Plus, x) => push(x, ops, held, depth + 1),
        Expr::Unary(UnOp::Neg, x) => {
            push(x, ops, held, depth + 1)?;
            ops.push(Op::Neg);
            Some(())
        }
        Expr::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div), a, b) => {
            push(a, ops, held, depth + 1)?;
            push(b, ops, held, depth + 1)?;
            *held = held.checked_sub(1)?;
            ops.push(match op {
                BinOp::Add => Op::Add,
                BinOp::Sub => Op::Sub,
                BinOp::Mul => Op::Mul,
                _ => Op::Div,
            });
            Some(())
        }
        _ => None,
    }
}
