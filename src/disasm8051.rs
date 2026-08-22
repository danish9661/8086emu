//! Intel 8051 (MCS-51) disassembler — decodes from the 64 KiB code space for
//! the IDE's disassembly view. Unrecognized opcodes fall back to `DB`.

use crate::cpu::{Disasm, Mem};

fn rd8(mem: &Mem, off: &mut u32) -> u8 { let v = mem.read(*off as usize); *off += 1; v }
fn rd16(mem: &Mem, off: &mut u32) -> u16 { let lo = rd8(mem, off) as u16; let hi = rd8(mem, off) as u16; (hi << 8) | lo }
fn rd16be(mem: &Mem, off: &mut u32) -> u16 { let hi = rd8(mem, off) as u16; let lo = rd8(mem, off) as u16; (hi << 8) | lo }

fn rel8(mem: &Mem, off: &mut u32) -> String {
    let d = rd8(mem, off) as i8;
    format!("${:04X}", (*off as i32 + d as i32) as u32 & 0xFFFF)
}
fn addr11(mem: &Mem, off: &mut u32, op: u8) -> String {
    let operand = rd8(mem, off); // off is now address after the 2-byte instr
    let page = (op & 0xE0) as u32;
    let target = (*off & 0xF800) | ((page << 3) & 0xF800) | operand as u32;
    format!("${:04X}", target & 0xFFFF)
}

/// Disassemble up to `count` instructions starting at `start`.
pub fn disasm(mem: &Mem, start: u32, count: usize) -> Vec<Disasm> {
    let mut out = Vec::new();
    let mut off = start & 0xFFFF;
    for _ in 0..count {
        let addr = off;
        let op = rd8(mem, &mut off);
        let text = decode(mem, &mut off, op);
        let mut consumed = (off - addr) as usize;
        if consumed == 0 { consumed = 1; off = off.wrapping_add(1) & 0xFFFF; }
        let mut bytes = Vec::new();
        for i in 0..consumed { bytes.push(mem.read((addr + i as u32) as usize)); }
        out.push(Disasm { addr, bytes, text });
    }
    out
}

fn decode(mem: &Mem, off: &mut u32, op: u8) -> String {
    let r = |n: u8| format!("R{}", n & 7);
    match op {
        0x00 => "NOP".to_string(),
        0x01 => { let a = addr11(mem, off, op); format!("AJMP {a}") }
        0x02 => { let a = rd16be(mem, off); format!("LJMP ${a:04X}") }
        0x03 => "RR A".to_string(),
        0x04 => "INC A".to_string(),
        0x05 => { let d = rd8(mem, off); format!("INC ${d:02X}") }
        0x06 => "INC @R0".to_string(),
        0x07 => "INC @R1".to_string(),
        0x08..=0x0F => format!("INC {}", r(op)),
        0x10 => { let b = rd8(mem, off); let t = rel8(mem, off); format!("JBC ${b:02X},{t}") }
        0x11 => { let a = addr11(mem, off, op); format!("ACALL {a}") }
        0x12 => { let a = rd16be(mem, off); format!("LCALL ${a:04X}") }
        0x13 => "RRC A".to_string(),
        0x14 => "DEC A".to_string(),
        0x15 => { let d = rd8(mem, off); format!("DEC ${d:02X}") }
        0x16 => "DEC @R0".to_string(),
        0x17 => "DEC @R1".to_string(),
        0x18..=0x1F => format!("DEC {}", r(op)),
        0x20 => { let b = rd8(mem, off); let t = rel8(mem, off); format!("JB ${b:02X},{t}") }
        0x21 | 0x41 | 0x61 | 0x81 | 0xA1 | 0xC1 | 0xE1 => { let a = addr11(mem, off, op); format!("AJMP {a}") }
        0x22 => "RET".to_string(),
        0x23 => "RL A".to_string(),
        0x24 => { let v = rd8(mem, off); format!("ADD A,#${v:02X}") }
        0x25 => { let d = rd8(mem, off); format!("ADD A,${d:02X}") }
        0x26 => "ADD A,@R0".to_string(),
        0x27 => "ADD A,@R1".to_string(),
        0x28..=0x2F => format!("ADD A,{}", r(op)),
        0x30 => { let b = rd8(mem, off); let t = rel8(mem, off); format!("JNB ${b:02X},{t}") }
        0x31 | 0x51 | 0x71 | 0x91 | 0xB1 | 0xD1 | 0xF1 => { let a = addr11(mem, off, op); format!("ACALL {a}") }
        0x32 => "RETI".to_string(),
        0x33 => "RLC A".to_string(),
        0x34 => { let v = rd8(mem, off); format!("ADDC A,#${v:02X}") }
        0x35 => { let d = rd8(mem, off); format!("ADDC A,${d:02X}") }
        0x36 => "ADDC A,@R0".to_string(),
        0x37 => "ADDC A,@R1".to_string(),
        0x38..=0x3F => format!("ADDC A,{}", r(op)),
        0x40 => { let t = rel8(mem, off); format!("JC {t}") }
        0x42 => { let d = rd8(mem, off); format!("ORL ${d:02X},A") }
        0x43 => { let d = rd8(mem, off); let v = rd8(mem, off); format!("ORL ${d:02X},#${v:02X}") }
        0x44 => { let v = rd8(mem, off); format!("ORL A,#${v:02X}") }
        0x45 => { let d = rd8(mem, off); format!("ORL A,${d:02X}") }
        0x46 => "ORL A,@R0".to_string(),
        0x47 => "ORL A,@R1".to_string(),
        0x48..=0x4F => format!("ORL A,{}", r(op)),
        0x50 => { let t = rel8(mem, off); format!("JNC {t}") }
        0x52 => { let d = rd8(mem, off); format!("ANL ${d:02X},A") }
        0x53 => { let d = rd8(mem, off); let v = rd8(mem, off); format!("ANL ${d:02X},#${v:02X}") }
        0x54 => { let v = rd8(mem, off); format!("ANL A,#${v:02X}") }
        0x55 => { let d = rd8(mem, off); format!("ANL A,${d:02X}") }
        0x56 => "ANL A,@R0".to_string(),
        0x57 => "ANL A,@R1".to_string(),
        0x58..=0x5F => format!("ANL A,{}", r(op)),
        0x60 => { let t = rel8(mem, off); format!("JZ {t}") }
        0x62 => { let d = rd8(mem, off); format!("XRL ${d:02X},A") }
        0x63 => { let d = rd8(mem, off); let v = rd8(mem, off); format!("XRL ${d:02X},#${v:02X}") }
        0x64 => { let v = rd8(mem, off); format!("XRL A,#${v:02X}") }
        0x65 => { let d = rd8(mem, off); format!("XRL A,${d:02X}") }
        0x66 => "XRL A,@R0".to_string(),
        0x67 => "XRL A,@R1".to_string(),
        0x68..=0x6F => format!("XRL A,{}", r(op)),
        0x70 => { let t = rel8(mem, off); format!("JNZ {t}") }
        0x72 => { let b = rd8(mem, off); format!("ORL C,${b:02X}") }
        0x74 => { let v = rd8(mem, off); format!("MOV A,#${v:02X}") }
        0x75 => { let d = rd8(mem, off); let v = rd8(mem, off); format!("MOV ${d:02X},#${v:02X}") }
        0x76 | 0x77 => format!("DB {op:02X}h"),
        0x78..=0x7F => { let v = rd8(mem, off); format!("MOV {},#${v:02X}", r(op)) }
        0x80 => { let t = rel8(mem, off); format!("SJMP {t}") }
        0x82 => { let b = rd8(mem, off); format!("ANL C,${b:02X}") }
        0x84 => "DIV AB".to_string(),
        0x85 => { let d1 = rd8(mem, off); let d2 = rd8(mem, off); format!("MOV ${d1:02X},${d2:02X}") }
        0x88..=0x8F => { let d = rd8(mem, off); format!("MOV ${d:02X},{}", r(op)) }
        0x90 => { let v = rd16be(mem, off); format!("MOV DPTR,#${v:04X}") }
        0x92 => { let b = rd8(mem, off); format!("MOV ${b:02X},C") }
        0x94 => { let v = rd8(mem, off); format!("SUBB A,#${v:02X}") }
        0x95 => { let d = rd8(mem, off); format!("SUBB A,${d:02X}") }
        0x96 => "SUBB A,@R0".to_string(),
        0x97 => "SUBB A,@R1".to_string(),
        0x98..=0x9F => format!("SUBB A,{}", r(op)),
        0xA0 => { let b = rd8(mem, off); format!("ORL C,/$${b:02X}") }
        0xA2 => { let b = rd8(mem, off); format!("MOV C,${b:02X}") }
        0xA3 => "INC DPTR".to_string(),
        0xA4 => "MUL AB".to_string(),
        0xA5 => format!("DB {op:02X}h"),
        0xA6 => { let d = rd8(mem, off); format!("MOV @R0,${d:02X}") }
        0xA7 => { let d = rd8(mem, off); format!("MOV @R1,${d:02X}") }
        0xA8..=0xAF => { let d = rd8(mem, off); format!("MOV {},${d:02X}", r(op)) }
        0xB0 => { let b = rd8(mem, off); format!("ANL C,/$${b:02X}") }
        0xB2 => { let b = rd8(mem, off); format!("CPL ${b:02X}") }
        0xB3 => "CPL C".to_string(),
        0xB4 => { let v = rd8(mem, off); let t = rel8(mem, off); format!("CJNE A,#${v:02X},{t}") }
        0xB5 => { let d = rd8(mem, off); let t = rel8(mem, off); format!("CJNE A,${d:02X},{t}") }
        0xB6 => { let v = rd8(mem, off); let t = rel8(mem, off); format!("CJNE @R0,#${v:02X},{t}") }
        0xB7 => { let v = rd8(mem, off); let t = rel8(mem, off); format!("CJNE @R1,#${v:02X},{t}") }
        0xB8..=0xBF => { let v = rd8(mem, off); let t = rel8(mem, off); format!("CJNE {},#${v:02X},{t}", r(op)) }
        0xC0 => { let d = rd8(mem, off); format!("PUSH ${d:02X}") }
        0xC2 => { let b = rd8(mem, off); format!("CLR ${b:02X}") }
        0xC3 => "CLR C".to_string(),
        0xC4 => "SWAP A".to_string(),
        0xC5 => { let d = rd8(mem, off); format!("XCH A,${d:02X}") }
        0xC6 => "XCH A,@R0".to_string(),
        0xC7 => "XCH A,@R1".to_string(),
        0xC8..=0xCF => format!("XCH A,{}", r(op)),
        0xD0 => { let d = rd8(mem, off); format!("POP ${d:02X}") }
        0xD2 => { let b = rd8(mem, off); format!("SETB ${b:02X}") }
        0xD3 => "SETB C".to_string(),
        0xD4 => "DA A".to_string(),
        0xD5 => { let d = rd8(mem, off); let t = rel8(mem, off); format!("DJNZ ${d:02X},{t}") }
        0xD6 => "XCHD A,@R0".to_string(),
        0xD7 => "XCHD A,@R1".to_string(),
        0xD8..=0xDF => { let t = rel8(mem, off); format!("DJNZ {},{t}", r(op)) }
        0xE0 => "MOVX A,@DPTR".to_string(),
        0xE2 => "MOVX A,@R0".to_string(),
        0xE3 => "MOVX A,@R1".to_string(),
        0xE4 => "CLR A".to_string(),
        0xE5 => { let d = rd8(mem, off); format!("MOV A,${d:02X}") }
        0xE6 => "MOV A,@R0".to_string(),
        0xE7 => "MOV A,@R1".to_string(),
        0xE8..=0xEF => format!("MOV A,{}", r(op)),
        0xF0 => "MOVX @DPTR,A".to_string(),
        0xF2 => "MOVX @R0,A".to_string(),
        0xF3 => "MOVX @R1,A".to_string(),
        0xF4 => "CPL A".to_string(),
        0xF5 => { let d = rd8(mem, off); format!("MOV ${d:02X},A") }
        0xF6 => "MOV @R0,A".to_string(),
        0xF7 => "MOV @R1,A".to_string(),
        0xF8..=0xFF => format!("MOV {},A", r(op)),
        _ => format!("DB {op:02X}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(bytes: &[u8]) -> String {
        let size = 0x10000;
        let mut m = Mem::new(size);
        m.load(0, bytes);
        disasm(&m, 0, 1)[0].text.clone()
    }

    #[test]
    fn disasm_mov_ljmp_sjmp() {
        assert_eq!(d(&[0x74, 0x2A]), "MOV A,#$2A");
        assert_eq!(d(&[0x02, 0x12, 0x34]), "LJMP $1234");
        assert!(d(&[0x80, 0xFB]).starts_with("SJMP $"));
    }

    #[test]
    fn disasm_db_fallback() {
        assert_eq!(d(&[0xA5]), "DB A5h");
        assert_eq!(d(&[0x00]), "NOP");
    }
}
