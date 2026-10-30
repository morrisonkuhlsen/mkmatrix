//! Localized aliases accepted at user-entry boundaries; file syntax stays English.

use crate::{Expr, ParseError, parser};

/// Formula-entry syntax selected by the interface language. Other interface languages
/// currently use the canonical English function table until a translation table is supplied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FormulaLocale {
    #[default]
    En,
    Es,
    /// Brazilian Portuguese: `=SOMA(A1:A3; 1,5)`, `=PROCV(…)`.
    Pt,
}

const SPANISH: &[(&str, &str)] = &[
    ("SUM", "SUMA"),
    ("AVERAGE", "PROMEDIO"),
    ("IF", "SI"),
    ("COUNT", "CONTAR"),
    ("SUMIF", "SUMAR.SI"),
    ("COUNTIF", "CONTAR.SI"),
    ("VLOOKUP", "BUSCARV"),
    ("TODAY", "HOY"),
    // Financial block (#1986): the loan-schedule and investment staples.
    ("NPV", "VNA"),
    ("IRR", "TIR"),
    ("PMT", "PAGO"),
    ("PV", "VA"),
    ("FV", "VF"),
    ("TRUE", "VERDADERO"),
    ("FALSE", "FALSO"),
];

/// Brazilian Portuguese function names, as Portuguese-speaking users know them from other
/// spreadsheets (public documentation; function names are facts, not expression). Functions
/// not listed keep their English name, which works in every syntax; the matrix functions keep
/// MK Matrix's own `MX.` names, the same in every language.
const PORTUGUESE: &[(&str, &str)] = &[
    // Maths.
    ("SUM", "SOMA"),
    ("SUMIF", "SOMASE"),
    ("SUMIFS", "SOMASES"),
    ("SUMPRODUCT", "SOMARPRODUTO"),
    ("PRODUCT", "MULT"),
    ("ROUND", "ARRED"),
    ("ROUNDUP", "ARREDONDAR.PARA.CIMA"),
    ("ROUNDDOWN", "ARREDONDAR.PARA.BAIXO"),
    ("TRUNC", "TRUNCAR"),
    ("CEILING", "TETO"),
    ("FLOOR", "ARREDMULTB"),
    ("SQRT", "RAIZ"),
    ("POWER", "POTÊNCIA"),
    ("SIGN", "SINAL"),
    ("RAND", "ALEATÓRIO"),
    ("RANDBETWEEN", "ALEATÓRIOENTRE"),
    ("SUBTOTAL", "SUBTOTAL"),
    // Statistics.
    ("AVERAGE", "MÉDIA"),
    ("AVERAGEIF", "MÉDIASE"),
    ("AVERAGEIFS", "MÉDIASES"),
    ("COUNT", "CONT.NÚM"),
    ("COUNTA", "CONT.VALORES"),
    ("COUNTBLANK", "CONTAR.VAZIO"),
    ("COUNTIF", "CONT.SE"),
    ("COUNTIFS", "CONT.SES"),
    ("MAX", "MÁXIMO"),
    ("MIN", "MÍNIMO"),
    ("MAXIFS", "MÁXIMOSES"),
    ("MINIFS", "MÍNIMOSES"),
    ("MEDIAN", "MED"),
    ("MODE", "MODO"),
    ("STDEV", "DESVPAD"),
    ("LARGE", "MAIOR"),
    ("SMALL", "MENOR"),
    ("RANK", "ORDEM"),
    // Logic.
    ("IF", "SE"),
    ("IFS", "SES"),
    ("IFERROR", "SEERRO"),
    ("IFNA", "SENÃODISP"),
    ("AND", "E"),
    ("OR", "OU"),
    ("NOT", "NÃO"),
    ("XOR", "OUEXCL"),
    ("SWITCH", "PARÂMETRO"),
    ("TRUE", "VERDADEIRO"),
    ("FALSE", "FALSO"),
    // Lookup and reference.
    ("VLOOKUP", "PROCV"),
    ("HLOOKUP", "PROCH"),
    ("XLOOKUP", "PROCX"),
    ("LOOKUP", "PROC"),
    ("INDEX", "ÍNDICE"),
    ("MATCH", "CORRESP"),
    ("XMATCH", "CORRESPX"),
    ("CHOOSE", "ESCOLHER"),
    ("OFFSET", "DESLOC"),
    ("INDIRECT", "INDIRETO"),
    ("ROW", "LIN"),
    ("ROWS", "LINS"),
    ("COLUMN", "COL"),
    ("COLUMNS", "COLS"),
    ("UNIQUE", "ÚNICO"),
    ("SORT", "CLASSIFICAR"),
    ("FILTER", "FILTRO"),
    ("SEQUENCE", "SEQUÊNCIA"),
    // Text.
    ("CONCATENATE", "CONCATENAR"),
    ("TEXTJOIN", "UNIRTEXTO"),
    ("LEFT", "ESQUERDA"),
    ("RIGHT", "DIREITA"),
    ("MID", "EXT.TEXTO"),
    ("LEN", "NÚM.CARACT"),
    ("UPPER", "MAIÚSCULA"),
    ("LOWER", "MINÚSCULA"),
    ("PROPER", "PRI.MAIÚSCULA"),
    ("TRIM", "ARRUMAR"),
    ("SUBSTITUTE", "SUBSTITUIR"),
    ("REPLACE", "MUDAR"),
    ("FIND", "PROCURAR"),
    ("SEARCH", "LOCALIZAR"),
    ("TEXT", "TEXTO"),
    ("VALUE", "VALOR"),
    // Information.
    ("ISBLANK", "ÉCÉL.VAZIA"),
    ("ISNUMBER", "ÉNÚM"),
    ("ISTEXT", "ÉTEXTO"),
    ("ISERROR", "ÉERROS"),
    ("ISERR", "ÉERRO"),
    ("ISNA", "É.NÃO.DISP"),
    // Date and time.
    ("TODAY", "HOJE"),
    ("NOW", "AGORA"),
    ("DATE", "DATA"),
    ("YEAR", "ANO"),
    ("MONTH", "MÊS"),
    ("DAY", "DIA"),
    ("WEEKDAY", "DIA.DA.SEMANA"),
    ("HOUR", "HORA"),
    ("MINUTE", "MINUTO"),
    ("SECOND", "SEGUNDO"),
    ("EOMONTH", "FIMMÊS"),
    ("EDATE", "DATAM"),
    ("WORKDAY", "DIATRABALHO"),
    ("NETWORKDAYS", "DIATRABALHOTOTAL"),
    ("DATEDIF", "DATADIF"),
    // Financial.
    ("NPV", "VPL"),
    ("IRR", "TIR"),
    ("PMT", "PGTO"),
    ("PV", "VP"),
    ("FV", "VF"),
    ("RATE", "TAXA"),
];

impl FormulaLocale {
    pub fn list_separator(self) -> char {
        if self == Self::En { ',' } else { ';' }
    }
    pub fn decimal_separator(self) -> char {
        if self == Self::En { '.' } else { ',' }
    }
    pub fn array_column_separator(self) -> char {
        if self == Self::En { ',' } else { '\\' }
    }
    /// English name -> this syntax's name, for the functions its table covers.
    pub fn function_aliases(self) -> &'static [(&'static str, &'static str)] {
        self.table()
    }
    fn table(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::En => &[],
            Self::Es => SPANISH,
            Self::Pt => PORTUGUESE,
        }
    }
    /// The names of TRUE and FALSE in this syntax (`None` in English, whose are the canonical ones).
    fn booleans(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::En => None,
            Self::Es => Some(("VERDADERO", "FALSO")),
            Self::Pt => Some(("VERDADEIRO", "FALSO")),
        }
    }
    // Names compare as Excel does, whatever the script: `média` is `MÉDIA`.
    pub fn function_name(self, canonical: &str) -> &str {
        self.table().iter().find(|(en, _)| crate::same_name(en, canonical)).map(|(_, local)| *local).unwrap_or(canonical)
    }
    pub fn canonical_name(self, localized: &str) -> &str {
        self.table().iter().find(|(_, local)| crate::same_name(local, localized)).map(|(en, _)| *en).unwrap_or(localized)
    }
}

/// Parses UI input using the selected function table and separators. Existing names and
/// LET/LAMBDA bindings take precedence over aliases. File and API syntax uses [`crate::parse`].
pub fn parse_input(src: &str, locale: FormulaLocale, known_name: impl Fn(&str) -> bool) -> Result<Expr, ParseError> {
    let mut expr = parser::parse_syntax(src, locale)?;
    if locale != FormulaLocale::En {
        normalize(&mut expr, locale, &known_name, &mut Vec::new(), 0, false)?;
    }
    Ok(expr)
}

/// Prints canonical expressions for editing. Name collisions retain the canonical spelling
/// so displaying then committing a formula never changes a defined-name reference.
pub fn print_input(expr: &Expr, locale: FormulaLocale, known_name: impl Fn(&str) -> bool) -> Result<String, ParseError> {
    // Check before cloning/printing. A flat operator chain such as A1+A2+… nests on its left
    // without nesting in the text: cloning, printing and `normalize` follow that left spine in
    // a loop, so it does not count (Excel accepts 4,096 operands, #916). Other nesting does.
    let mut pending = vec![(expr, 0usize)];
    while let Some((node, depth)) = pending.pop() {
        if depth > parser::MAX_DEPTH {
            return Err(ParseError { msg: "formula is nested too deeply".into(), pos: 0 });
        }
        match node {
            Expr::Unary(_, child) | Expr::Paren(child) => pending.push((child, depth + 1)),
            Expr::Binary(_, left, right) => {
                pending.push((left, depth));
                pending.push((right, depth + 1));
            }
            Expr::Call(_, args) => pending.extend(args.iter().map(|e| (e, depth + 1))),
            Expr::Invoke(callee, args) => {
                pending.push((callee, depth + 1));
                pending.extend(args.iter().map(|e| (e, depth + 1)));
            }
            Expr::Array(rows) => pending.extend(rows.iter().flatten().map(|e| (e, depth + 1))),
            _ => {}
        }
    }
    if locale == FormulaLocale::En {
        return Ok(crate::print(expr));
    }
    let mut expr = expr.clone();
    normalize(&mut expr, locale, &known_name, &mut Vec::new(), 0, true)?;
    let canonical = crate::print(&expr);
    let tokens = crate::lexer::tokenize(&canonical).map_err(|e| ParseError { msg: e.msg, pos: e.pos })?;
    let mut out = String::new();
    let mut array_depth = 0usize;
    for token in tokens {
        use crate::lexer::Tok;
        let text = canonical.get(token.start..token.end).unwrap_or("");
        match token.tok {
            Tok::LBrace => {
                array_depth += 1;
                out.push_str(text);
            }
            Tok::RBrace => {
                array_depth = array_depth.saturating_sub(1);
                out.push_str(text);
            }
            Tok::Number(_) => out.push_str(&text.replace('.', &locale.decimal_separator().to_string())),
            Tok::Comma => out.push(if array_depth > 0 { locale.array_column_separator() } else { locale.list_separator() }),
            _ => out.push_str(text),
        }
    }
    Ok(out)
}

/// TRUE or FALSE when `name` is how `locale` writes one.
pub(crate) fn boolean_alias(locale: FormulaLocale, name: &str) -> Option<bool> {
    let (yes, no) = locale.booleans()?;
    if crate::same_name(name, yes) {
        Some(true)
    } else if crate::same_name(name, no) {
        Some(false)
    } else {
        None
    }
}

fn normalize(
    expr: &mut Expr,
    locale: FormulaLocale,
    known_name: &impl Fn(&str) -> bool,
    bindings: &mut Vec<String>,
    depth: usize,
    display: bool,
) -> Result<(), ParseError> {
    if depth > parser::MAX_DEPTH {
        return Err(ParseError { msg: "formula is nested too deeply".into(), pos: 0 });
    }
    let bound = |name: &str, bindings: &[String]| bindings.iter().rev().any(|n| crate::same_name(n, name)) || known_name(name);
    match expr {
        Expr::Bool(value) if display => {
            if let Some((yes, no)) = locale.booleans() {
                let alias = if *value { yes } else { no };
                if !bound(alias, bindings) {
                    *expr = Expr::Name(alias.into());
                }
            }
        }
        Expr::Name(name) if !display && !bound(name, bindings) => {
            if let Some(value) = boolean_alias(locale, name) {
                *expr = Expr::Bool(value);
            }
        }
        Expr::Call(name, args) if name == "LET" && args.len() >= 3 && !args.len().is_multiple_of(2) => {
            let saved = bindings.len();
            if let Some((body, pairs)) = args.split_last_mut() {
                for pair in pairs.chunks_mut(2) {
                    if let [declaration, value] = pair {
                        // The new name is not in scope in its own value expression.
                        normalize(value, locale, known_name, bindings, depth + 1, display)?;
                        if let Expr::Name(name) = declaration {
                            bindings.push(name.clone());
                        }
                    }
                }
                normalize(body, locale, known_name, bindings, depth + 1, display)?;
            }
            bindings.truncate(saved);
        }
        Expr::Call(name, args) if name == "LAMBDA" => {
            let saved = bindings.len();
            if let Some((body, params)) = args.split_last_mut() {
                for param in params {
                    if let Expr::Name(name) = param {
                        bindings.push(name.clone());
                    }
                }
                normalize(body, locale, known_name, bindings, depth + 1, display)?;
            }
            bindings.truncate(saved);
        }
        Expr::Call(name, args) => {
            let alias = if display { locale.function_name(name) } else { locale.canonical_name(name) };
            if !bound(name, bindings) && (!display || !bound(alias, bindings)) {
                *name = alias.to_string();
            }
            for arg in args {
                normalize(arg, locale, known_name, bindings, depth + 1, display)?;
            }
        }
        Expr::Invoke(callee, args) => {
            normalize(callee, locale, known_name, bindings, depth + 1, display)?;
            for arg in args {
                normalize(arg, locale, known_name, bindings, depth + 1, display)?;
            }
        }
        Expr::Unary(_, child) | Expr::Paren(child) => normalize(child, locale, known_name, bindings, depth + 1, display)?,
        Expr::Binary(..) => {
            // Follow the left spine of a flat chain (A1+A2+…) in a loop: it is not nesting.
            // Each call leaves `bindings` as it found them, so the order does not matter.
            let mut node = expr;
            while let Expr::Binary(_, left, right) = node {
                normalize(right, locale, known_name, bindings, depth + 1, display)?;
                node = left;
            }
            normalize(node, locale, known_name, bindings, depth, display)?;
        }
        Expr::Array(rows) if display => {
            for row in rows {
                for child in row {
                    normalize(child, locale, known_name, bindings, depth + 1, display)?;
                }
            }
        }
        // Array elements have already been parsed as literals. Sheet/structured references
        // contain user-defined labels and must not be translated.
        _ => {}
    }
    Ok(())
}
