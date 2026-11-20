//! MK Matrix's actions on a matrix (#31): each writes one formula that computes from the block
//! (`=X'`, `=MX.INV(X)`, `=X@Y`…) in a free spot beside it, where the result spills. The floating
//! actions by a selected block and the cell menu run these, as do the control channel, the CLI
//! and MCP.

use mkmatrix_core::RangeRef;
use serde_json::{Value as Json, json};

use super::*;
use crate::matrices;

/// The most cells a block may have: past this a result's free spot is not worth searching for
/// (the MX functions refuse larger blocks anyway).
const MAX_CELLS: u64 = 1_000_000;

/// What an action writes and the rows × columns its result takes.
#[derive(Clone, Copy)]
enum Op {
    Transpose,
    Inverse,
    Determinant,
    Eigenvalues,
    SingularValues,
    Multiply,
    Solve,
}

impl Op {
    fn id(self) -> &'static str {
        match self {
            Op::Transpose => "matrix.transpose",
            Op::Inverse => "matrix.inverse",
            Op::Determinant => "matrix.determinant",
            Op::Eigenvalues => "matrix.eigenvalues",
            Op::SingularValues => "matrix.singularValues",
            Op::Multiply => "matrix.multiply",
            Op::Solve => "matrix.solve",
        }
    }
    fn square(self) -> bool {
        matches!(self, Op::Inverse | Op::Determinant | Op::Eigenvalues)
    }
    /// The second operand's param, for the two that take one.
    fn operand(self) -> Option<&'static str> {
        match self {
            Op::Multiply => Some("by"),
            Op::Solve => Some("rhs"),
            _ => None,
        }
    }
    /// The formula for matrix `a` (and second operand `b`).
    fn formula(self, a: &str, b: &str) -> String {
        match self {
            Op::Transpose => format!("={a}'"),
            Op::Inverse => format!("=MX.INV({a})"),
            Op::Determinant => format!("=MX.DET({a})"),
            Op::Eigenvalues => format!("=MX.EIG({a})"),
            Op::SingularValues => format!("=MX.SVD({a})"),
            Op::Multiply => format!("={a}@{b}"),
            Op::Solve => format!("={a}\\{b}"),
        }
    }
    /// The result's rows × columns for an `r` × `c` block and a second operand of `br` × `bc`.
    fn shape(self, (r, c): (u32, u32), (_, bc): (u32, u32)) -> (u32, u32) {
        match self {
            Op::Transpose => (c, r),
            Op::Inverse => (r, c),
            Op::Determinant => (1, 1),
            // A column, or two (real and imaginary parts) when some are complex.
            Op::Eigenvalues => (r, 2),
            Op::SingularValues => (r.min(c), 1),
            Op::Multiply => (r, bc),
            Op::Solve => (c, bc),
        }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "matrix.transpose",
            "Transpose",
            ["Matrix"],
            None,
            "{range?: \"B2:D9\" (default: the selection), at?: \"F2\" (default: the first free spot right of the block, else below it)} → {cell, formula}. Writes =X' (the block's name when it is a named matrix, else its address); the result spills there.",
            has_doc,
            |s, p| run(s, p, Op::Transpose)
        ),
        cmd!(
            "matrix.inverse",
            "Inverse",
            ["Matrix"],
            None,
            "{range?, at?} → {cell, formula}. Writes =MX.INV(X) for a square block.",
            has_doc,
            |s, p| run(s, p, Op::Inverse)
        ),
        cmd!(
            "matrix.determinant",
            "Determinant",
            ["Matrix"],
            None,
            "{range?, at?} → {cell, formula}. Writes =MX.DET(X) for a square block.",
            has_doc,
            |s, p| run(s, p, Op::Determinant)
        ),
        cmd!(
            "matrix.eigenvalues",
            "Eigenvalues",
            ["Matrix"],
            None,
            "{range?, at?} → {cell, formula}. Writes =MX.EIG(X) for a square block (room is kept for a second column, the imaginary parts).",
            has_doc,
            |s, p| run(s, p, Op::Eigenvalues)
        ),
        cmd!(
            "matrix.singularValues",
            "Singular Values",
            ["Matrix"],
            None,
            "{range?, at?} → {cell, formula}. Writes =MX.SVD(X).",
            has_doc,
            |s, p| run(s, p, Op::SingularValues)
        ),
        cmd!(
            "matrix.multiply",
            "Multiply",
            ["Matrix"],
            None,
            "{range?, by: \"F2:G4\" or a named matrix, at?} → {cell, formula}. Writes =X@Y.",
            has_doc,
            |s, p| run(s, p, Op::Multiply)
        ),
        cmd!(
            "matrix.solve",
            "Solve",
            ["Matrix"],
            None,
            "{range?, rhs: \"F2:F4\" or a named matrix, at?} → {cell, formula}. Writes =X\\b, the solution of X·x = b (least squares for a tall X).",
            has_doc,
            |s, p| run(s, p, Op::Solve)
        ),
    ]
}

/// The second operand, as the formula cites it, and its rows × columns: a named matrix (on any
/// sheet) or a range, `Sheet2!A1:B2` included.
fn operand(s: &Session, si: usize, op: Op, key: &str, p: &Json) -> Result<(String, (u32, u32))> {
    let Some(text) = str_param(p, key).map(str::trim).filter(|t| !t.is_empty()) else {
        return Err(bad(op.id(), format!("missing `{key}`: a range such as \"F2:G4\" or a named matrix")));
    };
    let wb = &s.doc()?.wb;
    let named = (0..wb.sheets.len()).flat_map(|i| matrices::on_sheet(wb, i)).find(|(n, _)| mkmatrix_formula::same_name(n, text));
    if let Some((name, r)) = named {
        return Ok((name, (r.height(), r.width())));
    }
    let (sheet, body) = split_sheet(text);
    let r = RangeRef::parse(body).ok_or_else(|| bad(op.id(), format!("`{key}` is not a range or a named matrix: `{text}`")))?;
    if let Some(name) = &sheet
        && wb.sheet_index(name).is_none()
    {
        return Err(bad(op.id(), format!("no sheet named `{name}`")));
    }
    let cited = match sheet {
        Some(name) if wb.sheet_index(&name) != Some(si) => format!("{}!{}", mkmatrix_core::quote_sheet(&name), r.a1()),
        _ => r.a1(),
    };
    Ok((cited, (r.height(), r.width())))
}

fn run(s: &mut Session, p: &Json, op: Op) -> Result<Json> {
    let si = target_sheet(s, p)?;
    let r = target_range(s, p)?;
    if r.is_full_rows() || r.is_full_cols() || r.count() > MAX_CELLS {
        return Err(bad(op.id(), format!("the matrix may have at most {MAX_CELLS} cells")));
    }
    if op.square() && r.height() != r.width() {
        return Err(bad(op.id(), format!("needs a square matrix; {} is {}×{}", r.a1(), r.height(), r.width())));
    }
    let (b, b_shape) = match op.operand() {
        Some(key) => operand(s, si, op, key, p)?,
        None => (String::new(), (1, 1)),
    };
    let a = matrices::reference(&s.doc()?.wb, si, r);
    let at = match cell_param_on(s, p, "at", si)? {
        Some(c) => c,
        None => {
            let sh = s.doc()?.wb.sheet(si).ok_or_else(|| bad(op.id(), "no such sheet"))?;
            matrices::free_spot(sh, r, op.shape((r.height(), r.width()), b_shape))
                .ok_or_else(|| EngineError::Other("There are no empty cells next to the matrix for the result.".into()))?
        }
    };
    let formula = op.formula(&a, &b);
    let sheet = s.doc()?.wb.sheet(si).map(|sh| sh.name.clone()).unwrap_or_default();
    let cell = format!("{}!{}", mkmatrix_core::quote_sheet(&sheet), at.a1());
    super::edit::cell_set(s, &json!({"cell": cell, "input": formula}))?;
    Ok(json!({"cell": at.a1(), "formula": formula}))
}

#[cfg(test)]
mod tests {
    use mkmatrix_core::Value;
    use serde_json::json;

    use crate::Session;

    fn session() -> Session {
        let mut s = Session::new();
        s.new_workbook();
        s.execute("range.setValues", json!({"range": "A1", "values": [[2, 1], [1, 3]]})).unwrap();
        s
    }

    fn value(s: &Session, a1: &str) -> Value {
        s.doc().unwrap().wb.sheet(0).unwrap().value(mkmatrix_core::CellRef::parse(a1).unwrap())
    }

    #[test]
    fn each_action_writes_its_formula_past_the_block() {
        let mut s = session();
        let r = s.execute("matrix.transpose", json!({"range": "A1:B2"})).unwrap();
        assert_eq!(r, json!({"cell": "D1", "formula": "=A1:B2'"}));
        assert_eq!(value(&s, "E1"), Value::Number(1.0));
        // D1:E2 now holds the transpose: the inverse goes further right.
        let r = s.execute("matrix.inverse", json!({"range": "A1:B2"})).unwrap();
        assert_eq!(r["cell"], "F1");
        assert_eq!(value(&s, "F1"), Value::Number(0.6));
        let r = s.execute("matrix.determinant", json!({"range": "A1:B2", "at": "A5"})).unwrap();
        assert_eq!(r, json!({"cell": "A5", "formula": "=MX.DET(A1:B2)"}));
        assert_eq!(value(&s, "A5"), Value::Number(5.0));
    }

    #[test]
    fn a_named_matrix_is_cited_by_name_and_the_selection_is_the_default() {
        let mut s = session();
        s.execute("formulas.defineName", json!({"name": "X", "refersTo": "=Sheet1!$A$1:$B$2"})).unwrap();
        s.execute("range.setValues", json!({"range": "A8", "values": [[1], [2]]})).unwrap();
        s.execute("selection.set", json!({"range": "A1:B2"})).unwrap();
        let r = s.execute("matrix.multiply", json!({"by": "A8:A9", "at": "D8"})).unwrap();
        assert_eq!(r["formula"], "=X@A8:A9");
        assert_eq!(value(&s, "D8"), Value::Number(4.0));
        assert_eq!(value(&s, "D9"), Value::Number(7.0));
        let r = s.execute("matrix.solve", json!({"rhs": "A8:A9", "at": "F8"})).unwrap();
        assert_eq!(r["formula"], "=X\\A8:A9");
        assert_eq!(value(&s, "F8"), Value::Number(0.2));
    }

    #[test]
    fn bad_calls_are_errors_that_change_nothing() {
        let mut s = session();
        s.execute("range.setValues", json!({"range": "A3", "values": [[5, 6]]})).unwrap();
        for (id, p) in [
            ("matrix.inverse", json!({"range": "A1:B3"})),
            ("matrix.determinant", json!({"range": "A:B"})),
            ("matrix.multiply", json!({"range": "A1:B2"})),
            ("matrix.multiply", json!({"range": "A1:B2", "by": "1)+NOW("})),
            ("matrix.solve", json!({"range": "A1:B2", "rhs": "Nowhere!A1:A2"})),
            ("matrix.transpose", json!({"range": "A1:B2", "at": "nope"})),
        ] {
            let before = s.doc().unwrap().wb.clone();
            assert!(s.execute(id, p.clone()).is_err(), "{id} {p}");
            assert_eq!(*s.doc().unwrap().wb, *before, "{id} {p} changed the workbook");
        }
    }
}
