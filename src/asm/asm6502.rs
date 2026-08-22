//! MOS 6502 assembler: two-pass, case-insensitive, `;` comments, labels, and
//! `ORG` / `DB` / `DW` / `DD` / `EQU` / `END`. Operand forms select the
//! addressing mode: `#imm`, `zp`, `zp,X`, `zp,Y`, `(zp,X)`, `(zp),Y`, `abs`,
//! `abs,X`, `abs,Y`, `(abs)` (JMP indirect), and bare labels for branches.

use std::collections::HashMap;
use crate::asm::common::{clean_line, parse_expr, AsmErr, LineInfo, Stmt};

/// Determine (Mode, operand_value_or_label) from an operand string.
fn parse_mode(op: &str) -> Result<(u8, String), String> {
    // 0=IMP,1=IMM,2=ZP,3=ZPX,4=ZPY,5=IZX,6=IZY,7=ABS,8=ABX,9=ABY,10=IND,11=REL
    let s = op.trim();
    if s.is_empty() { return Ok((0, String::new())); }
    if let Some(im) = s.strip_prefix('#') {
        return Ok((1, im.trim().to_string()));
    }
    if let Some(rest) = s.strip_prefix('(') {
        let inner = rest.trim_end_matches(')');
        if let Some((z, idx)) = inner.split_once(',') {
            let idx = idx.trim().to_ascii_uppercase();
            if idx == "X" { return Ok((5, z.trim().to_string())); }
            if idx == "Y" { return Ok((6, z.trim().to_string())); }
            return Err(format!("bad indirect operand '{s}'"));
        }
        return Ok((10, inner.trim().to_string()));
    }
    if let Some((base, idx)) = s.split_once(',') {
        let idx = idx.trim().to_ascii_uppercase();
        let base = base.trim();
        return match idx.as_str() {
            "X" => Ok((3, base.to_string())),
            "Y" => Ok((4, base.to_string())),
            _ => Err(format!("bad indexed operand '{s}'")),
        };
    }
    // bare value: ZP if fits in one byte, else ABS
    let v = parse_expr(s, &HashMap::new(), 0, 0).ok();
    if let Some(val) = v {
        if val <= 0xFF { Ok((2, s.to_string())) } else { Ok((7, s.to_string())) }
    } else {
        // label: choose ABS by default (branch detection handled separately)
        Ok((7, s.to_string()))
    }
}

/// Split into (mnemonic, operands). For data directives we keep a
/// comma-separated list; for ordinary instructions the whole operand is kept
/// as a single string so indexed forms like `msg,X` are not split apart.
fn split_stmt_6502(line: &str) -> (String, Vec<String>) {
    let line = line.trim();
    if let Some(pos) = line.find(char::is_whitespace) {
        let mnem = line[..pos].trim().to_string();
        let rest = line[pos + 1..].trim();
        let up = mnem.to_ascii_uppercase();
        if up == "DB" || up == "DW" || up == "DD" || up == "DQ" {
            let items: Vec<String> = rest.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (mnem, items)
        } else if rest.is_empty() {
            (mnem, vec![])
        } else {
            (mnem, vec![rest.to_string()])
        }
    } else {
        (line.to_string(), vec![])
    }
}

fn opcode_for(mnem: &str, mode: u8) -> Option<u8> {
    let m = mode;
    Some(match (mnem, m) {
        ("LDA", 1) => 0xA9, ("LDA", 2) => 0xA5, ("LDA", 3) => 0xB5, ("LDA", 7) => 0xAD,
        ("LDA", 8) => 0xBD, ("LDA", 9) => 0xB9, ("LDA", 5) => 0xA1, ("LDA", 6) => 0xB1,
        ("LDX", 1) => 0xA2, ("LDX", 2) => 0xA6, ("LDX", 4) => 0xB6, ("LDX", 7) => 0xAE, ("LDX", 9) => 0xBE,
        ("LDY", 1) => 0xA0, ("LDY", 2) => 0xA4, ("LDY", 3) => 0xB4, ("LDY", 7) => 0xAC, ("LDY", 8) => 0xBC,
        ("STA", 2) => 0x85, ("STA", 3) => 0x95, ("STA", 7) => 0x8D, ("STA", 8) => 0x9D,
        ("STA", 9) => 0x99, ("STA", 5) => 0x81, ("STA", 6) => 0x91,
        ("STX", 2) => 0x86, ("STX", 4) => 0x96, ("STX", 7) => 0x8E,
        ("STY", 2) => 0x84, ("STY", 3) => 0x94, ("STY", 7) => 0x8C,
        ("ADC", 1) => 0x69, ("ADC", 2) => 0x65, ("ADC", 3) => 0x75, ("ADC", 7) => 0x6D,
        ("ADC", 8) => 0x7D, ("ADC", 9) => 0x79, ("ADC", 5) => 0x61, ("ADC", 6) => 0x71,
        ("SBC", 1) => 0xE9, ("SBC", 2) => 0xE5, ("SBC", 3) => 0xF5, ("SBC", 7) => 0xED,
        ("SBC", 8) => 0xFD, ("SBC", 9) => 0xF9, ("SBC", 5) => 0xE1, ("SBC", 6) => 0xF1,
        ("AND", 1) => 0x29, ("AND", 2) => 0x25, ("AND", 3) => 0x35, ("AND", 7) => 0x2D,
        ("AND", 8) => 0x3D, ("AND", 9) => 0x39, ("AND", 5) => 0x21, ("AND", 6) => 0x31,
        ("ORA", 1) => 0x09, ("ORA", 2) => 0x05, ("ORA", 3) => 0x15, ("ORA", 7) => 0x0D,
        ("ORA", 8) => 0x1D, ("ORA", 9) => 0x19, ("ORA", 5) => 0x01, ("ORA", 6) => 0x11,
        ("EOR", 1) => 0x49, ("EOR", 2) => 0x45, ("EOR", 3) => 0x55, ("EOR", 7) => 0x4D,
        ("EOR", 8) => 0x5D, ("EOR", 9) => 0x59, ("EOR", 5) => 0x41, ("EOR", 6) => 0x51,
        ("ASL", 0) => 0x0A, ("ASL", 2) => 0x06, ("ASL", 3) => 0x16, ("ASL", 7) => 0x0E, ("ASL", 8) => 0x1E,
        ("LSR", 0) => 0x4A, ("LSR", 2) => 0x46, ("LSR", 3) => 0x56, ("LSR", 7) => 0x4E, ("LSR", 8) => 0x5E,
        ("ROL", 0) => 0x2A, ("ROL", 2) => 0x26, ("ROL", 3) => 0x36, ("ROL", 7) => 0x2E, ("ROL", 8) => 0x3E,
        ("ROR", 0) => 0x6A, ("ROR", 2) => 0x66, ("ROR", 3) => 0x76, ("ROR", 7) => 0x6E, ("ROR", 8) => 0x7E,
        ("INC", 2) => 0xE6, ("INC", 3) => 0xF6, ("INC", 7) => 0xEE, ("INC", 8) => 0xFE,
        ("DEC", 2) => 0xC6, ("DEC", 3) => 0xD6, ("DEC", 7) => 0xCE, ("DEC", 8) => 0xDE,
        ("CMP", 1) => 0xC9, ("CMP", 2) => 0xC5, ("CMP", 3) => 0xD5, ("CMP", 7) => 0xCD,
        ("CMP", 8) => 0xDD, ("CMP", 9) => 0xD9, ("CMP", 5) => 0xC1, ("CMP", 6) => 0xD1,
        ("CPX", 1) => 0xE0, ("CPX", 2) => 0xE4, ("CPX", 7) => 0xEC,
        ("CPY", 1) => 0xC0, ("CPY", 2) => 0xC4, ("CPY", 7) => 0xCC,
        ("BIT", 2) => 0x24, ("BIT", 7) => 0x2C,
        ("JMP", 7) => 0x4C, ("JMP", 10) => 0x6C,
        ("JSR", 7) => 0x20,
        ("BCC", 11) => 0x90, ("BCS", 11) => 0xB0, ("BEQ", 11) => 0xF0, ("BNE", 11) => 0xD0,
        ("BMI", 11) => 0x30, ("BPL", 11) => 0x10, ("BVC", 11) => 0x50, ("BVS", 11) => 0x70,
        ("CLC", 0) => 0x18, ("SEC", 0) => 0x38, ("CLI", 0) => 0x58, ("SEI", 0) => 0x78,
        ("CLV", 0) => 0xB8, ("CLD", 0) => 0xD8, ("SED", 0) => 0xF8,
        ("TAX", 0) => 0xAA, ("TAY", 0) => 0xA8, ("TSX", 0) => 0xBA, ("TXA", 0) => 0x8A,
        ("TXS", 0) => 0x9A, ("TYA", 0) => 0x98,
        ("DEX", 0) => 0xCA, ("DEY", 0) => 0x88, ("INX", 0) => 0xE8, ("INY", 0) => 0xC8,
        ("PHA", 0) => 0x48, ("PHP", 0) => 0x08, ("PLA", 0) => 0x68, ("PLP", 0) => 0x28,
        ("RTS", 0) => 0x60, ("RTI", 0) => 0x40, ("NOP", 0) => 0xEA, ("BRK", 0) => 0x00,
        _ => return None,
    })
}

fn is_branch(mnem: &str) -> bool {
    matches!(mnem, "BCC" | "BCS" | "BEQ" | "BNE" | "BMI" | "BPL" | "BVC" | "BVS")
}

pub fn assemble(source: &str) -> (Vec<u8>, Vec<AsmErr>, Vec<LineInfo>) {
    let mut errs = Vec::new();
    let mut syms: HashMap<String, u32> = HashMap::new();
    let mut seq: Vec<(usize, Stmt)> = Vec::new();
    let mut origin: u32 = 0;
    let mut addr: u32 = 0;
    for (lineno, raw) in source.lines().enumerate() {
        let cleaned = clean_line(raw);
        if cleaned.is_empty() { continue; }
        // strip a leading label (e.g. "loop:" or "msg: DB ...")
        let cleaned = if let Some(idx) = cleaned.find(':') {
            let (lab, after) = cleaned.split_at(idx);
            let lab_ok = !lab.contains(char::is_whitespace)
                && lab.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
            if lab_ok {
                syms.insert(lab.trim().to_string(), addr);
                after[1..].trim().to_string()
            } else { cleaned.to_string() }
        } else { cleaned.to_string() };
        if cleaned.is_empty() { continue; }
        if let Some(rest) = cleaned.strip_suffix(':') {
            syms.insert(rest.trim().to_string(), addr);
            continue;
        }
        if let Some((name, expr)) = cleaned.split_once(" EQU ") {
            match parse_expr(expr.trim(), &syms, addr, origin) {
                Ok(v) => { syms.insert(name.trim().to_string(), v); }
                Err(e) => errs.push(AsmErr::new(lineno + 1, e)),
            }
            continue;
        }
        let (mnem, ops) = split_stmt_6502(&cleaned);
        let up = mnem.to_uppercase();
        let stmt = match up.as_str() {
            "ORG" => match parse_expr(&ops[0], &syms, addr, origin) { Ok(v) => Stmt::Org(v), Err(e) => { errs.push(AsmErr::new(lineno + 1, e)); Stmt::Ignore } },
            "DB" => Stmt::Db(ops.clone()),
            "DW" => Stmt::Dw(ops.clone()),
            "DD" => Stmt::Dd(ops.clone()),
            "END" => Stmt::End,
            "" => Stmt::Ignore,
            _ => Stmt::Instr { mnemonic: up, ops: ops.clone() },
        };
        match &stmt {
            Stmt::Org(v) => addr = *v,
            Stmt::Db(items) => { for _ in items { addr += 1; } }
            Stmt::Dw(items) => addr += items.len() as u32 * 2,
            Stmt::Dd(items) => addr += items.len() as u32 * 4,
            Stmt::Dq(items) => addr += items.len() as u32 * 8,
            Stmt::Instr { .. } => addr += enc_len(&stmt),
            Stmt::Equ(..) | Stmt::Ignore => {}
            Stmt::End => { seq.push((lineno + 1, stmt)); break; }
        }
        seq.push((lineno + 1, stmt));
    }
    // pass 2: emit
    addr = origin;
    let mut code = Vec::new();
    let mut info = Vec::new();
    let syms2 = syms.clone();
    for (ln, stmt) in &seq {
        match stmt {
            Stmt::Org(v) => { code.resize(*v as usize, 0); addr = *v; }
            Stmt::Equ(..) => {}
            Stmt::End => break,
            Stmt::Ignore => {}
            Stmt::Db(items) => {
                let start = addr;
                for it in items { if let Ok(v) = parse_expr(it, &syms2, addr, origin) { code.push(v as u8); addr += 1; } else { errs.push(AsmErr::new(*ln, format!("bad DB '{it}'"))); } }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Dw(items) => {
                let start = addr;
                for it in items { if let Ok(v) = parse_expr(it, &syms2, addr, origin) { code.extend_from_slice(&(v as u16).to_le_bytes()); addr += 2; } else { errs.push(AsmErr::new(*ln, format!("bad DW '{it}'"))); } }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Dd(items) => {
                let start = addr;
                for it in items { if let Ok(v) = parse_expr(it, &syms2, addr, origin) { code.extend_from_slice(&v.to_le_bytes()); addr += 4; } else { errs.push(AsmErr::new(*ln, format!("bad DD '{it}'"))); } }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Dq(items) => {
                let start = addr;
                for it in items {
                    let raw: [u8; 8] = if it.contains('.') || it.contains("e") || it.contains("E") {
                        match it.trim().parse::<f64>() { Ok(f) => f.to_le_bytes(), Err(e) => { errs.push(AsmErr::new(*ln, format!("bad float '{it}': {e}"))); continue; } }
                    } else {
                        match parse_expr(it, &syms2, addr, origin) { Ok(v) => (v as u64).to_le_bytes(), Err(e) => { errs.push(AsmErr::new(*ln, e)); continue; } }
                    };
                    code.extend_from_slice(&raw);
                    addr += 8;
                }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Instr { mnemonic, ops } => {
                let start = addr;
                match enc_instr(mnemonic, ops, &syms2, addr, origin) {
                    Ok(b) => { code.extend_from_slice(&b); addr += b.len() as u32; info.push(LineInfo { line: *ln as u32, addr: start, bytes: b }); }
                    Err(e) => errs.push(AsmErr::new(*ln, e)),
                }
            }
        }
    }
    (code, errs, info)
}

fn enc_len(stmt: &Stmt) -> u32 {
    if let Stmt::Instr { mnemonic, ops } = stmt {
        let op = ops.first().map(|s| s.as_str()).unwrap_or("");
        if is_branch(mnemonic) { 2 }
        else {
            let (mode, _) = match parse_mode(op) { Ok(x) => x, Err(_) => (0, String::new()) };
            if mode == 0 { 1 } else if mode == 1 || mode == 2 || mode == 3 || mode == 4 || mode == 5 || mode == 6 || mode == 11 { 2 } else { 3 }
        }
    } else { 0 }
}

fn enc_instr(mnem: &str, ops: &[String], syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<Vec<u8>, String> {
    let op = ops.first().map(|s| s.as_str()).unwrap_or("");
    if is_branch(mnem) {
        let target = parse_expr(op, syms, cur, origin)?;
        let off = (target as i32 - (cur as i32 + 2)) as i8;
        let oc = opcode_for(mnem, 11).ok_or_else(|| format!("bad branch {mnem}"))?;
        return Ok(vec![oc, off as u8]);
    }
    let (mode, operand) = parse_mode(op)?;
    let oc = opcode_for(mnem, mode).ok_or_else(|| format!("unsupported {mnem} with this operand"))?;
    let mut b = vec![oc];
    match mode {
        0 => {}
        1 | 2 | 3 | 4 | 5 | 6 | 11 => {
            let v = parse_expr(&operand, syms, cur, origin)?;
            b.push((v & 0xFF) as u8);
        }
        7 | 8 | 9 | 10 => {
            let v = parse_expr(&operand, syms, cur, origin)?;
            b.push((v & 0xFF) as u8);
            b.push(((v >> 8) & 0xFF) as u8);
        }
        _ => {}
    }
    Ok(b)
}
