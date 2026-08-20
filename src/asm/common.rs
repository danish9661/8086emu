//! Shared assembler infrastructure: tokenizing, numbers, expressions,
//! directives (ORG/DB/DW/EQU/END), per-ISA assembly drivers.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct AsmErr {
    pub line: usize,
    pub msg: String,
}

impl AsmErr {
    pub fn new(line: usize, msg: impl Into<String>) -> Self {
        AsmErr { line, msg: msg.into() }
    }
}

/// A single source line with the original text (uppercased, comment stripped).
pub struct Line {
    pub num: usize,
    pub text: String,
}

/// One parsed statement.
#[derive(Debug, Clone)]
pub enum Stmt {
    Org(u32),
    Db(Vec<String>),
    Dw(Vec<String>),
    Equ(String, String),
    End,
    Instr { mnemonic: String, ops: Vec<String> },
    Ignore,
}

pub struct Program {
    pub stmts: Vec<(usize, Stmt)>, // (source line number, statement)
}

pub fn clean_line(raw: &str) -> String {
    let s = if let Some(i) = raw.find(';') { &raw[..i] } else { raw };
    let mut out = String::with_capacity(s.len());
    let mut in_str = false;
    for c in s.trim().chars() {
        if c == '\'' { in_str = !in_str; }
        out.push(if in_str { c } else { c.to_ascii_uppercase() });
    }
    out
}

/// Split a line into mnemonic + operand strings.
pub fn split_stmt(line: &str) -> (String, Vec<String>) {
    let mut it = line.splitn(2, char::is_whitespace);
    let mnem = it.next().unwrap_or("").to_string();
    let rest = it.next().unwrap_or("").trim();
    let ops: Vec<String> = if rest.is_empty() {
        vec![]
    } else {
        split_operands(rest)
    };
    (mnem, ops)
}

/// Split on commas (top level only, respecting [] and '').
pub fn split_operands(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '\'' => { in_str = !in_str; cur.push(c); }
            '[' | '(' if !in_str => { depth += 1; cur.push(c); }
            ']' | ')' if !in_str => { depth -= 1; cur.push(c); }
            ',' if depth == 0 && !in_str => { out.push(cur.trim().to_string()); cur.clear(); }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() { out.push(cur.trim().to_string()); }
    out
}

/// Parse a single numeric literal: decimal, 0x.., trailing h/d/b/q/o, 'char'.
pub fn parse_number(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() { return None; }
    if s.starts_with('\'') && s.ends_with('\'') && s.len() == 3 {
        return Some(s.chars().nth(1)? as u32);
    }
    if let Some(hex) = s.strip_prefix("0X") {
        return u32::from_str_radix(hex, 16).ok();
    }
    let last = s.chars().last()?;
    match last {
        'H' | 'h' => return u32::from_str_radix(&s[..s.len() - 1], 16).ok(),
        'D' | 'd' => return s[..s.len() - 1].parse().ok(),
        'B' | 'b' => return u32::from_str_radix(&s[..s.len() - 1], 2).ok(),
        'O' | 'o' | 'Q' | 'q' => return u32::from_str_radix(&s[..s.len() - 1], 8).ok(),
        _ => {}
    }
    s.parse::<u32>().ok()
}

fn parse_term(t: &str, syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<u32, String> {
    let t = t.trim();
    if t.is_empty() { return Err("empty term".into()); }
    if let Some(n) = parse_number(t) { return Ok(n); }
    if t == "$" { return Ok(cur); }
    if t == "$$" { return Ok(origin); }
    if let Some(v) = syms.get(t) { return Ok(*v); }
    Err(format!("unknown symbol '{t}'"))
}

/// Evaluate `a+b-c` style expressions with labels, `$` and `$$`.
pub fn parse_expr(s: &str, syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<u32, String> {
    let s = s.trim();
    if s.is_empty() { return Err("empty expression".into()); }
    let mut value = 0u32;
    let mut op = '+';
    let mut term = String::new();
    for c in s.chars() {
        if c == '+' || c == '-' {
            let v = parse_term(&term, syms, cur, origin)?;
            value = match op {
                '+' => value.wrapping_add(v),
                _ => value.wrapping_sub(v),
            };
            op = c;
            term.clear();
        } else {
            term.push(c);
        }
    }
    let v = parse_term(&term, syms, cur, origin)?;
    value = match op {
        '+' => value.wrapping_add(v),
        _ => value.wrapping_sub(v),
    };
    Ok(value)
}

/// Split a DB/DW operand list ("'hi', 34, 0ABh").
pub fn split_data_items(s: &str) -> Vec<String> {
    split_operands(s)
}

/// First parse pass: turn lines into statements; also handle labels/EQU.
/// Returns statements, labels, and errors.
pub fn parse_program<F>(
    source: &str,
    _is_8086: bool,
    label_ok: F,
) -> (Vec<(usize, Stmt)>, Vec<AsmErr>)
where
    F: Fn(&str) -> bool,
{
    let mut stmts = Vec::new();
    let mut errs = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let ln = i + 1;
        let text = clean_line(raw);
        if text.is_empty() { continue; }
        // label: "name:" or "name EQU ..."
        let (label, rest) = if let Some(idx) = text.find(':') {
            let maybe = text[..idx].trim();
            if label_ok(maybe) && !maybe.is_empty() {
                (Some(maybe.to_string()), text[idx + 1..].trim().to_string())
            } else {
                (None, text.clone())
            }
        } else {
            (None, text.clone())
        };
        let mut pending_label = label;
        if rest.is_empty() {
            if let Some(l) = pending_label {
                stmts.push((ln, Stmt::Equ(l, "$".to_string())));
            }
            continue;
        }
        let (mnem, ops) = split_stmt(&rest);
        match mnem.as_str() {
            "ORG" => {
                if let Some(l) = pending_label.take() {
                    stmts.push((ln, Stmt::Equ(l, "$".to_string())));
                }
                if let Some(o) = ops.first() {
if let Some(v) = parse_number(o) {
                    stmts.push((ln, Stmt::Org(v)));
                } else {
                        stmts.push((ln, Stmt::Org(0)));
                        errs.push(AsmErr::new(ln, format!("bad ORG address '{o}'")));
                    }
                } else {
                    errs.push(AsmErr::new(ln, "ORG needs an address"));
                }
            }
            "DB" | "DW" => {
                if let Some(l) = pending_label.take() {
                    stmts.push((ln, Stmt::Equ(l, "$".to_string())));
                }
                let items = split_data_items(&ops.join(","));
                if mnem == "DB" { stmts.push((ln, Stmt::Db(items))); } else { stmts.push((ln, Stmt::Dw(items))); }
            }
            "EQU" => {
                if let Some(l) = pending_label {
                    if let Some(v) = ops.first() {
                        stmts.push((ln, Stmt::Equ(l, v.clone())));
                    } else {
                        errs.push(AsmErr::new(ln, "EQU needs a value"));
                    }
                } else {
                    errs.push(AsmErr::new(ln, "EQU without a label"));
                }
            }
            "END" => { stmts.push((ln, Stmt::End)); break; }
            _ if is_ignored_dir(&mnem) => {
                if let Some(l) = pending_label.take() {
                    stmts.push((ln, Stmt::Equ(l, "$".to_string())));
                }
                stmts.push((ln, Stmt::Ignore));
            }
            _ => {
                if let Some(l) = pending_label.take() {
                    stmts.push((ln, Stmt::Equ(l, "$".to_string())));
                }
                stmts.push((ln, Stmt::Instr { mnemonic: mnem, ops }));
            }
        }
    }
    (stmts, errs)
}

/// Directives we accept but ignore (they influence only tooling, not code).
fn is_ignored_dir(m: &str) -> bool {
    matches!(
        m,
        "NAME" | "MODEL" | "STACK" | "ASSUME" | "SEGMENT" | "ENDS" | "PROC" | "ENDP"
            | "MACRO" | "ENDM" | "PUBLIC" | "EXTRN" | "INCLUDE" | "DOSSEG" | ".MODEL"
            | ".CODE" | ".DATA" | ".STACK" | "BIT" | "DATA" | "CODE" | "XDATA"
    )
}

/// Compute symbol table from EQU statements (values that are constant).
pub fn equ_symbols(stmts: &[(usize, Stmt)]) -> HashMap<String, u32> {
    let mut syms = HashMap::new();
    for (_, s) in stmts {
        if let Stmt::Equ(name, expr) = s {
            // only direct constants; labels resolved later
            if let Some(n) = parse_number(expr) {
                syms.insert(name.clone(), n);
            }
        }
    }
    syms
}

/// All label names appearing in the program (EQU targets). Pass 1 uses these
/// as placeholders (value 0) so forward references assemble with stable sizes.
pub fn all_label_names(stmts: &[(usize, Stmt)]) -> Vec<String> {
    stmts
        .iter()
        .filter_map(|(_, s)| match s {
            Stmt::Equ(name, _) => Some(name.clone()),
            _ => None,
        })
        .collect()
}
