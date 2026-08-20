//! 8086 assembler — encodes the subset the 8086 core executes.

use super::common::*;
use std::collections::HashMap;

const R16: [&str; 8] = ["AX", "CX", "DX", "BX", "SP", "BP", "SI", "DI"];
const R8: [&str; 8] = ["AL", "CL", "DL", "BL", "AH", "CH", "DH", "BH"];

#[derive(Debug, Clone, PartialEq)]
enum Operand {
    Reg8(u8),
    Reg16(u8),
    Seg(u8),
    Mem { size: Option<bool>, base: Option<u8>, idx: Option<u8>, disp: i32, seg_ov: Option<u8> }, // size: None=auto, Some(true)=word
    Imm(u32),
    FarPtr(u16, u16),
}

impl Operand {
    fn word(&self) -> bool {
        match self {
            Operand::Reg8(_) => false,
            Operand::Reg16(_) | Operand::Seg(_) => true,
            Operand::Mem { size, .. } => size.unwrap_or(true),
            _ => true,
        }
    }
}

fn reg16(name: &str) -> Option<u8> { R16.iter().position(|r| *r == name).map(|i| i as u8) }
fn reg8(name: &str) -> Option<u8> { R8.iter().position(|r| *r == name).map(|i| i as u8) }
fn seg_reg(name: &str) -> Option<u8> {
    match name { "ES" => Some(0), "CS" => Some(1), "SS" => Some(2), "DS" => Some(3), _ => None }
}

/// Parse one operand (with symbols available).
fn parse_operand(
    s: &str,
    syms: &HashMap<String, u32>,
    cur: u32,
    origin: u32,
) -> Result<Operand, String> {
    let s = s.trim().to_string();
    let up = s.to_ascii_uppercase();
    let up = up.strip_prefix("OFFSET ").unwrap_or(&up).to_string();
    if let Some(r) = reg16(&up) { return Ok(Operand::Reg16(r)); }
    if let Some(r) = reg8(&up) { return Ok(Operand::Reg8(r)); }
    if let Some(r) = seg_reg(&up) { return Ok(Operand::Seg(r)); }
    if up == "BYTE PTR" || up == "WORD PTR" { return Err("size specifier needs a memory operand".into()); }

    // far pointer: seg:off
    if let Some((a, b)) = up.split_once(':') {
        if a.chars().all(|c| c.is_ascii_hexdigit() && !is_alpha(c)) && b.chars().all(|c| c.is_ascii_hexdigit() && !is_alpha(c)) {
            return Ok(Operand::FarPtr(
                u16::from_str_radix(a, 16).unwrap_or(0),
                u16::from_str_radix(b, 16).unwrap_or(0),
            ));
        }
    }

    // memory operand: optional size ptr, then [expr]
    let (size, rest) = if let Some(r) = up.strip_prefix("BYTE PTR") {
        (Some(false), r.trim())
    } else if let Some(r) = up.strip_prefix("WORD PTR") {
        (Some(true), r.trim())
    } else {
        (None, up.as_str())
    };
    if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        // segment override prefix like DS:[...]
        let mut seg_ov = None;
        let inner = if let Some((seg, rest)) = inner.split_once(":") {
            seg_ov = seg_reg(seg.trim());
            rest.trim()
        } else {
            inner
        };
        let terms = split_expr_terms(inner);
        let mut base: Option<u8> = None;
        let mut idx: Option<u8> = None;
        let mut disp = 0i32;
        for t in terms {
            if let Some(r) = reg16(t.trim()) {
                match r {
                    3 => { if base.is_none() { base = Some(3); } else if idx.is_none() { idx = Some(3); } else { return Err(format!("too many registers in [{inner}]")); } }
                    6 => { if idx.is_none() { idx = Some(6); } else if base.is_none() { base = Some(6); } else { return Err(format!("too many registers in [{inner}]")); } }
                    5 => { if base.is_none() { base = Some(5); } else { return Err(format!("invalid addressing [{inner}]")); } }
                    7 => { if idx.is_none() { idx = Some(7); } else { return Err(format!("invalid addressing [{inner}]")); } }
                    _ => return Err(format!("invalid index/base register in [{inner}]")),
                }
            } else {
                let v = parse_expr(t.trim(), syms, cur, origin)? as i32;
                disp += v;
            }
        }
        if base.is_none() && idx.is_none() {
            base = None;
            idx = None;
        }
        return Ok(Operand::Mem { size, base, idx, disp, seg_ov });
    }
    // immediate
    let v = parse_expr(&up, syms, cur, origin)?;
    Ok(Operand::Imm(v))
}

fn split_expr_terms(s: &str) -> Vec<String> {
    // split on + and - preserving sign
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut neg = false;
    for c in s.chars() {
        match c {
            '+' => { out.push(if neg { format!("-{cur}") } else { cur.clone() }); cur.clear(); neg = false; }
            '-' => { out.push(if neg { format!("-{cur}") } else { cur.clone() }); cur.clear(); neg = true; }
            _ => cur.push(c),
        }
    }
    out.push(if neg { format!("-{cur}") } else { cur.clone() });
    out
}

fn is_alpha(c: char) -> bool { c.is_ascii_alphabetic() }

fn modrm_byte(mod_: u8, reg: u8, rm: u8) -> u8 { (mod_ << 6) | (reg << 3) | rm }

/// Encode a memory operand into modrm + displacement bytes.
fn encode_mem(op: &Operand, reg: u8) -> Result<Vec<u8>, String> {
    let Operand::Mem { base, idx, disp, .. } = op else { return Err("not memory".into()) };
    let mut out = Vec::new();
    match (base, idx) {
        (Some(b), Some(i)) => {
            // valid: BX/BP + SI/DI
            if !matches!(b, 3 | 5) || !matches!(i, 6 | 7) {
                return Err("invalid base+index combination".into());
            }
            let rm = match (b, i) { (3, 6) => 0, (3, 7) => 1, (5, 6) => 2, _ => 3 };
            emit_modrm(&mut out, *disp, modrm_byte(0, reg, rm));
        }
        (Some(b), None) => {
            let rm = match b { 3 => 7, 5 => 6, 6 => 4, _ => 5 };
            emit_modrm(&mut out, *disp, modrm_byte(0, reg, rm));
        }
        (None, Some(i)) => {
            let rm = match i { 6 => 4, _ => 5 };
            emit_modrm(&mut out, *disp, modrm_byte(0, reg, rm));
        }
        (None, None) => {
            // [disp16]
            out.push(modrm_byte(0, reg, 6));
            out.extend_from_slice(&((*disp as u16).to_le_bytes()));
        }
    }
    Ok(out)
}

fn emit_modrm(out: &mut Vec<u8>, disp: i32, base_byte: u8) {
    if disp == 0 {
        out.push(base_byte & 0xC0 | base_byte & 0x3F);
    } else if (-128..=127).contains(&disp) {
        out.push(0x40 | (base_byte & 0x3F));
        out.push(disp as i8 as u8);
    } else {
        out.push(0x80 | (base_byte & 0x3F));
        out.extend_from_slice(&(disp as u16).to_le_bytes());
    }
}

fn seg_prefix(seg: u8) -> Option<u8> {
    match seg { 0 => Some(0x26), 1 => Some(0x2E), 2 => Some(0x36), _ => Some(0x3E) }
}

/// Encode one instruction. `cur` is the address of this instruction,
/// `syms` the symbol table. Returns bytes.
fn encode_instr(
    mnemonic: &str,
    ops: &[String],
    syms: &HashMap<String, u32>,
    cur: u32,
    origin: u32,
) -> Result<Vec<u8>, String> {
    let mut o = Vec::new();
    if matches!(mnemonic, "REP" | "REPE" | "REPZ" | "REPNE" | "REPNZ") {
        if ops.is_empty() { return Err("REP needs an instruction".into()); }
        let p = match mnemonic { "REP" | "REPE" | "REPZ" => 0xF3, _ => 0xF2 };
        o.push(p);
        let inner = encode_instr(&ops[0], &ops[1..], syms, cur + 1, origin)?;
        o.extend(inner);
        return Ok(o);
    }
    let parsed: Vec<Operand> = ops
        .iter()
        .map(|p| parse_operand(p, syms, cur, origin))
        .collect::<Result<_, _>>()?;

    let a = parsed.first();
    let b = parsed.get(1);
    let seg_ov_bytes = |o: &Operand| -> Vec<u8> {
        if let Operand::Mem { seg_ov: Some(s), .. } = o {
            if let Some(p) = seg_prefix(*s) { return vec![p]; }
        }
        vec![]
    };

    macro_rules! memcode {
        ($op:expr, $reg:expr) => {{
            let op = &$op;
            let mut pre = seg_ov_bytes(op);
            let body = encode_mem(op, $reg)?;
            pre.extend(body);
            pre
        }};
    }

    match mnemonic {
        // ---------------- MOV ----------------
        "MOV" => {
            let (d, s) = (a, b);
            match (d, s) {
                (Some(Operand::Reg16(r)), Some(Operand::Reg16(sr))) if *r != *sr => {
                    o.push(0x8B); o.push(modrm_byte(3, *r, *sr));
                }
                (Some(Operand::Reg8(r)), Some(Operand::Reg8(sr))) => {
                    o.push(0x8A); o.push(modrm_byte(3, *r, *sr));
                }
                (Some(Operand::Reg16(r)), Some(Operand::Imm(v))) => {
                    if *r == 0 { o.push(0xB8); } else { o.push(0xB8 + *r); }
                    o.extend_from_slice(&(*v as u16).to_le_bytes());
                }
                (Some(Operand::Reg8(r)), Some(Operand::Imm(v))) => {
                    if *r == 0 { o.push(0xB0); } else { o.push(0xB0 + *r); }
                    o.push(*v as u8);
                }
                (Some(Operand::Seg(s)), Some(Operand::Reg16(r))) => {
                    o.push(0x8E); o.push(modrm_byte(3, *s, *r));
                }
                (Some(Operand::Seg(s)), Some(m @ Operand::Mem { .. })) => {
                    o.push(0x8E); o.extend(memcode!(m, *s));
                }
                (Some(Operand::Reg16(r)), Some(Operand::Seg(s))) => {
                    o.push(0x8C); o.push(modrm_byte(3, *s, *r));
                }
                (Some(m @ Operand::Mem { .. }), Some(Operand::Seg(s))) => {
                    o.push(0x8C); o.extend(memcode!(m, *s));
                }
                (Some(m @ Operand::Mem { .. }), Some(Operand::Reg16(r))) => {
                    o.push(0x89); o.extend(memcode!(m, *r));
                }
                (Some(m @ Operand::Mem { .. }), Some(Operand::Reg8(r))) => {
                    o.push(0x88); o.extend(memcode!(m, *r));
                }
                (Some(Operand::Reg16(r)), Some(m @ Operand::Mem { .. })) => {
                    o.push(0x8B); o.extend(memcode!(m, *r));
                }
                (Some(Operand::Reg8(r)), Some(m @ Operand::Mem { .. })) => {
                    o.push(0x8A); o.extend(memcode!(m, *r));
                }
                (Some(m @ Operand::Mem { .. }), Some(Operand::Imm(v))) => {
                    let word = m.word();
                    o.push(if word { 0xC7 } else { 0xC6 });
                    o.extend(memcode!(m, 0));
                    if word { o.extend_from_slice(&(*v as u16).to_le_bytes()); } else { o.push(*v as u8); }
                }
                (Some(Operand::Imm(_)), Some(Operand::Imm(_))) | (None, _) | (Some(_), None) => {
                    return Err("MOV: bad operands".into());
                }
                _ => {
                    // MOV AX, moffs etc. handled generically
                    if let (Some(Operand::Reg16(r)), Some(m @ Operand::Mem { .. })) = (d, s) {
                        let _ = (r, m);
                    }
                    return Err(format!("MOV: unsupported operands: {ops:?}"));
                }
            }
        }
        // ---------------- PUSH / POP ----------------
        "PUSH" => match a {
            Some(Operand::Reg16(r)) => o.push(0x50 + *r),
            Some(Operand::Seg(s)) => o.push(match s { 0 => 0x06, 1 => 0x0E, 2 => 0x16, _ => 0x1E }),
            Some(Operand::Imm(v)) => {
                if *v <= 0xFF && (*v as i8 as u32) == *v {
                    o.push(0x6A); o.push(*v as u8);
                } else {
                    o.push(0x68); o.extend_from_slice(&(*v as u16).to_le_bytes());
                }
            }
            Some(m @ Operand::Mem { .. }) => { o.push(0xFF); o.extend(memcode!(m, 6)); }
            _ => return Err("PUSH: bad operand".into()),
        },
        "POP" => match a {
            Some(Operand::Reg16(r)) => o.push(0x58 + *r),
            Some(Operand::Seg(s)) => o.push(match s { 0 => 0x07, 1 => 0x0F, 2 => 0x17, _ => 0x1F }),
            Some(m @ Operand::Mem { .. }) => { o.push(0x8F); o.extend(memcode!(m, 0)); }
            _ => return Err("POP: bad operand".into()),
        },
        // ---------------- arithmetic group ----------------
        "ADD" | "ADC" | "SUB" | "SBB" | "AND" | "OR" | "XOR" | "CMP" => {
            let base = match mnemonic {
                "ADD" => 0x00, "OR" => 0x08, "ADC" => 0x10, "SBB" => 0x18,
                "AND" => 0x20, "SUB" => 0x28, "XOR" => 0x30, _ => 0x38,
            };
            let imm_ext = match mnemonic {
                "ADD" => 0, "OR" => 1, "ADC" => 2, "SBB" => 3, "AND" => 4, "SUB" => 5, "XOR" => 6, _ => 7,
            };
            let (d, s) = (a, b);
            // accumulator immediate form
            if let (Some(Operand::Reg8(0)), Some(Operand::Imm(v))) = (d, s) {
                o.push(base + 4); o.push(*v as u8);
                return Ok(o);
            }
            if let (Some(Operand::Reg16(0)), Some(Operand::Imm(v))) = (d, s) {
                o.push(base + 5); o.extend_from_slice(&(*v as u16).to_le_bytes());
                return Ok(o);
            }
            match (d, s) {
                (Some(m @ Operand::Mem { .. }), Some(Operand::Reg16(r))) => {
                    o.push(base + 1); o.extend(memcode!(m, *r));
                }
                (Some(m @ Operand::Mem { .. }), Some(Operand::Reg8(r))) => {
                    o.push(base); o.extend(memcode!(m, *r));
                }
                (Some(Operand::Reg16(r)), Some(m @ Operand::Mem { .. })) => {
                    o.push(base + 3); o.extend(memcode!(m, *r));
                }
                (Some(Operand::Reg8(r)), Some(m @ Operand::Mem { .. })) => {
                    o.push(base + 2); o.extend(memcode!(m, *r));
                }
                (Some(Operand::Reg16(r)), Some(Operand::Reg16(sr))) => {
                    o.push(base + 1); o.push(modrm_byte(3, *r, *sr));
                }
                (Some(Operand::Reg8(r)), Some(Operand::Reg8(sr))) => {
                    o.push(base); o.push(modrm_byte(3, *r, *sr));
                }
                (Some(m @ Operand::Mem { .. }), Some(Operand::Imm(v))) => {
                    let word = m.word();
                    let op_byte = if word { 0x81 } else { 0x80 };
                    let fits8 = (*v as i8 as u32) == *v || !word && *v <= 0xFF;
                    let b = if word && fits8 { 0x83 } else { op_byte };
                    o.push(b);
                    o.extend(memcode!(m, imm_ext));
                    if b == 0x83 { o.push(*v as u8); }
                    else if word { o.extend_from_slice(&(*v as u16).to_le_bytes()); }
                    else { o.push(*v as u8); }
                }
                (Some(Operand::Reg16(r)), Some(Operand::Imm(v))) => {
                    let fits8 = (*v as i8 as u32) == *v;
                    if fits8 { o.push(0x83); } else { o.push(0x81); }
                    o.push(modrm_byte(3, imm_ext, *r));
                    if fits8 { o.push(*v as u8); } else { o.extend_from_slice(&(*v as u16).to_le_bytes()); }
                }
                (Some(Operand::Reg8(r)), Some(Operand::Imm(v))) => {
                    o.push(0x80); o.push(modrm_byte(3, imm_ext, *r)); o.push(*v as u8);
                }
                _ => return Err(format!("{mnemonic}: bad operands")),
            }
        }
        // ---------------- TEST ----------------
        "TEST" => {
            let (d, s) = (a, b);
            if let (Some(Operand::Reg8(0)), Some(Operand::Imm(v))) = (d, s) {
                o.push(0xA8); o.push(*v as u8);
            } else if let (Some(Operand::Reg16(0)), Some(Operand::Imm(v))) = (d, s) {
                o.push(0xA9); o.extend_from_slice(&(*v as u16).to_le_bytes());
            } else if let (Some(Operand::Reg8(r)), Some(Operand::Reg8(sr))) = (d, s) {
                o.push(0x84); o.push(modrm_byte(3, *r, *sr));
            } else if let (Some(Operand::Reg16(r)), Some(Operand::Reg16(sr))) = (d, s) {
                o.push(0x85); o.push(modrm_byte(3, *r, *sr));
            } else if let (Some(m @ Operand::Mem { .. }), Some(Operand::Reg8(r))) = (d, s) {
                o.push(0x84); o.extend(memcode!(m, *r));
            } else if let (Some(m @ Operand::Mem { .. }), Some(Operand::Reg16(r))) = (d, s) {
                o.push(0x85); o.extend(memcode!(m, *r));
            } else if let (Some(m @ Operand::Mem { .. }), Some(Operand::Imm(v))) = (d, s) {
                let word = m.word();
                o.push(if word { 0xF7 } else { 0xF6 });
                o.extend(memcode!(m, 0));
                if word { o.extend_from_slice(&(*v as u16).to_le_bytes()); } else { o.push(*v as u8); }
            } else if let (Some(Operand::Reg16(r)), Some(Operand::Imm(v))) = (d, s) {
                o.push(0xF7); o.push(modrm_byte(3, 0, *r)); o.extend_from_slice(&(*v as u16).to_le_bytes());
            } else if let (Some(Operand::Reg8(r)), Some(Operand::Imm(v))) = (d, s) {
                o.push(0xF6); o.push(modrm_byte(3, 0, *r)); o.push(*v as u8);
            } else {
                return Err("TEST: bad operands".into());
            }
        }
        // ---------------- XCHG / LEA ----------------
        "XCHG" => {
            let (d, s) = (a, b);
            match (d, s) {
                (Some(Operand::Reg16(r)), Some(Operand::Reg16(0))) | (Some(Operand::Reg16(0)), Some(Operand::Reg16(r))) if *r != 0 => {
                    o.push(0x90 + *r);
                }
                (Some(Operand::Reg16(r)), Some(Operand::Reg16(sr))) => { o.push(0x87); o.push(modrm_byte(3, *r, *sr)); }
                (Some(Operand::Reg8(r)), Some(Operand::Reg8(sr))) => { o.push(0x86); o.push(modrm_byte(3, *r, *sr)); }
                (Some(m @ Operand::Mem { .. }), Some(Operand::Reg16(r))) => { o.push(0x87); o.extend(memcode!(m, *r)); }
                (Some(m @ Operand::Mem { .. }), Some(Operand::Reg8(r))) => { o.push(0x86); o.extend(memcode!(m, *r)); }
                (Some(Operand::Reg16(r)), Some(m @ Operand::Mem { .. })) => { o.push(0x87); o.extend(memcode!(m, *r)); }
                (Some(Operand::Reg8(r)), Some(m @ Operand::Mem { .. })) => { o.push(0x86); o.extend(memcode!(m, *r)); }
                _ => return Err("XCHG: bad operands".into()),
            }
        }
        "LEA" => {
            if let (Some(Operand::Reg16(r)), Some(m @ Operand::Mem { .. })) = (a, b) {
                o.push(0x8D); o.extend(memcode!(m, *r));
            } else {
                return Err("LEA: needs reg16, mem".into());
            }
        }
        // ---------------- INC / DEC / NEG / NOT / MUL / IMUL / DIV / IDIV ----------------
        "INC" => match a {
            Some(Operand::Reg16(r)) => o.push(0x40 + *r),
            Some(Operand::Reg8(r)) => { o.push(0xFE); o.push(modrm_byte(3, 0, *r)); }
            Some(m @ Operand::Mem { .. }) => {
                if m.word() { o.push(0xFF); o.extend(memcode!(m, 0)); }
                else { o.push(0xFE); o.extend(memcode!(m, 0)); }
            }
            _ => return Err("INC: bad operand".into()),
        },
        "DEC" => match a {
            Some(Operand::Reg16(r)) => o.push(0x48 + *r),
            Some(Operand::Reg8(r)) => { o.push(0xFE); o.push(modrm_byte(3, 1, *r)); }
            Some(m @ Operand::Mem { .. }) => {
                if m.word() { o.push(0xFF); o.extend(memcode!(m, 1)); }
                else { o.push(0xFE); o.extend(memcode!(m, 1)); }
            }
            _ => return Err("DEC: bad operand".into()),
        },
        "NEG" => match a {
            Some(m @ Operand::Mem { .. }) => { o.push(if m.word() { 0xF7 } else { 0xF6 }); o.extend(memcode!(m, 3)); }
            Some(Operand::Reg16(r)) => { o.push(0xF7); o.push(modrm_byte(3, 3, *r)); }
            Some(Operand::Reg8(r)) => { o.push(0xF6); o.push(modrm_byte(3, 3, *r)); }
            _ => return Err("NEG: bad operand".into()),
        },
        "NOT" => match a {
            Some(m @ Operand::Mem { .. }) => { o.push(if m.word() { 0xF7 } else { 0xF6 }); o.extend(memcode!(m, 2)); }
            Some(Operand::Reg16(r)) => { o.push(0xF7); o.push(modrm_byte(3, 2, *r)); }
            Some(Operand::Reg8(r)) => { o.push(0xF6); o.push(modrm_byte(3, 2, *r)); }
            _ => return Err("NOT: bad operand".into()),
        },
        "MUL" | "IMUL" | "DIV" | "IDIV" => {
            let ext = match mnemonic { "MUL" => 4, "IMUL" => 5, "DIV" => 6, _ => 7 };
            match a {
                Some(Operand::Reg16(r)) => { o.push(0xF7); o.push(modrm_byte(3, ext, *r)); }
                Some(Operand::Reg8(r)) => { o.push(0xF6); o.push(modrm_byte(3, ext, *r)); }
                Some(m @ Operand::Mem { .. }) => {
                    o.push(if m.word() { 0xF7 } else { 0xF6 });
                    o.extend(memcode!(m, ext));
                }
                _ => return Err(format!("{mnemonic}: bad operand")),
            }
        }
        // ---------------- shifts ----------------
        "ROL" | "ROR" | "RCL" | "RCR" | "SHL" | "SHR" | "SAR" => {
            let ext = match mnemonic { "ROL" => 0, "ROR" => 1, "RCL" => 2, "RCR" => 3, "SHL" => 4, "SHR" => 5, _ => 7 };
            let (target, count) = (a, b);
            let word = !matches!(target, Some(Operand::Reg8(_)) | Some(Operand::Mem { size: Some(false), .. }));
            let op_byte = match (count, word) {
                (Some(Operand::Imm(1)), _) => if word { 0xD1 } else { 0xD0 },
                (Some(Operand::Reg16(1)), _) => if word { 0xD1 } else { 0xD0 },
                (Some(Operand::Reg16(4)), _) => if word { 0xD3 } else { 0xD2 }, // CL
                (Some(Operand::Imm(n)), _) => {
                    o.push(if word { 0xC1 } else { 0xC0 });
                    if let Some(Operand::Reg16(r)) = target {
                        o.push(modrm_byte(3, ext, *r));
                    } else if let Some(m @ Operand::Mem { .. }) = target {
                        o.extend(memcode!(m, ext));
                    }
                    o.push(*n as u8);
                    return Ok(o);
                }
                _ => return Err("shift: bad count".into()),
            };
            o.push(op_byte);
            match target {
                Some(Operand::Reg16(r)) => o.push(modrm_byte(3, ext, *r)),
                Some(Operand::Reg8(r)) => o.push(modrm_byte(3, ext, *r)),
                Some(m @ Operand::Mem { .. }) => o.extend(memcode!(m, ext)),
                _ => return Err("shift: bad target".into()),
            }
        }
        // ---------------- jumps / calls ----------------
        "JMP" => match a {
            Some(Operand::FarPtr(seg, off)) => { o.push(0xEA); o.extend_from_slice(&off.to_le_bytes()); o.extend_from_slice(&seg.to_le_bytes()); }
            Some(Operand::Mem { .. }) => {
                let m = parsed[0].clone();
                let far = m.word() && matches!(ops.first().map(|s| s.to_ascii_uppercase().contains("FAR")), Some(true));
                let _ = far;
                o.push(0xFF); o.extend(memcode!(m, 4));
            }
            Some(Operand::Imm(target)) => {
                let disp = (*target as i32) - (cur as i32 + 2);
                if (-128..=127).contains(&disp) {
                    o.push(0xEB); o.push(disp as i8 as u8);
                } else {
                    let d = (*target as i32) - (cur as i32 + 3);
                    o.push(0xE9); o.extend_from_slice(&(d as i16).to_le_bytes());
                }
            }
            _ => return Err("JMP: bad operand".into()),
        },
        "CALL" => match a {
            Some(Operand::FarPtr(seg, off)) => { o.push(0x9A); o.extend_from_slice(&off.to_le_bytes()); o.extend_from_slice(&seg.to_le_bytes()); }
            Some(m @ Operand::Mem { .. }) => { o.push(0xFF); o.extend(memcode!(m, 2)); }
            Some(Operand::Reg16(r)) => { o.push(0xFF); o.push(modrm_byte(3, 2, *r)); }
            Some(Operand::Imm(target)) => {
                let d = (*target as i32) - (cur as i32 + 3);
                o.push(0xE8); o.extend_from_slice(&(d as i16).to_le_bytes());
            }
            _ => return Err("CALL: bad operand".into()),
        },
        "RET" => { o.push(0xC3); }
        "RETF" => { o.push(0xCB); }
        "IRET" => { o.push(0xCF); }
        "INT" => match a {
            Some(Operand::Imm(3)) => o.push(0xCC),
            Some(Operand::Imm(n)) => { o.push(0xCD); o.push(*n as u8); }
            _ => return Err("INT: bad operand".into()),
        },
        "INTO" => o.push(0xCE),
        "INT3" => o.push(0xCC),
        "LOOP" | "LOOPZ" | "LOOPE" | "LOOPNZ" | "LOOPNE" | "JCXZ" => {
            let op = match mnemonic { "LOOP" => 0xE2, "LOOPZ" | "LOOPE" => 0xE1, "LOOPNZ" | "LOOPNE" => 0xE0, _ => 0xE3 };
            match a {
                Some(Operand::Imm(target)) => {
                    let d = (*target as i32) - (cur as i32 + 2);
                    if !(-128..=127).contains(&d) { return Err(format!("{mnemonic}: target out of range")); }
                    o.push(op); o.push(d as i8 as u8);
                }
                _ => return Err(format!("{mnemonic}: bad operand")),
            }
        }
        // ---------------- conditional jumps ----------------
        "JO" | "JNO" | "JB" | "JNAE" | "JC" | "JAE" | "JNB" | "JNC" | "JE" | "JZ"
        | "JNE" | "JNZ" | "JBE" | "JNA" | "JA" | "JNBE" | "JS" | "JNS" | "JP" | "JPE"
        | "JNP" | "JPO" | "JL" | "JNGE" | "JGE" | "JNL" | "JLE" | "JNG" | "JG" | "JNLE" => {
            let code = match mnemonic {
                "JO" => 0x70, "JNO" => 0x71, "JB" | "JNAE" | "JC" => 0x72,
                "JAE" | "JNB" | "JNC" => 0x73, "JE" | "JZ" => 0x74, "JNE" | "JNZ" => 0x75,
                "JBE" | "JNA" => 0x76, "JA" | "JNBE" => 0x77, "JS" => 0x78, "JNS" => 0x79,
                "JP" | "JPE" => 0x7A, "JNP" | "JPO" => 0x7B, "JL" | "JNGE" => 0x7C,
                "JGE" | "JNL" => 0x7D, "JLE" | "JNG" => 0x7E, _ => 0x7F,
            };
            match a {
                Some(Operand::Imm(target)) => {
                    let d = (*target as i32) - (cur as i32 + 2);
                    if (-128..=127).contains(&d) {
                        o.push(code); o.push(d as i8 as u8);
                    } else {
                        let d2 = (*target as i32) - (cur as i32 + 6);
                        let ncode = code + 0x10;
                        o.push(0x0F); o.push(ncode); o.extend_from_slice(&(d2 as i16).to_le_bytes());
                    }
                }
                _ => return Err(format!("{mnemonic}: bad operand")),
            }
        }
        // ---------------- string ops ----------------
        "MOVSB" | "MOVSW" | "CMPSB" | "CMPSW" | "STOSB" | "STOSW" | "LODSB" | "LODSW"
        | "SCASB" | "SCASW" | "INSB" | "INSW" | "OUTSB" | "OUTSW" => {
            o.push(match mnemonic {
                "MOVSB" => 0xA4, "MOVSW" => 0xA5, "CMPSB" => 0xA6, "CMPSW" => 0xA7,
                "STOSB" => 0xAA, "STOSW" => 0xAB, "LODSB" => 0xAC, "LODSW" => 0xAD,
                "SCASB" => 0xAE, "SCASW" => 0xAF,
                "INSB" => 0x6C, "INSW" => 0x6D, "OUTSB" => 0x6E, _ => 0x6F,
            });
        }
        "BOUND" => {
            match (a, b) {
                (Some(Operand::Reg16(r)), Some(m @ Operand::Mem { .. })) => {
                    o.push(0x62);
                    o.extend(memcode!(m, *r));
                }
                _ => return Err("BOUND needs r16, m16".into()),
            }
        }
        "REP" | "REPE" | "REPZ" | "REPNE" | "REPNZ" => {
            unreachable!("REP handled before operand parsing")
        }
        // ---------------- flag ops / misc ----------------
        "CLC" => o.push(0xF8), "STC" => o.push(0xF9), "CMC" => o.push(0xF5),
        "CLI" => o.push(0xFA), "STI" => o.push(0xFB),
        "CLD" => o.push(0xFC), "STD" => o.push(0xFD),
        "LAHF" => o.push(0x9F), "SAHF" => o.push(0x9E),
        "CBW" => o.push(0x98), "CWD" => o.push(0x99),
        "NOP" => o.push(0x90),
        "HLT" => o.push(0xF4),
        "XLAT" => o.push(0xD7),
        "WAIT" => o.push(0x9B),
        _ => return Err(format!("unknown mnemonic '{mnemonic}'")),
    }
    Ok(o)
}

/// Assemble 8086 source. Two+ passes until label addresses stabilize.
pub fn assemble(source: &str) -> (Vec<u8>, Vec<AsmErr>) {
    let mut errs = Vec::new();
    let (stmts, parse_errs) = parse_program(source, true, |l| {
        l.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '@')
    });
    errs.extend(parse_errs);

    let mut syms = equ_symbols(&stmts);
    for n in all_label_names(&stmts) {
        syms.entry(n).or_insert(0);
    }
    let mut code = Vec::new();
    let origin = 0u32;

    // iterate until addresses stable; carry label values forward so
    // forward references resolve in later passes
    let mut prev_labels: HashMap<String, u32> = syms.clone();
    for _pass in 0..10 {
        let mut addr = origin;
        let mut cur_code = Vec::new();
        let mut labels: HashMap<String, u32> = prev_labels.clone();
        let mut line_err = false;
        for (ln, stmt) in &stmts {
            match stmt {
                Stmt::Org(a) => {
                    if *a < addr {
                        if _pass == 0 {
                            errs.push(AsmErr::new(*ln, format!("ORG {a} goes backwards (current address {addr})")));
                        }
                    } else {
                        cur_code.resize(*a as usize, 0);
                        addr = *a;
                    }
                }
                Stmt::Equ(name, expr) => {
                    if let Ok(v) = parse_expr(expr, &labels, addr, origin) {
                        labels.insert(name.clone(), v);
                    }
                }
                Stmt::End => break,
                Stmt::Ignore => {}
                Stmt::Db(items) => {
                    let mut n = 0;
                    for it in items {
                        if let Some(s) = string_literal(it) {
                            for c in s.bytes() { cur_code.push(c); addr += 1; n += 1; }
                            continue;
                        }
                        match parse_expr(it, &labels, addr, origin) {
                            Ok(v) => {
                                if v > 0xFF {
                                    errs.push(AsmErr::new(*ln, format!("DB value {v} out of range")));
                                    line_err = true;
                                } else {
                                    cur_code.push(v as u8); addr += 1; n += 1;
                                }
                            }
                            Err(e) => { errs.push(AsmErr::new(*ln, e)); line_err = true; }
                        }
                    }
                    let _ = n;
                }
                Stmt::Dw(items) => {
                    for it in items {
                        match parse_expr(it, &labels, addr, origin) {
                            Ok(v) => { cur_code.extend_from_slice(&(v as u16).to_le_bytes()); addr += 2; }
                            Err(e) => { errs.push(AsmErr::new(*ln, e)); line_err = true; }
                        }
                    }
                }
                Stmt::Instr { mnemonic, ops } => {
                    match encode_instr(mnemonic, ops, &labels, addr, origin) {
                        Ok(bytes) => {
                            addr += bytes.len() as u32;
                            cur_code.extend(bytes);
                        }
                        Err(e) => {
                            if line_err { /* dedupe */ }
                            errs.push(AsmErr::new(*ln, e));
                        }
                    }
                }
            }
        }
        if labels == prev_labels {
            code = cur_code;
            break;
        }
        prev_labels = labels;
        code = cur_code;
    }
    (code, errs)
}

fn string_literal(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.starts_with('\'') && s.ends_with('\'') { Some(&s[1..s.len() - 1]) } else { None }
}
