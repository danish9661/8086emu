//! MOS 6502 8-bit core (binary arithmetic; decimal mode flag is tracked but
//! ADC/SBC use binary arithmetic). 64 KiB flat address space; page 0 = zero
//! page, page 1 = stack (SP offset by 0x100). Vectors live at $FFFA..$FFFF.
//! `BRK` halts; `RTI` returns. The tiny I/O convention mirrors the others: a
//! `WCHR`/`OUT` could be added, but std output uses a `0x0000`? — we expose
//! `Output` and a `serial` port: writing port `0x01` prints A (kept for IDE
//! parity with 8085). Also `WCHR` is exposed via the `out`/serial path.

use crate::cpu::{Cpu, Mem, Output, FlagSet, Reg, Disasm, RunResult};

const STACK_BASE: u32 = 0x100;

#[derive(Clone)]
pub struct Cpu6502 {
    pub mem: Mem,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub sp: u8,
    pub p: u8, // flags: N V - B D I Z C
    pub halt: bool,
    pub out: Output,
    pub halted_reason: Option<String>,
    pub ports: [u8; 256],
}

impl Default for Cpu6502 {
    fn default() -> Self {
        let mut c = Cpu6502 {
            mem: Mem::new(1 << 16),
            a: 0, x: 0, y: 0, pc: 0, sp: 0xFD, p: 0x24,
            halt: false, out: Output::default(), halted_reason: None, ports: [0; 256],
        };
        c.reset();
        c
    }
}

impl Cpu6502 {
    // flag bit positions
    const C: u8 = 0;
    const Z: u8 = 1;
    const I: u8 = 2;
    const D: u8 = 3;
    const B: u8 = 4;
    const V: u8 = 6;
    const N: u8 = 7;

    fn get(&self, b: u8) -> bool { (self.p >> b) & 1 != 0 }
    fn set(&mut self, b: u8, v: bool) {
        if v { self.p |= 1 << b; } else { self.p &= !(1 << b); }
    }
    fn set_nz(&mut self, v: u8) {
        self.set(Self::Z, v == 0);
        self.set(Self::N, v & 0x80 != 0);
    }
    fn rd(&self, a: u32) -> u8 { self.mem.read(a as usize) }
    fn wr(&mut self, a: u32, v: u8) { self.mem.write(a as usize, v); }
    fn push(&mut self, v: u8) {
        self.wr(STACK_BASE + self.sp as u32, v);
        self.sp = self.sp.wrapping_sub(1);
    }
    fn pop(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.rd(STACK_BASE + self.sp as u32)
    }
    fn fetch(&mut self) -> u8 {
        let v = self.rd(self.pc as u32);
        self.pc = self.pc.wrapping_add(1);
        v
    }
    fn fetch16(&mut self) -> u16 {
        let lo = self.fetch();
        let hi = self.fetch();
        lo as u16 | ((hi as u16) << 8)
    }
    /// Load a ROM image at `addr` (also marks that range read-only).
    pub fn load_rom(&mut self, data: &[u8], addr: u32) {
        self.mem.load(addr as usize, data);
        self.mem.set_rom(addr as usize, data.len());
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Inst {
    LDA, LDX, LDY, STA, STX, STY, ADC, SBC, INC, DEC, AND, ORA, EOR, ASL, LSR,
    ROL, ROR, CMP, CPX, CPY, BIT, JMP, JSR, RTS, RTI, BCC, BCS, BEQ, BNE,
    BMI, BPL, BVC, BVS, CLC, SEC, CLI, SEI, CLV, CLD, SED, TAX, TAY, TSX,
    TXA, TXS, TYA, DEX, DEY, INX, INY, PHA, PHP, PLA, PLP, NOP, BRK,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode { IMP, IMM, ZP, ZPX, ZPY, IZX, IZY, ABS, ABX, ABY, IND, REL }

fn decode(op: u8) -> Option<(Inst, Mode)> {
    use Inst::*; use Mode::*;
    Some(match op {
        0xA9 => (LDA, IMM), 0xA5 => (LDA, ZP), 0xB5 => (LDA, ZPX), 0xAD => (LDA, ABS),
        0xBD => (LDA, ABX), 0xB9 => (LDA, ABY), 0xA1 => (LDA, IZX), 0xB1 => (LDA, IZY),
        0xA2 => (LDX, IMM), 0xA6 => (LDX, ZP), 0xB6 => (LDX, ZPY), 0xAE => (LDX, ABS),
        0xBE => (LDX, ABY),
        0xA0 => (LDY, IMM), 0xA4 => (LDY, ZP), 0xB4 => (LDY, ZPX), 0xAC => (LDY, ABS),
        0xBC => (LDY, ABX),
        0x85 => (STA, ZP), 0x95 => (STA, ZPX), 0x8D => (STA, ABS), 0x9D => (STA, ABX),
        0x99 => (STA, ABY), 0x81 => (STA, IZX), 0x91 => (STA, IZY),
        0x86 => (STX, ZP), 0x96 => (STX, ZPY), 0x8E => (STX, ABS),
        0x84 => (STY, ZP), 0x94 => (STY, ZPX), 0x8C => (STY, ABS),
        0x69 => (ADC, IMM), 0x65 => (ADC, ZP), 0x75 => (ADC, ZPX), 0x6D => (ADC, ABS),
        0x7D => (ADC, ABX), 0x79 => (ADC, ABY), 0x61 => (ADC, IZX), 0x71 => (ADC, IZY),
        0xE9 => (SBC, IMM), 0xE5 => (SBC, ZP), 0xF5 => (SBC, ZPX), 0xED => (SBC, ABS),
        0xFD => (SBC, ABX), 0xF9 => (SBC, ABY), 0xE1 => (SBC, IZX), 0xF1 => (SBC, IZY),
        0xC6 => (DEC, ZP), 0xD6 => (DEC, ZPX), 0xCE => (DEC, ABS), 0xDE => (DEC, ABX),
        0xE6 => (INC, ZP), 0xF6 => (INC, ZPX), 0xEE => (INC, ABS), 0xFE => (INC, ABX),
        0x29 => (AND, IMM), 0x25 => (AND, ZP), 0x35 => (AND, ZPX), 0x2D => (AND, ABS),
        0x3D => (AND, ABX), 0x39 => (AND, ABY), 0x21 => (AND, IZX), 0x31 => (AND, IZY),
        0x09 => (ORA, IMM), 0x05 => (ORA, ZP), 0x15 => (ORA, ZPX), 0x0D => (ORA, ABS),
        0x1D => (ORA, ABX), 0x19 => (ORA, ABY), 0x01 => (ORA, IZX), 0x11 => (ORA, IZY),
        0x49 => (EOR, IMM), 0x45 => (EOR, ZP), 0x55 => (EOR, ZPX), 0x4D => (EOR, ABS),
        0x5D => (EOR, ABX), 0x59 => (EOR, ABY), 0x41 => (EOR, IZX), 0x51 => (EOR, IZY),
        0x0A => (ASL, IMP), 0x06 => (ASL, ZP), 0x16 => (ASL, ZPX), 0x0E => (ASL, ABS), 0x1E => (ASL, ABX),
        0x4A => (LSR, IMP), 0x46 => (LSR, ZP), 0x56 => (LSR, ZPX), 0x4E => (LSR, ABS), 0x5E => (LSR, ABX),
        0x2A => (ROL, IMP), 0x26 => (ROL, ZP), 0x36 => (ROL, ZPX), 0x2E => (ROL, ABS), 0x3E => (ROL, ABX),
        0x6A => (ROR, IMP), 0x66 => (ROR, ZP), 0x76 => (ROR, ZPX), 0x6E => (ROR, ABS), 0x7E => (ROR, ABX),
        0xC9 => (CMP, IMM), 0xC5 => (CMP, ZP), 0xD5 => (CMP, ZPX), 0xCD => (CMP, ABS),
        0xDD => (CMP, ABX), 0xD9 => (CMP, ABY), 0xC1 => (CMP, IZX), 0xD1 => (CMP, IZY),
        0xE0 => (CPX, IMM), 0xE4 => (CPX, ZP), 0xEC => (CPX, ABS),
        0xC0 => (CPY, IMM), 0xC4 => (CPY, ZP), 0xCC => (CPY, ABS),
        0x24 => (BIT, ZP), 0x2C => (BIT, ABS),
        0x4C => (JMP, ABS), 0x6C => (JMP, IND), 0x20 => (JSR, ABS), 0x60 => (RTS, IMP), 0x40 => (RTI, IMP),
        0x90 => (BCC, REL), 0xB0 => (BCS, REL), 0xF0 => (BEQ, REL), 0xD0 => (BNE, REL),
        0x30 => (BMI, REL), 0x10 => (BPL, REL), 0x50 => (BVC, REL), 0x70 => (BVS, REL),
        0x18 => (CLC, IMP), 0x38 => (SEC, IMP), 0x58 => (CLI, IMP), 0x78 => (SEI, IMP),
        0xB8 => (CLV, IMP), 0xD8 => (CLD, IMP), 0xF8 => (SED, IMP),
        0xAA => (TAX, IMP), 0xA8 => (TAY, IMP), 0xBA => (TSX, IMP), 0x8A => (TXA, IMP),
        0x9A => (TXS, IMP), 0x98 => (TYA, IMP),
        0xCA => (DEX, IMP), 0x88 => (DEY, IMP), 0xE8 => (INX, IMP), 0xC8 => (INY, IMP),
        0x48 => (PHA, IMP), 0x08 => (PHP, IMP), 0x68 => (PLA, IMP), 0x28 => (PLP, IMP),
        0xEA => (NOP, IMP), 0x00 => (BRK, IMP),
        _ => return None,
    })
}

impl Cpu6502 {
    fn operand_addr(&mut self, mode: Mode) -> u32 {
        match mode {
            Mode::IMP | Mode::REL => 0,
            Mode::IMM => { let a = self.pc as u32; self.pc = self.pc.wrapping_add(1); a }
            Mode::ZP => self.fetch() as u32,
            Mode::ZPX => (self.fetch().wrapping_add(self.x)) as u32,
            Mode::ZPY => (self.fetch().wrapping_add(self.y)) as u32,
            Mode::IZX => {
                let z = self.fetch().wrapping_add(self.x);
                let lo = self.rd(z as u32);
                let hi = self.rd(z.wrapping_add(1) as u32);
                lo as u32 | ((hi as u32) << 8)
            }
            Mode::IZY => {
                let z = self.fetch();
                let lo = self.rd(z as u32);
                let hi = self.rd(z.wrapping_add(1) as u32);
                let base = lo as u32 | ((hi as u32) << 8);
                base.wrapping_add(self.y as u32)
            }
            Mode::ABS => self.fetch16() as u32,
            Mode::ABX => (self.fetch16() as u32).wrapping_add(self.x as u32) as u32,
            Mode::ABY => (self.fetch16() as u32).wrapping_add(self.y as u32) as u32,
            Mode::IND => {
                let ptr = self.fetch16();
                let lo = self.rd(ptr as u32);
                let hi = self.rd((ptr & 0xFF00 | (ptr.wrapping_add(1) & 0xFF)) as u32);
                lo as u32 | ((hi as u32) << 8)
            }
        }
    }

    fn execute(&mut self, inst: Inst, mode: Mode, addr: u32) {
        use Inst::*; use Mode::*;
        match inst {
            LDA => { let v = self.rd(addr); self.a = v; self.set_nz(v); }
            LDX => { let v = self.rd(addr); self.x = v; self.set_nz(v); }
            LDY => { let v = self.rd(addr); self.y = v; self.set_nz(v); }
            STA => self.wr(addr, self.a),
            STX => self.wr(addr, self.x),
            STY => self.wr(addr, self.y),
            AND => { let v = self.a & self.rd(addr); self.a = v; self.set_nz(v); }
            ORA => { let v = self.a | self.rd(addr); self.a = v; self.set_nz(v); }
            EOR => { let v = self.a ^ self.rd(addr); self.a = v; self.set_nz(v); }
            ADC => {
                let m = self.rd(addr);
                let c = if self.get(Self::C) { 1u16 } else { 0 };
                let sum = self.a as u16 + m as u16 + c;
                let r = sum as u8;
                self.set(Self::C, sum > 0xFF);
                self.set(Self::V, ((self.a ^ r) & (m ^ r) & 0x80) != 0);
                self.a = r; self.set_nz(r);
            }
            SBC => {
                let m = self.rd(addr);
                let c = if self.get(Self::C) { 1i16 } else { 0 };
                let diff = self.a as i16 - m as i16 - (1 - c);
                let r = (diff & 0xFF) as u8;
                self.set(Self::C, diff >= 0);
                self.set(Self::V, ((self.a ^ r) & (self.a ^ m) & 0x80) != 0);
                self.a = r; self.set_nz(r);
            }
            CMP => self.cmp(self.a, self.rd(addr)),
            CPX => self.cmp(self.x, self.rd(addr)),
            CPY => self.cmp(self.y, self.rd(addr)),
            BIT => {
                let m = self.rd(addr);
                self.set(Self::Z, (self.a & m) == 0);
                self.set(Self::V, m & 0x40 != 0);
                self.set(Self::N, m & 0x80 != 0);
            }
            ASL => { let r = self.shift(true, true, mode == IMP, self.a, addr); if mode == IMP { self.a = r; } }
            LSR => { let r = self.shift(true, false, mode == IMP, self.a, addr); if mode == IMP { self.a = r; } }
            ROL => { let r = self.shift(false, true, mode == IMP, self.a, addr); if mode == IMP { self.a = r; } }
            ROR => { let r = self.shift(false, false, mode == IMP, self.a, addr); if mode == IMP { self.a = r; } }
            INC => { let v = self.rd(addr).wrapping_add(1); self.wr(addr, v); self.set_nz(v); }
            DEC => { let v = self.rd(addr).wrapping_sub(1); self.wr(addr, v); self.set_nz(v); }
            JMP => { self.pc = addr as u16; }
            JSR => {
                let ret = self.pc.wrapping_sub(1);
                self.push((ret >> 8) as u8);
                self.push(ret as u8);
                self.pc = addr as u16;
            }
            RTS => { let lo = self.pop(); let hi = self.pop(); self.pc = (lo as u16 | ((hi as u16) << 8)).wrapping_add(1); }
            RTI => { self.plp(); let lo = self.pop(); let hi = self.pop(); self.pc = lo as u16 | ((hi as u16) << 8); }
            BCC => self.branch(!self.get(Self::C)),
            BCS => self.branch(self.get(Self::C)),
            BEQ => self.branch(self.get(Self::Z)),
            BNE => self.branch(!self.get(Self::Z)),
            BMI => self.branch(self.get(Self::N)),
            BPL => self.branch(!self.get(Self::N)),
            BVC => self.branch(!self.get(Self::V)),
            BVS => self.branch(self.get(Self::V)),
            CLC => self.set(Self::C, false),
            SEC => self.set(Self::C, true),
            CLI => self.set(Self::I, false),
            SEI => self.set(Self::I, true),
            CLV => self.set(Self::V, false),
            CLD => self.set(Self::D, false),
            SED => self.set(Self::D, true),
            TAX => { self.x = self.a; self.set_nz(self.x); }
            TAY => { self.y = self.a; self.set_nz(self.y); }
            TSX => { self.x = self.sp; self.set_nz(self.x); }
            TXA => { self.a = self.x; self.set_nz(self.a); }
            TXS => { self.sp = self.x; }
            TYA => { self.a = self.y; self.set_nz(self.a); }
            DEX => { self.x = self.x.wrapping_sub(1); self.set_nz(self.x); }
            DEY => { self.y = self.y.wrapping_sub(1); self.set_nz(self.y); }
            INX => { self.x = self.x.wrapping_add(1); self.set_nz(self.x); }
            INY => { self.y = self.y.wrapping_add(1); self.set_nz(self.y); }
            PHA => self.push(self.a),
            PHP => self.push(self.p | (1 << Self::B) | (1 << 5)),
            PLA => { self.a = self.pop(); self.set_nz(self.a); }
            PLP => self.plp(),
            BRK => { self.halt = true; self.halted_reason = Some("BRK".into()); }
            NOP => {}
        }
    }

    fn shift(&mut self, left: bool, through_c: bool, acc: bool, val: u8, addr: u32) -> u8 {
        let v = if acc { val } else { self.rd(addr) };
        let c_in = if through_c { if self.get(Self::C) { 1u8 } else { 0 } } else { 0 };
        let (r, newc) = if left {
            ((v << 1) | c_in, v & 0x80 != 0)
        } else {
            ((v >> 1) | (c_in << 7), v & 1 != 0)
        };
        if !acc { self.wr(addr, r); }
        self.set(Self::C, newc);
        self.set_nz(r);
        r
    }

    fn cmp(&mut self, reg: u8, m: u8) {
        let r = reg.wrapping_sub(m);
        self.set(Self::C, reg >= m);
        self.set_nz(r);
    }

    fn branch(&mut self, take: bool) {
        let off = self.fetch() as i8;
        if take {
            self.pc = (self.pc as i32 + off as i32) as u16;
        }
    }

    fn plp(&mut self) {
        let v = self.pop();
        // Restore flags; the pushed B (bit 4) is not a live flag and bit 5 is
        // always 1 in P.
        self.p = (v & !(1 << Self::B)) | (1 << 5);
    }

    /// Decode one instruction to a string for the disassembler.
    pub fn decode_str(&self, op: u8, pc: u16) -> (String, u16) {
        let (inst, mode) = match decode(op) { Some(x) => x, None => return (format!(".byte ${op:02X}"), pc + 1) };
        let mnem = match inst {
            Inst::LDA => "LDA", Inst::LDX => "LDX", Inst::LDY => "LDY", Inst::STA => "STA",
            Inst::STX => "STX", Inst::STY => "STY", Inst::ADC => "ADC", Inst::SBC => "SBC",
            Inst::INC => "INC", Inst::DEC => "DEC", Inst::AND => "AND", Inst::ORA => "ORA",
            Inst::EOR => "EOR", Inst::ASL => "ASL", Inst::LSR => "LSR", Inst::ROL => "ROL",
            Inst::ROR => "ROR", Inst::CMP => "CMP", Inst::CPX => "CPX", Inst::CPY => "CPY",
            Inst::BIT => "BIT", Inst::JMP => "JMP", Inst::JSR => "JSR", Inst::RTS => "RTS",
            Inst::RTI => "RTI", Inst::BCC => "BCC", Inst::BCS => "BCS", Inst::BEQ => "BEQ",
            Inst::BNE => "BNE", Inst::BMI => "BMI", Inst::BPL => "BPL", Inst::BVC => "BVC",
            Inst::BVS => "BVS", Inst::CLC => "CLC", Inst::SEC => "SEC", Inst::CLI => "CLI",
            Inst::SEI => "SEI", Inst::CLV => "CLV", Inst::CLD => "CLD", Inst::SED => "SED",
            Inst::TAX => "TAX", Inst::TAY => "TAY", Inst::TSX => "TSX", Inst::TXA => "TXA",
            Inst::TXS => "TXS", Inst::TYA => "TYA", Inst::DEX => "DEX", Inst::DEY => "DEY",
            Inst::INX => "INX", Inst::INY => "INY", Inst::PHA => "PHA", Inst::PHP => "PHP",
            Inst::PLA => "PLA", Inst::PLP => "PLP", Inst::BRK => "BRK", Inst::NOP => "NOP",
        };
        let (operand, next) = self.mode_text(mode, pc);
        (format!("{mnem} {operand}"), next)
    }

    fn mode_text(&self, mode: Mode, pc: u16) -> (String, u16) {
        let mut p = pc.wrapping_add(1);
        match mode {
            Mode::IMP => ("".into(), p),
            Mode::IMM => { let v = self.rd(p as u32); p = p.wrapping_add(1); (format!("#${v:02X}"), p) }
            Mode::ZP => { let v = self.rd(p as u32); p = p.wrapping_add(1); (format!("${v:02X}"), p) }
            Mode::ZPX => { let v = self.rd(p as u32); p = p.wrapping_add(1); (format!("${v:02X},X"), p) }
            Mode::ZPY => { let v = self.rd(p as u32); p = p.wrapping_add(1); (format!("${v:02X},Y"), p) }
            Mode::IZX => { let v = self.rd(p as u32); p = p.wrapping_add(1); (format!("(${v:02X},X)"), p) }
            Mode::IZY => { let v = self.rd(p as u32); p = p.wrapping_add(1); (format!("(${v:02X}),Y"), p) }
            Mode::ABS => { let a16 = self.abs16(p); p = p.wrapping_add(2); (format!("${a16:04X}"), p) }
            Mode::ABX => { let a16 = self.abs16(p); p = p.wrapping_add(2); (format!("${a16:04X},X"), p) }
            Mode::ABY => { let a16 = self.abs16(p); p = p.wrapping_add(2); (format!("${a16:04X},Y"), p) }
            Mode::IND => { let a16 = self.abs16(p); p = p.wrapping_add(2); (format!("(${a16:04X})"), p) }
            Mode::REL => { let off = self.rd(p as u32) as i8; p = p.wrapping_add(1); let t = (p as i32 + off as i32) as u16; (format!("${t:04X}"), p) }
        }
    }

    fn abs16(&self, p: u16) -> u16 {
        let lo = self.rd(p as u32);
        let hi = self.rd(p.wrapping_add(1) as u32);
        lo as u16 | ((hi as u16) << 8)
    }
}

impl Cpu for Cpu6502 {
    fn reset(&mut self) {
        self.a = 0; self.x = 0; self.y = 0; self.sp = 0xFD; self.p = 0x24;
        self.halt = false; self.halted_reason = None; self.out = Output::default();
        // fetch reset vector
        let lo = self.mem.read(0xFFFC);
        let hi = self.mem.read(0xFFFD);
        self.pc = lo as u16 | ((hi as u16) << 8);
    }

    fn step(&mut self) -> bool {
        if self.halt { return false; }
        let op = self.fetch();
        let (inst, mode) = match decode(op) {
            Some(x) => x,
            None => { self.halt = true; self.halted_reason = Some(format!("illegal opcode ${op:02X}")); return false; }
        };
        let addr = self.operand_addr(mode);
        // side-effecting I/O hook: writing port 0x01 prints A
        if inst == Inst::STA {
            if (mode == Mode::ZP || mode == Mode::ABS) && addr == 0x01 { self.out.put_char(self.a as char); }
            if (mode == Mode::ZP || mode == Mode::ABS) && addr == 0xF001 { self.out.put_char(self.a as char); }
        }
        self.execute(inst, mode, addr);
        true
    }

    fn pc(&self) -> u32 { self.pc as u32 }
    fn set_pc(&mut self, addr: u32) { self.pc = addr as u16; }
    fn set_reg(&mut self, name: &str, val: u32) {
        match name.to_ascii_uppercase().as_str() {
            "A" | "ACC" => self.a = val as u8,
            "X" => self.x = val as u8,
            "Y" => self.y = val as u8,
            "PC" => self.pc = val as u16,
            "SP" => self.sp = val as u8,
            "P" | "PSR" | "FLAGS" => self.p = val as u8,
            _ => {}
        }
    }
    fn regs(&self) -> Vec<Reg> {
        vec![
            Reg::new("A", self.a as u32),
            Reg::new("X", self.x as u32),
            Reg::new("Y", self.y as u32),
            Reg::new("PC", self.pc as u32),
            Reg::new("SP", self.sp as u32),
            Reg::new("P", self.p as u32),
        ]
    }
    fn flags(&self) -> FlagSet {
        FlagSet {
            carry: self.get(Self::C),
            zero: self.get(Self::Z),
            sign: self.get(Self::N),
            overflow: self.get(Self::V),
            interrupt: self.get(Self::I),
            direction: self.get(Self::D),
            ..Default::default()
        }
    }
    fn mem_read(&self, addr: u32, len: usize) -> Vec<u8> {
        (0..len).map(|i| self.rd(addr + i as u32)).collect()
    }
    fn mem_write(&mut self, addr: u32, data: &[u8]) {
        for (i, b) in data.iter().enumerate() { self.wr(addr + i as u32, *b); }
    }
    fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(8 + self.mem.size());
        v.extend_from_slice(&self.pc.to_le_bytes());
        v.push(self.a); v.push(self.x); v.push(self.y); v.push(self.sp); v.push(self.p);
        v.push(if self.halt { 1 } else { 0 });
        v.extend_from_slice(&self.mem.data);
        v
    }
    fn restore(&mut self, data: &[u8]) {
        let mut o = 0;
        let g4 = |d: &[u8], p: &mut usize| { let v = u16::from_le_bytes([d[*p], d[*p+1]]); *p += 2; v };
        self.pc = g4(data, &mut o);
        self.a = data[o]; self.x = data[o+1]; self.y = data[o+2]; self.sp = data[o+3]; self.p = data[o+4];
        o += 5;
        self.halt = data[o] != 0; o += 1;
        for b in &mut self.mem.data { *b = data[o]; o += 1; }
    }
    fn is_halted(&self) -> bool { self.halt }

    fn disasm(&self, addr: u32, count: usize) -> Vec<Disasm> {
        let mut out = Vec::new();
        let mut a = addr as u16;
        for _ in 0..count {
            let op = self.rd(a as u32);
            let (text, next) = self.decode_str(op, a);
            let len = (next.wrapping_sub(a)) as usize;
            let bytes: Vec<u8> = (0..len).map(|i| self.rd(a.wrapping_add(i as u16) as u32)).collect();
            out.push(Disasm { addr: a as u32, bytes, text });
            a = next;
        }
        out
    }
}
