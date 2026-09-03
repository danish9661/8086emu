//! MOS/Zilog Z80 CPU core.
//!
//! Registers: A/F, B/C, D/E, H/L (+ shadow AF'/BC'/DE'/HL'), IX, IY, SP, PC,
//! I, R. 16-bit (64 KiB) address space. Flags use the classic layout
//! (S Z - H - P/V N C). Implements a broad subset of the documented Z80
//! instruction set including IX/IY indexed and bit/rotate (CB / DDCB / FDCB)
//! forms. Hardware interrupts (maskable INT, NMI) and the simple I/O
//! convention (`OUT (1),A` prints A) are supported.
//!
//! The decoder mirrors the classic opcode-prefix structure: `0xCB` (bit
//! ops), `0xED` (misc/16-bit ALU/block), `0xDD` (IX) and `0xFD` (IY), with
//! `0xDD 0xCB` / `0xFD 0xCB` for indexed bit ops.

use crate::cpu::{Cpu, Disasm, FlagSet, Mem, Output, Reg};

#[derive(Clone)]
pub struct CpuZ80 {
    a: u8, f: u8,
    b: u8, c: u8, d: u8, e: u8, h: u8, l: u8,
    a2: u8, f2: u8, b2: u8, c2: u8, d2: u8, e2: u8, h2: u8, l2: u8,
    ix: u16, iy: u16,
    sp: u16, pc: u16,
    i: u8, r: u8,
    iff1: bool, iff2: bool, im: u8,
    halted: bool,
    pub out: Output,
    pub ports: [u8; 256],
    mem: Mem,
    pending_int: bool,
    pending_nmi: bool,
}

impl Default for CpuZ80 {
    fn default() -> Self {
        let mut m = CpuZ80 {
            a: 0, f: 0, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0,
            a2: 0, f2: 0, b2: 0, c2: 0, d2: 0, e2: 0, h2: 0, l2: 0,
            ix: 0, iy: 0, sp: 0xFFFF, pc: 0, i: 0, r: 0,
            iff1: false, iff2: false, im: 0, halted: false,
            out: Output::default(), ports: [0; 256], mem: Mem::new(1 << 16),
            pending_int: false, pending_nmi: false,
        };
        m.reset();
        m
    }
}

const S: u8 = 1 << 7;
const Z: u8 = 1 << 6;
const H: u8 = 1 << 4;
const PV: u8 = 1 << 2;
const N: u8 = 1 << 1;
const C: u8 = 1 << 0;

impl CpuZ80 {
    fn get_flag(&self, bit: u8) -> bool { self.f & bit != 0 }
    fn set_flag(&mut self, bit: u8, v: bool) { if v { self.f |= bit; } else { self.f &= !bit; } }

    fn parity(v: u8) -> bool { v.count_ones() % 2 == 0 }

    fn set_szp(&mut self, v: u8) {
        self.set_flag(S, v & 0x80 != 0);
        self.set_flag(Z, v == 0);
        self.set_flag(PV, Self::parity(v));
    }

    fn bc(&self) -> u16 { ((self.b as u16) << 8) | self.c as u16 }
    fn de(&self) -> u16 { ((self.d as u16) << 8) | self.e as u16 }
    fn hl(&self) -> u16 { ((self.h as u16) << 8) | self.l as u16 }
    fn af(&self) -> u16 { ((self.a as u16) << 8) | self.f as u16 }
    fn set_bc(&mut self, v: u16) { self.b = (v >> 8) as u8; self.c = v as u8; }
    fn set_de(&mut self, v: u16) { self.d = (v >> 8) as u8; self.e = v as u8; }
    fn set_hl(&mut self, v: u16) { self.h = (v >> 8) as u8; self.l = v as u8; }
    fn set_af(&mut self, v: u16) { self.a = (v >> 8) as u8; self.f = v as u8; }

    fn rd(&self, a: u16) -> u8 { self.mem.read(a as usize) }
    fn wr(&mut self, a: u16, v: u8) { self.mem.write(a as usize, v); }
    fn fetch(&mut self) -> u8 { let v = self.rd(self.pc); self.pc = self.pc.wrapping_add(1); v }
    fn fetch16(&mut self) -> u16 { let lo = self.fetch(); let hi = self.fetch(); ((hi as u16) << 8) | lo as u16 }
    fn fetch_disp(&mut self) -> i16 { self.fetch() as i8 as i16 }

    fn push(&mut self, v: u16) { self.sp = self.sp.wrapping_sub(1); self.wr(self.sp, (v >> 8) as u8); self.sp = self.sp.wrapping_sub(1); self.wr(self.sp, v as u8); }
    fn pop(&mut self) -> u16 { let lo = self.rd(self.sp); self.sp = self.sp.wrapping_add(1); let hi = self.rd(self.sp); self.sp = self.sp.wrapping_add(1); ((hi as u16) << 8) | lo as u16 }

    fn in_port(&mut self, port: u16) -> u8 {
        let p = (port & 0xFF) as usize;
        self.ports[p]
    }
    fn out_port(&mut self, port: u16, v: u8) {
        let p = (port & 0xFF) as usize;
        self.ports[p] = v;
        if p == 0x01 { self.out.put_char(v as char); }
    }

    fn add8(&mut self, v: u8, carry: bool) -> u8 {
        let cy = if carry && self.get_flag(C) { 1 } else { 0 };
        let (r1, c1) = self.a.overflowing_add(v);
        let (r2, c2) = r1.overflowing_add(cy);
        let h = (self.a & 0xF) + (v & 0xF) + cy > 0xF;
        let ov = ((self.a ^ r2) & (v ^ r2) & 0x80) != 0;
        self.set_flag(S, r2 & 0x80 != 0);
        self.set_flag(Z, r2 == 0);
        self.set_flag(H, h);
        self.set_flag(PV, ov);
        self.set_flag(N, false);
        self.set_flag(C, c1 || c2);
        r2
    }
    fn sub8(&mut self, v: u8, carry: bool, store: bool) -> u8 {
        let cy = if carry && self.get_flag(C) { 1 } else { 0 };
        let (r1, c1) = self.a.overflowing_sub(v);
        let (r2, c2) = r1.overflowing_sub(cy);
        let h = ((self.a & 0xF) as i16 - (v & 0xF) as i16 - cy as i16) < 0;
        let ov = ((self.a ^ v) & (self.a ^ r2) & 0x80) != 0;
        let res = r2;
        if store { self.a = res; }
        self.set_flag(S, res & 0x80 != 0);
        self.set_flag(Z, res == 0);
        self.set_flag(H, h);
        self.set_flag(PV, ov);
        self.set_flag(N, true);
        self.set_flag(C, c1 || c2);
        res
    }
    fn logic(&mut self, v: u8, op: u8) {
        let r = match op { 0 => self.a & v, 1 => self.a | v, 2 => self.a ^ v, _ => v };
        if op != 3 { self.a = r; }
        self.set_flag(S, r & 0x80 != 0);
        self.set_flag(Z, r == 0);
        self.set_flag(H, op == 0);
        self.set_flag(PV, Self::parity(r));
        self.set_flag(N, false);
        self.set_flag(C, false);
    }
    fn inc8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        let h = (v & 0xF) == 0xF;
        let ov = v == 0x7F;
        self.set_flag(S, r & 0x80 != 0);
        self.set_flag(Z, r == 0);
        self.set_flag(H, h);
        self.set_flag(PV, ov);
        self.set_flag(N, false);
        r
    }
    fn dec8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        let h = (v & 0xF) == 0;
        let ov = v == 0x80;
        self.set_flag(S, r & 0x80 != 0);
        self.set_flag(Z, r == 0);
        self.set_flag(H, h);
        self.set_flag(PV, ov);
        self.set_flag(N, true);
        r
    }

    fn daa(&mut self) {
        let mut v = self.a as u16;
        let mut carry = false;
        let half = self.get_flag(H);
        let neg = self.get_flag(N);
        let c = self.get_flag(C);
        if !neg {
            if half || (v & 0x0F) > 9 { v = v.wrapping_add(6); }
            if c || v > 0x9F { v = v.wrapping_add(0x60); carry = true; }
        } else {
            if half { v = v.wrapping_sub(6); }
            if c { v = v.wrapping_sub(0x60); carry = true; }
        }
        let r = (v & 0xFF) as u8;
        let h2 = if neg { (self.a & 0x0F) < 6 } else { (self.a & 0x0F) > 9 || half };
        self.set_flag(H, h2);
        self.set_flag(C, carry);
        self.a = r;
        self.set_flag(S, r & 0x80 != 0);
        self.set_flag(Z, r == 0);
        self.set_flag(PV, Self::parity(r));
    }

    fn rlc(&mut self, v: u8) -> u8 { let r = (v << 1) | (v >> 7); self.set_flag(C, v & 0x80 != 0); self.set_szp(r); self.set_flag(N, false); self.set_flag(H, false); r }
    fn rrc(&mut self, v: u8) -> u8 { let r = (v >> 1) | (v << 7); self.set_flag(C, v & 1 != 0); self.set_szp(r); self.set_flag(N, false); self.set_flag(H, false); r }
    fn rl(&mut self, v: u8) -> u8 { let cy = if self.get_flag(C) { 1 } else { 0 }; let r = (v << 1) | cy; self.set_flag(C, v & 0x80 != 0); self.set_szp(r); self.set_flag(N, false); self.set_flag(H, false); r }
    fn rr(&mut self, v: u8) -> u8 { let cy = if self.get_flag(C) { 1 } else { 0 }; let r = (v >> 1) | (cy << 7); self.set_flag(C, v & 1 != 0); self.set_szp(r); self.set_flag(N, false); self.set_flag(H, false); r }
    fn sla(&mut self, v: u8) -> u8 { let r = v << 1; self.set_flag(C, v & 0x80 != 0); self.set_szp(r); self.set_flag(N, false); self.set_flag(H, false); r }
    fn sra(&mut self, v: u8) -> u8 { let r = (v >> 1) | (v & 0x80); self.set_flag(C, v & 1 != 0); self.set_szp(r); self.set_flag(N, false); self.set_flag(H, false); r }
    fn srl(&mut self, v: u8) -> u8 { let r = v >> 1; self.set_flag(C, v & 1 != 0); self.set_szp(r); self.set_flag(N, false); self.set_flag(H, false); r }

    fn r8(&self, idx: u8) -> u8 {
        match idx { 0 => self.b, 1 => self.c, 2 => self.d, 3 => self.e, 4 => self.h, 5 => self.l, 6 => self.rd(self.hl()), 7 => self.a, _ => 0 }
    }
    fn set_r8(&mut self, idx: u8, v: u8) {
        match idx { 0 => self.b = v, 1 => self.c = v, 2 => self.d = v, 3 => self.e = v, 4 => self.h = v, 5 => self.l = v, 6 => self.wr(self.hl(), v), 7 => self.a = v, _ => {} }
    }
    fn rp(&self, idx: u8) -> u16 {
        match idx { 0 => self.bc(), 1 => self.de(), 2 => self.hl(), 3 => self.sp, _ => 0 }
    }
    fn set_rp(&mut self, idx: u8, v: u16) {
        match idx { 0 => self.set_bc(v), 1 => self.set_de(v), 2 => self.set_hl(v), 3 => self.sp = v, _ => {} }
    }
    fn rp2(&self, idx: u8) -> u16 {
        match idx { 0 => self.bc(), 1 => self.de(), 2 => self.hl(), 3 => self.af(), _ => 0 }
    }
    fn set_rp2(&mut self, idx: u8, v: u16) {
        match idx { 0 => self.set_bc(v), 1 => self.set_de(v), 2 => self.set_hl(v), 3 => self.set_af(v), _ => {} }
    }
    fn cond(&self, cc: u8) -> bool {
        match cc { 0 => !self.get_flag(Z), 1 => self.get_flag(Z), 2 => !self.get_flag(C), 3 => self.get_flag(C), 4 => !self.get_flag(S), 5 => self.get_flag(S), 6 => !self.get_flag(PV), 7 => self.get_flag(PV), _ => false }
    }

    fn alu(&mut self, g: u8, v: u8) {
        match g {
            0 => { self.a = self.add8(v, false); }
            1 => { self.a = self.add8(v, true); }
            2 => { self.sub8(v, false, true); }
            3 => { self.sub8(v, true, true); }
            4 => self.logic(v, 0),
            5 => self.logic(v, 1),
            6 => self.logic(v, 2),
            7 => { self.sub8(v, false, false); }
            _ => {}
        }
    }

    /// Execute one instruction; returns false if CPU should stop.
    fn run_step(&mut self) -> bool {
        if self.halted { return false; }
        self.r = self.r.wrapping_add(1);
        let op = self.fetch();
        match op {
            0xCB => { let sub = self.fetch(); self.exec_cb(sub); }
            0xDD => { let next = self.fetch(); if next == 0xCB { self.exec_ddcb(false); } else { self.exec_xy(next, false); } }
            0xFD => { let next = self.fetch(); if next == 0xCB { self.exec_ddcb(true); } else { self.exec_xy(next, true); } }
            0xED => { let next = self.fetch(); self.exec_ed(next); }
            _ => self.exec_main(op),
        }
        true
    }

    fn exec_main(&mut self, op: u8) {
        match op {
            0x00 => {}
            0x76 => { self.halted = true; }
            0x07 => { self.a = self.rlc(self.a); }
            0x0F => { self.a = self.rrc(self.a); }
            0x17 => { self.a = self.rl(self.a); }
            0x1F => { self.a = self.rr(self.a); }
            0x2F => { self.a = !self.a; self.set_flag(H, self.get_flag(C)); self.set_flag(N, true); /* H = C for CPL */ }
            0x37 => { self.set_flag(C, true); self.set_flag(H, false); self.set_flag(N, false); }
            0x3F => { let c = self.get_flag(C); self.set_flag(C, !c); self.set_flag(H, c); self.set_flag(N, false); }
            0x27 => self.daa(),
            0xF3 => { self.iff1 = false; self.iff2 = false; }
            0xFB => { self.iff1 = true; self.iff2 = true; }
            // 8-bit LD imm
            0x06 => self.b = self.fetch(),
            0x0E => self.c = self.fetch(),
            0x16 => self.d = self.fetch(),
            0x1E => self.e = self.fetch(),
            0x26 => self.h = self.fetch(),
            0x2E => self.l = self.fetch(),
            0x36 => { let a = self.hl(); let v = self.fetch(); self.wr(a, v); }
            0x3E => self.a = self.fetch(),
            // INC/DEC 8-bit
            0x04 => self.b = self.inc8(self.b),
            0x05 => self.b = self.dec8(self.b),
            0x0C => self.c = self.inc8(self.c),
            0x0D => self.c = self.dec8(self.c),
            0x14 => self.d = self.inc8(self.d),
            0x15 => self.d = self.dec8(self.d),
            0x1C => self.e = self.inc8(self.e),
            0x1D => self.e = self.dec8(self.e),
            0x24 => self.h = self.inc8(self.h),
            0x25 => self.h = self.dec8(self.h),
            0x2C => self.l = self.inc8(self.l),
            0x2D => self.l = self.dec8(self.l),
            0x34 => { let a = self.hl(); let v = self.rd(a); let r = self.inc8(v); self.wr(a, r); }
            0x35 => { let a = self.hl(); let v = self.rd(a); let r = self.dec8(v); self.wr(a, r); }
            0x3C => self.a = self.inc8(self.a),
            0x3D => self.a = self.dec8(self.a),
            // 16-bit LD / exchange
            0x01 => { let v = self.fetch16(); self.set_bc(v); }
            0x11 => { let v = self.fetch16(); self.set_de(v); }
            0x21 => { let v = self.fetch16(); self.set_hl(v); }
            0x31 => self.sp = self.fetch16(),
            0x02 => self.wr(self.bc(), self.a),
            0x12 => self.wr(self.de(), self.a),
            0x0A => self.a = self.rd(self.bc()),
            0x1A => self.a = self.rd(self.de()),
            0x32 => { let nn = self.fetch16(); self.wr(nn, self.a); }
            0x3A => { let nn = self.fetch16(); self.a = self.rd(nn); }
            0x22 => { let nn = self.fetch16(); self.wr(nn, self.l); self.wr(nn.wrapping_add(1), self.h); }
            0x2A => { let nn = self.fetch16(); self.l = self.rd(nn); self.h = self.rd(nn.wrapping_add(1)); }
            0x08 => { let af = self.af(); self.set_af((self.a2 as u16) << 8 | self.f2 as u16); self.a2 = (af >> 8) as u8; self.f2 = af as u8; }
            0xF9 => self.sp = self.hl(),
            0xEB => { let de = self.de(); self.set_de(self.hl()); self.set_hl(de); }
            0xE3 => { let sp = self.sp; let lo = self.rd(sp); let hi = self.rd(sp.wrapping_add(1)); let hl = self.hl(); self.wr(sp, hl as u8); self.wr(sp.wrapping_add(1), (hl >> 8) as u8); self.set_hl((hi as u16) << 8 | lo as u16); }
            // INC/DEC 16-bit
            0x03 => self.set_bc(self.bc().wrapping_add(1)),
            0x13 => self.set_de(self.de().wrapping_add(1)),
            0x23 => self.set_hl(self.hl().wrapping_add(1)),
            0x33 => self.sp = self.sp.wrapping_add(1),
            0x0B => self.set_bc(self.bc().wrapping_sub(1)),
            0x1B => self.set_de(self.de().wrapping_sub(1)),
            0x2B => self.set_hl(self.hl().wrapping_sub(1)),
            0x3B => self.sp = self.sp.wrapping_sub(1),
            // ADD HL,rp
            0x09 => { self.add_hl(self.bc()); }
            0x19 => { self.add_hl(self.de()); }
            0x29 => { self.add_hl(self.hl()); }
            0x39 => { self.add_hl(self.sp); }
            // LD r,r and (HL)
            0x40..=0x7F => {
                if op == 0x76 { self.halted = true; }
                else {
                    let dst = (op >> 3) & 7;
                    let src = op & 7;
                    let v = if src == 6 { self.rd(self.hl()) } else { self.r8(src) };
                    if dst == 6 { self.wr(self.hl(), v); } else { self.set_r8(dst, v); }
                }
            }
            // ALU A,r / A,(HL)
            0x80..=0xBF => {
                let g = (op >> 3) & 7;
                let v = if (op & 7) == 6 { self.rd(self.hl()) } else { self.r8(op & 7) };
                self.alu(g, v);
            }
            // ALU A,n
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                let g = (op >> 3) & 7;
                let v = self.fetch();
                self.alu(g, v);
            }
            // jumps / calls / returns
            0xC3 => { self.pc = self.fetch16(); }
            0x18 => { let e = self.fetch() as i8; self.pc = (self.pc as i32 + e as i32) as u16; }
            0x20 => { let e = self.fetch() as i8; if !self.get_flag(Z) { self.pc = (self.pc as i32 + e as i32) as u16; } }
            0x28 => { let e = self.fetch() as i8; if self.get_flag(Z) { self.pc = (self.pc as i32 + e as i32) as u16; } }
            0x30 => { let e = self.fetch() as i8; if !self.get_flag(C) { self.pc = (self.pc as i32 + e as i32) as u16; } }
            0x38 => { let e = self.fetch() as i8; if self.get_flag(C) { self.pc = (self.pc as i32 + e as i32) as u16; } }
            0x10 => { self.b = self.b.wrapping_sub(1); let e = self.fetch() as i8; if self.b != 0 { self.pc = (self.pc as i32 + e as i32) as u16; } }
            0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => { let nn = self.fetch16(); if self.cond((op >> 3) & 7) { self.pc = nn; } }
            0xE9 => { self.pc = self.hl(); }
            0xC9 => { self.pc = self.pop(); }
            0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => { if self.cond((op >> 3) & 7) { self.pc = self.pop(); } }
            0xCD => { let nn = self.fetch16(); self.push(self.pc); self.pc = nn; }
            0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => { let nn = self.fetch16(); if self.cond((op >> 3) & 7) { self.push(self.pc); self.pc = nn; } }
            // RST n (n = 0x00,0x08,...,0x38)
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => { let n = (op & 0x38) as u16; self.push(self.pc); self.pc = n; }
            // PUSH / POP
            0xC5 => self.push(self.bc()),
            0xD5 => self.push(self.de()),
            0xE5 => self.push(self.hl()),
            0xF5 => self.push(self.af()),
            0xC1 => { let v = self.pop(); self.set_bc(v); }
            0xD1 => { let v = self.pop(); self.set_de(v); }
            0xE1 => { let v = self.pop(); self.set_hl(v); }
            0xF1 => { let v = self.pop(); self.set_af(v); }
            0xD3 => { let n = self.fetch(); self.out_port(((self.a as u16) << 8) | n as u16, self.a); }
            0xDB => { let n = self.fetch(); self.a = self.in_port(((self.a as u16) << 8) | n as u16); }
            _ => { self.halted = true; /* undefined opcode: halt, don't silently NOP */ }
        }
    }

    fn exec_ed(&mut self, op: u8) {
        match op {
            0x44 => { // NEG
                let v = self.a;
                let r = (0u8).wrapping_sub(v);
                let c = v != 0;
                let h = (v & 0xF) != 0;
                let ov = v == 0x80;
                self.a = r;
                self.set_flag(S, r & 0x80 != 0);
                self.set_flag(Z, r == 0);
                self.set_flag(H, h);
                self.set_flag(PV, ov);
                self.set_flag(N, true);
                self.set_flag(C, c);
            }
            0x47 => self.i = self.a,
            0x4F => self.r = self.a,
            0x57 => self.a = self.i,
            0x5F => self.a = self.r,
            0x67 => self.rld(),
            0x6F => self.rrd(),
            0x46 => self.im = 0,
            0x56 => self.im = 1,
            0x5E => self.im = 2,
            0x45 | 0x4D | 0x55 | 0x5D | 0x65 | 0x6D | 0x75 | 0x7D => { self.pc = self.pop(); self.iff1 = self.iff2; }
            0x40..=0x7F if (op & 0xC7) == 0x40 => {
                // IN r,(C): read port (BC) into r (or (HL) when r == 6)
                let r = (op >> 3) & 7;
                let v = self.in_port(self.bc());
                if r == 6 { self.wr(self.hl(), v); } else { self.set_r8(r, v); }
            }
            0x41..=0x7F if (op & 0xC7) == 0x41 => {
                // OUT (C),r: write r (or (HL) when r == 6) to port (BC)
                let r = (op >> 3) & 7;
                let v = if r == 6 { self.rd(self.hl()) } else { self.r8(r) };
                self.out_port(self.bc(), v);
            }
            0x43 | 0x53 | 0x63 | 0x73 => { let rp = (op >> 4) & 3; let v = self.rp(rp); let nn = self.fetch16(); self.wr(nn, v as u8); self.wr(nn.wrapping_add(1), (v >> 8) as u8); }
            0x4B | 0x5B | 0x6B | 0x7B => { let rp = (op >> 4) & 3; let nn = self.fetch16(); let lo = self.rd(nn); let hi = self.rd(nn.wrapping_add(1)); self.set_rp(rp, ((hi as u16) << 8) | lo as u16); }
            0x4A | 0x5A | 0x6A | 0x7A => { let carry = true; let rp = (op >> 4) & 3; let val = self.rp(rp); self.add16(val, carry); }
            0x42 | 0x52 | 0x62 | 0x72 => { let carry = true; let rp = (op >> 4) & 3; let val = self.rp(rp); self.sub16(val, carry); }
            0xA0 => self.ldi(false),
            0xB0 => self.ldi(true),
            0xA8 => self.ldd(false),
            0xB8 => self.ldd(true),
            0xA1 => self.cpi(false),
            0xB1 => self.cpi(true),
            0xA9 => self.cpd(false),
            0xB9 => self.cpd(true),
            _ => { self.halted = true; }
        }
    }

    fn add_hl(&mut self, val: u16) {
        let hl = self.hl() as u32;
        let v = val as u32;
        let (r, c) = hl.overflowing_add(v);
        let h = ((hl & 0xFFF) + (v & 0xFFF)) > 0xFFF;
        let ov = ((hl ^ r) & (v ^ r) & 0x8000) != 0;
        self.set_hl(r as u16);
        self.set_flag(H, h);
        self.set_flag(PV, ov);
        self.set_flag(N, false);
        self.set_flag(C, c);
    }
    fn add16(&mut self, val: u16, _carry_ignored: bool) {
        let hl = self.hl() as u32;
        let v = val as u32;
        let cy = if self.get_flag(C) { 1 } else { 0 };
        let (r, c) = hl.overflowing_add(v + cy);
        let h = ((hl & 0xFFF) + (v & 0xFFF) + cy) > 0xFFF;
        let ov = ((hl ^ r) & (v ^ r) & 0x8000) != 0;
        self.set_hl(r as u16);
        self.set_flag(H, h);
        self.set_flag(PV, ov);
        self.set_flag(N, false);
        self.set_flag(C, c);
    }
    fn sub16(&mut self, val: u16, _carry_ignored: bool) {
        let hl = self.hl() as u32;
        let v = val as u32;
        let cy = if self.get_flag(C) { 1 } else { 0 };
        let (r, c) = hl.overflowing_sub(v + cy);
        let h = ((hl & 0xFFF) as i32 - (v & 0xFFF) as i32 - cy as i32) < 0;
        let ov = ((hl ^ v) & (hl ^ r) & 0x8000) != 0;
        self.set_hl(r as u16);
        self.set_flag(H, h);
        self.set_flag(PV, ov);
        self.set_flag(N, true);
        self.set_flag(C, c);
    }
    fn rld(&mut self) {
        let a = self.hl();
        let v = self.rd(a);
        let hi = self.a & 0xF0;
        let lo = v >> 4;
        self.a = hi | lo;
        self.wr(a, (v << 4) | (self.a & 0x0F));
        self.set_szp(self.a);
        self.set_flag(N, false);
        self.set_flag(H, false);
    }
    fn rrd(&mut self) {
        let a = self.hl();
        let v = self.rd(a);
        let hi = self.a & 0xF0;
        let lo = v & 0x0F;
        self.a = hi | lo;
        self.wr(a, (self.a & 0x0F) << 4 | (v >> 4));
        self.set_szp(self.a);
        self.set_flag(N, false);
        self.set_flag(H, false);
    }
    fn ldi(&mut self, rep: bool) {
        loop {
            let v = self.rd(self.hl());
            self.wr(self.de(), v);
            self.set_hl(self.hl().wrapping_add(1));
            self.set_de(self.de().wrapping_add(1));
            let bc = self.bc().wrapping_sub(1);
            self.set_bc(bc);
            if !rep { break; }
            if bc == 0 { break; }
        }
        self.set_flag(PV, false);
        self.set_flag(N, false);
    }
    fn ldd(&mut self, rep: bool) {
        loop {
            let v = self.rd(self.hl());
            self.wr(self.de(), v);
            self.set_hl(self.hl().wrapping_sub(1));
            self.set_de(self.de().wrapping_sub(1));
            let bc = self.bc().wrapping_sub(1);
            self.set_bc(bc);
            if !rep { break; }
            if bc == 0 { break; }
        }
        self.set_flag(PV, false);
        self.set_flag(N, false);
    }
    fn cpi(&mut self, rep: bool) {
        loop {
            let v = self.rd(self.hl());
            self.sub8(v, false, false);
            self.set_hl(self.hl().wrapping_add(1));
            let bc = self.bc().wrapping_sub(1);
            self.set_bc(bc);
            if !rep { break; }
            if bc == 0 || self.get_flag(Z) { break; }
        }
        self.set_flag(PV, self.bc() != 0);
    }
    fn cpd(&mut self, rep: bool) {
        loop {
            let v = self.rd(self.hl());
            self.sub8(v, false, false);
            self.set_hl(self.hl().wrapping_sub(1));
            let bc = self.bc().wrapping_sub(1);
            self.set_bc(bc);
            if !rep { break; }
            if bc == 0 || self.get_flag(Z) { break; }
        }
        self.set_flag(PV, self.bc() != 0);
    }

    fn exec_xy(&mut self, op: u8, iy: bool) {
        let base = if iy { self.iy } else { self.ix };
        match op {
            0x21 => { if iy { self.iy = self.fetch16(); } else { self.ix = self.fetch16(); } }
            0x22 => { let nn = self.fetch16(); let v = if iy { self.iy } else { self.ix }; self.wr(nn, v as u8); self.wr(nn.wrapping_add(1), (v >> 8) as u8); }
            0x2A => { let nn = self.fetch16(); let lo = self.rd(nn); let hi = self.rd(nn.wrapping_add(1)); let v = ((hi as u16) << 8) | lo as u16; if iy { self.iy = v; } else { self.ix = v; } }
            0x09 => {
                let rp = (op >> 4) & 3;
                let val = match rp { 0 => self.bc(), 1 => self.de(), 2 => if iy { self.iy } else { self.ix }, 3 => self.sp, _ => 0 };
                let cur = if iy { self.iy } else { self.ix };
                let (r, c) = cur.overflowing_add(val);
                let h = (cur & 0xFFF) + (val & 0xFFF) > 0xFFF;
                if iy { self.iy = r; } else { self.ix = r; }
                self.set_flag(H, h);
                self.set_flag(C, c);
                self.set_flag(N, false);
            }
            0x23 => { if iy { self.iy = self.iy.wrapping_add(1); } else { self.ix = self.ix.wrapping_add(1); } }
            0x2B => { if iy { self.iy = self.iy.wrapping_sub(1); } else { self.ix = self.ix.wrapping_sub(1); } }
            0xE5 => { let v = if iy { self.iy } else { self.ix }; self.push(v); }
            0xE1 => { let v = self.pop(); if iy { self.iy = v; } else { self.ix = v; } }
            0xF9 => { self.sp = if iy { self.iy } else { self.ix }; }
            0xE9 => { self.pc = if iy { self.iy } else { self.ix }; }
            0x34 => { let d = self.fetch_disp(); let a = (base as i32 + d as i32) as u16; let v = self.rd(a); let r = self.inc8(v); self.wr(a, r); }
            0x35 => { let d = self.fetch_disp(); let a = (base as i32 + d as i32) as u16; let v = self.rd(a); let r = self.dec8(v); self.wr(a, r); }
            0x36 => { let d = self.fetch_disp(); let a = (base as i32 + d as i32) as u16; let v = self.fetch(); self.wr(a, v); }
            0x40..=0x7F => {
                let d = self.fetch_disp();
                let a = (base as i32 + d as i32) as u16;
                let dst = (op >> 3) & 7;
                let src = op & 7;
                let v = if src == 6 { self.rd(a) } else { self.r8(src) };
                if dst == 6 { self.wr(a, v); } else { self.set_r8(dst, v); }
            }
            0x80..=0xBF => {
                let d = self.fetch_disp();
                let a = (base as i32 + d as i32) as u16;
                let g = (op >> 3) & 7;
                let v = self.rd(a);
                self.alu(g, v);
            }
            _ => { self.halted = true; }
        }
    }

    fn exec_cb(&mut self, sub: u8) {
        let idx = sub & 7;
        let rot = (sub >> 3) & 7;
        match sub & 0xC0 {
            0x00 => {
                let v = self.r8(idx);
                let r = match rot { 0 => self.rlc(v), 1 => self.rrc(v), 2 => self.rl(v), 3 => self.rr(v), 4 => self.sla(v), 5 => self.sra(v), 6 => { let r = (v << 1) | (v >> 7); self.set_flag(C, v & 0x80 != 0); self.set_szp(r); self.set_flag(N, false); self.set_flag(H, false); r }, 7 => self.srl(v), _ => v };
                self.set_r8(idx, r);
            }
            0x40 => { let b = (sub >> 3) & 7; let v = self.r8(idx); self.set_flag(Z, (v >> b) & 1 == 0); self.set_flag(S, b == 7 && (v & 0x80 != 0)); self.set_flag(H, true); self.set_flag(PV, (v >> b) & 1 == 0); self.set_flag(N, false); }
            0x80 => { let b = (sub >> 3) & 7; self.set_r8(idx, self.r8(idx) & !(1 << b)); }
            0xC0 => { let b = (sub >> 3) & 7; self.set_r8(idx, self.r8(idx) | (1 << b)); }
            _ => {}
        }
    }

    fn exec_ddcb(&mut self, iy: bool) {
        let d = self.fetch_disp();
        let sub = self.fetch();
        let base = if iy { self.iy } else { self.ix };
        let a = (base as i32 + d as i32) as u16;
        let rot = (sub >> 3) & 7;
        match sub & 0xC0 {
            0x00 => {
                let v = self.rd(a);
                let r = match rot { 0 => self.rlc(v), 1 => self.rrc(v), 2 => self.rl(v), 3 => self.rr(v), 4 => self.sla(v), 5 => self.sra(v), 6 => { let r = (v << 1) | (v >> 7); self.set_flag(C, v & 0x80 != 0); self.set_szp(r); self.set_flag(N, false); self.set_flag(H, false); r }, 7 => self.srl(v), _ => v };
                self.wr(a, r);
            }
            0x40 => { let b = (sub >> 3) & 7; let v = self.rd(a); self.set_flag(Z, (v >> b) & 1 == 0); self.set_flag(S, b == 7 && (v & 0x80 != 0)); self.set_flag(H, true); self.set_flag(PV, (v >> b) & 1 == 0); self.set_flag(N, false); }
            0x80 => { let b = (sub >> 3) & 7; self.wr(a, self.rd(a) & !(1 << b)); }
            0xC0 => { let b = (sub >> 3) & 7; self.wr(a, self.rd(a) | (1 << b)); }
            _ => {}
        }
    }
// __APPEND_EXEC__
}

impl CpuZ80 {
    pub fn request_int(&mut self) { self.pending_int = true; }
    pub fn request_nmi(&mut self) { self.pending_nmi = true; }
    pub fn set_im(&mut self, m: u8) { self.im = m & 3; }
    pub fn port_read(&self, port: u8) -> u8 { self.ports[port as usize] }
    pub fn port_write(&mut self, port: u8, v: u8) { self.ports[port as usize] = v; }
    pub fn rom_region(&self) -> (u32, u32) { let (b, l) = self.mem.rom_range(); (b as u32, l as u32) }
    pub fn load_rom(&mut self, data: &[u8], addr: u32) {
        self.mem.load(addr as usize, data);
        self.mem.set_rom(addr as usize, data.len());
    }
    fn r8name(i: u8) -> &'static str { match i { 0 => "B", 1 => "C", 2 => "D", 3 => "E", 4 => "H", 5 => "L", 6 => "(HL)", 7 => "A", _ => "?" } }
    fn cc_name(cc: u8) -> &'static str { match cc { 0 => "NZ", 1 => "Z", 2 => "NC", 3 => "C", 4 => "PO", 5 => "PE", 6 => "P", 7 => "M", _ => "?" } }
}

impl Cpu for CpuZ80 {
    fn reset(&mut self) {
        self.a = 0; self.f = 0; self.b = 0; self.c = 0; self.d = 0; self.e = 0; self.h = 0; self.l = 0;
        self.a2 = 0; self.f2 = 0; self.b2 = 0; self.c2 = 0; self.d2 = 0; self.e2 = 0; self.h2 = 0; self.l2 = 0;
        self.ix = 0; self.iy = 0; self.sp = 0xFFFF; self.pc = 0; self.i = 0; self.r = 0;
        self.iff1 = false; self.iff2 = false; self.im = 0; self.halted = false; self.pending_int = false; self.pending_nmi = false;
        self.out = Output::default();
    }

    fn step(&mut self) -> bool {
        if self.pending_nmi {
            self.pending_nmi = false;
            self.iff1 = false;
            self.push(self.pc);
            self.pc = 0x0066;
            return true;
        }
        if self.pending_int && self.iff1 {
            self.pending_int = false;
            self.iff2 = self.iff1;
            self.iff1 = false;
            let addr = if self.im == 2 { ((self.i as u16) << 8) as u16 } else { 0x0038 };
            self.push(self.pc);
            self.pc = addr;
            return true;
        }
        self.run_step()
    }

    fn pc(&self) -> u32 { self.pc as u32 }
    fn set_pc(&mut self, addr: u32) { self.pc = addr as u16; }
    fn set_reg(&mut self, name: &str, val: u32) {
        match name.to_ascii_uppercase().as_str() {
            "A" | "ACC" => self.a = val as u8,
            "F" | "FLAGS" => self.f = val as u8,
            "B" => self.b = val as u8,
            "C" => self.c = val as u8,
            "D" => self.d = val as u8,
            "E" => self.e = val as u8,
            "H" => self.h = val as u8,
            "L" => self.l = val as u8,
            "I" => self.i = val as u8,
            "R" => self.r = val as u8,
            "PC" => self.pc = val as u16,
            "SP" => self.sp = val as u16,
            "IX" => self.ix = val as u16,
            "IY" => self.iy = val as u16,
            "AF" => self.set_af(val as u16),
            "BC" => self.set_bc(val as u16),
            "DE" => self.set_de(val as u16),
            "HL" => self.set_hl(val as u16),
            _ => {}
        }
    }
    fn regs(&self) -> Vec<Reg> {
        vec![
            Reg::new("A", self.a as u32), Reg::new("F", self.f as u32),
            Reg::new("B", self.b as u32), Reg::new("C", self.c as u32),
            Reg::new("D", self.d as u32), Reg::new("E", self.e as u32),
            Reg::new("H", self.h as u32), Reg::new("L", self.l as u32),
            Reg::new("IX", self.ix as u32), Reg::new("IY", self.iy as u32),
            Reg::new("SP", self.sp as u32), Reg::new("PC", self.pc as u32),
            Reg::new("I", self.i as u32), Reg::new("R", self.r as u32),
        ]
    }
    fn flags(&self) -> FlagSet {
        FlagSet {
            carry: self.get_flag(C),
            zero: self.get_flag(Z),
            sign: self.get_flag(S),
            parity: self.get_flag(PV),
            aux: self.get_flag(H),
            overflow: self.get_flag(PV),
            direction: false,
            interrupt: self.iff1,
            trap: false,
        }
    }
    fn mem_read(&self, addr: u32, len: usize) -> Vec<u8> { (addr..addr + len as u32).map(|a| self.rd(a as u16)).collect() }
    fn mem_write(&mut self, addr: u32, data: &[u8]) { for (i, b) in data.iter().enumerate() { self.wr(addr as u16 + i as u16, *b); } }
    fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.push(self.a); v.push(self.f); v.push(self.b); v.push(self.c); v.push(self.d); v.push(self.e); v.push(self.h); v.push(self.l);
        v.push(self.a2); v.push(self.f2); v.push(self.b2); v.push(self.c2); v.push(self.d2); v.push(self.e2); v.push(self.h2); v.push(self.l2);
        v.extend_from_slice(&self.ix.to_le_bytes()); v.extend_from_slice(&self.iy.to_le_bytes());
        v.extend_from_slice(&self.sp.to_le_bytes()); v.extend_from_slice(&self.pc.to_le_bytes());
        v.push(self.i); v.push(self.r);
        v.push(self.iff1 as u8); v.push(self.iff2 as u8); v.push(self.im);
        v.push(self.halted as u8); v.push(self.pending_int as u8); v.push(self.pending_nmi as u8);
        v.extend_from_slice(&self.mem.data);
        v.extend_from_slice(&self.ports);
        v
    }
    fn restore(&mut self, data: &[u8]) {
        // Fixed header is 33 bytes; then 64 KiB RAM + 256 ports.
        const HEADER: usize = 16 + 8 + 4 + 4 + 5;
        const NEED: usize = HEADER + 65536 + 256;
        if data.len() < NEED {
            return;
        }
        let mut p = 0;
        macro_rules! rd8 { () => {{ let x = data[p]; p += 1; x } } }
        macro_rules! rd16 { () => {{ let mut b = [0u8; 2]; b.copy_from_slice(&data[p..p + 2]); p += 2; u16::from_le_bytes(b) } } }
        self.a = rd8!(); self.f = rd8!(); self.b = rd8!(); self.c = rd8!(); self.d = rd8!(); self.e = rd8!(); self.h = rd8!(); self.l = rd8!();
        self.a2 = rd8!(); self.f2 = rd8!(); self.b2 = rd8!(); self.c2 = rd8!(); self.d2 = rd8!(); self.e2 = rd8!(); self.h2 = rd8!(); self.l2 = rd8!();
        self.ix = rd16!(); self.iy = rd16!(); self.sp = rd16!(); self.pc = rd16!();
        self.i = rd8!(); self.r = rd8!();
        self.iff1 = rd8!() != 0; self.iff2 = rd8!() != 0; self.im = rd8!();
        self.halted = rd8!() != 0; self.pending_int = rd8!() != 0; self.pending_nmi = rd8!() != 0;
        let n = self.mem.data.len();
        self.mem.data.copy_from_slice(&data[p..p + n]); p += n;
        self.ports.copy_from_slice(&data[p..p + 256]);
    }
    fn is_halted(&self) -> bool { self.halted }
    fn disasm(&self, addr: u32, count: usize) -> Vec<Disasm> {
        let mut out = Vec::new();
        let mut pc = addr as u16;
        for _ in 0..count {
            let start = pc;
            let op = self.rd(pc);
            let mut bytes = vec![op];
            pc = pc.wrapping_add(1);
            let text = self.dasm_op(op, &mut pc, &mut bytes);
            out.push(Disasm { addr: start as u32, bytes, text });
        }
        out
    }
}

impl CpuZ80 {
    fn dasm_op(&self, op: u8, pc: &mut u16, bytes: &mut Vec<u8>) -> String {
        let mut rd = || { let v = self.rd(*pc); *pc = pc.wrapping_add(1); v };
        let mut rd16 = || { let lo = rd(); let hi = rd(); ((hi as u16) << 8) | lo as u16 };
        let mut push = |b: u8| bytes.push(b);
        match op {
            0x00 => "NOP".into(),
            0x76 => "HALT".into(),
            0x07 => "RLCA".into(), 0x0F => "RRCA".into(), 0x17 => "RLA".into(), 0x1F => "RRA".into(),
            0x2F => "CPL".into(), 0x37 => "SCF".into(), 0x3F => "CCF".into(), 0x27 => "DAA".into(),
            0xF3 => "DI".into(), 0xFB => "EI".into(),
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                let n = rd(); push(n);
                let r = match op { 0x06 => "B", 0x0E => "C", 0x16 => "D", 0x1E => "E", 0x26 => "H", 0x2E => "L", 0x36 => "(HL)", _ => "A" };
                format!("LD {r},{n}")
            }
            0x04 | 0x05 | 0x0C | 0x0D | 0x14 | 0x15 | 0x1C | 0x1D | 0x24 | 0x25 | 0x2C | 0x2D | 0x34 | 0x35 | 0x3C | 0x3D => {
                let m = if op & 1 == 0 { "INC" } else { "DEC" };
                let r = match op { 0x04 | 0x05 => "B", 0x0C | 0x0D => "C", 0x14 | 0x15 => "D", 0x1C | 0x1D => "E", 0x24 | 0x25 => "H", 0x2C | 0x2D => "L", 0x34 | 0x35 => "(HL)", _ => "A" };
                format!("{m} {r}")
            }
            0x01 | 0x11 | 0x21 | 0x31 => { let nn = rd16(); push((nn & 0xFF) as u8); push((nn >> 8) as u8); format!("LD {},{nn}", if op == 0x21 { "HL" } else if op == 0x11 { "DE" } else if op == 0x01 { "BC" } else { "SP" }) }
            0x03 | 0x13 | 0x23 | 0x33 | 0x0B | 0x1B | 0x2B | 0x3B => { let m = if op & 8 == 0 { "INC" } else { "DEC" }; let r = ["BC", "DE", "HL", "SP"][(op >> 4) as usize & 3]; format!("{m} {r}") }
            0x09 => "ADD HL,BC".into(),
            0x19 => "ADD HL,DE".into(),
            0x29 => "ADD HL,HL".into(),
            0x39 => "ADD HL,SP".into(),
            0x40..=0x7F if op != 0x76 => { let dst = (op >> 3) & 7; let src = op & 7; format!("LD {}({}),{}", if dst == 6 { "" } else { "" }, Self::r8name(dst), Self::r8name(src)) }
            0x80..=0xBF => { let g = ["ADD", "ADC", "SUB", "SBC", "AND", "OR", "XOR", "CP"][((op >> 3) & 7) as usize]; let r = Self::r8name(op & 7); format!("{g} A,{r}") }
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => { let n = rd(); push(n); let g = ["ADD", "ADC", "SUB", "SBC", "AND", "OR", "XOR", "CP"][((op >> 3) & 7) as usize]; format!("{g} A,{n}") }
            0xC3 => { let nn = rd16(); push((nn & 0xFF) as u8); push((nn >> 8) as u8); format!("JP {nn}") }
            0x18 => { let e = rd() as i8; push(e as u8); format!("JR {}", *pc as i32 + e as i32) }
            0x20 | 0x28 | 0x30 | 0x38 => { let e = rd() as i8; push(e as u8); let c = ["NZ", "Z", "NC", "C"][((op >> 3) & 7 ^ 1) as usize % 4]; format!("JR {c},{}", *pc as i32 + e as i32) }
            0x10 => { let e = rd() as i8; push(e as u8); format!("DJNZ {}", *pc as i32 + e as i32) }
            0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => { let nn = rd16(); push((nn & 0xFF) as u8); push((nn >> 8) as u8); format!("JP {},{nn}", Self::cc_name((op >> 3) & 7)) }
            0xE9 => "JP (HL)".into(),
            0xC9 => "RET".into(),
            0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => format!("RET {}", Self::cc_name((op >> 3) & 7)),
            0xCD => { let nn = rd16(); push((nn & 0xFF) as u8); push((nn >> 8) as u8); format!("CALL {nn}") }
            0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => { let nn = rd16(); push((nn & 0xFF) as u8); push((nn >> 8) as u8); format!("CALL {},{nn}", Self::cc_name((op >> 3) & 7)) }
            0xC5 | 0xD5 | 0xE5 | 0xF5 => { let r = ["BC", "DE", "HL", "AF"][((op >> 4) & 3) as usize]; format!("PUSH {r}") }
            0xC1 | 0xD1 | 0xE1 | 0xF1 => { let r = ["BC", "DE", "HL", "AF"][((op >> 4) & 3) as usize]; format!("POP {r}") }
            0xD3 => { let n = rd(); push(n); format!("OUT ({n}),A") }
            0xDB => { let n = rd(); push(n); format!("IN A,({n})") }
            0x32 => { let nn = rd16(); push((nn & 0xFF) as u8); push((nn >> 8) as u8); "LD (nn),A".into() }
            0x3A => { let nn = rd16(); push((nn & 0xFF) as u8); push((nn >> 8) as u8); "LD A,(nn)".into() }
            0xCB => { let sub = rd(); push(sub); let r = Self::r8name(sub & 7); match sub & 0xC0 { 0x00 => { let opn = ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SLL", "SRL"][((sub >> 3) & 7) as usize]; format!("{opn} {r}") } 0x40 => format!("BIT {},{}", (sub >> 3) & 7, r), 0x80 => format!("RES {},{}", (sub >> 3) & 7, r), _ => format!("SET {},{}", (sub >> 3) & 7, r) } }
            _ => format!(".byte {op}"),
        }
    }
}

