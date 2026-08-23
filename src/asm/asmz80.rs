//! Z80 assembler (two-pass). Supports ORG/DB/DW/DD/DQ/EQU/END and a broad
//! subset of Z80 mnemonics including IX/IY indexed and CB bit/rotate forms.

use crate::asm::common::{clean_line, parse_expr, AsmErr, LineInfo, Stmt};
use std::collections::HashMap;

#[derive(Clone)]
enum Operand {
    None,
    Reg(u8),
    Imm(u16),
    IndHL,
    IndIX(i8),
    IndIY(i8),
    IndNN(u16),
    C,
    Rp(u8),
    IX,
    IY,
    AF,
}

fn reg_idx(n: &str) -> Option<u8> {
    Some(match n.to_ascii_uppercase().as_str() {
        "B" => 0, "C" => 1, "D" => 2, "E" => 3, "H" => 4, "L" => 5, "A" => 7, _ => return None,
    })
}
fn cc_idx(n: &str) -> Option<u8> {
    Some(match n.to_ascii_uppercase().as_str() {
        "NZ" => 0, "Z" => 1, "NC" => 2, "C" => 3, "PO" => 4, "PE" => 5, "P" => 6, "M" => 7, _ => return None,
    })
}
fn bit_op(n: &str) -> Option<u8> {
    Some(match n.to_ascii_uppercase().as_str() {
        "RLC" => 0, "RRC" => 1, "RL" => 2, "RR" => 3, "SLA" => 4, "SRA" => 5, "SLL" => 6, "SRL" => 7, _ => return None,
    })
}
fn alu_g(n: &str) -> Option<u8> {
    Some(match n.to_ascii_uppercase().as_str() {
        "ADD" => 0, "ADC" => 1, "SUB" => 2, "SBC" => 3, "AND" => 4, "OR" => 5, "XOR" => 6, "CP" => 7, _ => return None,
    })
}
fn split2(s: &str) -> (&str, &str) {
    match s.find(',') { Some(i) => (&s[..i], s[i + 1..].trim()), None => (s, "") }
}
fn parse_disp(rest: &str) -> i8 {
    let rest = rest.trim();
    if rest.is_empty() { return 0; }
    let rest = if rest.starts_with('+') { &rest[1..] } else { rest };
    match parse_expr(rest, &HashMap::new(), 0, 0) { Ok(v) => v as i8, Err(_) => 0 }
}

fn parse_operand(s: &str) -> Result<Operand, String> {
    let s = s.trim();
    if s.is_empty() { return Ok(Operand::None); }
    if s.eq_ignore_ascii_case("IX") { return Ok(Operand::IX); }
    if s.eq_ignore_ascii_case("IY") { return Ok(Operand::IY); }
    if s.eq_ignore_ascii_case("HL") { return Ok(Operand::Rp(2)); }
    if s.eq_ignore_ascii_case("BC") { return Ok(Operand::Rp(0)); }
    if s.eq_ignore_ascii_case("DE") { return Ok(Operand::Rp(1)); }
    if s.eq_ignore_ascii_case("SP") { return Ok(Operand::Rp(3)); }
    if s.eq_ignore_ascii_case("AF") { return Ok(Operand::AF); }
    if let Some(r) = reg_idx(s) { return Ok(Operand::Reg(r)); }
    if s == "(HL)" { return Ok(Operand::IndHL); }
    if s == "(C)" || s == "(c)" { return Ok(Operand::C); }
    if s.starts_with('(') {
        let inner = s[1..s.len() - 1].trim();
        if let Some(rest) = inner.strip_prefix("IX") { return Ok(Operand::IndIX(parse_disp(rest))); }
        if let Some(rest) = inner.strip_prefix("IY") { return Ok(Operand::IndIY(parse_disp(rest))); }
        let v = parse_expr(inner, &HashMap::new(), 0, 0).map_err(|e| e)?;
        return Ok(Operand::IndNN(v as u16));
    }
    if let Some(im) = s.strip_prefix('#') {
        let v = parse_expr(im, &HashMap::new(), 0, 0).map_err(|e| e)?;
        return Ok(Operand::Imm(v as u16));
    }
    let v = parse_expr(s, &HashMap::new(), 0, 0).map_err(|e| e)?;
    Ok(Operand::Imm(v as u16))
}

// __Z80ASM__

fn encode(mnem: &str, operand: &str, syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<Vec<u8>, String> {
    let m = mnem.to_ascii_uppercase();
    match m.as_str() {
        "NOP" => Ok(vec![0x00]), "HALT" => Ok(vec![0x76]), "DI" => Ok(vec![0xF3]), "EI" => Ok(vec![0xFB]),
        "CPL" => Ok(vec![0x2F]), "SCF" => Ok(vec![0x37]), "CCF" => Ok(vec![0x3F]), "DAA" => Ok(vec![0x27]),
        "RLCA" => Ok(vec![0x07]), "RRCA" => Ok(vec![0x0F]), "RLA" => Ok(vec![0x17]), "RRA" => Ok(vec![0x1F]),
        "EX" => {
            let (a, b) = split2(operand);
            if a.eq_ignore_ascii_case("DE") && b.eq_ignore_ascii_case("HL") { Ok(vec![0xEB]) }
            else if a.eq_ignore_ascii_case("AF") && b.eq_ignore_ascii_case("AF'") { Ok(vec![0x08]) }
            else if a.eq_ignore_ascii_case("(SP)") && b.eq_ignore_ascii_case("HL") { Ok(vec![0xE3]) }
            else { Err("bad EX".into()) }
        }
        "LD" => encode_ld(operand, syms, cur, origin),
        "PUSH" => {
            let up = operand.trim().to_ascii_uppercase();
            let (pre, base) = match up.as_str() {
                "BC" => (0u8, 0xC5u8), "DE" => (0, 0xD5), "HL" => (0, 0xE5), "AF" => (0, 0xF5),
                "IX" => (0xDD, 0xE5), "IY" => (0xFD, 0xE5), _ => return Err("bad PUSH".into()),
            };
            if pre != 0 { Ok(vec![pre, base]) } else { Ok(vec![base]) }
        }
        "POP" => {
            let up = operand.trim().to_ascii_uppercase();
            let (pre, base) = match up.as_str() {
                "BC" => (0u8, 0xC1u8), "DE" => (0, 0xD1), "HL" => (0, 0xE1), "AF" => (0, 0xF1),
                "IX" => (0xDD, 0xE1), "IY" => (0xFD, 0xE1), _ => return Err("bad POP".into()),
            };
            if pre != 0 { Ok(vec![pre, base]) } else { Ok(vec![base]) }
        }
        "INC" => encode_incdec(operand, true),
        "DEC" => encode_incdec(operand, false),
        "ADD" | "ADC" | "SUB" | "SBC" | "AND" | "OR" | "XOR" | "CP" => {
            let g = alu_g(&m).unwrap();
            let up = operand.trim_start().to_ascii_uppercase();
            if up.starts_with("HL") || up.starts_with("IX") || up.starts_with("IY") { return encode_add16(operand, &m); }
            let (dst, src) = split2(operand);
            if !dst.trim().eq_ignore_ascii_case("A") { return Err("ALU requires A".into()); }
            encode_alu(g, src.trim(), syms, cur, origin)
        }
        "JP" => encode_jp(operand, syms, cur, origin),
        "JR" => encode_jr(operand, syms, cur, origin),
        "DJNZ" => {
            let target = parse_expr(operand.trim(), syms, cur + 2, origin)?;
            let e = (target as i32 - (cur as i32 + 2)) as i8;
            Ok(vec![0x10, e as u8])
        }
        "CALL" => encode_call(operand, syms, cur, origin),
        "RST" => {
            let n = parse_expr(operand.trim(), syms, cur, origin)? as u8;
            if n % 8 != 0 || n > 0x38 { return Err("RST target must be 0,8,...,56".into()); }
            Ok(vec![0xC7 | (n & 0x38)])
        }
        "RET" => {
            if operand.trim().is_empty() { Ok(vec![0xC9]) }
            else if let Some(cc) = cc_idx(operand.trim()) { Ok(vec![0xC0 | (cc << 3)]) }
            else { Err("bad RET".into()) }
        }
        "RETI" => Ok(vec![0xED, 0x4D]), "RETN" => Ok(vec![0xED, 0x45]),
        "OUT" => {
            let inner = operand.split(',').next().unwrap_or("").trim();
            if let Some(n) = inner.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
                if n.trim().eq_ignore_ascii_case("C") { Ok(vec![0xED, 0x79]) }
                else { let v = parse_expr(n.trim(), syms, cur, origin)? as u8; Ok(vec![0xD3, v]) }
            } else { Err("bad OUT".into()) }
        }
        "IN" => {
            let (a, b) = split2(operand);
            if b.trim().eq_ignore_ascii_case("(C)") {
                if let Some(r) = reg_idx(a.trim()) { Ok(vec![0xED, 0x40 | (r << 3)]) } else { Err("bad IN".into()) }
            } else if b.starts_with('(') && b.ends_with(')') {
                let n = b[1..b.len() - 1].trim();
                let v = parse_expr(n, syms, cur, origin)? as u8;
                Ok(vec![0xDB, v])
            } else { Err("bad IN".into()) }
        }
        "RLC" | "RRC" | "RL" | "RR" | "SLA" | "SRA" | "SRL" => { let b = bit_op(&m).unwrap(); encode_cb_rot(b, operand, syms) }
        "BIT" | "RES" | "SET" => encode_bits(m.as_str(), operand, syms),
        "LDI" => Ok(vec![0xED, 0xA0]), "LDIR" => Ok(vec![0xED, 0xB0]), "LDD" => Ok(vec![0xED, 0xA8]), "LDDR" => Ok(vec![0xED, 0xB8]),
        "CPI" => Ok(vec![0xED, 0xA1]), "CPIR" => Ok(vec![0xED, 0xB1]), "NEG" => Ok(vec![0xED, 0x44]), "RLD" => Ok(vec![0xED, 0x67]), "RRD" => Ok(vec![0xED, 0x6F]),
        _ => Err(format!("unsupported Z80 mnemonic '{mnem}'")),
    }
}

fn encode_ld(operand: &str, _syms: &HashMap<String, u32>, _cur: u32, _origin: u32) -> Result<Vec<u8>, String> {
    let (dst, src) = split2(operand);
    let d = dst.trim(); let s = src.trim();
    let dv = parse_operand(d)?; let sv = parse_operand(s)?;
    if let (Operand::Reg(r), Operand::Imm(n)) = (&dv, &sv) {
        let base = match r { 0 => 0x06u8, 1 => 0x0E, 2 => 0x16, 3 => 0x1E, 4 => 0x26, 5 => 0x2E, 6 => 0x36, 7 => 0x3E, _ => 0 };
        return Ok(vec![base, *n as u8]);
    }
    if let (Operand::Reg(rd), Operand::Reg(rs)) = (&dv, &sv) { return Ok(vec![0x40 | (rd << 3) | rs]); }
    if let (Operand::Reg(rd), Operand::IndHL) = (&dv, &sv) { return Ok(vec![0x40 | (rd << 3) | 6]); }
    if let (Operand::IndHL, Operand::Reg(rs)) = (&dv, &sv) { return Ok(vec![0x70 | rs]); }
    if let Operand::Reg(7) = dv { if let Operand::IndNN(nn) = sv { return Ok(vec![0x3A, nn as u8, (nn >> 8) as u8]); } }
    if let Operand::IndNN(nn) = dv { if let Operand::Reg(7) = sv { return Ok(vec![0x32, nn as u8, (nn >> 8) as u8]); } }
    if let Operand::Reg(7) = dv {
        if let Operand::Rp(0) = sv { return Ok(vec![0x0A]); }
        if let Operand::Rp(1) = sv { return Ok(vec![0x1A]); }
    }
    if let Operand::Rp(0) = dv { if let Operand::Reg(7) = sv { return Ok(vec![0x02]); } }
    if let Operand::Rp(1) = dv { if let Operand::Reg(7) = sv { return Ok(vec![0x12]); } }
    if let Operand::Rp(p) = dv { if let Operand::Imm(nn) = sv { let base = match p { 0 => 0x01u8, 1 => 0x11, 2 => 0x21, 3 => 0x31, _ => 0 }; return Ok(vec![base, nn as u8, (nn >> 8) as u8]); } }
    if let Operand::Reg(7) = dv { if let Operand::Imm(n) = sv { return Ok(vec![0x3E, n as u8]); } }
    if d.eq_ignore_ascii_case("A") && s.eq_ignore_ascii_case("I") { return Ok(vec![0xED, 0x57]); }
    if d.eq_ignore_ascii_case("A") && s.eq_ignore_ascii_case("R") { return Ok(vec![0xED, 0x5F]); }
    if d.eq_ignore_ascii_case("I") && s.eq_ignore_ascii_case("A") { return Ok(vec![0xED, 0x47]); }
    if d.eq_ignore_ascii_case("R") && s.eq_ignore_ascii_case("A") { return Ok(vec![0xED, 0x4F]); }
    if d.eq_ignore_ascii_case("SP") && s.eq_ignore_ascii_case("HL") { return Ok(vec![0xF9]); }
    if let Operand::IndNN(nn) = dv { if let Operand::Rp(2) = sv { return Ok(vec![0x22, nn as u8, (nn >> 8) as u8]); } }
    if let Operand::Rp(2) = dv { if let Operand::IndNN(nn) = sv { return Ok(vec![0x2A, nn as u8, (nn >> 8) as u8]); } }
    if d.eq_ignore_ascii_case("IX") { if let Operand::Imm(nn) = sv { return Ok(vec![0xDD, 0x21, nn as u8, (nn >> 8) as u8]); } }
    if d.eq_ignore_ascii_case("IY") { if let Operand::Imm(nn) = sv { return Ok(vec![0xFD, 0x21, nn as u8, (nn >> 8) as u8]); } }
    if let Operand::IndNN(nn) = dv {
        if s.eq_ignore_ascii_case("IX") { return Ok(vec![0xDD, 0x22, nn as u8, (nn >> 8) as u8]); }
        if s.eq_ignore_ascii_case("IY") { return Ok(vec![0xFD, 0x22, nn as u8, (nn >> 8) as u8]); }
    }
    if let Operand::IX = dv { if let Operand::IndNN(nn) = sv { return Ok(vec![0xDD, 0x2A, nn as u8, (nn >> 8) as u8]); } }
    if let Operand::IY = dv { if let Operand::IndNN(nn) = sv { return Ok(vec![0xFD, 0x2A, nn as u8, (nn >> 8) as u8]); } }
    match (&dv, &sv) {
        (Operand::Reg(rd), Operand::IndIX(d)) => Ok(vec![0xDD, 0x40 | (*rd << 3) | 6, *d as u8]),
        (Operand::IndIX(d), Operand::Reg(rs)) => Ok(vec![0xDD, 0x70 | rs, *d as u8]),
        (Operand::IndIX(d), Operand::Imm(n)) => Ok(vec![0xDD, 0x36, *d as u8, *n as u8]),
        (Operand::Reg(rd), Operand::IndIY(d)) => Ok(vec![0xFD, 0x40 | (*rd << 3) | 6, *d as u8]),
        (Operand::IndIY(d), Operand::Reg(rs)) => Ok(vec![0xFD, 0x70 | rs, *d as u8]),
        (Operand::IndIY(d), Operand::Imm(n)) => Ok(vec![0xFD, 0x36, *d as u8, *n as u8]),
        _ => Err(format!("unsupported LD {operand}")),
    }
}

fn encode_incdec(operand: &str, inc: bool) -> Result<Vec<u8>, String> {
    let dv = parse_operand(operand.trim())?;
    let base = if inc { 0x04u8 } else { 0x05u8 };
    match dv {
        Operand::Reg(r) => Ok(vec![base | (r << 3)]),
        Operand::IndHL => Ok(vec![if inc { 0x34 } else { 0x35 }]),
        Operand::IndIX(d) => Ok(vec![0xDD, if inc { 0x34 } else { 0x35 }, d as u8]),
        Operand::IndIY(d) => Ok(vec![0xFD, if inc { 0x34 } else { 0x35 }, d as u8]),
        Operand::Rp(p) => {
            let b = match (p, inc) { (0, true) => 0x03, (0, false) => 0x0B, (1, true) => 0x13, (1, false) => 0x1B, (2, true) => 0x23, (2, false) => 0x2B, (3, true) => 0x33, (3, false) => 0x3B, _ => 0 };
            Ok(vec![b])
        }
        Operand::IX => Ok(vec![0xDD, if inc { 0x23 } else { 0x2B }]),
        Operand::IY => Ok(vec![0xFD, if inc { 0x23 } else { 0x2B }]),
        _ => Err("bad INC/DEC".into()),
    }
}

fn encode_add16(operand: &str, m: &str) -> Result<Vec<u8>, String> {
    let (a, b) = split2(operand);
    let a = a.trim(); let b = b.trim();
    let g = alu_g(m).unwrap();
    if g != 0 && g != 1 { return Err(format!("{m} not 16-bit here")); }
    let pre = if a.eq_ignore_ascii_case("IX") { Some(0xDD) } else if a.eq_ignore_ascii_case("IY") { Some(0xFD) } else { None };
    let p = if b.eq_ignore_ascii_case("BC") { 0 } else if b.eq_ignore_ascii_case("DE") { 1 } else if b.eq_ignore_ascii_case("HL") || b.eq_ignore_ascii_case("IX") || b.eq_ignore_ascii_case("IY") { 2 } else if b.eq_ignore_ascii_case("SP") { 3 } else { return Err("bad 16-bit add".into()) };
    let op = 0x09 | (p << 4);
    match pre { Some(px) => Ok(vec![px, op]), None => Ok(vec![op]) }
}

// __Z80ASM_APPEND__

fn encode_alu(g: u8, operand: &str, _syms: &HashMap<String, u32>, _cur: u32, _origin: u32) -> Result<Vec<u8>, String> {
    let op = parse_operand(operand)?;
    match op {
        Operand::Imm(n) => { let base = [0xC6u8, 0xCE, 0xD6, 0xDE, 0xE6, 0xEE, 0xF6, 0xFE][g as usize]; Ok(vec![base, n as u8]) }
        Operand::Reg(r) => Ok(vec![0x80 | (g << 3) | r]),
        Operand::IndHL => Ok(vec![0x80 | (g << 3) | 6]),
        Operand::IndIX(d) => Ok(vec![0xDD, 0x80 | (g << 3) | 6, d as u8]),
        Operand::IndIY(d) => Ok(vec![0xFD, 0x80 | (g << 3) | 6, d as u8]),
        _ => Err("bad ALU operand".into()),
    }
}

fn encode_jp(operand: &str, syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<Vec<u8>, String> {
    let operand = operand.trim();
    if operand.eq_ignore_ascii_case("(HL)") { return Ok(vec![0xE9]); }
    if operand.eq_ignore_ascii_case("(IX)") { return Ok(vec![0xDD, 0xE9]); }
    if operand.eq_ignore_ascii_case("(IY)") { return Ok(vec![0xFD, 0xE9]); }
    if let Some((cc, rest)) = operand.split_once(',') {
        let cc = cc.trim(); let rest = rest.trim();
        if let Some(c) = cc_idx(cc) {
            let nn = parse_expr(rest, syms, cur + 3, origin)?;
            return Ok(vec![0xC2 | (c << 3), nn as u8, (nn >> 8) as u8]);
        }
    }
    let nn = parse_expr(operand, syms, cur + 3, origin)?;
    Ok(vec![0xC3, nn as u8, (nn >> 8) as u8])
}

fn encode_jr(operand: &str, syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<Vec<u8>, String> {
    let operand = operand.trim();
    if let Some((cc, rest)) = operand.split_once(',') {
        let cc = cc.trim(); let rest = rest.trim();
        let base = match cc.to_ascii_uppercase().as_str() { "NZ" => 0x20, "Z" => 0x28, "NC" => 0x30, "C" => 0x38, _ => return Err("bad JR cc".into()) };
        let target = parse_expr(rest, syms, cur + 2, origin)?;
        let e = (target as i32 - (cur as i32 + 2)) as i8;
        return Ok(vec![base, e as u8]);
    }
    let target = parse_expr(operand, syms, cur + 2, origin)?;
    let e = (target as i32 - (cur as i32 + 2)) as i8;
    Ok(vec![0x18, e as u8])
}

fn encode_call(operand: &str, syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<Vec<u8>, String> {
    let operand = operand.trim();
    if let Some((cc, rest)) = operand.split_once(',') {
        let cc = cc.trim(); let rest = rest.trim();
        if let Some(c) = cc_idx(cc) {
            let nn = parse_expr(rest, syms, cur + 3, origin)?;
            return Ok(vec![0xC4 | (c << 3), nn as u8, (nn >> 8) as u8]);
        }
    }
    let nn = parse_expr(operand, syms, cur + 3, origin)?;
    Ok(vec![0xCD, nn as u8, (nn >> 8) as u8])
}

fn encode_cb_rot(b: u8, operand: &str, _syms: &HashMap<String, u32>) -> Result<Vec<u8>, String> {
    let op = parse_operand(operand)?;
    match op {
        Operand::Reg(r) => Ok(vec![0xCB, (b << 3) | r]),
        Operand::IndHL => Ok(vec![0xCB, (b << 3) | 6]),
        Operand::IndIX(d) => Ok(vec![0xDD, 0xCB, d as u8, (b << 3) | 6]),
        Operand::IndIY(d) => Ok(vec![0xFD, 0xCB, d as u8, (b << 3) | 6]),
        _ => Err("bad rotate operand".into()),
    }
}

fn encode_bits(m: &str, operand: &str, _syms: &HashMap<String, u32>) -> Result<Vec<u8>, String> {
    let (bit_s, rest) = split2(operand);
    let bit: u8 = bit_s.trim().parse().map_err(|_| "bad bit".to_string())?;
    let op = parse_operand(rest.trim())?;
    let top = match m { "BIT" => 0x40u8, "RES" => 0x80, "SET" => 0xC0, _ => 0 };
    match op {
        Operand::Reg(r) => Ok(vec![0xCB, top | (bit << 3) | r]),
        Operand::IndHL => Ok(vec![0xCB, top | (bit << 3) | 6]),
        Operand::IndIX(d) => Ok(vec![0xDD, 0xCB, d as u8, top | (bit << 3) | 6]),
        Operand::IndIY(d) => Ok(vec![0xFD, 0xCB, d as u8, top | (bit << 3) | 6]),
        _ => Err("bad bit operand".into()),
    }
}

fn split_stmt_z80(line: &str) -> (String, Vec<String>) {
    let line = line.trim();
    if let Some(pos) = line.find(char::is_whitespace) {
        let mnem = line[..pos].trim().to_string();
        let rest = line[pos + 1..].trim();
        let up = mnem.to_ascii_uppercase();
        if up == "DB" || up == "DW" || up == "DD" || up == "DQ" {
            let items: Vec<String> = rest.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
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

pub fn assemble(source: &str) -> (Vec<u8>, Vec<AsmErr>, Vec<LineInfo>) {
    let mut syms: HashMap<String, u32> = HashMap::new();
    let mut addr: u32 = 0;
    let mut seq: Vec<(usize, Stmt)> = Vec::new();
    let mut errs: Vec<AsmErr> = Vec::new();
    for (lineno, raw) in source.lines().enumerate() {
        let cleaned = clean_line(raw);
        if cleaned.is_empty() { continue; }
        let cleaned = if let Some(idx) = cleaned.find(':') {
            let (lab, after) = cleaned.split_at(idx);
            let lab_ok = !lab.contains(char::is_whitespace) && lab.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
            if lab_ok { syms.insert(lab.trim().to_string(), addr); after[1..].trim().to_string() } else { cleaned.to_string() }
        } else { cleaned.to_string() };
        if cleaned.is_empty() { continue; }
        if let Some(rest) = cleaned.strip_suffix(':') { syms.insert(rest.trim().to_string(), addr); continue; }
        if let Some((name, expr)) = cleaned.split_once(" EQU ") {
            match parse_expr(expr.trim(), &syms, addr, 0) { Ok(v) => { syms.insert(name.trim().to_string(), v); } Err(e) => errs.push(AsmErr::new(lineno + 1, e)) }
            continue;
        }
        let (mnem, ops) = split_stmt_z80(&cleaned);
        let up = mnem.to_ascii_uppercase();
        if up == "ORG" {
            match parse_expr(&ops.join(" "), &syms, addr, 0) {
                Ok(v) => { addr = v; seq.push((lineno + 1, Stmt::Org(v))); }
                Err(e) => errs.push(AsmErr::new(lineno + 1, e)),
            }
            continue;
        }
        if up == "END" { seq.push((lineno + 1, Stmt::End)); break; }
        let stmt = if up == "DB" { Stmt::Db(ops) }
            else if up == "DW" { Stmt::Dw(ops) }
            else if up == "DD" { Stmt::Dd(ops) }
            else if up == "DQ" { Stmt::Dq(ops) }
            else { Stmt::Instr { mnemonic: mnem, ops } };
        let len = match &stmt {
            Stmt::Db(items) => items.len() as u32,
            Stmt::Dw(items) => items.len() as u32 * 2,
            Stmt::Dd(items) => items.len() as u32 * 4,
            Stmt::Dq(items) => items.len() as u32 * 8,
            Stmt::Instr { mnemonic, ops } => match encode(mnemonic, &ops.concat(), &syms, addr, 0) { Ok(b) => b.len() as u32, Err(_) => 1 },
            _ => 0,
        };
        addr += len;
        seq.push((lineno + 1, stmt));
    }
    let mut code = Vec::new();
    let mut info = Vec::new();
    let syms2 = syms.clone();
    addr = 0;
    for (ln, stmt) in &seq {
        match stmt {
            Stmt::Org(v) => { code.resize(*v as usize, 0); addr = *v; }
            Stmt::Equ(..) => {}
            Stmt::End => break,
            Stmt::Ignore => {}
            Stmt::Db(items) => { let start = addr; for it in items { if let Ok(v) = parse_expr(it, &syms2, addr, 0) { code.push(v as u8); addr += 1; } else { errs.push(AsmErr::new(*ln, format!("bad DB '{it}'"))); } } info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() }); }
            Stmt::Dw(items) => { let start = addr; for it in items { if let Ok(v) = parse_expr(it, &syms2, addr, 0) { code.extend_from_slice(&(v as u16).to_le_bytes()); addr += 2; } else { errs.push(AsmErr::new(*ln, format!("bad DW '{it}'"))); } } info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() }); }
            Stmt::Dd(items) => { let start = addr; for it in items { if let Ok(v) = parse_expr(it, &syms2, addr, 0) { code.extend_from_slice(&v.to_le_bytes()); addr += 4; } else { errs.push(AsmErr::new(*ln, format!("bad DD '{it}'"))); } } info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() }); }
            Stmt::Dq(items) => { let start = addr; for it in items { let raw: [u8; 8] = if it.contains('.') || it.contains('e') || it.contains('E') { match it.trim().parse::<f64>() { Ok(f) => f.to_le_bytes(), Err(e) => { errs.push(AsmErr::new(*ln, format!("bad float '{it}': {e}"))); continue; } } } else { match parse_expr(it, &syms2, addr, 0) { Ok(v) => (v as u64).to_le_bytes(), Err(e) => { errs.push(AsmErr::new(*ln, e)); continue; } } }; code.extend_from_slice(&raw); addr += 8; } info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() }); }
            Stmt::Instr { mnemonic, ops } => {
                let operand = ops.concat();
                match encode(mnemonic, &operand, &syms2, addr, 0) {
                    Ok(b) => { let start = addr; code.extend_from_slice(&b); addr += b.len() as u32; info.push(LineInfo { line: *ln as u32, addr: start, bytes: b }); }
                    Err(e) => errs.push(AsmErr::new(*ln, e)),
                }
            }
        }
    }
    (code, errs, info)
}
