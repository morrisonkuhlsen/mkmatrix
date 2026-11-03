//! A sheet read on threads is the sheet read one row at a time, whatever the part holds.

use crate::read_xlsx;
use crate::sheet_read::rows::{DECLINE, READ};

const SST: &str = r#"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>one</t></si><si><t>two</t></si></sst>"#;

fn package(sheet: &[u8]) -> Vec<u8> {
    let wb = r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="S" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#;
    super::make_zip(&[
        ("_rels/.rels", super::ROOT_RELS.as_bytes()),
        ("xl/workbook.xml", wb.as_bytes()),
        ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
        ("xl/sharedStrings.xml", SST.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet),
    ])
}

fn sheet(rows: &str) -> String {
    format!(
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetFormatPr defaultRowHeight="15"/><cols><col min="1" max="1" width="20" customWidth="1"/></cols><sheetData>{rows}</sheetData><mergeCells count="1"><mergeCell ref="Z1:Z2"/></mergeCells></worksheet>"#
    )
}

/// Rows enough to fill several blocks.
fn filler(from: u32, n: u32) -> String {
    (from..from + n).map(|r| format!(r#"<row r="{r}"><c r="A{r}"><v>{r}</v></c><c r="B{r}" t="s"><v>{}</v></c></row>"#, r % 2)).collect()
}

/// Reads `part` both ways and compares everything; says whether threads took it.
fn compare(what: &str, part: &[u8]) -> bool {
    let bytes = package(part);
    DECLINE.with(|d| d.set(true));
    let alone = read_xlsx(&bytes);
    DECLINE.with(|d| d.set(false));
    READ.with(|n| n.set(0));
    let threads = read_xlsx(&bytes);
    let took = READ.with(std::cell::Cell::get) > 0;
    match (alone, threads) {
        (Ok((a, ra)), Ok((b, rb))) => {
            assert_eq!(a.sheets, b.sheets, "{what}: the sheets differ");
            let sorted = |mut v: Vec<String>| {
                v.sort();
                v
            };
            assert_eq!(sorted(ra.warnings), sorted(rb.warnings), "{what}: the warnings differ");
        }
        (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string(), "{what}"),
        (a, b) => panic!("{what}: {:?} against {:?}", a.err().map(|e| e.to_string()), b.err().map(|e| e.to_string())),
    }
    took
}

#[test]
fn clean_sheets_are_read_on_threads_and_agree() {
    let cases = [
        ("plain", filler(1, 200)),
        (
            "empty and styled rows",
            format!(
                r#"{}<row r="500" ht="30" customHeight="1"/><row r="501" hidden="1"/><row r="502" s="0" customFormat="1"></row>{}"#,
                filler(1, 50),
                filler(600, 50)
            ),
        ),
        ("rows out of order", format!("{}{}", filler(300, 60), filler(1, 60))),
        ("a row said twice", format!("{}{}", filler(1, 60), filler(30, 60))),
        (
            "every kind of value",
            format!(
                r##"<row r="1"><c r="A1" t="b"><v>1</v></c><c r="B1" t="e"><v>#N/A</v></c><c r="C1" t="str"><v>x</v></c><c r="D1" t="inlineStr"><is><t>in</t></is></c><c r="E1" t="d"><v>2024-03-15</v></c><c r="F1"><v>2.5</v></c><c r="G1" t="s"><v>9</v></c><c r="H1"><v>abc</v></c><c r="I1" t="d"><v>nope</v></c></row>{}"##,
                filler(2, 80)
            ),
        ),
        (
            "bad cells and rows",
            format!(
                r#"{}<row r="1048577"><c r="A1"><v>1</v></c></row><row r="0"/><row r="70"><c r="?"><v>1</v></c><c r="XFE70"><v>1</v></c><c><v>7</v></c><c><v>8</v></c></row>{}"#,
                filler(1, 60),
                filler(80, 60)
            ),
        ),
        (
            "formulas of one shape, across blocks",
            (1..200).map(|r| format!(r#"<row r="{r}"><c r="A{r}"><v>{r}</v></c><c r="B{r}"><f>A{r}*2+$A$1</f><v>0</v></c></row>"#)).collect(),
        ),
        (
            "shapes that don't fill the same way",
            (1..80).map(|r| format!(r#"<row r="{r}"><c r="B{r}"><f>A{r}+{}</f><v>0</v></c></row>"#, r % 3)).collect(),
        ),
        (
            "formulas that don't parse",
            format!(
                r#"<row r="1"><c r="B1"><f>1+</f><v>5</v></c></row><row r="2"><c r="B2"><f>SUM(</f></c><c r="C2"><f></f><v>3</v></c></row>{}"#,
                filler(3, 80)
            ),
        ),
        ("shared formulas, masters and cells in other blocks", {
            let cells: String = (2..150)
                .map(|r| {
                    format!(r#"<row r="{r}"><c r="B{r}"><f t="shared" si="0"/><v>0</v></c><c r="C{r}"><f t="shared" si="1"/><v>0</v></c></row>"#)
                })
                .collect();
            format!(
                r#"<row r="1"><c r="B1"><f t="shared" si="0" ref="B1:B149">A1*2</f><v>0</v></c><c r="C1"><f t="shared" si="1" ref="C1:C149">1+</f><v>0</v></c></row>{cells}"#
            )
        }),
        (
            "a shared formula with no master, and one redefined",
            format!(
                r#"<row r="1"><c r="B1"><f t="shared" si="7"/><v>9</v></c><c r="C1"><f t="shared"/><v>9</v></c></row>{}<row r="200"><c r="B200"><f t="shared" si="7">A200</f></c></row>{}<row r="300"><c r="B300"><f t="shared" si="7"/></c></row>"#,
                filler(2, 80),
                filler(201, 80)
            ),
        ),
        (
            "arrays and their cached cells, across blocks",
            format!(
                r#"<row r="1"><c r="B1" cm="1"><f t="array" ref="B1:B90">SEQUENCE(90)</f><v>1</v></c><c r="C1"><f t="array" ref="C1:C90">A1:A90*2</f><v>1</v></c></row>{}"#,
                (2..100)
                    .map(|r| format!(r#"<row r="{r}"><c r="B{r}"><v>{r}</v></c><c r="C{r}"><v>{r}</v></c><c r="D{r}"><v>{r}</v></c></row>"#))
                    .collect::<String>()
            ),
        ),
        ("data tables", format!(r#"<row r="1"><c r="B1"><f t="dataTable" ref="B1:B2"/><v>1</v></c></row>{}"#, filler(2, 80))),
        ("value metadata", format!(r#"<row r="1"><c r="B1" vm="1"><f>A1</f><v>1</v></c><c r="C1" vm="0"><v>1</v></c></row>{}"#, filler(2, 80))),
        ("whitespace between rows", format!("\n  {}\n  <row\n r=\"900\"\n>\n<c r=\"A900\"><v>1</v></c></row>\n", filler(1, 80))),
    ];
    for (what, rows) in &cases {
        assert!(compare(what, sheet(rows).as_bytes()), "{what}: threads should have read this");
    }
}

#[test]
fn what_threads_decline_agrees_with_one_thread() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("rows with no number", sheet(&format!(r#"{}<row><c><v>1</v></c></row>{}"#, filler(1, 60), filler(100, 60))).into_bytes()),
        (
            "a row number that isn't one",
            sheet(&format!(r#"{}<row r="abc"><c r="A1"><v>1</v></c></row><row r="-1"/>{}"#, filler(1, 60), filler(100, 60))).into_bytes(),
        ),
        ("a comment hiding a row", sheet(&format!(r#"{}<!-- {} -->{}"#, filler(1, 60), filler(500, 60), filler(100, 60))).into_bytes()),
        (
            "CDATA hiding a row",
            sheet(&format!(r#"{}<row r="99"><c r="A99" t="str"><v><![CDATA[{}]]></v></c></row>{}"#, filler(1, 60), filler(500, 60), filler(100, 60)))
                .into_bytes(),
        ),
        (
            "a namespace prefix",
            format!(
                r#"<x:worksheet xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><x:sheetData>{}</x:sheetData></x:worksheet>"#,
                filler(1, 30)
                    .replace("<row", "<x:row")
                    .replace("</row", "</x:row")
                    .replace("<c ", "<x:c ")
                    .replace("</c>", "</x:c>")
                    .replace("<v>", "<x:v>")
                    .replace("</v>", "</x:v>")
            )
            .into_bytes(),
        ),
        ("no rows", sheet("").into_bytes()),
        ("no sheet data", b"<worksheet><row r=\"1\"/></worksheet>".to_vec()),
        ("cut short", sheet(&filler(1, 200)).as_bytes().get(..3000).map(<[u8]>::to_vec).unwrap_or_default()),
        ("a row left open", sheet(&format!(r#"{}<row r="70"><c r="A70"><v>1</v></c>{}"#, filler(1, 60), filler(100, 60))).into_bytes()),
        ("something else among the rows", sheet(&format!(r#"{}<other/>{}"#, filler(1, 60), filler(100, 60))).into_bytes()),
        ("text among the rows", sheet(&format!(r#"{}stray{}"#, filler(1, 60), filler(100, 60))).into_bytes()),
        ("UTF-16", {
            let text = sheet(&filler(1, 60));
            let mut out = vec![0xFF, 0xFE];
            out.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            out
        }),
        ("not XML at all", vec![0x3C; 2000]),
        ("empty", Vec::new()),
    ];
    for (what, part) in &cases {
        compare(what, part);
    }
}

#[test]
fn a_comment_or_cdata_cut_by_a_block_sends_the_sheet_back_to_one_thread() {
    let comment = sheet(&format!(r#"{}<!-- {} -->{}"#, filler(1, 60), filler(500, 60), filler(100, 60)));
    assert!(!compare("comment", comment.as_bytes()));
    let cdata =
        sheet(&format!(r#"{}<row r="99"><c r="A99" t="str"><v><![CDATA[{}]]></v></c></row>{}"#, filler(1, 60), filler(500, 60), filler(100, 60)));
    assert!(!compare("cdata", cdata.as_bytes()));
    let unnumbered = sheet(&format!(r#"{}<row r="abc"><c><v>1</v></c></row>{}"#, filler(1, 60), filler(100, 60)));
    assert!(!compare("a number that isn't", unnumbered.as_bytes()));
}

#[test]
fn a_block_is_cut_only_at_a_row_tag() {
    use crate::sheet_read::rows::next_row_tag;
    let data = b"<rowBreaks/><rows/> <row r=\"1\"/><row>";
    assert_eq!(next_row_tag(data, 0), Some(20));
    assert_eq!(next_row_tag(data, 21), Some(32));
    assert_eq!(next_row_tag(data, 33), None);
    assert_eq!(next_row_tag(data, 9999), None);
}
