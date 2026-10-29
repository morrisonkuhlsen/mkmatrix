//! Layout of formula text for reading: [`pretty`] breaks long function calls into one argument
//! per line, indented by nesting; [`compact`] puts a formula back on one line. Both only move
//! whitespace outside text, so the formula means the same and parses to the same tree.

use crate::FormulaLocale;
use crate::lexer::{Tok, tokenize_locale};

/// A call (or parenthesised group) whose one-line text is longer than this goes on several lines.
const WIDTH: usize = 48;
/// One level of indentation.
const INDENT: &str = "    ";
/// Deeper than this the layout gives up (the text stays as it is).
const MAX_DEPTH: usize = 256;

enum Node {
    /// Source text of one token (an operand or an operator).
    Leaf(String),
    /// Whitespace between tokens.
    Space,
    /// `IF(`…`)`, `(`…`)` or `{`…`}`: the opener, the arguments with the separator after each
    /// one but the last, and the closer.
    Group { open: String, args: Vec<Vec<Node>>, seps: Vec<String>, close: String, braces: bool },
}

/// `text` (with or without the leading `=`) laid out with each long call's arguments on their
/// own lines. `None` when it isn't a formula the lexer reads, or its brackets don't match.
pub fn pretty(text: &str, locale: FormulaLocale) -> Option<String> {
    let (eq, nodes) = parse(text, locale)?;
    let mut out = String::from(eq);
    render(&nodes, 0, &mut out);
    Some(out)
}

/// `text` on one line: the whitespace [`pretty`] adds taken out, other spacing kept as one space.
pub fn compact(text: &str, locale: FormulaLocale) -> Option<String> {
    let (eq, nodes) = parse(text, locale)?;
    let mut out = String::from(eq);
    flat(&nodes, &mut out);
    Some(out)
}

fn parse(text: &str, locale: FormulaLocale) -> Option<(&'static str, Vec<Node>)> {
    let (eq, body) = match text.strip_prefix('=') {
        Some(b) => ("=", b),
        None => ("", text),
    };
    let tokens = tokenize_locale(body, locale).ok()?;
    let mut i = 0;
    let (nodes, close) = group(body, &tokens, &mut i, false, 0)?;
    // A closer with nothing open is unbalanced.
    if close.is_some() {
        return None;
    }
    Some((eq, nodes.into_iter().next().unwrap_or_default()))
}

/// Reads tokens up to the closer of the group being read (`inside`) or the end: the arguments,
/// split at separators when inside a group, and the closer's text.
#[allow(clippy::type_complexity)]
fn group(
    src: &str,
    tokens: &[crate::lexer::Token],
    i: &mut usize,
    inside: bool,
    depth: usize,
) -> Option<(Vec<Vec<Node>>, Option<(Vec<String>, String)>)> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut args: Vec<Vec<Node>> = vec![Vec::new()];
    let mut seps: Vec<String> = Vec::new();
    while let Some(t) = tokens.get(*i) {
        *i += 1;
        let text = src.get(t.start..t.end)?.to_string();
        match &t.tok {
            Tok::Eof => break,
            Tok::Func(_) | Tok::LParen | Tok::LBrace => {
                let braces = t.tok == Tok::LBrace;
                let (inner, close) = group(src, tokens, i, true, depth + 1)?;
                // An opener never closed is unbalanced.
                let (inner_seps, close) = close?;
                args.last_mut()?.push(Node::Group { open: text, args: inner, seps: inner_seps, close, braces });
            }
            // Outside any group a closer is unbalanced: the caller sees it returned.
            Tok::RParen | Tok::RBrace => return Some((args, Some((seps, text)))),
            Tok::Comma | Tok::Semicolon if inside => {
                seps.push(text);
                args.push(Vec::new());
            }
            Tok::Space => args.last_mut()?.push(Node::Space),
            _ => args.last_mut()?.push(Node::Leaf(text)),
        }
    }
    if inside {
        return None;
    }
    Some((args, None))
}

/// Whether a space between `prev` and `next` (leaves) means something: between two operands it
/// is the intersection operator (`A1:B2 B1:C3`), so it stays.
fn keep_space(nodes: &[Node], at: usize) -> bool {
    let operand = |n: Option<&Node>| {
        matches!(n, Some(Node::Leaf(s)) if s.chars().next().is_some_and(|c| c.is_alphanumeric() || matches!(c, '$' | '\'' | '"' | '#' | '_' | '[')) )
            || matches!(n, Some(Node::Group { .. }))
    };
    let prev = nodes[..at].iter().rev().find(|n| !matches!(n, Node::Space));
    let next = nodes.get(at + 1..).and_then(|r| r.iter().find(|n| !matches!(n, Node::Space)));
    operand(prev) && operand(next)
}

/// One line: spaces only where they mean something.
fn flat(nodes: &[Node], out: &mut String) {
    let mut spaced = false;
    for (k, n) in nodes.iter().enumerate() {
        match n {
            Node::Leaf(s) => {
                out.push_str(s);
                spaced = false;
            }
            Node::Space => {
                if !spaced && keep_space(nodes, k) {
                    out.push(' ');
                    spaced = true;
                }
            }
            Node::Group { open, args, seps, close, .. } => {
                out.push_str(open);
                for (a, arg) in args.iter().enumerate() {
                    flat(arg, out);
                    if let Some(sep) = seps.get(a) {
                        out.push_str(sep);
                    }
                }
                out.push_str(close);
                spaced = false;
            }
        }
    }
}

fn flat_len(nodes: &[Node]) -> usize {
    let mut s = String::new();
    flat(nodes, &mut s);
    s.chars().count()
}

/// Laid out at `level`: a group too long for one line (array constants excepted) puts each
/// argument on its own line one level deeper, and its closer back at `level`.
fn render(nodes: &[Node], level: usize, out: &mut String) {
    let mut spaced = false;
    for (k, n) in nodes.iter().enumerate() {
        match n {
            Node::Leaf(s) => {
                out.push_str(s);
                spaced = false;
            }
            Node::Space => {
                if !spaced && keep_space(nodes, k) {
                    out.push(' ');
                    spaced = true;
                }
            }
            Node::Group { open, args, seps, close, braces } => {
                spaced = false;
                let one = std::slice::from_ref(n);
                if *braces || flat_len(one) <= WIDTH || args.iter().all(Vec::is_empty) {
                    flat(one, out);
                    continue;
                }
                out.push_str(open);
                // LET's names sit beside their values: `total, SUM(…),` on one line.
                let pairs = open.eq_ignore_ascii_case("LET(") || open.eq_ignore_ascii_case("_xlfn.LET(");
                for (a, arg) in args.iter().enumerate() {
                    if pairs && a % 2 == 1 && a + 1 < args.len() {
                        out.push(' ');
                    } else {
                        out.push('\n');
                        out.push_str(&INDENT.repeat(level + 1));
                    }
                    // An argument's own leading and trailing spaces go: the line break replaces them.
                    let start = arg.iter().position(|x| !matches!(x, Node::Space)).unwrap_or(arg.len());
                    let end = arg.iter().rposition(|x| !matches!(x, Node::Space)).map_or(start, |e| e + 1);
                    render(arg.get(start..end).unwrap_or_default(), level + 1, out);
                    if let Some(sep) = seps.get(a) {
                        out.push_str(sep);
                    }
                }
                out.push('\n');
                out.push_str(&INDENT.repeat(level));
                out.push_str(close);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: FormulaLocale = FormulaLocale::En;

    #[test]
    fn short_formulas_stay_on_one_line() {
        assert_eq!(pretty("=SUM(A1:A3, B1)", EN).as_deref(), Some("=SUM(A1:A3,B1)"));
        assert_eq!(pretty("=1+2", EN).as_deref(), Some("=1+2"));
    }

    #[test]
    fn long_calls_take_one_argument_per_line_indented_by_nesting() {
        let f = "=IF(AND(A1>0,B1>0),VLOOKUP(A1,Sheet2!$A$1:$D$500,4,FALSE),IFERROR(INDEX(C:C,MATCH(B1,D:D,0)),\"none, really\"))";
        let p = pretty(f, EN).unwrap();
        assert_eq!(
            p,
            "=IF(\n    AND(A1>0,B1>0),\n    VLOOKUP(A1,Sheet2!$A$1:$D$500,4,FALSE),\n    IFERROR(\n        INDEX(C:C,MATCH(B1,D:D,0)),\n        \"none, really\"\n    )\n)"
        );
        // Back on one line, and the same formula either way.
        assert_eq!(compact(&p, EN).as_deref(), Some(f));
        assert_eq!(crate::parse(&p[1..]).ok(), crate::parse(&f[1..]).ok());
    }

    #[test]
    fn deep_nesting_indents_each_level() {
        let f = "=LET(total,SUM(Sales[Amount]),share,total/SUM(Sales[Amount],Returns[Amount],Other[Amount]),ROUND(share*100,2))";
        let p = pretty(f, EN).unwrap();
        assert_eq!(
            p,
            "=LET(\n    total, SUM(Sales[Amount]),\n    share, total/SUM(Sales[Amount],Returns[Amount],Other[Amount]),\n    ROUND(share*100,2)\n)"
        );
        assert_eq!(crate::parse(&p[1..]).ok(), crate::parse(&f[1..]).ok());
    }

    #[test]
    fn the_intersection_space_and_text_survive() {
        assert_eq!(compact("=SUM(A1:B2 B1:C3)", EN).as_deref(), Some("=SUM(A1:B2 B1:C3)"));
        assert_eq!(compact("= \"a  b\" & \" c\"", EN).as_deref(), Some("=\"a  b\"&\" c\""));
        // Array constants stay on one line.
        let long = format!("=SUM({{{}}})", (1..40).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
        let p = pretty(&long, EN).unwrap();
        assert_eq!(p.lines().count(), 3, "SUM breaks, its array doesn't: {p}");
    }

    #[test]
    fn unbalanced_or_unreadable_text_is_left_alone() {
        assert_eq!(pretty("=SUM(1,2", EN), None);
        assert_eq!(pretty("=1)+2", EN), None);
        assert_eq!(pretty("=\"open", EN), None);
        let deep = format!("={}1{}", "(".repeat(400), ")".repeat(400));
        assert_eq!(pretty(&deep, EN), None);
    }

    #[test]
    fn other_separators_are_kept() {
        let f = "=SI(Y(A1>0;B1>0);BUSCARV(A1;Hoja2!$A$1:$D$500;4;FALSO);\"ninguno\")";
        let p = pretty(f, FormulaLocale::Es).unwrap();
        assert!(p.contains(";\n"), "{p}");
        assert_eq!(compact(&p, FormulaLocale::Es).as_deref(), Some(f));
    }
}
