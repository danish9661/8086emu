//! 8051 (MCS-51) assembler.

use super::common::*;
use std::collections::HashMap;

const SFRS: [(&str, u8); 21] = [
    ("P0", 0x80), ("SP", 0x81), ("DPL", 0x82), ("DPH", 0x83), ("PCON", 0x87),
    ("TCON", 0x88), ("TMOD", 0x89), ("TL0", 0x8A), ("TL1", 0x8B), ("TH0", 0x8C),
    ("TH1", 0x8D), ("P1", 0x90), ("SCON", 0x98), ("SBUF", 0x99), ("P2", 0xA0),
    ("IE", 0xA8), ("P3", 0xB0), ("IP", 0xB8), ("PSW", 0xD0), ("ACC", 0xE0), ("B", 0xF0),
];

fn direct_addr(s: &str, syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<u8, String> {
    let up = s.trim().to_ascii_uppercase();
    if let Some((_, a)) = SFRS.iter().find(|(n, _)| *n == up) {
        return Ok(*a);
    }
    if up == "A" { return Ok(0xE0); }
    let v = parse_expr(s, syms, cur, origin)?;
    if v > 0xFF { return Err(format!("direct address {v} out of range")); }
    Ok(v as u8)
}

fn bit_addr(s: &str, syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<u8, String> {
    let up = s.trim().to_ascii_uppercase();
    // named port bits: P0.0..P3.7
    for (name, base) in SFRS.iter() {
        if !matches!(*name, "P0" | "P1" | "P2" | "P3") { continue; }
        if let Some(bit) = up.strip_prefix(name).and_then(|r| r.strip_prefix('.')) {
            let b = bit.parse::<u32>().map_err(|_| format!("bad bit '{bit}'"))?;
            if b > 7 { return Err("bit must be 0-7".into()); }
            return Ok(*base + b as u8);
        }
    }
    if up == "C" { return Ok(0xD7); } // PSW.7 carry
    if up == "OV" { return Ok(0xD2); }
    if up == "AC" { return Ok(0xD6); }
    if up == "P" { return Ok(0xD0); }
    // TCON bits (SFR 0x88): IT0 IE0 IT1 IE1 TR0 TF0 TR1 TF1
    match up.as_str() {
        "IT0" => return Ok(0x88),
        "IE0" => return Ok(0x89),
        "IT1" => return Ok(0x8A),
        "IE1" => return Ok(0x8B),
        "TR0" => return Ok(0x8C),
        "TF0" => return Ok(0x8D),
        "TR1" => return Ok(0x8E),
        "TF1" => return Ok(0x8F),
        // SCON bits (SFR 0x98): RI TI
        "RI" => return Ok(0x98),
        "TI" => return Ok(0x99),
        // IE bits (SFR 0xA8): EX0 ET0 EX1 ET1 ES EA
        "EX0" => return Ok(0xA8),
        "ET0" => return Ok(0xA9),
        "EX1" => return Ok(0xAA),
        "ET1" => return Ok(0xAB),
        "ES" => return Ok(0xAC),
        "EA" => return Ok(0xAF),
        // IP bits (SFR 0xB8): PX0 PT0 PX1 PT1 PS
        "PX0" => return Ok(0xB8),
        "PT0" => return Ok(0xB9),
        "PX1" => return Ok(0xBA),
        "PT1" => return Ok(0xBB),
        "PS" => return Ok(0xBC),
        _ => {}
    }
    let v = parse_expr(s, syms, cur, origin)?;
    if v > 0xFF { return Err(format!("bit address {v} out of range")); }
    Ok(v as u8)
}

fn enc(
    mnemonic: &str,
    ops: &[String],
    syms: &HashMap<String, u32>,
    cur: u32,
    origin: u32,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    // returns (bytes, patch list of (offset, target)) for rel jumps
    let mut o = Vec::new();
    let mut rel_patches: Vec<u8> = Vec::new(); // offsets within `o` needing 2-byte rel patch
    let imm = |s: &str| -> Result<u8, String> {
        let v = parse_expr(s, syms, cur, origin)?;
        if v > 0xFF { return Err(format!("value {v} out of byte range")); }
        Ok(v as u8)
    };
    let addr16 = |s: &str| -> Result<u16, String> {
        let v = parse_expr(s, syms, cur, origin)?;
        if v > 0xFFFF { return Err(format!("address {v} out of range")); }
        Ok(v as u16)
    };
    let direct = |s: &str| direct_addr(s, syms, cur, origin);
    let bit = |s: &str| bit_addr(s, syms, cur, origin);

    let is_rn = |s: &str| -> Option<u8> {
        if let Some(r) = s.strip_prefix('R') {
            let n = r.parse::<u8>().ok()?;
            if n < 8 { return Some(n); }
        }
        None
    };
    let is_ri = |s: &str| -> Option<u8> {
        if s == "@R0" { Some(0) } else if s == "@R1" { Some(1) } else { None }
    };

    let rel = |s: &str| -> Result<u8, String> {
        // called after the rel byte slot is placed: target resolved later
        let _ = parse_expr(s, syms, cur, origin)?; // validate symbol
        Ok(0)
    };

    match mnemonic {
        // ----- MOV -----
        "MOV" => {
            if ops.len() != 2 { return Err("MOV needs 2 operands".into()); }
            let (d, s) = (&ops[0], &ops[1]);
            let du = d.to_ascii_uppercase();
            let su = s.to_ascii_uppercase();
            match (du.as_str(), su.as_str()) {
                ("DPTR", _) if su.starts_with('#') => { // MOV DPTR,#imm16
                let v = parse_expr(&s[1..], syms, cur, origin)? as u16;
                o.push(0x90);
                o.push((v >> 8) as u8);
                o.push(v as u8);
            }
            ("A", _) if su.starts_with('R') => { o.push(0xE8 + is_rn(s).ok_or("bad Rn")?); }
                ("A", "@R0" | "@R1") => o.push(0xE6 + is_ri(s).unwrap()),
                ("A", _) if su.starts_with('#') => { o.push(0x74); o.push(imm(&s[1..])?); }
                ("A", _) => { o.push(0xE5); o.push(direct(s)?); }
                (_, "A") if du.starts_with('R') => { o.push(0xF8 + is_rn(d).ok_or("bad Rn")?); }
                (_, "A") if du.starts_with('@') => { o.push(0xF6 + is_ri(d).ok_or("bad @Ri")?); }
                (_, "A") => { o.push(0xF5); o.push(direct(d)?); }
                (_, _) if su.starts_with('#') && du.starts_with('R') => { o.push(0x78 + is_rn(d).ok_or("bad Rn")?); o.push(imm(&s[1..])?); }
                (_, _) if su.starts_with('#') && du.starts_with('@') => { o.push(0x76 + is_ri(d).ok_or("bad @Ri")?); o.push(imm(&s[1..])?); }
                (_, _) if su.starts_with('#') => { o.push(0x75); o.push(direct(d)?); o.push(imm(&s[1..])?); }
                (_, _) if du.starts_with('@') => { o.push(0xA6 + is_ri(d).ok_or("bad @Ri")?); o.push(direct(s)?); }
                (_, _) if du.starts_with('R') => { o.push(0xA8 + is_rn(d).ok_or("bad Rn")?); o.push(direct(s)?); }
                ("C", _) => { o.push(0xA2); o.push(bit(s)?); } // MOV C,bit
                (_, "C") => { o.push(0x92); o.push(bit(d)?); } // MOV bit,C
                (_, _) => { o.push(0x85); o.push(direct(s)?); o.push(direct(d)?); }
            }
        }
        "MOVC" => {
            let s = ops.join(",").to_ascii_uppercase();
            if s == "A,@A+DPTR" { o.push(0x93); }
            else if s == "A,@A+PC" { o.push(0x83); }
            else { return Err("MOVC needs A,@A+DPTR or A,@A+PC".into()); }
        }
        "MOVX" => {
            if ops.len() != 2 { return Err("MOVX needs 2 operands".into()); }
            let d = ops[0].to_ascii_uppercase();
            let s = ops[1].to_ascii_uppercase();
            match (d.as_str(), s.as_str()) {
                ("A", "@R0") => o.push(0xE2), ("A", "@R1") => o.push(0xE3),
                ("A", "@DPTR") => o.push(0xE0),
                ("@R0", "A") => o.push(0xF2), ("@R1", "A") => o.push(0xF3),
                ("@DPTR", "A") => o.push(0xF0),
                _ => return Err("MOVX: unsupported form".into()),
            }
        }
        "PUSH" => { o.push(0xC0); o.push(direct(&ops[0])?); }
        "POP" => { o.push(0xD0); o.push(direct(&ops[0])?); }
        "XCH" => {
            let d = ops[0].to_ascii_uppercase();
            if !d.starts_with('A') { return Err("XCH needs A".into()); }
            let s = ops[1].to_ascii_uppercase();
            if s.starts_with('R') { o.push(0xC8 + is_rn(&s).ok_or("bad Rn")?); }
            else if s.starts_with('@') { o.push(0xC6 + is_ri(&s).ok_or("bad @Ri")?); }
            else { o.push(0xC5); o.push(direct(&s)?); }
        }
        "XCHD" => {
            let s = ops[1].to_ascii_uppercase();
            o.push(0xD6 + is_ri(&s).ok_or("XCHD needs @Ri")?);
        }
        "SWAP" => o.push(0xC4),
        // ----- arithmetic -----
        "ADD" | "ADDC" | "SUBB" => {
            let base = match mnemonic { "ADD" => 0x20, "ADDC" => 0x30, _ => 0x90 };
            if ops.len() != 2 { return Err(format!("{mnemonic} needs 2 operands")); }
            let s = ops[1].to_ascii_uppercase();
            if s.starts_with('R') { o.push(base + 0x08 + is_rn(&s).ok_or("bad Rn")?); }
            else if s.starts_with('@') { o.push(base + 0x06 + is_ri(&s).ok_or("bad @Ri")?); }
            else if let Some(v) = s.strip_prefix('#') { o.push(base + 0x04); o.push(imm(v)?); }
            else { o.push(base + 0x05); o.push(direct(&s)?); }
        }
        "INC" => {
            let d = ops[0].to_ascii_uppercase();
            if d == "A" { o.push(0x04); }
            else if d == "DPTR" { o.push(0xA3); }
            else if d.starts_with('R') { o.push(0x08 + is_rn(&d).ok_or("bad Rn")?); }
            else if d.starts_with('@') { o.push(0x06 + is_ri(&d).ok_or("bad @Ri")?); }
            else { o.push(0x05); o.push(direct(&d)?); }
        }
        "DEC" => {
            let d = ops[0].to_ascii_uppercase();
            if d == "A" { o.push(0x14); }
            else if d.starts_with('R') { o.push(0x18 + is_rn(&d).ok_or("bad Rn")?); }
            else if d.starts_with('@') { o.push(0x16 + is_ri(&d).ok_or("bad @Ri")?); }
            else { o.push(0x15); o.push(direct(&d)?); }
        }
        "MUL" => o.push(0xA4), "DIV" => o.push(0x84), "DA" => o.push(0xD4),
        // ----- logical -----
        "ANL" | "ORL" | "XRL" => {
            let base = match mnemonic { "ANL" => 0x50, "ORL" => 0x40, _ => 0x60 };
            if ops.len() != 2 { return Err(format!("{mnemonic} needs 2 operands")); }
            let (d, s) = (&ops[0], &ops[1]);
            let du = d.to_ascii_uppercase();
            let su = s.to_ascii_uppercase();
            if su.starts_with('R') && du == "A" { o.push(base + 0x08 + is_rn(&su).ok_or("bad Rn")?); }
            else if su.starts_with('@') && du == "A" { o.push(base + 0x06 + is_ri(&su).ok_or("bad @Ri")?); }
            else if let Some(v) = su.strip_prefix('#') {
                if du == "A" { o.push(base + 0x04); o.push(imm(v)?); }
                else { o.push(base + 0x03); o.push(direct(d)?); o.push(imm(v)?); } // dir, #imm
            }
            else if su == "A" { if let Some(v) = du.strip_prefix('#') { o.push(base + 0x02); o.push(direct(v)?); } else { o.push(base + 0x02); o.push(direct(d)?); } }
            else if let Some(v) = su.strip_prefix('#') { o.push(base + 0x03); o.push(direct(d)?); o.push(imm(v)?); }
            else if du == "A" { o.push(base + 0x05); o.push(direct(s)?); }
            else if du == "C" {
                // ANL C,bit / ORL C,bit (XRL has no C form on the 8051)
                if mnemonic == "XRL" { return Err("XRL C,bit does not exist on the 8051".into()); }
                o.push(if mnemonic == "ANL" { 0x82 } else { 0x72 });
                o.push(bit_addr(s, syms, cur, origin)?);
            }
            else { return Err(format!("{mnemonic}: unsupported form")); }
        }
        "CLR" => {
            let d = ops[0].to_ascii_uppercase();
            if d == "A" { o.push(0xE4); } else if d == "C" { o.push(0xC3); } else { o.push(0xC2); o.push(bit(&d)?); }
        }
        "CPL" => {
            let d = ops[0].to_ascii_uppercase();
            if d == "A" { o.push(0xF4); } else if d == "C" { o.push(0xB3); } else { o.push(0xB2); o.push(bit(&d)?); }
        }
        "RL" => o.push(0x23), "RR" => o.push(0x03), "RLC" => o.push(0x33), "RRC" => o.push(0x13),
        // ----- bit ops -----
        "SETB" => {
            let d = ops[0].to_ascii_uppercase();
            if d == "C" { o.push(0xD3); } else { o.push(0xD2); o.push(bit(&d)?); }
        }
        "ANLC" => { let d = ops[0].to_ascii_uppercase();
            if let Some(r) = d.strip_prefix('/') { o.push(0xB0); o.push(bit(r)?); }
            else { o.push(0x82); o.push(bit(&d)?); }
        }
        "ORLC" => { let d = ops[0].to_ascii_uppercase();
            if let Some(r) = d.strip_prefix('/') { o.push(0xA0); o.push(bit(r)?); }
            else { o.push(0x72); o.push(bit(&d)?); }
        }
        // ----- branches -----
        "JMP" => {
            let s = ops.join(",").to_ascii_uppercase();
            if s == "@A+DPTR" { o.push(0x73); }
            else { return Err("JMP needs @A+DPTR".into()); }
        }
        "SJMP" => { o.push(0x80); rel_patches.push(o.len() as u8); o.push(rel(&ops[0])?); }
        "LJMP" => { o.push(0x02); o.extend_from_slice(&addr16(&ops[0])?.to_be_bytes()); }
        "AJMP" => {
            let t = addr16(&ops[0])?;
            let a11 = t & 0x7FF;
            o.push(0x01 | (((a11 >> 8) & 7) as u8) << 5);
            o.push(a11 as u8);
        }
        "JZ" => { o.push(0x60); rel_patches.push(o.len() as u8); o.push(rel(&ops[0])?); }
        "JNZ" => { o.push(0x70); rel_patches.push(o.len() as u8); o.push(rel(&ops[0])?); }
        "JC" => { o.push(0x40); rel_patches.push(o.len() as u8); o.push(rel(&ops[0])?); }
        "JNC" => { o.push(0x50); rel_patches.push(o.len() as u8); o.push(rel(&ops[0])?); }
        "JB" => { o.push(0x20); o.push(bit(&ops[0])?); rel_patches.push(o.len() as u8); o.push(rel(&ops[1])?); }
        "JNB" => { o.push(0x30); o.push(bit(&ops[0])?); rel_patches.push(o.len() as u8); o.push(rel(&ops[1])?); }
        "JBC" => { o.push(0x10); o.push(bit(&ops[0])?); rel_patches.push(o.len() as u8); o.push(rel(&ops[1])?); }
        "CJNE" => {
            if ops.len() != 3 { return Err("CJNE needs 3 operands".into()); }
            let (a, b, t) = (&ops[0], &ops[1], &ops[2]);
            let au = a.to_ascii_uppercase();
            let bu = b.to_ascii_uppercase();
            if au == "A" && bu.starts_with('#') { o.push(0xB4); o.push(imm(&bu[1..])?); }
            else if au == "A" { o.push(0xB5); o.push(direct(&bu)?); }
            else if au.starts_with('R') && bu.starts_with('#') { o.push(0xB8 + is_rn(&au).ok_or("bad Rn")?); o.push(imm(&bu[1..])?); }
            else if au.starts_with('@') && bu.starts_with('#') { o.push(0xB6 + is_ri(&au).ok_or("bad @Ri")?); o.push(imm(&bu[1..])?); }
            else { return Err("CJNE: unsupported form".into()); }
            rel_patches.push(o.len() as u8);
            o.push(rel(t)?);
        }
        "DJNZ" => {
            let (d, t) = (&ops[0], &ops[1]);
            let du = d.to_ascii_uppercase();
            if du.starts_with('R') { o.push(0xD8 + is_rn(&du).ok_or("bad Rn")?); }
            else { o.push(0xD5); o.push(direct(d)?); }
            rel_patches.push(o.len() as u8);
            o.push(rel(t)?);
        }
        "ACALL" => {
            let t = addr16(&ops[0])?;
            let a11 = t & 0x7FF;
            o.push(0x11 | (((a11 >> 8) & 7) as u8) << 5);
            o.push(a11 as u8);
        }
        "LCALL" => { o.push(0x12); o.extend_from_slice(&addr16(&ops[0])?.to_be_bytes()); }
        "RET" => o.push(0x22), "RETI" => o.push(0x32),
        "NOP" => o.push(0x00),
        _ => return Err(format!("unknown mnemonic '{mnemonic}'")),
    }
    Ok((o, rel_patches))
}

/// Assemble 8051 source.
pub fn assemble(source: &str) -> (Vec<u8>, Vec<AsmErr>, Vec<LineInfo>) {
    let mut errs = Vec::new();
    let (stmts, parse_errs) = parse_program(source, false, |l| {
        !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '@')
    });
    errs.extend(parse_errs);

    let mut syms = equ_symbols(&stmts);
    for n in all_label_names(&stmts) {
        syms.entry(n).or_insert(0);
    }
    let origin = 0u32;

    // pass 1: sizes + labels
    let mut addr = origin;
    for (ln, stmt) in &stmts {
        match stmt {
            Stmt::Org(a) => {
                if *a < addr {
                    errs.push(AsmErr::new(*ln, format!("ORG {a} goes backwards (current address {addr})")));
                } else {
                    addr = *a;
                }
            }
            Stmt::Equ(name, expr) => {
                if let Ok(v) = parse_expr(expr, &syms, addr, origin) {
                    syms.insert(name.clone(), v);
                }
            }
            Stmt::End => break,
            Stmt::Ignore => {}
            Stmt::Db(items) => {
                for it in items {
                    if let Some(s) = str_lit(it) {
                        addr += s.len() as u32;
                    } else if parse_expr(it, &syms, addr, origin).is_ok() {
                        addr += 1;
                    }
                }
            }
            Stmt::Dw(items) => addr += items.len() as u32 * 2,
            Stmt::Dq(items) => addr += items.len() as u32 * 8,
            Stmt::Instr { mnemonic, ops } => {
                match enc(mnemonic, ops, &syms, addr, origin) {
                    Ok((b, _)) => addr += b.len() as u32,
                    Err(e) => errs.push(AsmErr::new(*ln, e)),
                }
            }
        }
    }

    // pass 2: emit + patch rels
    let mut code = Vec::new();
    let mut info = Vec::new();
    addr = origin;
    let mut syms2 = syms.clone();
    for (ln, stmt) in &stmts {
        match stmt {
            Stmt::Org(a) => {
                if *a < addr {
                    errs.push(AsmErr::new(*ln, format!("ORG {a} goes backwards (current address {addr})")));
                } else {
                    code.resize(*a as usize, 0);
                    addr = *a;
                }
            }
            Stmt::Equ(name, expr) => {
                if let Ok(v) = parse_expr(expr, &syms2, addr, origin) {
                    syms2.insert(name.clone(), v);
                }
            }
            Stmt::End => break,
            Stmt::Ignore => {}
            Stmt::Db(items) => {
                let start = addr;
                for it in items {
                    if let Some(s) = str_lit(it) {
                        for c in s.bytes() { code.push(c); addr += 1; }
                    } else {
                        match parse_expr(it, &syms2, addr, origin) {
                            Ok(v) => { code.push(v as u8); addr += 1; }
                            Err(e) => errs.push(AsmErr::new(*ln, e)),
                        }
                    }
                }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Dw(items) => {
                let start = addr;
                for it in items {
                    match parse_expr(it, &syms2, addr, origin) {
                        Ok(v) => { code.extend_from_slice(&(v as u16).to_be_bytes()); addr += 2; }
                        Err(e) => errs.push(AsmErr::new(*ln, e)),
                    }
                }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Dq(items) => {
                let start = addr;
                for it in items {
                    let raw: [u8; 8] = if it.contains('.') || it.contains("e") || it.contains("E") {
                        match it.trim().parse::<f64>() { Ok(f) => f.to_be_bytes(), Err(e) => { errs.push(AsmErr::new(*ln, format!("bad float '{it}': {e}"))); continue; } }
                    } else {
                        match parse_expr(it, &syms2, addr, origin) { Ok(v) => (v as u64).to_be_bytes(), Err(e) => { errs.push(AsmErr::new(*ln, e)); continue; } }
                    };
                    code.extend_from_slice(&raw); addr += 8;
                }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Instr { mnemonic, ops } => {
                match enc(mnemonic, ops, &syms2, addr, origin) {
                    Ok((bytes, rel_patches)) => {
                        let ins_len = bytes.len() as u32;
                        code.extend(&bytes);
                        let start = addr;
                        // patch rel bytes: rel = target - (addr_after_instruction)
                        for rel_off in rel_patches {
                            let slot_addr = addr + rel_off as u32;
                            let t = parse_expr(&ops[rel_target_op(rel_off, mnemonic)], &syms2, addr, origin).unwrap_or(0) as i32;
                            let after = addr + ins_len;
                            let d = t - after as i32;
                            if !(-128..=127).contains(&d) {
                                errs.push(AsmErr::new(*ln, format!("branch target out of range (delta {d})")));
                            }
                            code[slot_addr as usize] = d as i8 as u8;
                        }
                        addr += ins_len;
                        info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
                    }
                    Err(e) => errs.push(AsmErr::new(*ln, e)),
                }
            }
        }
    }
    let _ = syms;
    (code, errs, info)
}

fn rel_target_op(rel_off: u8, mnemonic: &str) -> usize {
    let _ = rel_off;
    match mnemonic {
        "JB" | "JNB" | "JBC" | "DJNZ" => 1,
        "CJNE" => 2,
        _ => 0,
    }
}

fn str_lit(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.starts_with('\'') && s.ends_with('\'') { Some(&s[1..s.len() - 1]) } else { None }
}
