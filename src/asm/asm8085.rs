//! 8085 assembler — table-driven encoding of the full 8-bit ISA.

use super::common::*;
use std::collections::HashMap;

const REGS: [&str; 8] = ["B", "C", "D", "E", "H", "L", "M", "A"];

fn reg_index(name: &str) -> Option<u8> {
    REGS.iter().position(|r| *r == name).map(|i| i as u8)
}

fn rp_index(name: &str) -> Option<u8> {
    match name { "B" => Some(0), "D" => Some(1), "H" => Some(2), "SP" => Some(3), _ => None }
}

fn enc(mnemonic: &str, ops: &[String], syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<Vec<u8>, String> {
    let mut o = Vec::new();
    let val = |s: &str| parse_expr(s, syms, cur, origin);
    let imm = |s: &str| -> Result<u8, String> {
        let v = val(s)?;
        if v > 0xFF { return Err(format!("value {v} out of byte range")); }
        Ok(v as u8)
    };
    let addr16 = |s: &str| -> Result<u16, String> {
        let v = val(s)?;
        if v > 0xFFFF { return Err(format!("address {v} out of 16-bit range")); }
        Ok(v as u16)
    };

    match mnemonic {
        // MOV r,r'
        "MOV" => {
            if ops.len() != 2 { return Err("MOV needs 2 operands".into()); }
            let (d, s) = (&ops[0], &ops[1]);
            let rd = reg_index(d).ok_or_else(|| format!("bad register '{d}'"))?;
            let rs = reg_index(s).ok_or_else(|| format!("bad register '{s}'"))?;
            o.push(0x40 | (rd << 3) | rs);
        }
        // MVI
        "MVI" => {
            if ops.len() != 2 { return Err("MVI needs 2 operands".into()); }
            let r = reg_index(&ops[0]).ok_or_else(|| format!("bad register '{}'", ops[0]))?;
            if r == 6 {
                o.push(0x36);
            } else {
                o.push(0x06 | (r << 3));
            }
            o.push(imm(&ops[1])?);
        }
        // LXI / INX / DCX / DAD
        "LXI" => {
            if ops.len() != 2 { return Err("LXI needs 2 operands".into()); }
            let rp = rp_index(&ops[0]).ok_or_else(|| format!("bad register pair '{}'", ops[0]))?;
            o.push(0x01 | (rp << 4));
            o.extend_from_slice(&addr16(&ops[1])?.to_le_bytes());
        }
        "INX" => {
            let rp = rp_index(&ops[0]).ok_or_else(|| format!("bad register pair '{}'", ops[0]))?;
            o.push(0x03 | (rp << 4));
        }
        "DCX" => {
            let rp = rp_index(&ops[0]).ok_or_else(|| format!("bad register pair '{}'", ops[0]))?;
            o.push(0x0B | (rp << 4));
        }
        "DAD" => {
            let rp = rp_index(&ops[0]).ok_or_else(|| format!("bad register pair '{}'", ops[0]))?;
            o.push(0x09 | (rp << 4));
        }
        // memory moves
        "LDA" => { o.push(0x3A); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "STA" => { o.push(0x32); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "LHLD" => { o.push(0x2A); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "SHLD" => { o.push(0x22); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "LDAX" => match ops[0].as_str() { "B" => o.push(0x0A), "D" => o.push(0x1A), _ => return Err("LDAX needs B or D".into()) },
        "STAX" => match ops[0].as_str() { "B" => o.push(0x02), "D" => o.push(0x12), _ => return Err("STAX needs B or D".into()) },
        "XCHG" => o.push(0xEB),
        // ALU
        "ADD" | "ADC" | "SUB" | "SBB" | "ANA" | "XRA" | "ORA" | "CMP" => {
            let base = match mnemonic { "ADD" => 0x80, "ADC" => 0x88, "SUB" => 0x90, "SBB" => 0x98, "ANA" => 0xA0, "XRA" => 0xA8, "ORA" => 0xB0, _ => 0xB8 };
            let r = reg_index(&ops[0]).ok_or_else(|| format!("bad register '{}'", ops[0]))?;
            o.push(base | r);
        }
        "ADI" | "ACI" | "SUI" | "SBI" | "ANI" | "XRI" | "ORI" | "CPI" => {
            let op = match mnemonic { "ADI" => 0xC6, "ACI" => 0xCE, "SUI" => 0xD6, "SBI" => 0xDE, "ANI" => 0xE6, "XRI" => 0xEE, "ORI" => 0xF6, _ => 0xFE };
            o.push(op);
            o.push(imm(&ops[0])?);
        }
        // INR / DCR
        "INR" => {
            let r = reg_index(&ops[0]).ok_or_else(|| format!("bad register '{}'", ops[0]))?;
            if r == 6 { o.push(0x34); } else { o.push(0x04 | (r << 3)); }
        }
        "DCR" => {
            let r = reg_index(&ops[0]).ok_or_else(|| format!("bad register '{}'", ops[0]))?;
            if r == 6 { o.push(0x35); } else { o.push(0x05 | (r << 3)); }
        }
        // misc ALU
        "RLC" => o.push(0x07), "RRC" => o.push(0x0F), "RAL" => o.push(0x17), "RAR" => o.push(0x1F),
        "CMA" => o.push(0x2F), "CMC" => o.push(0x3F), "STC" => o.push(0x37), "DAA" => o.push(0x27),
        "RIM" => o.push(0x20), "SIM" => o.push(0x30),
        // jumps
        "JMP" => { o.push(0xC3); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "JC" => { o.push(0xDA); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "JNC" => { o.push(0xD2); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "JZ" => { o.push(0xCA); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "JNZ" => { o.push(0xC2); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "JP" => { o.push(0xF2); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "JM" => { o.push(0xFA); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "JPE" => { o.push(0xEA); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "JPO" => { o.push(0xE2); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        // calls
        "CALL" => { o.push(0xCD); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "CC" => { o.push(0xDC); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "CNC" => { o.push(0xD4); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "CZ" => { o.push(0xCC); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "CNZ" => { o.push(0xC4); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "CP" => { o.push(0xF4); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "CM" => { o.push(0xFC); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "CPE" => { o.push(0xEC); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        "CPO" => { o.push(0xE4); o.extend_from_slice(&addr16(&ops[0])?.to_le_bytes()); }
        // returns
        "RET" => o.push(0xC9), "RC" => o.push(0xD8), "RNC" => o.push(0xD0),
        "RZ" => o.push(0xC8), "RNZ" => o.push(0xC0), "RP" => o.push(0xF0),
        "RM" => o.push(0xF8), "RPE" => o.push(0xE8), "RPO" => o.push(0xE0),
        // stack (PUSH/POP accept B/D/H/PSW; SP has no push/pop on the 8085)
        "PUSH" => match ops[0].as_str() {
            "B" => o.push(0xC5), "D" => o.push(0xD5), "H" => o.push(0xE5), "PSW" => o.push(0xF5),
            _ => return Err(format!("bad pair '{}'", ops[0])),
        },
        "POP" => match ops[0].as_str() {
            "B" => o.push(0xC1), "D" => o.push(0xD1), "H" => o.push(0xE1), "PSW" => o.push(0xF1),
            _ => return Err(format!("bad pair '{}'", ops[0])),
        },
        "PUSHPSW" => o.push(0xF5),
        "POPPSW" => o.push(0xF1),
        "XTHL" => o.push(0xE3), "SPHL" => o.push(0xF9), "PCHL" => o.push(0xE9),
        // RST
        "RST" => {
            let n = ops[0].parse::<u32>().map_err(|_| "RST needs 0-7")?;
            if n > 7 { return Err("RST needs 0-7".into()); }
            o.push(0xC7 | ((n as u8) << 3));
        }
        // I/O
        "IN" => { o.push(0xDB); o.push(imm(&ops[0])?); }
        "OUT" => { o.push(0xD3); o.push(imm(&ops[0])?); }
        "EI" => o.push(0xFB), "DI" => o.push(0xF3),
        "HLT" => o.push(0x76), "NOP" => o.push(0x00),
        _ => return Err(format!("unknown mnemonic '{mnemonic}'")),
    }
    Ok(o)
}

/// Assemble 8085 source (single fixed-size pass; labels resolved on pass 2).
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
                    Ok(b) => addr += b.len() as u32,
                    Err(e) => errs.push(AsmErr::new(*ln, e)),
                }
            }
        }
    }
    let _ = (origin, addr);

    // pass 2: emit
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
                        Ok(v) => { code.extend_from_slice(&(v as u16).to_le_bytes()); addr += 2; }
                        Err(e) => errs.push(AsmErr::new(*ln, e)),
                    }
                }
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
                    code.extend_from_slice(&raw); addr += 8;
                }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Instr { mnemonic, ops } => {
                match enc(mnemonic, ops, &syms2, addr, origin) {
                    Ok(b) => { code.extend(&b); addr += b.len() as u32; info.push(LineInfo { line: *ln as u32, addr: addr - b.len() as u32, bytes: b }); }
                    Err(e) => errs.push(AsmErr::new(*ln, e)),
                }
            }
        }
    }
    let _ = syms;
    (code, errs, info)
}

fn str_lit(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.starts_with('\'') && s.ends_with('\'') { Some(&s[1..s.len() - 1]) } else { None }
}
