//! Sensitivity (beyond Excel): the constants a formula's result rests on, and how far a ± change
//! in each one moves it — the data behind a tornado chart. Every point is a [`Trial`], so the
//! workbook is left as it was.

use std::collections::VecDeque;

use super::*;

/// The most constant inputs one sensitivity tries (each costs two trials).
pub(crate) const MAX_INPUTS: usize = 40;
/// The most formula cells walked looking for them.
const MAX_WALK: usize = 2_000;

pub(super) fn sensitivity(s: &mut Session, p: &Json) -> Result<Json> {
    const CMD: &str = "data.sensitivity";
    let (si, at) = if p.get("cell").is_some() {
        cell_arg(s, p, "cell", CMD)?
    } else {
        let d = s.doc()?;
        (d.wb.active_sheet(), d.selection.active)
    };
    let change = f64_param(p, "change").unwrap_or(0.1);
    if !(change.is_finite() && change > 0.0 && change <= 10.0) {
        return Err(bad(CMD, "`change` must be a fraction above 0 and at most 10 (0.1 is ±10%)"));
    }
    let d = s.doc()?;
    let sh = d.wb.sheet(si).ok_or(EngineError::NoDocument)?;
    if !sh.cell(at).is_some_and(|c| c.formula.is_some()) {
        return Err(EngineError::Other("Cell must contain a formula.".into()));
    }
    let base = sh.value(at).as_f64().ok_or_else(|| EngineError::Other("The formula's result must be a number.".into()))?;
    let (inputs, more) = inputs_of(d, si, at);
    let mut trial = Trial::new(d);
    let reads = [(si, at)];
    let mut out: Vec<(Option<f64>, Json)> = Vec::with_capacity(inputs.len());
    for (isi, c, x) in inputs {
        // A zero moves by ±change itself (a percentage of nothing would not move it at all).
        let (lo, hi) = if x == 0.0 { (-change, change) } else { (x * (1.0 - change), x * (1.0 + change)) };
        let mut at_value = |v: f64| trial.run(&[(isi, c, Value::number(v))], &reads).into_iter().next().unwrap_or_default();
        let (rlo, rhi) = (at_value(lo), at_value(hi));
        let swing = match (rlo.as_f64(), rhi.as_f64()) {
            (Some(a), Some(b)) if (b - a).is_finite() => Some((b - a).abs()),
            _ => None,
        };
        let name = d.wb.sheet(isi).map(|sh| sh.name.clone()).unwrap_or_default();
        out.push((
            swing,
            json!({
                "sheet": name, "cell": c.a1(), "value": x, "low": lo, "high": hi,
                "lowResult": value_json(&rlo), "highResult": value_json(&rhi), "swing": swing,
            }),
        ));
    }
    // Widest swing first; an input that turns the result into an error comes after the numbers.
    out.sort_by(|a, b| match (a.0, b.0) {
        (Some(x), Some(y)) => y.total_cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    Ok(json!({
        "sheet": sh.name, "cell": at.a1(), "value": base, "change": change,
        "inputs": out.into_iter().map(|(_, j)| j).collect::<Vec<_>>(), "more": more,
    }))
}

/// The number constants `at` reads, directly or through other formulas, nearest first: at most
/// [`MAX_INPUTS`], with `true` when there were more (or the walk stopped early). Dates and
/// times are left out: a percentage of a date is no useful change.
fn inputs_of(d: &DocState, si: usize, at: CellRef) -> (Vec<(usize, CellRef, f64)>, bool) {
    let mut seen = HashSet::from([(si, at)]);
    let mut queue = VecDeque::from([(si, at)]);
    let (mut inputs, mut more, mut walked) = (Vec::new(), false, 0usize);
    while let Some(k) = queue.pop_front() {
        walked += 1;
        if walked > MAX_WALK {
            more = true;
            break;
        }
        for a in d.calc.graph.precedents_of(k) {
            let Some(psh) = d.wb.sheet(a.sheet) else { continue };
            // A spilled cell is its anchor's result: follow the anchor's formula.
            for (&anchor, r) in &psh.spill_ranges {
                if r.intersection(&a.range).is_some() && seen.insert((a.sheet, anchor)) {
                    queue.push_back((a.sheet, anchor));
                }
            }
            for (c, cell) in psh.cells.iter_range(a.range) {
                if !seen.insert((a.sheet, c)) {
                    continue;
                }
                if cell.formula.is_some() {
                    queue.push_back((a.sheet, c));
                } else if let Value::Number(n) = cell.value
                    && !mkmatrix_numfmt::NumberFormat::parse_cached(d.wb.styles.get(cell.style).num_fmt.as_str()).is_date()
                {
                    if inputs.len() < MAX_INPUTS {
                        inputs.push((a.sheet, c, n));
                    } else {
                        more = true;
                    }
                }
            }
        }
    }
    (inputs, more)
}
