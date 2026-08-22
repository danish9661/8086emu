//! Intel 8085 disassembler — decodes instructions from the flat 64 KiB memory
//! for the IDE's disassembly view. Unrecognized opcodes fall back to `DB`.

use crate::cpu::{Disasm, Mem};

const R: [&str; 8] = ["B", "C", "D", "E", "H", "L", "M", "A"];
const RP: [&str; 4] = ["B", "D", "H", "SP"];

fn reg(n: u8) -> &'static str { R[(n & 7) as usize] }
fn rp(n: u8) -> &'static str { RP[(n & 3) as usize] }

fn rd8(mem: &Mem, off: &mut u32) -> u8 { let v = mem.read(*off as usize); *off += 1; v }
fn rd16(mem: &Mem, off: &mut u32) -> u16 { let lo = rd8(mem, off) as u16; let hi = rd8(mem, off) as u16; (hi << 8) | lo }

fn a16(mem: &Mem, off: &mut u32) -> String {
    let v = rd16(mem, off);
    format!("${:04X}", v as u32)
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
    match op {
        0x00 => "NOP".to_string(),
        0x08 | 0x10 | 0x18 | 0x28 | 0x38 => "*NOP".to_string(),
        0x20 => "RIM".to_string(),
        0x30 => "SIM".to_string(),
        0x01 | 0x11 | 0x21 | 0x31 => { let p = rp((op >> 4) & 3); let v = rd16(mem, off); format!("LXI {p},${v:04X}") }
        0x02 => "STAX B".to_string(),
        0x12 => "STAX D".to_string(),
        0x03 | 0x13 | 0x23 | 0x33 => { let p = rp((op >> 4) & 3); format!("INX {p}") }
        0x04..=0x3F => {
            let r = (op >> 3) & 7;
            let k = op & 7;
            match k {
                4 => format!("INR {}", reg(r)),
                5 => format!("DCR {}", reg(r)),
                6 => { let v = rd8(mem, off); format!("MVI {},${v:02X}", reg(r)) }
                7 => { let rot = ["RLC", "RRC", "RAL", "RAR"]; rot[(r & 3) as usize].to_string() }
                _ => format!("DB {op:02X}h"),
            }
        }
        0x09 | 0x19 | 0x29 | 0x39 => { let p = rp((op >> 4) & 3); format!("DAD {p}") }
        0x0A => "LDAX B".to_string(),
        0x1A => "LDAX D".to_string(),
        0x0B | 0x1B | 0x2B | 0x3B => { let p = rp((op >> 4) & 3); format!("DCX {p}") }
        0x22 => { let a = a16(mem, off); format!("SHLD {a}") }
        0x2A => { let a = a16(mem, off); format!("LHLD {a}") }
        0x32 => { let a = a16(mem, off); format!("STA {a}") }
        0x3A => { let a = a16(mem, off); format!("LDA {a}") }
        0x40..=0x7F => {
            let dst = (op >> 3) & 7;
            let src = op & 7;
            if op == 0x76 { "HLT".to_string() } else { format!("MOV {},{}", reg(dst), reg(src)) }
        }
        0x80..=0xBF => {
            let mnem = ["ADD", "ADC", "SUB", "SBB", "ANA", "XRA", "ORA", "CMP"][((op >> 3) & 7) as usize];
            format!("{} {}", mnem, reg(op & 7))
        }
        0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => {
            let cc = ["NZ", "Z", "NC", "C", "PO", "PE", "P", "M"][((op >> 3) & 7) as usize];
            format!("R{cc}")
        }
        0xC1 | 0xD1 | 0xE1 | 0xF1 => { let p = match (op >> 4) & 3 { 0 => "B", 1 => "D", 2 => "H", _ => "PSW" }; format!("POP {p}") }
        0xC2 => { let a = a16(mem, off); format!("JNZ {a}") }
        0xC3 => { let a = a16(mem, off); format!("JMP {a}") }
        0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => {
            let cc = ["NZ", "Z", "NC", "C", "PO", "PE", "P", "M"][((op >> 3) & 7) as usize];
            let a = a16(mem, off); format!("C{cc} {a}")
        }
        0xC5 | 0xD5 | 0xE5 | 0xF5 => { let p = match (op >> 4) & 3 { 0 => "B", 1 => "D", 2 => "H", _ => "PSW" }; format!("PUSH {p}") }
        0xC6 => { let v = rd8(mem, off); format!("ADI ${v:02X}") }
        0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => { format!("RST {}", (op >> 3) & 7) }
        0xC9 => "RET".to_string(),
        0xCA => { let a = a16(mem, off); format!("JZ {a}") }
        0xCB => format!("DB {op:02X}h"),
        0xCD => { let a = a16(mem, off); format!("CALL {a}") }
        0xCE => { let v = rd8(mem, off); format!("ACI ${v:02X}") }
        0xD2 => { let a = a16(mem, off); format!("JNC {a}") }
        0xD3 => { let p = rd8(mem, off); format!("OUT ${p:02X}") }
        0xD6 => { let v = rd8(mem, off); format!("SUI ${v:02X}") }
        0xD9 => format!("DB {op:02X}h"),
        0xDA => { let a = a16(mem, off); format!("JC {a}") }
        0xDB => { let p = rd8(mem, off); format!("IN ${p:02X}") }
        0xDD => format!("DB {op:02X}h"),
        0xE2 => { let a = a16(mem, off); format!("JPO {a}") }
        0xE3 => "XTHL".to_string(),
        0xE6 => { let v = rd8(mem, off); format!("ANI ${v:02X}") }
        0xE9 => "PCHL".to_string(),
        0xEA => { let a = a16(mem, off); format!("JPE {a}") }
        0xEB => "XCHG".to_string(),
        0xED => format!("DB {op:02X}h"),
        0xEE => { let v = rd8(mem, off); format!("XRI ${v:02X}") }
        0xF2 => { let a = a16(mem, off); format!("JP {a}") }
        0xF3 => "DI".to_string(),
        0xF4 => { let a = a16(mem, off); format!("CP {a}") }
        0xF6 => { let v = rd8(mem, off); format!("ORI ${v:02X}") }
        0xF9 => "SPHL".to_string(),
        0xFA => { let a = a16(mem, off); format!("JM {a}") }
        0xFB => "EI".to_string(),
        0xFD => format!("DB {op:02X}h"),
        0xFE => { let v = rd8(mem, off); format!("CPI ${v:02X}") }
        _ => format!("DB {op:02X}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(bytes: &[u8]) -> String {
        let mut m = Mem::new(0x10000);
        m.load(0, bytes);
        disasm(&m, 0, 1)[0].text.clone()
    }

    #[test]
    fn disasm_mvi_call_hlt() {
        assert_eq!(d(&[0x3E, 0x42]), "MVI A,$42");
        assert_eq!(d(&[0x06, 0x10]), "MVI B,$10");
        assert_eq!(d(&[0xCD, 0x00, 0x10]), "CALL $1000");
        assert_eq!(d(&[0x76]), "HLT");
    }

    #[test]
    fn disasm_db_fallback() {
        assert_eq!(d(&[0xCB]), "DB CBh");
        assert_eq!(d(&[0x00]), "NOP");
    }
}
