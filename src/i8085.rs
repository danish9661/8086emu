//! Intel 8085 CPU core — full 8-bit ISA, 64 KiB memory.
//!
//! Output convention: `OUT 01h` prints the character in A to the Output
//! buffer (so lab programs can "print" headlessly).

use crate::cpu::{Cpu, FlagSet, Mem, Output, Reg};
use crate::i8155::I8155;
use crate::ppi8255::Ppi8255;
use crate::flash::ExternalFlash;
use crate::rtc::Rtc;
use crate::adc::Adc0808;
use crate::lcd::Lcd1602;
use crate::dma::Dma8237;

const MEM_SIZE: usize = 64 * 1024;
/// Default external SRAM chip size (8 KiB, like an Intel 6264).
const SRAM_SIZE: usize = 8 * 1024;

/// Approximate 8085 T-state (clock-cycle) cost per instruction. The 8155 timer
/// is clocked by these so its period tracks real time; 8085 T-states are
/// instruction-dependent (4–18), so this models the dominant cases. The 8086
/// PIT and 8051 timers use exact per-instruction counts.
fn i8085_tstates(op: u8) -> u8 {
    match op {
        0x00 => 4, // NOP
        0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x3E => 7, // MVI reg
        0x36 => 10, // MVI M
        0x07 | 0x0F | 0x17 | 0x1F | 0x27 | 0x2F | 0x37 => 4, // rotate / DAA
        0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x3C => 5, // INR reg
        0x34 => 10, // INR M
        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x3D => 5, // DCR reg
        0x35 => 10, // DCR M
        0x80..=0x87 => 4, // ADD reg
        0x86 => 7, // ADD M
        0x88..=0x8F => 4, // ADC
        0x8E => 7,
        0x90..=0x97 => 4, // SUB
        0x96 => 7,
        0x98..=0x9F => 4, // SBB
        0x9E => 7,
        0xA0..=0xA7 => 4, // ANA
        0xA6 => 7,
        0xA8..=0xAF => 4, // XRA
        0xAE => 7,
        0xB0..=0xB7 => 4, // ORA
        0xB6 => 7,
        0xB8..=0xBF => 4, // CMP
        0xBE => 7,
        0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => 7, // ALU imm
        0x09 | 0x19 | 0x29 | 0x39 => 10, // DAD
        0x01 | 0x11 | 0x21 | 0x31 => 10, // LXI
        0x32 | 0x3A | 0x22 | 0x2A => 13, // STA/LDA/SHLD/LHLD
        0x02 | 0x12 | 0x0A | 0x1A => 7, // STAX/LDAX
        0xC3 => 10, // JMP
        0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => 10, // Jcc
        0xCD => 17, // CALL
        0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => 17, // Ccc
        0xC9 => 10, // RET
        0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => 5, // Rcc
        0xE3 => 18, // XTHL
        0xE9 | 0xF9 => 5, // PCHL/SPHL
        0xC5 | 0xD5 | 0xE5 | 0xF5 => 11, // PUSH rp
        0xC1 | 0xD1 | 0xE1 | 0xF1 => 10, // POP rp
        0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => 11, // RST
        0xD3 | 0xDB => 10, // OUT / IN
        0xFB | 0xF3 | 0x76 => 4, // EI / DI / HLT
        _ => 4,
    }
}

pub struct Cpu8085 {
    pub a: u8, pub b: u8, pub c: u8, pub d: u8, pub e: u8,
    pub h: u8, pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub s: bool, pub z: bool, pub ac: bool, pub p: bool, pub cy: bool,
    pub mem: Mem,
    pub out: Output,
    pub halted: bool,
    pub int_enabled: bool,
    pub fault: Option<String>,
    pub mask_rst55: bool,
    pub mask_rst65: bool,
    pub mask_rst75: bool,
    pub pending_rst55: bool,
    pub pending_rst65: bool,
    pub pending_rst75: bool,
    pub pending_trap: bool,
    pub intr_vector: Option<u8>,
    pub sid: bool,
    pub sod: bool,
    /// I/O port space (256 ports); OUT to port 01h also prints A.
    pub ports: [u8; 256],
    /// External 8155 RAM/I/O/Timer (memory-mapped RAM at 0x8000..0x80FF,
    /// registers at I/O ports 0x80..0x85). Its 14-bit timer counts host
    /// clock cycles for cycle-accurate timing.
    pub i8155: I8155,
    /// External SRAM chip (memory-mapped, e.g. an 8 KiB 6264 at 0x9000).
    /// Its window is `[sram_base, sram_base + sram_len)` and is read/writable
    /// by instructions; it is stored separately from the main 64 KiB RAM.
    pub sram: Mem,
    pub sram_base: u32,
    pub sram_len: u32,
    /// 8255 PPI (0xE0..0xE3)
    pub ppi: Ppi8255,
    /// External Flash/EEPROM (8 KiB at 0xA000 for 8085)
    pub flash: ExternalFlash,
    /// RTC (0x70/0x71) + I2C shim 0x30/0x31
    pub rtc: Rtc,
    /// ADC0808 (0x28/0x29) + LCD1602 (0x38/0x39)
    pub adc: Adc0808,
    pub lcd: Lcd1602,
    /// 8237 DMA (0xD0..0xDF)
    pub dma: Dma8237,
    /// Total host clock cycles (T-states) executed — drives the 8155 timer.
    pub cycles: u64,
    /// Opcode of the most recently decoded instruction (for cycle accounting).
    last_op: u8,
}

impl Default for Cpu8085 {
    fn default() -> Self { Self::new() }
}

impl Cpu8085 {
    pub fn new() -> Self {
        let mut c = Cpu8085 {
            a: 0, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0,
            sp: 0xFFFF, pc: 0,
            s: false, z: false, ac: false, p: false, cy: false,
            mem: Mem::new(MEM_SIZE),
            out: Output::default(),
            halted: false,
            int_enabled: false,
            fault: None,
            mask_rst55: false, mask_rst65: false, mask_rst75: false,
            pending_rst55: false, pending_rst65: false, pending_rst75: false,
            pending_trap: false,
            intr_vector: None,
            sid: false, sod: false,
            ports: [0; 256],
            i8155: I8155::new(),
            sram: Mem::new(SRAM_SIZE),
            sram_base: 0x9000,
            sram_len: SRAM_SIZE as u32,
            ppi: Ppi8255::new(),
            flash: { let mut f = ExternalFlash::new(); f.configure(0xA000, 0x2000); f },
            rtc: Rtc::new(),
            adc: Adc0808::new(),
            lcd: Lcd1602::new(),
            dma: Dma8237::new(),
            cycles: 0,
            last_op: 0,
        };
        c.reset();
        c
    }

    pub fn last_error(&self) -> Option<String> { self.fault.clone() }

    /// Mark `[base, base+len)` of the main 64 KiB RAM as read-only ROM
    /// (e.g. a monitor/BIOS image you load with `load_rom`).
    pub fn set_rom_region(&mut self, base: u32, len: u32) {
        self.mem.set_rom(base as usize, len as usize);
    }

    /// Load a ROM image at `addr` and mark that range read-only.
    pub fn load_rom(&mut self, data: &[u8], addr: u32) {
        self.mem.set_rom(addr as usize, data.len());
        self.mem.load(addr as usize, data);
    }

    /// (Re)configure the external SRAM chip window (default 8 KiB at 0x9000).
    pub fn set_sram(&mut self, base: u32, len: u32) {
        let size = len.max(1) as usize;
        let size = size.next_power_of_two().max(1);
        self.sram = Mem::new(size);
        self.sram_base = base;
        self.sram_len = size as u32;
    }

    #[inline] fn parity(&self, x: u8) -> bool { (x.count_ones() & 1) == 0 }

    fn rp(&self, rp: u8) -> u16 {
        match rp {
            0 => (self.b as u16) << 8 | self.c as u16,
            1 => (self.d as u16) << 8 | self.e as u16,
            2 => (self.h as u16) << 8 | self.l as u16,
            _ => self.sp,
        }
    }
    fn set_rp(&mut self, rp: u8, v: u16) {
        match rp {
            0 => { self.b = (v >> 8) as u8; self.c = v as u8; }
            1 => { self.d = (v >> 8) as u8; self.e = v as u8; }
            2 => { self.h = (v >> 8) as u8; self.l = v as u8; }
            _ => { self.sp = v; }
        }
    }

    #[inline] fn reg(&self, r: u8) -> u8 {
        match r { 0 => self.b, 1 => self.c, 2 => self.d, 3 => self.e, 4 => self.h, 5 => self.l, _ => self.a }
    }
    #[inline] fn set_reg(&mut self, r: u8, v: u8) {
        match r { 0 => self.b = v, 1 => self.c = v, 2 => self.d = v, 3 => self.e = v, 4 => self.h = v, 5 => self.l = v, _ => self.a = v }
    }

    fn m(&self) -> u8 { self.rd(self.hl()) }
    fn set_m(&mut self, v: u8) { self.wr(self.hl(), v) }

    /// Unified memory read: routes 8155 RAM window, the external SRAM window,
    /// and the main RAM (ROM-protected via `Mem`).
    fn rd(&self, a: usize) -> u8 {
        if (0x8000..=0x80FF).contains(&a) {
            self.i8155.ram_read(a - 0x8000)
        } else if self.flash.in_range(a as u32) {
            self.flash.read(a as u32)
        } else if (a as u32) >= self.sram_base && (a as u32) < self.sram_base + self.sram_len {
            self.sram.read(a - self.sram_base as usize)
        } else {
            self.mem.read(a)
        }
    }

    /// Unified memory write: same routing as `rd`, with ROM protection on the
    /// main RAM enforced by `Mem::write`.
    fn wr(&mut self, a: usize, v: u8) {
        if (0x8000..=0x80FF).contains(&a) {
            self.i8155.ram_write(a - 0x8000, v);
        } else if self.flash.in_range(a as u32) {
            self.flash.write(a as u32, v);
        } else if (a as u32) >= self.sram_base && (a as u32) < self.sram_base + self.sram_len {
            self.sram.write(a - self.sram_base as usize, v);
        } else {
            self.mem.write(a, v);
        }
    }
    #[inline] fn hl(&self) -> usize { ((self.h as usize) << 8) | self.l as usize }

    #[inline] fn fetch8(&mut self) -> u8 {
        let b = self.rd(self.pc as usize);
        self.pc = self.pc.wrapping_add(1);
        b
    }
    #[inline] fn fetch16(&mut self) -> u16 {
        let lo = self.fetch8() as u16;
        let hi = self.fetch8() as u16;
        lo | hi << 8
    }

    fn push16(&mut self, v: u16) {
        self.sp = self.sp.wrapping_sub(2);
        self.wr(self.sp as usize, v as u8);
        self.wr(self.sp.wrapping_add(1) as usize, (v >> 8) as u8);
    }
    fn pop16(&mut self) -> u16 {
        let lo = self.rd(self.sp as usize) as u16;
        let hi = self.rd(self.sp.wrapping_add(1) as usize) as u16;
        self.sp = self.sp.wrapping_add(2);
        lo | hi << 8
    }

    fn set_flags_arith(&mut self, r: u8) {
        self.s = r & 0x80 != 0;
        self.z = r == 0;
        self.p = self.parity(r);
    }
    fn set_flags_logic(&mut self, r: u8) {
        self.cy = false;
        self.set_flags_arith(r);
    }

    fn add(&mut self, v: u8) {
        let r = self.a.wrapping_add(v);
        self.cy = self.a as u16 + v as u16 > 0xFF;
        self.ac = (self.a & 0xF) + (v & 0xF) > 0xF;
        self.a = r;
        self.set_flags_arith(r);
    }
    fn adc(&mut self, v: u8) {
        let ci = self.cy as u16;
        let r = self.a as u16 + v as u16 + ci;
        self.cy = r > 0xFF;
        self.ac = (self.a & 0xF) + (v & 0xF) + ci as u8 > 0xF;
        self.a = r as u8;
        self.set_flags_arith(self.a);
    }
    fn sub(&mut self, v: u8) {
        let r = self.a.wrapping_sub(v);
        self.cy = self.a < v;
        self.ac = (self.a & 0xF) < (v & 0xF);
        self.a = r;
        self.set_flags_arith(r);
    }
    fn sbb(&mut self, v: u8) {
        let bi = self.cy as u16;
        let r = self.a as u16;
        self.cy = r < v as u16 + bi;
        self.ac = (self.a & 0xF) < (v & 0xF) + bi as u8;
        self.a = (r.wrapping_sub(v as u16).wrapping_sub(bi)) as u8;
        self.set_flags_arith(self.a);
    }
    fn cmp(&mut self, v: u8) {
        let r = self.a.wrapping_sub(v);
        self.cy = self.a < v;
        self.ac = (self.a & 0xF) < (v & 0xF);
        self.set_flags_arith(r);
    }
    fn ana(&mut self, v: u8) {
        self.a &= v;
        self.ac = ((self.a | v) & 0x08) != 0;
        self.set_flags_logic(self.a);
    }
    fn xra(&mut self, v: u8) {
        self.a ^= v;
        self.ac = false;
        self.set_flags_logic(self.a);
    }
    fn ora(&mut self, v: u8) {
        self.a |= v;
        self.ac = false;
        self.set_flags_logic(self.a);
    }

    fn inr(&mut self, r: u8) {
        let old = self.reg(r);
        let nv = old.wrapping_add(1);
        self.ac = (old & 0xF) == 0xF;
        self.set_reg(r, nv);
        self.set_flags_arith(nv); // CY untouched
    }
    fn dcr(&mut self, r: u8) {
        let old = self.reg(r);
        let nv = old.wrapping_sub(1);
        self.ac = (old & 0xF) == 0;
        self.set_reg(r, nv);
        self.set_flags_arith(nv);
    }

    fn cond(&self, cc: u8) -> bool {
        match cc {
            0 => !self.z, // NZ
            1 => self.z,  // Z
            2 => !self.cy, // NC
            3 => self.cy,  // C
            4 => !self.p,  // PO
            5 => self.p,   // PE
            6 => !self.s,  // P (positive)
            _ => self.s,   // M
        }
    }

    fn daa(&mut self) {
        let mut a = self.a;
        let mut cy = self.cy;
        let mut ac = self.ac;
        if (a & 0x0F) > 9 || ac {
            let lo = (a & 0x0F).wrapping_add(6);
            ac = lo > 0x0F;
            a = (a & 0xF0) | (lo & 0x0F);
        }
        if (a >> 4) > 9 || (a >> 4) == 9 && (a & 0x0F) > 9 || cy {
            let hi = (a >> 4).wrapping_add(6);
            a = (hi << 4) | (a & 0x0F);
            cy = true;
        }
        self.a = a;
        self.cy = cy;
        self.ac = ac;
        self.set_flags_arith(a);
    }

    fn unimplemented(&mut self, op: u8) {
        self.fault = Some(format!("8085: unimplemented opcode {op:02X}h at PC {:04X}h", self.pc.wrapping_sub(1)));
        self.halted = true;
    }

    pub fn exec(&mut self) {
        let op = self.fetch8();
        self.last_op = op;
        match op {
            // MOV r,r'
            0x40..=0x7F if op & 0x07 != 6 && (op >> 3) & 7 != 6 => {
                let src = op & 7; let dst = (op >> 3) & 7;
                self.set_reg(dst, self.reg(src));
            }
            0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x7E => { // MOV r,M
                let dst = (op >> 3) & 7;
                self.set_reg(dst, self.m());
            }
            0x70..=0x77 if op != 0x76 => { // MOV M,r (76 = HLT)
                self.set_m(self.reg(op & 7));
            }
            0x76 => self.halted = true,
            // MVI r,data / MVI M,data
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x3E => {
                let v = self.fetch8();
                self.set_reg((op >> 3) & 7, v);
            }
            0x36 => { let v = self.fetch8(); self.set_m(v); }
            // LXI / INX / DCX / DAD
            0x01 | 0x11 | 0x21 | 0x31 => { let rp = (op >> 4) & 3; let v = self.fetch16(); self.set_rp(rp, v); }
            0x03 | 0x13 | 0x23 | 0x33 => { let rp = (op >> 4) & 3; let v = self.rp(rp).wrapping_add(1); self.set_rp(rp, v); }
            0x0B | 0x1B | 0x2B | 0x3B => { let rp = (op >> 4) & 3; let v = self.rp(rp).wrapping_sub(1); self.set_rp(rp, v); }
            0x09 | 0x19 | 0x29 | 0x39 => {
                let rp = (op >> 4) & 3;
                let hl = self.hl() as u32;
                let r = self.rp(rp) as u32;
                let sum = hl + r;
                self.cy = sum > 0xFFFF;
                self.set_rp(2, sum as u16);
            }
            0x0A => { self.a = self.rd(((self.b as u16) << 8 | self.c as u16) as usize); }
            0x1A => { self.a = self.rd(((self.d as u16) << 8 | self.e as u16) as usize); }
            0x02 => { self.wr(((self.b as u16) << 8 | self.c as u16) as usize, self.a); }
            0x12 => { self.wr(((self.d as u16) << 8 | self.e as u16) as usize, self.a); }
            0x2A => { let a = self.fetch16() as usize; self.l = self.rd(a); self.h = self.rd(a.wrapping_add(1)); }
            0x22 => { let a = self.fetch16() as usize; self.wr(a, self.l); self.wr(a.wrapping_add(1), self.h); }
            0x3A => { let a = self.fetch16() as usize; self.a = self.rd(a); }
            0x32 => { let a = self.fetch16() as usize; self.wr(a, self.a); }
            0xEB => { std::mem::swap(&mut self.h, &mut self.d); std::mem::swap(&mut self.l, &mut self.e); }
            // ALU
            0x80..=0xBF => {
                let r = op & 7;
                let v = if r == 6 { self.m() } else { self.reg(r) };
                match op >> 3 {
                    0x10 => self.add(v),  // ADD
                    0x11 => self.adc(v),  // ADC
                    0x12 => self.sub(v),  // SUB
                    0x13 => self.sbb(v),  // SBB
                    0x14 => self.ana(v),  // ANA
                    0x15 => self.xra(v),  // XRA
                    0x16 => self.ora(v),  // ORA
                    _ => self.cmp(v),      // CMP
                }
            }
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                let v = self.fetch8();
                match op {
                    0xC6 => self.add(v), 0xCE => self.adc(v), 0xD6 => self.sub(v),
                    0xDE => self.sbb(v), 0xE6 => self.ana(v), 0xEE => self.xra(v),
                    0xF6 => self.ora(v), _ => self.cmp(v),
                }
            }
            // INR / DCR
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x3C => self.inr((op >> 3) & 7),
            0x34 => { let old = self.m(); let nv = old.wrapping_add(1); self.ac = (old & 0xF) == 0xF; self.set_m(nv); self.set_flags_arith(nv); }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x3D => self.dcr((op >> 3) & 7),
            0x35 => { let old = self.m(); let nv = old.wrapping_sub(1); self.ac = (old & 0xF) == 0; self.set_m(nv); self.set_flags_arith(nv); }
            // rotates / misc
            0x07 => { self.cy = self.a & 0x80 != 0; self.a = self.a.rotate_left(1); } // RLC (CY = MSB)
            0x0F => { self.cy = self.a & 1 != 0; self.a = self.a.rotate_right(1); }   // RRC (CY = LSB)
            0x17 => { let c = self.a >> 7; self.a = (self.a << 1) | self.cy as u8; self.cy = c != 0; } // RAL
            0x1F => { let c = self.a & 1; self.a = (self.a >> 1) | ((self.cy as u8) << 7); self.cy = c != 0; } // RAR
            0x27 => self.daa(),
            0x2F => self.a = !self.a, // CMA
            0x37 => self.cy = true,   // STC
            0x3F => self.cy = !self.cy, // CMC
            0x20 => { // RIM: SID | I7.5 | I6.5 | I5.5 | IE | M7.5 | M6.5 | M5.5
                self.a = (self.sid as u8) << 7
                    | (self.pending_rst75 as u8) << 6
                    | (self.pending_rst65 as u8) << 5
                    | (self.pending_rst55 as u8) << 4
                    | (self.int_enabled as u8) << 3
                    | (self.mask_rst75 as u8) << 2
                    | (self.mask_rst65 as u8) << 1
                    | self.mask_rst55 as u8;
            }
            0x30 => { // SIM: SOD | S1 | S0 | R7.5 | MSE | M7.5 | M6.5 | M5.5
                if self.a & 0x08 != 0 { // MSE
                    self.mask_rst55 = self.a & 0x01 != 0;
                    self.mask_rst65 = self.a & 0x02 != 0;
                    self.mask_rst75 = self.a & 0x04 != 0;
                }
                if self.a & 0x10 != 0 { self.pending_rst75 = false; } // R7.5
                self.sod = self.a & 0x80 != 0;
            }
            0x00 => {} // NOP
            // Jumps
            0xC3 | 0xCA | 0xC2 | 0xDA | 0xD2 | 0xFA | 0xF2 | 0xEA | 0xE2 => {
                let addr = self.fetch16();
                let cc = match op { 0xC3 => -1i8, 0xCA => 1, 0xC2 => 0, 0xDA => 3, 0xD2 => 2, 0xFA => 7, 0xF2 => 6, 0xEA => 5, _ => 4 };
                if cc < 0 || self.cond(cc as u8) { self.pc = addr; }
            }
            // Calls
            0xCD | 0xCC | 0xC4 | 0xDC | 0xD4 | 0xFC | 0xF4 | 0xEC | 0xE4 => {
                let addr = self.fetch16();
                let cc = match op { 0xCD => -1i8, 0xCC => 1, 0xC4 => 0, 0xDC => 3, 0xD4 => 2, 0xFC => 7, 0xF4 => 6, 0xEC => 5, _ => 4 };
                if cc < 0 || self.cond(cc as u8) {
                    let pc = self.pc;
                    self.push16(pc);
                    self.pc = addr;
                }
            }
            // Returns
            0xC9 | 0xC8 | 0xC0 | 0xD8 | 0xD0 | 0xF8 | 0xF0 | 0xE8 | 0xE0 => {
                let cc = match op { 0xC9 => -1i8, 0xC8 => 1, 0xC0 => 0, 0xD8 => 3, 0xD0 => 2, 0xF8 => 7, 0xF0 => 6, 0xE8 => 5, _ => 4 };
                if cc < 0 || self.cond(cc as u8) { let v = self.pop16(); self.pc = v; }
            }
            // PUSH / POP
            0xC5 | 0xD5 | 0xE5 => { let rp = (op >> 4) & 3; let v = self.rp(rp); self.push16(v); }
            0xF5 => { // PUSH PSW
                let mut f = 0u8;
                if self.cy { f |= 1; }
                if self.p { f |= 4; }
                if self.ac { f |= 0x10; }
                if self.z { f |= 0x40; }
                if self.s { f |= 0x80; }
                self.push16((self.a as u16) << 8 | f as u16);
            }
            0xC1 | 0xD1 | 0xE1 => { let rp = (op >> 4) & 3; let v = self.pop16(); self.set_rp(rp, v); }
            0xF1 => { // POP PSW
                let v = self.pop16();
                self.a = (v >> 8) as u8;
                let f = v as u8;
                self.cy = f & 1 != 0; self.p = f & 4 != 0; self.ac = f & 0x10 != 0;
                self.z = f & 0x40 != 0; self.s = f & 0x80 != 0;
            }
            0xE3 => { let t = self.pop16(); let h = self.hl() as u16; self.push16(h); self.set_rp(2, t); } // XTHL
            0xE9 => self.pc = self.hl() as u16, // PCHL
            0xF9 => self.sp = self.hl() as u16, // SPHL
            // RST
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let pc = self.pc;
                self.push16(pc);
                self.pc = (op & 0x38) as u16;
            }
            0xD3 => { // OUT port
                let port = self.fetch8() as usize;
                if (0x80..=0x85).contains(&port) {
                    self.i8155.write_reg(port - 0x80, self.a);
                } else if (0xE0..=0xE3).contains(&port) {
                    match port { 0xE0=>self.ppi.write_pa(self.a), 0xE1=>self.ppi.write_pb(self.a), 0xE2=>self.ppi.write_pc(self.a), 0xE3=>self.ppi.write_ctrl(self.a), _=>{} }
                } else if port==0xE9 { self.flash.command(self.a); }
                else if port==0x70 { self.rtc.write_sel(self.a); }
                else if port==0x71 { self.rtc.write_data(self.a); }
                else if port==0x30 { self.rtc.i2c_write(self.a); }
                else if port==0x28 { self.adc.write_ctrl(self.a); }
                else if port==0x38 { self.lcd.write_cmd(self.a); }
                else if port==0x39 { self.lcd.write_data(self.a); }
                else if (0xD0..=0xDF).contains(&port) { self.dma.write((port-0xD0) as u8, self.a); }
                else {
                    self.ports[port] = self.a;
                    if port == 0x01 {
                        self.out.put_char(self.a as char);
                    }
                }
            }
            0xDB => { // IN port
                let port = self.fetch8() as usize;
                self.a = if (0x80..=0x85).contains(&port) {
                    self.i8155.read_reg(port - 0x80)
                } else if (0xE0..=0xE3).contains(&port) {
                    match port { 0xE0=>self.ppi.read_pa(), 0xE1=>self.ppi.read_pb(), 0xE2=>self.ppi.read_pc(), 0xE3=>self.ppi.read_ctrl(), _=>0 }
                } else if port==0xE8 { self.flash.status() }
                else if port==0x70 { self.rtc.read_sel() }
                else if port==0x71 { self.rtc.read_data() }
                else if port==0x31 { self.rtc.i2c_read() }
                else if port==0x28 { self.adc.read_status() }
                else if port==0x29 { self.adc.read_data() }
                else if port==0x38 { self.lcd.read_status() }
                else if port==0x39 { self.lcd.read_data() }
                else if (0xD0..=0xDF).contains(&port) { self.dma.read((port-0xD0) as u8) }
                else {
                    self.ports[port]
                };
            }
            0xFB => self.int_enabled = true,
            0xF3 => self.int_enabled = false,
            _ => self.unimplemented(op),
        }
    }

    pub fn request_interrupt(&mut self, kind: &str) -> Result<(), String> {
        match kind.to_ascii_uppercase().as_str() {
            "TRAP" => { self.pending_trap = true; Ok(()) }
            "RST75" | "7.5" => { self.pending_rst75 = true; Ok(()) }
            "RST65" | "6.5" => { self.pending_rst65 = true; Ok(()) }
            "RST55" | "5.5" => { self.pending_rst55 = true; Ok(()) }
            _ => Err(format!("unknown 8085 interrupt '{kind}' (use TRAP, RST75, RST65, RST55)")),
        }
    }

    /// INTR is externally vectored: the device supplies the vector (RST n or CALL).
    pub fn request_intr(&mut self, vector: u8) {
        self.intr_vector = Some(vector);
    }

    fn service_interrupts(&mut self) {
        if self.pending_trap {
            self.pending_trap = false;
            self.take_interrupt(0x24, false); // TRAP: non-maskable, keeps IE
            return;
        }
        if !self.int_enabled { return; }
        if self.pending_rst75 && !self.mask_rst75 {
            self.pending_rst75 = false;
            self.take_interrupt(0x3C, true);
        } else if self.pending_rst65 && !self.mask_rst65 {
            self.pending_rst65 = false;
            self.take_interrupt(0x34, true);
        } else if self.pending_rst55 && !self.mask_rst55 {
            self.pending_rst55 = false;
            self.take_interrupt(0x2C, true);
        } else if let Some(v) = self.intr_vector {
            self.intr_vector = None;
            self.take_interrupt(v as u16, true);
        }
    }

    fn take_interrupt(&mut self, vector: u16, clear_iff: bool) {
        let psw = (self.a as u16) << 8
            | (self.s as u16) << 7 | (self.z as u16) << 6 | (self.ac as u16) << 4
            | (self.p as u16) << 2 | self.cy as u16;
        self.push16(psw);
        self.push16(self.pc);
        if clear_iff { self.int_enabled = false; }
        self.pc = vector;
    }

    /// Cheap check used to skip `service_interrupts` when nothing can fire.
    fn has_interrupt(&self) -> bool {
        self.pending_trap
            || (self.int_enabled
                && (self.pending_rst75 && !self.mask_rst75
                    || self.pending_rst65 && !self.mask_rst65
                    || self.pending_rst55 && !self.mask_rst55
                    || self.intr_vector.is_some()))
    }
}

impl Cpu for Cpu8085 {
    fn reset(&mut self) {
        self.a = 0; self.b = 0; self.c = 0; self.d = 0; self.e = 0;
        self.h = 0; self.l = 0;
        self.sp = 0xFFFF; self.pc = 0;
        self.s = false; self.z = false; self.ac = false; self.p = false; self.cy = false;
        self.halted = false;
        self.int_enabled = false;
        self.fault = None;
        self.mask_rst55 = false; self.mask_rst65 = false; self.mask_rst75 = false;
        self.pending_rst55 = false; self.pending_rst65 = false; self.pending_rst75 = false;
        self.pending_trap = false;
        self.intr_vector = None;
        self.sid = false; self.sod = false;
    }

    fn step(&mut self) -> bool {
        if self.halted { return false; }
        self.exec();
        if !self.halted {
            let ts = i8085_tstates(self.last_op);
            self.cycles += ts as u64;
            self.i8155.advance(ts as u64); // I8155::advance self-gates when idle
            self.adc.tick();
            self.lcd.tick();
            self.flash.tick();
            if self.has_interrupt() {
                self.service_interrupts();
            }
        }
        !self.halted
    }

    fn pc(&self) -> u32 { self.pc as u32 }

    fn set_pc(&mut self, addr: u32) { self.pc = addr as u16; }

    fn set_reg(&mut self, name: &str, val: u32) {
        let v = val as u8;
        match name.to_ascii_uppercase().as_str() {
            "A" => self.a = v, "B" => self.b = v, "C" => self.c = v, "D" => self.d = v,
            "E" => self.e = v, "H" => self.h = v, "L" => self.l = v,
            "SP" => self.sp = val as u16,
            "PC" => self.pc = val as u16,
            _ => {}
        }
    }

    fn regs(&self) -> Vec<Reg> {
        vec![
            Reg::new("A", self.a as u32),
            Reg::new("B", self.b as u32),
            Reg::new("C", self.c as u32),
            Reg::new("D", self.d as u32),
            Reg::new("E", self.e as u32),
            Reg::new("H", self.h as u32),
            Reg::new("L", self.l as u32),
            Reg::new("SP", self.sp as u32),
            Reg::new("PC", self.pc as u32),
        ]
    }

    fn flags(&self) -> FlagSet {
        FlagSet {
            carry: self.cy, zero: self.z, sign: self.s, parity: self.p,
            aux: self.ac, overflow: false, direction: false, interrupt: self.int_enabled,
            trap: false,
        }
    }

    fn mem_read(&self, addr: u32, len: usize) -> Vec<u8> {
        (0..len).map(|i| self.rd((addr + i as u32) as usize)).collect()
    }

    fn mem_write(&mut self, addr: u32, data: &[u8]) {
        for (i, b) in data.iter().enumerate() {
            self.wr((addr + i as u32) as usize, *b);
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(16 + MEM_SIZE + 256 + 267 + 8 + SRAM_SIZE + 200_000);
        v.push(5); // v5 adds ppi/flash/rtc/adc/lcd/dma
        v.extend_from_slice(&[self.a, self.b, self.c, self.d, self.e, self.h, self.l]);
        v.extend_from_slice(&self.sp.to_le_bytes());
        v.extend_from_slice(&self.pc.to_le_bytes());
        let f = (self.s as u8) << 7 | (self.z as u8) << 6 | (self.ac as u8) << 4
              | (self.p as u8) << 2 | self.cy as u8;
        v.push(f);
        v.push(self.halted as u8);
        v.push(self.int_enabled as u8);
        v.push(self.mask_rst55 as u8);
        v.push(self.mask_rst65 as u8);
        v.push(self.mask_rst75 as u8);
        v.push(self.pending_rst55 as u8);
        v.push(self.pending_rst65 as u8);
        v.push(self.pending_rst75 as u8);
        v.push(self.pending_trap as u8);
        v.push(self.intr_vector.map_or(0xFF, |v| v));
        v.push(self.sid as u8);
        v.push(self.sod as u8);
        v.extend_from_slice(&self.mem.data);
        v.extend_from_slice(&self.ports);
        v.extend_from_slice(&self.i8155.snapshot());
        v.extend_from_slice(&self.cycles.to_le_bytes());
        v.extend_from_slice(&self.sram.data);
        v.extend_from_slice(&self.sram_base.to_le_bytes());
        v.extend_from_slice(&self.sram_len.to_le_bytes());
        let (rb, rl) = self.mem.rom_range();
        v.extend_from_slice(&(rb as u32).to_le_bytes());
        v.extend_from_slice(&(rl as u32).to_le_bytes());
        v.extend_from_slice(&self.ppi.snapshot());
        v.extend_from_slice(&self.flash.snapshot());
        v.extend_from_slice(&self.rtc.snapshot());
        v.extend_from_slice(&self.adc.snapshot());
        v.extend_from_slice(&self.lcd.snapshot());
        v.extend_from_slice(&self.dma.snapshot());
        v
    }

    fn restore(&mut self, data: &[u8]) {
        if data.is_empty() { return; }
        let ver = data[0];
        if data.len() < 14 { return; }
        self.a = data[1]; self.b = data[2]; self.c = data[3]; self.d = data[4];
        self.e = data[5]; self.h = data[6]; self.l = data[7];
        self.sp = u16::from_le_bytes([data[8], data[9]]);
        self.pc = u16::from_le_bytes([data[10], data[11]]);
        let f = data[12];
        self.s = f & 0x80 != 0; self.z = f & 0x40 != 0; self.ac = f & 0x10 != 0;
        self.p = f & 0x04 != 0; self.cy = f & 0x01 != 0;
        self.halted = data[13] != 0;
        self.int_enabled = data.get(14).is_some_and(|b| *b != 0);
        self.mask_rst55 = data.get(15).is_some_and(|b| *b != 0);
        self.mask_rst65 = data.get(16).is_some_and(|b| *b != 0);
        self.mask_rst75 = data.get(17).is_some_and(|b| *b != 0);
        self.pending_rst55 = data.get(18).is_some_and(|b| *b != 0);
        self.pending_rst65 = data.get(19).is_some_and(|b| *b != 0);
        self.pending_rst75 = data.get(20).is_some_and(|b| *b != 0);
        self.pending_trap = data.get(21).is_some_and(|b| *b != 0);
        self.intr_vector = data.get(22).and_then(|b| (*b != 0xFF).then_some(*b));
        self.sid = data.get(23).is_some_and(|b| *b != 0);
        self.sod = data.get(24).is_some_and(|b| *b != 0);
        let body = &data[25..];
        let n = body.len().min(MEM_SIZE);
        self.mem.data[..n].copy_from_slice(&body[..n]);
        self.ports = [0; 256];
        if ver >= 2 && body.len() > MEM_SIZE {
            let start = MEM_SIZE;
            let n2 = body.len().saturating_sub(start).min(256);
            self.ports[..n2].copy_from_slice(&body[start..start + n2]);
        }
        if ver >= 3 {
            let n = data.len();
            let appended = if ver >= 4 { SRAM_SIZE + 16 } else { 0 };
            let v3end = n - appended;
            self.i8155.restore(&data[v3end - 275..v3end - 8]);
            let mut cy = [0u8; 8];
            cy.copy_from_slice(&data[v3end - 8..v3end]);
            self.cycles = u64::from_le_bytes(cy);
            if ver >= 4 && data.len() >= appended {
                let sram_data = &data[n - appended..n - 16];
                self.sram = Mem::new(SRAM_SIZE);
                let m = sram_data.len().min(SRAM_SIZE);
                self.sram.data[..m].copy_from_slice(&sram_data[..m]);
                self.sram_base = u32::from_le_bytes([data[n - 16], data[n - 15], data[n - 14], data[n - 13]]);
                self.sram_len = u32::from_le_bytes([data[n - 12], data[n - 11], data[n - 10], data[n - 9]]);
                let rb = u32::from_le_bytes([data[n - 8], data[n - 7], data[n - 6], data[n - 5]]);
                let rl = u32::from_le_bytes([data[n - 4], data[n - 3], data[n - 2], data[n - 1]]);
                self.mem.set_rom(rb as usize, rl as usize);
            }
            if ver >= 5 {
                // new peripherals appended after v4 data; parse sequentially from end of v4
                // v4 size is variable due to flash etc, but we appended after, so start = n (end of v4)
                // Actually data for v5 is after the v4 tail; we can parse from n
                let mut off = n;
                if data.len() >= off + 11 { self.ppi.restore(&data[off..off+11]); off+=11; }
                if data.len() >= off + 12 {
                    let flen = u32::from_le_bytes([data[off+8],data[off+9],data[off+10],data[off+11]]) as usize;
                    let ftot = 12+flen+3;
                    if data.len() >= off+ftot { self.flash.restore(&data[off..off+ftot]); off+=ftot; }
                }
                if data.len() >= off+65 { self.rtc.restore(&data[off..off+65]); off+=65; }
                if data.len() >= off+13 { self.adc.restore(&data[off..off+13]); off+=13; }
                if data.len() >= off+86 { self.lcd.restore(&data[off..off+86]); off+=86; }
                if off < data.len() { self.dma.restore(&data[off..]); }
            }
        }
    }

    fn is_halted(&self) -> bool { self.halted }

    fn cycles(&self) -> u64 { self.cycles }
}
