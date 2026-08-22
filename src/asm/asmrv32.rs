//! RV32I assembler: two-pass, case-insensitive, `;` comments, labels, and the
//! directives `ORG` / `DB` / `DW` / `DD` / `EQU` / `END`. Registers may be
//! written as `x0`..`x31` or by ABI alias (`a0`, `t0`, `sp`, ...). Immediate
//! operands accept decimal / `0x..` / `..h` / `..b` / `..o` and label
//! expressions; load/store accept `offset(rs)` and branches/jal accept a label.

use std::collections::HashMap;
use crate::asm::common::{clean_line, split_stmt, parse_expr, parse_number, AsmErr, LineInfo, Stmt};

fn reg_index(s: &str) -> Option<usize> {
    let s = s.trim();
    if let Some(r) = s.strip_prefix('X').and_then(|x| x.parse::<usize>().ok()) {
        return Some(r);
    }
    let abi: &[(&str, usize)] = &[
        ("ZERO", 0), ("RA", 1), ("SP", 2), ("GP", 3), ("TP", 4),
        ("T0", 5), ("T1", 6), ("T2", 7), ("S0", 8), ("FP", 8), ("S1", 9),
        ("A0", 10), ("A1", 11), ("A2", 12), ("A3", 13), ("A4", 14), ("A5", 15),
        ("A6", 16), ("A7", 17),
        ("S2", 18), ("S3", 19), ("S4", 20), ("S5", 21), ("S6", 22), ("S7", 23),
        ("S8", 24), ("S9", 25), ("S10", 26), ("S11", 27),
        ("T3", 28), ("T4", 29), ("T5", 30), ("T6", 31),
    ];
    abi.iter().find(|(n, _)| *n == s).map(|(_, i)| *i)
}

fn itype(opcode: u32, rd: usize, f3: u32, rs1: usize, imm12: u32) -> Vec<u8> {
    let insn = opcode | (rd as u32) << 7 | f3 << 12 | (rs1 as u32) << 15 | (imm12 & 0xfff) << 20;
    insn.to_le_bytes().to_vec()
}

fn stype(opcode: u32, f3: u32, rs1: usize, rs2: usize, imm12: u32) -> Vec<u8> {
    let imm4_0 = imm12 & 0x1f;
    let imm11_5 = (imm12 >> 5) & 0x7f;
    let insn = opcode | imm4_0 << 7 | f3 << 12 | (rs1 as u32) << 15 | (rs2 as u32) << 20 | imm11_5 << 25;
    insn.to_le_bytes().to_vec()
}

fn btype(f3: u32, rs1: usize, rs2: usize, target: u32, cur: u32) -> Vec<u8> {
    let d = (target as i32).wrapping_sub((cur as i32) + 4);
    let imm = d as u32;
    let bimm = ((imm >> 12) & 1) << 31
        | ((imm >> 11) & 1) << 7
        | ((imm >> 5) & 0x3f) << 25
        | ((imm >> 1) & 0xf) << 8;
    let insn = 0x63 | bimm | f3 << 12 | (rs1 as u32) << 15 | (rs2 as u32) << 20;
    insn.to_le_bytes().to_vec()
}

fn jtype(opcode: u32, rd: usize, target: u32, cur: u32) -> Vec<u8> {
    let d = (target as i32).wrapping_sub((cur as i32) + 4);
    let imm = d as u32;
    let jimm = ((imm >> 20) & 1) << 31
        | ((imm >> 1) & 0x3ff) << 21
        | ((imm >> 11) & 1) << 20
        | ((imm >> 12) & 0xff) << 12;
    let insn = opcode | (rd as u32) << 7 | jimm;
    insn.to_le_bytes().to_vec()
}

/// Parse a load/store operand of the form `offset(rs)` (or `label`).
fn parse_mem(s: &str) -> Result<(u32, String), String> {
    let s = s.trim();
    if let Some((off, rs)) = s.split_once('(') {
        let rs = rs.trim_end_matches(')').to_string();
        let off = parse_number(off.trim())
            .ok_or_else(|| format!("bad offset '{off}'"))?;
        Ok((off, rs))
    } else {
        Ok((0, "X0".to_string()))
    }
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
        let (mnem, ops) = split_stmt(&cleaned);
        let up = mnem.to_uppercase();
        let stmt = match up.as_str() {
            "ORG" => match parse_expr(&ops[0], &syms, addr, origin) {
                Ok(v) => Stmt::Org(v),
                Err(e) => { errs.push(AsmErr::new(lineno + 1, e)); Stmt::Ignore }
            },
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
            Stmt::Instr { .. } => addr += 4,
            Stmt::Equ(..) | Stmt::Ignore => {}
            Stmt::End => {}
        }
        if matches!(stmt, Stmt::End) {
            seq.push((lineno + 1, stmt));
            break;
        }
        seq.push((lineno + 1, stmt));
    }
    // pass 1: address sizing + resolve EQU (labels were captured in the first loop)
    addr = origin;
    let mut pass1: Vec<(usize, Stmt)> = Vec::new();
    for (ln, stmt) in &seq {
        match stmt {
            Stmt::Org(v) => { if *v < addr { errs.push(AsmErr::new(*ln, format!("ORG {v} goes backwards"))); } addr = *v; }
            Stmt::Equ(name, expr) => { if let Ok(v) = parse_expr(expr, &syms, addr, origin) { syms.insert(name.clone(), v); } }
            Stmt::Db(items) => { for _ in items { addr += 1; } }
            Stmt::Dw(items) => addr += items.len() as u32 * 2,
            Stmt::Dd(items) => addr += items.len() as u32 * 4,
            Stmt::Dq(items) => addr += items.len() as u32 * 8,
            Stmt::End => break,
            Stmt::Ignore => {}
            Stmt::Instr { .. } => addr += 4,
        }
        pass1.push((*ln, stmt.clone()));
    }
    // pass 2: emit
    addr = origin;
    let mut code = Vec::new();
    let mut info = Vec::new();
    let syms2 = syms.clone();
    for (ln, stmt) in &pass1 {
        match stmt {
            Stmt::Org(v) => { code.resize(*v as usize, 0); addr = *v; }
            Stmt::Equ(..) => {}
            Stmt::End => break,
            Stmt::Ignore => {}
            Stmt::Db(items) => {
                let start = addr;
                for it in items {
                    if let Ok(v) = parse_expr(it, &syms2, addr, origin) { code.push(v as u8); addr += 1; }
                    else { errs.push(AsmErr::new(*ln, format!("bad DB operand '{it}'"))); }
                }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Dw(items) => {
                let start = addr;
                for it in items {
                    if let Ok(v) = parse_expr(it, &syms2, addr, origin) { code.extend_from_slice(&(v as u16).to_le_bytes()); addr += 2; }
                    else { errs.push(AsmErr::new(*ln, format!("bad DW operand '{it}'"))); }
                }
                info.push(LineInfo { line: *ln as u32, addr: start, bytes: code[start as usize..addr as usize].to_vec() });
            }
            Stmt::Dd(items) => {
                let start = addr;
                for it in items {
                    if let Ok(v) = parse_expr(it, &syms2, addr, origin) { code.extend_from_slice(&v.to_le_bytes()); addr += 4; }
                    else { errs.push(AsmErr::new(*ln, format!("bad DD operand '{it}'"))); }
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

fn rtype(opcode: u32, rd: usize, f3: u32, rs1: usize, rs2: usize, f7: u32) -> Vec<u8> {
    let insn = opcode | (rd as u32) << 7 | f3 << 12 | (rs1 as u32) << 15 | (rs2 as u32) << 20 | f7 << 25;
    insn.to_le_bytes().to_vec()
}

fn enc_instr(mnem: &str, ops: &[String], syms: &HashMap<String, u32>, cur: u32, origin: u32) -> Result<Vec<u8>, String> {
    let r = |s: &str| reg_index(s).ok_or_else(|| format!("bad register '{s}'"));
    let iv = |s: &str| parse_expr(s, syms, cur, origin);
    let op0 = || ops.get(0).ok_or_else(|| format!("{mnem}: missing operand")).cloned();
    let op1 = || ops.get(1).ok_or_else(|| format!("{mnem}: missing operand")).cloned();
    let op2 = || ops.get(2).ok_or_else(|| format!("{mnem}: missing operand")).cloned();
    match mnem {
        "LUI" => { let rd = r(&op0()?)?; let v = iv(&op1()?)? & 0xfffff000; Ok((0x37 | (rd as u32) << 7 | v << 12).to_le_bytes().to_vec()) }
        "AUIPC" => { let rd = r(&op0()?)?; let v = iv(&op1()?)? & 0xfffff000; Ok((0x17 | (rd as u32) << 7 | v << 12).to_le_bytes().to_vec()) }
        "JAL" => { let rd = r(&op0()?)?; let t = iv(&op1()?)?; Ok(jtype(0x6f, rd, t, cur)) }
        "JALR" => { let rd = r(&op0()?)?; let (off, rs) = parse_mem(&op1()?)?; let rs = r(&rs)?; Ok(itype(0x67, rd, 0, rs, off)) }
        "BEQ" => Ok(btype(0, r(&op0()?)?, r(&op1()?)?, iv(&op2()?)?, cur)),
        "BNE" => Ok(btype(1, r(&op0()?)?, r(&op1()?)?, iv(&op2()?)?, cur)),
        "BLT" => Ok(btype(4, r(&op0()?)?, r(&op1()?)?, iv(&op2()?)?, cur)),
        "BGE" => Ok(btype(5, r(&op0()?)?, r(&op1()?)?, iv(&op2()?)?, cur)),
        "BLTU" => Ok(btype(6, r(&op0()?)?, r(&op1()?)?, iv(&op2()?)?, cur)),
        "BGEU" => Ok(btype(7, r(&op0()?)?, r(&op1()?)?, iv(&op2()?)?, cur)),
        "LB" => { let rd = r(&op0()?)?; let (off, rs) = parse_mem(&op1()?)?; Ok(itype(0x03, rd, 0, r(&rs)?, off)) }
        "LH" => { let rd = r(&op0()?)?; let (off, rs) = parse_mem(&op1()?)?; Ok(itype(0x03, rd, 1, r(&rs)?, off)) }
        "LW" => { let rd = r(&op0()?)?; let (off, rs) = parse_mem(&op1()?)?; Ok(itype(0x03, rd, 2, r(&rs)?, off)) }
        "LBU" => { let rd = r(&op0()?)?; let (off, rs) = parse_mem(&op1()?)?; Ok(itype(0x03, rd, 4, r(&rs)?, off)) }
        "LHU" => { let rd = r(&op0()?)?; let (off, rs) = parse_mem(&op1()?)?; Ok(itype(0x03, rd, 5, r(&rs)?, off)) }
        "SB" => { let rs2 = r(&op0()?)?; let (off, rs) = parse_mem(&op1()?)?; Ok(stype(0x23, 0, r(&rs)?, rs2, off)) }
        "SH" => { let rs2 = r(&op0()?)?; let (off, rs) = parse_mem(&op1()?)?; Ok(stype(0x23, 1, r(&rs)?, rs2, off)) }
        "SW" => { let rs2 = r(&op0()?)?; let (off, rs) = parse_mem(&op1()?)?; Ok(stype(0x23, 2, r(&rs)?, rs2, off)) }
        "ADDI" => { let rd = r(&op0()?)?; let rs1 = r(&op1()?)?; let imm = iv(&op2()?)?; Ok(itype(0x13, rd, 0, rs1, imm)) }
        "SLTI" => { let rd = r(&op0()?)?; let rs1 = r(&op1()?)?; let imm = iv(&op2()?)?; Ok(itype(0x13, rd, 2, rs1, imm)) }
        "SLTIU" => { let rd = r(&op0()?)?; let rs1 = r(&op1()?)?; let imm = iv(&op2()?)?; Ok(itype(0x13, rd, 3, rs1, imm)) }
        "XORI" => { let rd = r(&op0()?)?; let rs1 = r(&op1()?)?; let imm = iv(&op2()?)?; Ok(itype(0x13, rd, 4, rs1, imm)) }
        "ORI" => { let rd = r(&op0()?)?; let rs1 = r(&op1()?)?; let imm = iv(&op2()?)?; Ok(itype(0x13, rd, 6, rs1, imm)) }
        "ANDI" => { let rd = r(&op0()?)?; let rs1 = r(&op1()?)?; let imm = iv(&op2()?)?; Ok(itype(0x13, rd, 7, rs1, imm)) }
        "SLLI" => { let rd = r(&op0()?)?; let rs = r(&op1()?)?; let sh = iv(&op2()?)? & 0x1f; Ok(itype(0x13, rd, 1, rs, sh)) }
        "SRLI" => { let rd = r(&op0()?)?; let rs = r(&op1()?)?; let sh = iv(&op2()?)? & 0x1f; Ok(itype(0x13, rd, 5, rs, sh)) }
        "SRAI" => { let rd = r(&op0()?)?; let rs = r(&op1()?)?; let sh = (iv(&op2()?)? & 0x1f) | 0x400; Ok(itype(0x13, rd, 5, rs, sh)) }
        "ADD" => Ok(rtype(0x33, r(&op0()?)?, 0, r(&op1()?)?, r(&op2()?)?, 0)),
        "SUB" => Ok(rtype(0x33, r(&op0()?)?, 0, r(&op1()?)?, r(&op2()?)?, 0x20)),
        "SLL" => Ok(rtype(0x33, r(&op0()?)?, 1, r(&op1()?)?, r(&op2()?)?, 0)),
        "SLT" => Ok(rtype(0x33, r(&op0()?)?, 2, r(&op1()?)?, r(&op2()?)?, 0)),
        "SLTU" => Ok(rtype(0x33, r(&op0()?)?, 3, r(&op1()?)?, r(&op2()?)?, 0)),
        "XOR" => Ok(rtype(0x33, r(&op0()?)?, 4, r(&op1()?)?, r(&op2()?)?, 0)),
        "SRL" => Ok(rtype(0x33, r(&op0()?)?, 5, r(&op1()?)?, r(&op2()?)?, 0)),
        "SRA" => Ok(rtype(0x33, r(&op0()?)?, 5, r(&op1()?)?, r(&op2()?)?, 0x20)),
        "OR" => Ok(rtype(0x33, r(&op0()?)?, 6, r(&op1()?)?, r(&op2()?)?, 0)),
        "AND" => Ok(rtype(0x33, r(&op0()?)?, 7, r(&op1()?)?, r(&op2()?)?, 0)),
        // M-extension (opcode 0x33, f7 = 0x01)
        "MUL" => Ok(rtype(0x33, r(&op0()?)?, 0, r(&op1()?)?, r(&op2()?)?, 0x01)),
        "MULH" => Ok(rtype(0x33, r(&op0()?)?, 1, r(&op1()?)?, r(&op2()?)?, 0x01)),
        "MULHSU" => Ok(rtype(0x33, r(&op0()?)?, 2, r(&op1()?)?, r(&op2()?)?, 0x01)),
        "MULHU" => Ok(rtype(0x33, r(&op0()?)?, 3, r(&op1()?)?, r(&op2()?)?, 0x01)),
        "DIV" => Ok(rtype(0x33, r(&op0()?)?, 4, r(&op1()?)?, r(&op2()?)?, 0x01)),
        "DIVU" => Ok(rtype(0x33, r(&op0()?)?, 5, r(&op1()?)?, r(&op2()?)?, 0x01)),
        "REM" => Ok(rtype(0x33, r(&op0()?)?, 6, r(&op1()?)?, r(&op2()?)?, 0x01)),
        "REMU" => Ok(rtype(0x33, r(&op0()?)?, 7, r(&op1()?)?, r(&op2()?)?, 0x01)),
        "FENCE" => Ok(0x0000000fu32.to_le_bytes().to_vec()),
        "ECALL" => Ok(0x00000073u32.to_le_bytes().to_vec()),
        "EBREAK" => Ok(0x00100073u32.to_le_bytes().to_vec()),
        _ => Err(format!("unsupported instruction: {mnem}")),
    }
}
