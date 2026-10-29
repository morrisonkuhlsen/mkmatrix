//! MK Matrix's names for its matrix functions: the `MX.` family. Excel's own matrix functions
//! (MMULT, MINVERSE, MDETERM, TRANSPOSE, MUNIT) go by their MX. names in the app and are read
//! and written under Excel's names in files, so a workbook still opens in Excel. The functions
//! only MK Matrix has are written as user-defined (`_xludf.MX.SOLVE`): Excel shows their saved
//! values and `#NAME?` once it recalculates them.

/// Each MX. function and the Excel function it is, when there is one.
pub const MX_FUNCTIONS: &[(&str, Option<&str>)] = &[
    ("MX.MUL", Some("MMULT")),
    ("MX.INV", Some("MINVERSE")),
    ("MX.DET", Some("MDETERM")),
    ("MX.T", Some("TRANSPOSE")),
    ("MX.EYE", Some("MUNIT")),
    ("MX.SOLVE", None),
    ("MX.PINV", None),
    ("MX.RANK", None),
    ("MX.TRACE", None),
    ("MX.NORM", None),
    ("MX.COND", None),
    ("MX.EIG", None),
    ("MX.EIGVEC", None),
    ("MX.SVD", None),
    ("MX.QR", None),
    ("MX.LU", None),
    ("MX.CHOL", None),
    ("MX.POW", None),
    // Not a matrix function, but MK Matrix's own and array-shaped like them: written as
    // `_xludf.SQL` and read back as SQL.
    ("SQL", None),
];

/// The MX. name for an Excel function name (upper-case), e.g. `MMULT` → `MX.MUL`.
pub fn from_excel(name: &str) -> Option<&'static str> {
    MX_FUNCTIONS.iter().find(|(_, x)| x.is_some_and(|x| x.eq_ignore_ascii_case(name))).map(|(mx, _)| *mx)
}

/// The Excel name for an MX. function, e.g. `MX.MUL` → `MMULT`; `None` for one Excel lacks.
pub fn excel_name(name: &str) -> Option<&'static str> {
    MX_FUNCTIONS.iter().find(|(mx, _)| mx.eq_ignore_ascii_case(name)).and_then(|(_, x)| *x)
}

/// The MX. name as spelled here, for a name that is one (any case).
pub fn canonical(name: &str) -> Option<&'static str> {
    MX_FUNCTIONS.iter().find(|(mx, _)| mx.eq_ignore_ascii_case(name)).map(|(mx, _)| *mx)
}

/// How a file spells an MX. function: Excel's name, or the user-defined `_xludf.` form.
pub fn file_name(name: &str) -> Option<String> {
    let mx = canonical(name)?;
    Some(excel_name(mx).map_or_else(|| format!("_xludf.{mx}"), str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_through_files() {
        assert_eq!(from_excel("MMULT"), Some("MX.MUL"));
        assert_eq!(from_excel("transpose"), Some("MX.T"));
        assert_eq!(from_excel("SUM"), None);
        assert_eq!(excel_name("MX.INV"), Some("MINVERSE"));
        assert_eq!(excel_name("MX.SOLVE"), None);
        assert_eq!(file_name("mx.mul").as_deref(), Some("MMULT"));
        assert_eq!(file_name("MX.SVD").as_deref(), Some("_xludf.MX.SVD"));
        assert_eq!(file_name("SUM"), None);
        for (mx, x) in MX_FUNCTIONS {
            if let Some(x) = x {
                assert_eq!(from_excel(x), Some(*mx));
            }
        }
    }
}
