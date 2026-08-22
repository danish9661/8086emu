//! 8086 disassembler — decodes instructions from memory for the IDE's
//! disassembly view. Covers the common user/programmable subset; unrecognized
//! opcodes fall back to a raw `DB` line so the view never desyncs.

use crate::cpu::{Disasm, Mem};

const R16: [&str; 8] = ["AX", "CX", "DX", "BX", "SP", "BP", "SI", "DI"];
const R8: [&str; 8] = ["AL", "CL", "DL", "BL", "AH", "CH", "DH", "BH"];
const SEG: [&str; 6] = ["ES", "CS", "SS", "DS", "FS", "GS"];

fn r16(i: u8) -> &'static str { R16[(i & 7) as usize] }
fn r8(i: u8) -> &'static str { R8[(i & 7) as usize] }
fn seg(i: u8) -> &'static str { SEG[(i & 7) as usize] }
fn reg_name(i: u8, w: bool) -> String {
    if w { r16(i).to_string() } else { r8(i).to_string() }
}

/// Decode a ModRM byte at `*off`, returning the r/m operand text and the reg
/// field (used by GRP-prefixed opcodes). `w` is the operand width (false=byte,
/// true=word) taken from the opcode — it selects the register name for the
/// register-direct (mod=3) form. Advances `*off` past ModRM (+ disp).
fn modrm(mem: &Mem, off: &mut u32, w: bool) -> (String, u8) {
    let b = mem.read(*off as usize);
    *off += 1;
    let mm = (b >> 6) & 3;
    let rm = b & 7;
    let reg = (b >> 3) & 7;
    if mm == 3 {
        return (reg_name(rm, w), reg);
    }
    let (base, disp) = match (mm, rm) {
        (0, 0) => ("BX+SI".to_string(), 0i32),
        (0, 1) => ("BX+DI".to_string(), 0),
        (0, 2) => ("BP+SI".to_string(), 0),
        (0, 3) => ("BP+DI".to_string(), 0),
        (0, 4) => ("SI".to_string(), 0),
        (0, 5) => ("DI".to_string(), 0),
        (0, 6) => {
            let d = mem.read16(*off as usize) as i32;
            *off += 2;
            return (format!("[{d:04X}]"), reg);
        }
        (0, 7) => ("BX".to_string(), 0),
        (1, 6) | (2, 6) => {
            let d = if mm == 1 { mem.read(*off as usize) as i8 as i32 } else { mem.read16(*off as usize) as i16 as i32 };
            *off += if mm == 1 { 1 } else { 2 };
            ("BP".to_string(), d)
        }
        (1, _) | (2, _) => {
            let base = match rm { 0 => "BX+SI", 1 => "BX+DI", 2 => "BP+SI", 3 => "BP+DI", 4 => "SI", 5 => "DI", _ => "BX" };
            let d = if mm == 1 { mem.read(*off as usize) as i8 as i32 } else { mem.read16(*off as usize) as i16 as i32 };
            *off += if mm == 1 { 1 } else { 2 };
            (base.to_string(), d)
        }
        _ => ("".to_string(), 0),
    };
    let s = if disp == 0 {
        format!("[{base}]")
    } else {
        format!("[{base}{:+}h]", disp)
    };
    (s, reg)
}

fn imm(mem: &Mem, off: &mut u32, w: bool) -> String {
    if w {
        let v = mem.read16(*off as usize);
        *off += 2;
        format!("{v:04X}h")
    } else {
        let v = mem.read(*off as usize);
        *off += 1;
        format!("{v:02X}h")
    }
}

fn rel16(mem: &Mem, off: &mut u32) -> String {
    let d = mem.read16(*off as usize) as i16;
    *off += 2;
    format!("${:04X}", (*off as i32 + d as i32) as u32 & 0xFFFF)
}
fn rel8(mem: &Mem, off: &mut u32) -> String {
    let d = mem.read(*off as usize) as i8;
    *off += 1;
    format!("${:04X}", (*off as i32 + d as i32) as u32 & 0xFFFF)
}

fn grp1(op: u8) -> &'static str {
    match op & 7 {
        0 => "ADD", 1 => "OR", 2 => "ADC", 3 => "SBB",
        4 => "AND", 5 => "SUB", 6 => "XOR", 7 => "CMP", _ => "?",
    }
}
fn grp2(op: u8) -> &'static str {
    match op & 7 {
        0 => "ROL", 1 => "ROR", 2 => "RCL", 3 => "RCR",
        4 => "SHL", 5 => "SHR", 6 => "SAL", 7 => "SAR", _ => "?",
    }
}
fn jcc(op: u8) -> &'static str {
    match op & 0x0F {
        0x0 => "JO", 0x1 => "JNO", 0x2 => "JB", 0x3 => "JAE",
        0x4 => "JE", 0x5 => "JNE", 0x6 => "JBE", 0x7 => "JA",
        0x8 => "JS", 0x9 => "JNS", 0xA => "JP", 0xB => "JNP",
        0xC => "JL", 0xD => "JGE", 0xE => "JLE", 0xF => "JG",
        _ => "J?",
    }
}

/// Disassemble up to `count` instructions starting at `start`.
pub fn disasm(mem: &Mem, start: u32, count: usize) -> Vec<Disasm> {
    let mut out = Vec::new();
    let mut off = start & 0xFFFFF;
    for _ in 0..count {
        let addr = off;
        let mut bytes = Vec::new();
        let mut seg_ov: Option<&str> = None;
        let mut rep = "";
        loop {
            let op = mem.read(off as usize);
            match op {
                0x26 => { seg_ov = Some("ES:"); off += 1; bytes.push(op); }
                0x2E => { seg_ov = Some("CS:"); off += 1; bytes.push(op); }
                0x36 => { seg_ov = Some("SS:"); off += 1; bytes.push(op); }
                0x3E => { seg_ov = Some("DS:"); off += 1; bytes.push(op); }
                0x64 => { seg_ov = Some("FS:"); off += 1; bytes.push(op); }
                0x65 => { seg_ov = Some("GS:"); off += 1; bytes.push(op); }
                0xF2 => { rep = "REPNZ "; off += 1; bytes.push(op); }
                0xF3 => { rep = "REP "; off += 1; bytes.push(op); }
                0xF0 => { rep = "LOCK "; off += 1; bytes.push(op); }
                _ => break,
            }
        }
        let op = mem.read(off as usize);
        bytes.push(op);
        off += 1;
        let text = decode(mem, &mut off, op, seg_ov, rep);
        let mut consumed = (off - addr) as usize;
        if consumed == 0 { consumed = 1; off = off.wrapping_add(1) & 0xFFFFF; }
        for i in 0..consumed {
            if i < bytes.len() { continue; }
            bytes.push(mem.read((addr + i as u32) as usize));
        }
        out.push(Disasm { addr, bytes, text });
    }
    out
}

fn decode(mem: &Mem, off: &mut u32, op: u8, seg_ov: Option<&str>, rep: &str) -> String {
    let p = seg_ov.unwrap_or("");
    match op {
        0x00 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}ADD {rm},{rg}", rm = rm, rg = r8(r)) }
        0x01 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}ADD {rm},{rg}", rm = rm, rg = r16(r)) }
        0x02 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}ADD {rg},{rm}", rm = rm, rg = r8(r)) }
        0x03 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}ADD {rg},{rm}", rm = rm, rg = r16(r)) }
        0x04 => { let v = imm(mem, off, false); format!("ADD AL,{v}") }
        0x05 => { let v = imm(mem, off, true); format!("ADD AX,{v}") }
        0x08 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}OR {rm},{rg}", rm = rm, rg = r8(r)) }
        0x09 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}OR {rm},{rg}", rm = rm, rg = r16(r)) }
        0x0A => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}OR {rg},{rm}", rm = rm, rg = r8(r)) }
        0x0B => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}OR {rg},{rm}", rm = rm, rg = r16(r)) }
        0x0C => { let v = imm(mem, off, false); format!("OR AL,{v}") }
        0x0D => { let v = imm(mem, off, true); format!("OR AX,{v}") }
        0x10 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}ADC {rm},{rg}", rm = rm, rg = r8(r)) }
        0x11 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}ADC {rm},{rg}", rm = rm, rg = r16(r)) }
        0x12 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}ADC {rg},{rm}", rm = rm, rg = r8(r)) }
        0x13 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}ADC {rg},{rm}", rm = rm, rg = r16(r)) }
        0x14 => { let v = imm(mem, off, false); format!("ADC AL,{v}") }
        0x15 => { let v = imm(mem, off, true); format!("ADC AX,{v}") }
        0x18 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}SBB {rm},{rg}", rm = rm, rg = r8(r)) }
        0x19 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}SBB {rm},{rg}", rm = rm, rg = r16(r)) }
        0x1A => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}SBB {rg},{rm}", rm = rm, rg = r8(r)) }
        0x1B => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}SBB {rg},{rm}", rm = rm, rg = r16(r)) }
        0x1C => { let v = imm(mem, off, false); format!("SBB AL,{v}") }
        0x1D => { let v = imm(mem, off, true); format!("SBB AX,{v}") }
        0x20 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}AND {rm},{rg}", rm = rm, rg = r8(r)) }
        0x21 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}AND {rm},{rg}", rm = rm, rg = r16(r)) }
        0x22 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}AND {rg},{rm}", rm = rm, rg = r8(r)) }
        0x23 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}AND {rg},{rm}", rm = rm, rg = r16(r)) }
        0x24 => { let v = imm(mem, off, false); format!("AND AL,{v}") }
        0x25 => { let v = imm(mem, off, true); format!("AND AX,{v}") }
        0x28 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}SUB {rm},{rg}", rm = rm, rg = r8(r)) }
        0x29 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}SUB {rm},{rg}", rm = rm, rg = r16(r)) }
        0x2A => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}SUB {rg},{rm}", rm = rm, rg = r8(r)) }
        0x2B => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}SUB {rg},{rm}", rm = rm, rg = r16(r)) }
        0x2C => { let v = imm(mem, off, false); format!("SUB AL,{v}") }
        0x2D => { let v = imm(mem, off, true); format!("SUB AX,{v}") }
        0x30 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}XOR {rm},{rg}", rm = rm, rg = r8(r)) }
        0x31 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}XOR {rm},{rg}", rm = rm, rg = r16(r)) }
        0x32 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}XOR {rg},{rm}", rm = rm, rg = r8(r)) }
        0x33 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}XOR {rg},{rm}", rm = rm, rg = r16(r)) }
        0x34 => { let v = imm(mem, off, false); format!("XOR AL,{v}") }
        0x35 => { let v = imm(mem, off, true); format!("XOR AX,{v}") }
        0x38 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}CMP {rm},{rg}", rm = rm, rg = r8(r)) }
        0x39 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}CMP {rm},{rg}", rm = rm, rg = r16(r)) }
        0x3A => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}CMP {rg},{rm}", rm = rm, rg = r8(r)) }
        0x3B => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{p}CMP {rg},{rm}", rm = rm, rg = r16(r)) }
        0x3C => { let v = imm(mem, off, false); format!("CMP AL,{v}") }
        0x3D => { let v = imm(mem, off, true); format!("CMP AX,{v}") }
        0x40..=0x47 => format!("INC {}", r16(op & 7)),
        0x48..=0x4F => format!("DEC {}", r16(op & 7)),
        0x50..=0x57 => format!("PUSH {}", r16(op & 7)),
        0x58..=0x5F => format!("POP {}", r16(op & 7)),
        0x68 => { let v = imm(mem, off, true); format!("PUSH {v}") }
        0x6A => { let v = imm(mem, off, false); format!("PUSH {v}") }
        0x69 => { let (rm, _) = modrm(mem, off, true); let v = imm(mem, off, true); format!("IMUL {rm},{v}") }
        0x6B => { let (rm, _) = modrm(mem, off, true); let v = imm(mem, off, false); format!("IMUL {rm},{v}") }
        0x70..=0x7F => { let t = jcc(op); let r = rel8(mem, off); format!("{t} {r}") }
        0x80 | 0x81 | 0x82 | 0x83 => {
            let w = op != 0x80;
            let (rm, r) = modrm(mem, off, w);
            let v = imm(mem, off, w);
            format!("{p}{} {rm},{v}", grp1(r))
        }
        0x84 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("TEST {rm},{rg}", rm = rm, rg = r8(r)) }
        0x85 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("TEST {rm},{rg}", rm = rm, rg = r16(r)) }
        0x86 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("XCHG {rm},{rg}", rm = rm, rg = r8(r)) }
        0x87 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("XCHG {rm},{rg}", rm = rm, rg = r16(r)) }
        0x88 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("MOV {rm},{rg}", rm = rm, rg = r8(r)) }
        0x89 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("MOV {rm},{rg}", rm = rm, rg = r16(r)) }
        0x8A => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("MOV {rg},{rm}", rm = rm, rg = r8(r)) }
        0x8B => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("MOV {rg},{rm}", rm = rm, rg = r16(r)) }
        0x8C => { let (rm, r) = modrm(mem, off, true); format!("MOV {rm},{sg}", rm = rm, sg = seg(r)) }
        0x8D => { let (rm, r) = modrm(mem, off, true); format!("LEA {rg},{rm}", rm = rm, rg = r16(r)) }
        0x8E => { let (rm, r) = modrm(mem, off, true); format!("MOV {sg},{rm}", rm = rm, sg = seg(r)) }
        0x8F => { let (rm, _) = modrm(mem, off, true); format!("POP {rm}") }
        0x90 => "NOP".to_string(),
        0x91..=0x97 => format!("XCHG AX,{}", r16(op & 7)),
        0x98 => "CBW".to_string(),
        0x99 => "CWD".to_string(),
        0x9A => { let o = mem.read16(*off as usize); *off += 2; let s = mem.read16(*off as usize); *off += 2; format!("CALL {s:04X}:{o:04X}") }
        0x9B => "WAIT".to_string(),
        0x9C => "PUSHF".to_string(),
        0x9D => "POPF".to_string(),
        0x9E => "SAHF".to_string(),
        0x9F => "LAHF".to_string(),
        0xA0 => { let a = mem.read16(*off as usize); *off += 2; format!("MOV AL,[{a:04X}]") }
        0xA1 => { let a = mem.read16(*off as usize); *off += 2; format!("MOV AX,[{a:04X}]") }
        0xA2 => { let a = mem.read16(*off as usize); *off += 2; format!("MOV [{a:04X}],AL") }
        0xA3 => { let a = mem.read16(*off as usize); *off += 2; format!("MOV [{a:04X}],AX") }
        0xA4 => format!("{rep}MOVSB"),
        0xA5 => format!("{rep}MOVSW"),
        0xA6 => format!("{rep}CMPSB"),
        0xA7 => format!("{rep}CMPSW"),
        0xA8 => { let v = imm(mem, off, false); format!("TEST AL,{v}") }
        0xA9 => { let v = imm(mem, off, true); format!("TEST AX,{v}") }
        0xAA => format!("{rep}STOSB"),
        0xAB => format!("{rep}STOSW"),
        0xAC => format!("{rep}LODSB"),
        0xAD => format!("{rep}LODSW"),
        0xAE => format!("{rep}SCASB"),
        0xAF => format!("{rep}SCASW"),
        0xB0..=0xB7 => format!("MOV {rg},{v}", rg = r8(op & 7), v = imm(mem, off, false)),
        0xB8..=0xBF => format!("MOV {rg},{v}", rg = r16(op & 7), v = imm(mem, off, true)),
        0xC0 | 0xC1 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); let v = imm(mem, off, false); format!("{g} {rm},{v}", g = grp2(r)) }
        0xC2 => { let v = imm(mem, off, true); format!("RET {v}") }
        0xC3 => "RET".to_string(),
        0xC4 => { let (rm, r) = modrm(mem, off, true); format!("LES {rg},{rm}", rm = rm, rg = r16(r)) }
        0xC5 => { let (rm, r) = modrm(mem, off, true); format!("LDS {rg},{rm}", rm = rm, rg = r16(r)) }
        0xC6 => { let (rm, _) = modrm(mem, off, (op & 1) == 1); let v = imm(mem, off, false); format!("MOV {rm},{v}") }
        0xC7 => { let (rm, _) = modrm(mem, off, (op & 1) == 1); let v = imm(mem, off, true); format!("MOV {rm},{v}") }
        0xC8 => { let sz = imm(mem, off, true); let lvl = imm(mem, off, false); format!("ENTER {sz},{lvl}") }
        0xC9 => "LEAVE".to_string(),
        0xCA => { let v = imm(mem, off, true); format!("RETF {v}") }
        0xCB => "RETF".to_string(),
        0xCC => "INT 3".to_string(),
        0xCD => { let v = imm(mem, off, false); format!("INT {v}") }
        0xCE => "INTO".to_string(),
        0xCF => "IRET".to_string(),
        0xD0 | 0xD1 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{g} {rm},1", g = grp2(r)) }
        0xD2 | 0xD3 => { let (rm, r) = modrm(mem, off, (op & 1) == 1); format!("{g} {rm},CL", g = grp2(r)) }
        0xD4 => { let v = imm(mem, off, false); format!("AAM {v}") }
        0xD5 => { let v = imm(mem, off, false); format!("AAD {v}") }
        0xD6 => "SALC".to_string(),
        0xD7 => "XLAT".to_string(),
        0xD8..=0xDF => "ESC/FPU".to_string(),
        0xE0 => { let r = rel8(mem, off); format!("LOOPNZ {r}") }
        0xE1 => { let r = rel8(mem, off); format!("LOOPZ {r}") }
        0xE2 => { let r = rel8(mem, off); format!("LOOP {r}") }
        0xE3 => { let r = rel8(mem, off); format!("JCXZ {r}") }
        0xE4 => { let p = imm(mem, off, false); format!("IN AL,{p}") }
        0xE5 => { let p = imm(mem, off, false); format!("IN AX,{p}") }
        0xE6 => { let p = imm(mem, off, false); format!("OUT {p},AL") }
        0xE7 => { let p = imm(mem, off, false); format!("OUT {p},AX") }
        0xE8 => { let r = rel16(mem, off); format!("CALL {r}") }
        0xE9 => { let r = rel16(mem, off); format!("JMP {r}") }
        0xEA => { let o = mem.read16(*off as usize); *off += 2; let s = mem.read16(*off as usize); *off += 2; format!("JMP {s:04X}:{o:04X}") }
        0xEB => { let r = rel8(mem, off); format!("JMP {r}") }
        0xEC => "IN AL,DX".to_string(),
        0xED => "IN AX,DX".to_string(),
        0xEE => "OUT DX,AL".to_string(),
        0xEF => "OUT DX,AX".to_string(),
        0xF0 => "LOCK".to_string(),
        0xF1 => "INT 1".to_string(),
        0xF2 => "REPNZ".to_string(),
        0xF3 => "REPZ".to_string(),
        0xF4 => "HLT".to_string(),
        0xF5 => "CMC".to_string(),
        0xF6 | 0xF7 => {
            let w = op == 0xF7;
            let (rm, r) = modrm(mem, off, w);
            match r {
                0 => { let v = imm(mem, off, w); format!("TEST {rm},{v}") }
                2 => format!("NOT {rm}"),
                3 => format!("NEG {rm}"),
                4 => format!("MUL {rm}"),
                5 => format!("IMUL {rm}"),
                6 => format!("DIV {rm}"),
                7 => format!("IDIV {rm}"),
                _ => format!("GRP3/{r} {rm}"),
            }
        }
        0xF8 => "CLC".to_string(),
        0xF9 => "STC".to_string(),
        0xFA => "CLI".to_string(),
        0xFB => "STI".to_string(),
        0xFC => "CLD".to_string(),
        0xFD => "STD".to_string(),
        0xFE => { let (rm, r) = modrm(mem, off, false); format!("{} {rm}", if r == 0 { "INC" } else { "DEC" }) }
        0xFF => {
            let (rm, r) = modrm(mem, off, true);
            let m = match r {
                0 => "INC", 1 => "DEC", 2 => "CALL", 3 => "CALL FAR",
                4 => "JMP", 5 => "JMP FAR", 6 => "PUSH", _ => "GRP5",
            };
            format!("{m} {rm}")
        }
        _ => format!("DB {op:02X}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::Mem;

    fn m(bytes: &[u8]) -> Mem {
        let mut mem = Mem::new(0x100000);
        mem.load(0x100, bytes);
        mem
    }

    #[test]
    fn disasm_basic() {
        // MOV AX,1 ; ADD AX,BX ; MOV [BX+DI+2],AX ; LOOP $ ; INT 21h
        let code: &[u8] = &[
            0xB8, 0x01, 0x00,       // MOV AX,0001
            0x01, 0xD8,             // ADD AX,BX
            0x89, 0x41, 0x02,       // MOV [BX+DI+02],AX
            0xE2, 0xFC,             // LOOP -4
            0xCD, 0x21,             // INT 21h
        ];
        let d = disasm(&m(code), 0x100, 5);
        assert_eq!(d[0].text, "MOV AX,0001h");
        assert_eq!(d[1].text, "ADD AX,BX");
        assert_eq!(d[2].text, "MOV [BX+DI+2h],AX");
        assert_eq!(d[3].text, "LOOP $0106");
        assert_eq!(d[4].text, "INT 21h");
        assert_eq!(d[0].bytes.len(), 3);
        assert_eq!(d[1].bytes.len(), 2);
        assert_eq!(d[2].bytes.len(), 3);
        assert_eq!(d[3].bytes.len(), 2);
        assert_eq!(d[4].bytes.len(), 2);
    }

    #[test]
    fn disasm_fallthrough_db() {
        // NOP ; DB 0Fh (undefined opcode) ; NOP
        let code: &[u8] = &[0x90, 0x0F, 0x90];
        let d = disasm(&m(code), 0x100, 3);
        assert_eq!(d[0].text, "NOP");
        assert_eq!(d[1].text, "DB 0Fh");
        assert_eq!(d[2].text, "NOP");
    }

    #[test]
    fn disasm_prefix_and_grp() {
        // REP MOVSB ; SHL AX,1 ; INT 21h
        let code: &[u8] = &[0xF3, 0xA4, 0xD1, 0xE0, 0xCD, 0x21];
        let d = disasm(&m(code), 0x100, 3);
        assert_eq!(d[0].text, "REP MOVSB");
        assert_eq!(d[1].text, "SHL AX,1");
        assert_eq!(d[2].text, "INT 21h");
    }
}
