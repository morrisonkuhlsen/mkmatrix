use mkmatrix_formula::{FormulaLocale, parse, parse_input, print, print_input};

const PT: FormulaLocale = FormulaLocale::Pt;

fn input(src: &str) -> String {
    print(&parse_input(src, PT, |_| false).unwrap())
}

fn shown(canonical: &str) -> String {
    print_input(&parse(canonical).unwrap(), PT, |_| false).unwrap()
}

#[test]
fn portuguese_names_separators_and_decimal_comma() {
    for (src, expected) in [
        ("=soma(A1:A10)", "SUM(A1:A10)"),
        ("MÉDIA(B1:B5)*1,5", "AVERAGE(B1:B5)*1.5"),
        // Any case, accents included.
        ("média(B1:B5)", "AVERAGE(B1:B5)"),
        ("SE(A1>0;\"sim\";\"não\")", "IF(A1>0,\"sim\",\"não\")"),
        ("PROCV(A2;B:D;3;FALSO)", "VLOOKUP(A2,B:D,3,FALSE)"),
        ("CONT.SE(A1:A10;\">0\")", "COUNTIF(A1:A10,\">0\")"),
        ("CONT.NÚM(A1:A10)", "COUNT(A1:A10)"),
        ("SEERRO(ÍNDICE(C:C;CORRESP(B1;D:D;0));\"\")", "IFERROR(INDEX(C:C,MATCH(B1,D:D,0)),\"\")"),
        ("E(VERDADEIRO;NÃO(FALSO))", "AND(TRUE,NOT(FALSE))"),
        ("PGTO(0,08/12;10;10000)", "PMT(0.08/12,10,10000)"),
        ("É.NÃO.DISP(A1)", "ISNA(A1)"),
        ("HOJE()-DATA(2026;1;1)", "TODAY()-DATE(2026,1,1)"),
        ("MX.MUL(A1:B2;D1:E2)", "MX.MUL(A1:B2,D1:E2)"),
        // English names keep working.
        ("SUM(1;2)+VLOOKUP(1;A:B;2;FALSO)", "SUM(1,2)+VLOOKUP(1,A:B,2,FALSE)"),
        ("LET(x;2;x*3)", "LET(x,2,x*3)"),
        ("SOMA({1\\2;3\\4})", "SUM({1,2;3,4})"),
        ("{VERDADEIRO\\FALSO}", "{TRUE,FALSE}"),
    ] {
        assert_eq!(input(src), expected, "{src}");
    }
}

#[test]
fn canonical_formulas_show_in_portuguese() {
    for (canonical, expected) in [
        ("SUM(A1:A10)", "SOMA(A1:A10)"),
        ("IF(A1>0.5,TRUE,FALSE)", "SE(A1>0,5;VERDADEIRO;FALSO)"),
        ("VLOOKUP(A2,B:D,3,FALSE)", "PROCV(A2;B:D;3;FALSO)"),
        ("AVERAGE({1,2;3,4})", "MÉDIA({1\\2;3\\4})"),
        // A function without a Portuguese name keeps its English one.
        ("XNPV(0.1,A1:A3,B1:B3)", "XNPV(0,1;A1:A3;B1:B3)"),
    ] {
        assert_eq!(shown(canonical), expected, "{canonical}");
        // And reads back as the same formula.
        assert_eq!(input(expected), canonical, "{expected}");
    }
}

#[test]
fn names_and_bindings_win_over_portuguese_aliases() {
    // A workbook name called SOMA is the name, not SUM.
    assert_eq!(print(&parse_input("SOMA+1", PT, |n| n.eq_ignore_ascii_case("SOMA")).unwrap()), "SOMA+1");
    // A LET binding named E stays a name inside its body.
    assert_eq!(input("LET(E;2;E*3)"), "LET(E,2,E*3)");
    // Text, sheet names and table columns are never translated.
    assert_eq!(input("\"SOMA;FALSO\"&'SOMA'!A1+T[SE]"), "\"SOMA;FALSO\"&SOMA!A1+T[SE]");
}
