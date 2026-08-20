//! Intel 8086 CPU core — segmented 16-bit, 1 MiB address space.
//!
//! Implements the mainline instruction set (see AGENTS.md) plus a DOS/BIOS
//! service subset so classic lab programs (INT 21h AH=09 string print,
//! AH=02 char print, AH=4C exit, INT 10h AH=0Eh) run headlessly.

use std::collections::VecDeque;

use crate::cpu::{Cpu, FlagSet, Mem, Output, Reg, RunResult};

const MEM_SIZE: usize = 1 << 20; // 1 MiB

const CF: u16 = 1 << 0;
const PF: u16 = 1 << 2;
const AF: u16 = 1 << 4;
const ZF: u16 = 1 << 6;
const SF: u16 = 1 << 7;
const TF: u16 = 1 << 8;
const IF: u16 = 1 << 9;
const DF: u16 = 1 << 10;
const OF: u16 = 1 << 11;

pub struct Cpu8086 {
    pub ax: u16, pub bx: u16, pub cx: u16, pub dx: u16,
    pub si: u16, pub di: u16, pub bp: u16, pub sp: u16,
    pub cs: u16, pub ds: u16, pub es: u16, pub ss: u16,
    pub ip: u16,
    pub flags: u16,
    pub mem: Mem,
    pub out: Output,
    pub halted: bool,
    pub fault: Option<String>,
    // pending string-op repeat prefix state
    rep: Option<bool>, // None=no prefix, Some(repe)=F3/F2
    seg_ov: Option<u16>, // segment override for next instruction
    // keyboard input: INT 21h AH=01/06/07/08/0C pop from here
    keybuf: VecDeque<u8>,
    input_pending: bool, // INT 21h read with empty buffer: IP re-pointed, CPU blocked
    /// I/O port space (256 ports); OUT to port 01h also prints AL.
    pub ports: [u8; 256],
    // hardware interrupts (latched, serviced at the end of step())
    pending_nmi: bool,   // NMI pin: non-maskable, vector 02h
    pending_intr: bool,  // INTR pin: maskable via IF, device-supplied vector
    intr_vector: u8,
}

impl Default for Cpu8086 {
    fn default() -> Self { Self::new() }
}

impl Cpu8086 {
    /// Raise a hardware interrupt. "NMI" (non-maskable, vector 02h) or
    /// "INTR" (maskable via IF, vector = data & 0xFF).
    pub fn request_interrupt(&mut self, kind: &str, data: u32) -> Result<(), String> {
        match kind.to_ascii_uppercase().as_str() {
            "NMI" => { self.pending_nmi = true; Ok(()) }
            "INTR" => { self.pending_intr = true; self.intr_vector = (data & 0xFF) as u8; Ok(()) }
            _ => Err(format!("unknown 8086 interrupt '{kind}' (use NMI or INTR)")),
        }
    }

    fn hardware_intr(&mut self, vector: u8) {
        let addr = vector as usize * 4;
        let ip = self.mem.read16(addr);
        let cs = self.mem.read16(addr + 2);
        self.push16(self.flags);
        self.push16(self.cs);
        self.push16(self.ip);
        self.set_flag(IF, false);
        self.set_flag(TF, false);
        self.cs = cs;
        self.ip = ip;
    }

    /// Service latched hardware interrupts at the end of a step (never while
    /// halted or blocked on input). NMI > INTR; INTR needs IF set.
    fn service_interrupts(&mut self) {
        if self.pending_nmi {
            self.pending_nmi = false;
            self.hardware_intr(2);
        } else if self.pending_intr && self.flag(IF) {
            self.pending_intr = false;
            self.hardware_intr(self.intr_vector);
        }
    }

    pub fn new() -> Self {
        let mut c = Cpu8086 {
            ax: 0, bx: 0, cx: 0, dx: 0,
            si: 0, di: 0, bp: 0, sp: 0xFFFF,
            cs: 0, ds: 0, es: 0, ss: 0,
            ip: 0, flags: 0x0002, // bit 1 always set
            mem: Mem::new(MEM_SIZE),
            out: Output::default(),
            halted: false,
            fault: None,
            rep: None,
            seg_ov: None,
            keybuf: VecDeque::new(),
            ports: [0; 256],
            pending_nmi: false,
            pending_intr: false,
            intr_vector: 0,
            input_pending: false,
        };
        c.reset();
        c
    }

    pub fn last_error(&self) -> Option<String> { self.fault.clone() }

    fn phys(&self, seg: u16, off: u16) -> usize {
        (((seg as u32) << 4) + off as u32) as usize
    }

    // ----- flag helpers -----
    #[inline] fn flag(&self, m: u16) -> bool { self.flags & m != 0 }
    #[inline] fn set_flag(&mut self, m: u16, v: bool) {
        if v { self.flags |= m } else { self.flags &= !m }
    }
    #[inline] fn parity(&self, x: u8) -> bool { (x.count_ones() & 1) == 0 }

    // ----- instruction fetch -----
    #[inline]
    fn fetch8(&mut self) -> u8 {
        let b = self.mem.read(self.phys(self.cs, self.ip));
        self.ip = self.ip.wrapping_add(1);
        b
    }
    #[inline]
    fn fetch16(&mut self) -> u16 {
        let v = self.mem.read16(self.phys(self.cs, self.ip));
        self.ip = self.ip.wrapping_add(2);
        v
    }

    // ----- effective address / modrm -----
    fn modrm(&mut self) -> (u8, u8, u8) {
        let m = self.fetch8();
        (m >> 6, (m >> 3) & 7, m & 7)
    }

    fn ea(&mut self, mod_: u8, rm: u8, default_seg: u16) -> (u16, u16) {
        if mod_ == 3 { return (0, 0); } // register operand: no address
        let base_seg = if mod_ == 0 && rm == 6 {
            default_seg // [disp16]
        } else {
            match rm {
                5 => self.ss, // [BP]
                6 if mod_ != 0 => self.ss, // [BP+disp8/16]
                _ => default_seg,
            }
        };
        let seg = self.seg_ov.take().unwrap_or(base_seg);
        let off = match (mod_, rm) {
            (0, 0) => self.bx.wrapping_add(self.si),
            (0, 1) => self.bx.wrapping_add(self.di),
            (0, 2) => self.bp.wrapping_add(self.si),
            (0, 3) => self.bp.wrapping_add(self.di),
            (0, 4) => self.si,
            (0, 5) => self.di,
            (0, 6) => self.fetch16(),
            (0, 7) => self.bx,
            (1, r) => {
                let d = self.fetch8() as i8 as i16;
                let base: u16 = match r {
                    0 => self.bx.wrapping_add(self.si), 1 => self.bx.wrapping_add(self.di),
                    2 => self.bp.wrapping_add(self.si), 3 => self.bp.wrapping_add(self.di),
                    4 => self.si, 5 => self.di, 6 => self.bp, _ => self.bx,
                };
                base.wrapping_add_signed(d)
            }
            _ => {
                let d = self.fetch16();
                let base: u16 = match rm {
                    0 => self.bx.wrapping_add(self.si), 1 => self.bx.wrapping_add(self.di),
                    2 => self.bp.wrapping_add(self.si), 3 => self.bp.wrapping_add(self.di),
                    4 => self.si, 5 => self.di, 6 => self.bp, _ => self.bx,
                };
                base.wrapping_add(d)
            }
        };
        (seg, off)
    }

    fn read_ea8(&mut self, seg: u16, off: u16) -> u8 { self.mem.read(self.phys(seg, off)) }
    fn read_ea16(&mut self, seg: u16, off: u16) -> u16 { self.mem.read16(self.phys(seg, off)) }
    fn write_ea8(&mut self, seg: u16, off: u16, v: u8) { self.mem.write(self.phys(seg, off), v) }
    fn write_ea16(&mut self, seg: u16, off: u16, v: u16) { self.mem.write16(self.phys(seg, off), v) }

    // register-or-memory operand helpers (mod==3 => register)
    fn rm8(&mut self, m: u8, rm: u8, seg: u16, off: u16) -> u8 {
        if m == 3 { self.reg8(rm) } else { self.read_ea8(seg, off) }
    }
    fn rm16(&mut self, m: u8, rm: u8, seg: u16, off: u16) -> u16 {
        if m == 3 { self.reg16(rm) } else { self.read_ea16(seg, off) }
    }
    fn write_rm8(&mut self, m: u8, rm: u8, seg: u16, off: u16, v: u8) {
        if m == 3 { self.set_reg8(rm, v) } else { self.write_ea8(seg, off, v) }
    }
    fn write_rm16(&mut self, m: u8, rm: u8, seg: u16, off: u16, v: u16) {
        if m == 3 { self.set_reg16(rm, v) } else { self.write_ea16(seg, off, v) }
    }

    // ----- push/pop -----
    fn push16(&mut self, v: u16) {
        self.sp = self.sp.wrapping_sub(2);
        self.mem.write16(self.phys(self.ss, self.sp), v);
    }
    fn pop16(&mut self) -> u16 {
        let v = self.mem.read16(self.phys(self.ss, self.sp));
        self.sp = self.sp.wrapping_add(2);
        v
    }

    // ----- register helpers -----
    #[inline] fn reg8(&self, i: u8) -> u8 {
        match i {
            0 => self.ax as u8,      // AL
            1 => self.cx as u8,      // CL
            2 => self.dx as u8,      // DL
            3 => self.bx as u8,      // BL
            4 => (self.ax >> 8) as u8, // AH
            5 => (self.cx >> 8) as u8, // CH
            6 => (self.dx >> 8) as u8, // DH
            _ => (self.bx >> 8) as u8, // BH
        }
    }
    #[inline] fn set_reg8(&mut self, i: u8, v: u8) {
        match i {
            0 => self.ax = (self.ax & 0xFF00) | v as u16, // AL
            1 => self.cx = (self.cx & 0xFF00) | v as u16, // CL
            2 => self.dx = (self.dx & 0xFF00) | v as u16, // DL
            3 => self.bx = (self.bx & 0xFF00) | v as u16, // BL
            4 => self.ax = (self.ax & 0x00FF) | ((v as u16) << 8), // AH
            5 => self.cx = (self.cx & 0x00FF) | ((v as u16) << 8), // CH
            6 => self.dx = (self.dx & 0x00FF) | ((v as u16) << 8), // DH
            _ => self.bx = (self.bx & 0x00FF) | ((v as u16) << 8), // BH
        }
    }
    #[inline] fn reg16(&self, i: u8) -> u16 {
        match i {
            0 => self.ax, 1 => self.cx, 2 => self.dx, 3 => self.bx,
            4 => self.sp, 5 => self.bp, 6 => self.si, _ => self.di,
        }
    }
    #[inline] fn set_reg16(&mut self, i: u8, v: u16) {
        match i {
            0 => self.ax = v, 1 => self.cx = v, 2 => self.dx = v, 3 => self.bx = v,
            4 => self.sp = v, 5 => self.bp = v, 6 => self.si = v, _ => self.di = v,
        }
    }

    // ----- arithmetic helpers -----
    fn flags_add8(&mut self, a: u8, b: u8, carry_in: bool) {
        let ci = carry_in as u8;
        let r = a.wrapping_add(b).wrapping_add(ci);
        self.set_flag(CF, (a as u16) + (b as u16) + ci as u16 > 0xFF);
        self.set_flag(ZF, r == 0);
        self.set_flag(SF, r & 0x80 != 0);
        self.set_flag(PF, self.parity(r));
        self.set_flag(OF, ((a ^ r) & (b ^ r) & 0x80) != 0);
        self.set_flag(AF, (a & 0xF) + (b & 0xF) + ci > 0xF);
    }
    fn flags_add16(&mut self, a: u16, b: u16, carry_in: bool) {
        let ci = carry_in as u16;
        let r = a.wrapping_add(b).wrapping_add(ci);
        self.set_flag(CF, (a as u32) + (b as u32) + ci as u32 > 0xFFFF);
        self.set_flag(ZF, r == 0);
        self.set_flag(SF, r & 0x8000 != 0);
        self.set_flag(PF, self.parity(r as u8));
        self.set_flag(OF, ((a ^ r) & (b ^ r) & 0x8000) != 0);
        self.set_flag(AF, (a & 0xF) + (b & 0xF) + ci > 0xF);
    }
    fn flags_sub8(&mut self, a: u8, b: u8, borrow_in: bool) {
        let bi = borrow_in as u8;
        let r = a.wrapping_sub(b).wrapping_sub(bi);
        self.set_flag(CF, (a as u16) < (b as u16) + bi as u16);
        self.set_flag(ZF, r == 0);
        self.set_flag(SF, r & 0x80 != 0);
        self.set_flag(PF, self.parity(r));
        self.set_flag(OF, ((a ^ b) & (a ^ r) & 0x80) != 0);
        self.set_flag(AF, (a & 0xF) < (b & 0xF) + bi);
    }
    fn flags_sub16(&mut self, a: u16, b: u16, borrow_in: bool) {
        let bi = borrow_in as u16;
        let r = a.wrapping_sub(b).wrapping_sub(bi);
        self.set_flag(CF, (a as u32) < (b as u32) + bi as u32);
        self.set_flag(ZF, r == 0);
        self.set_flag(SF, r & 0x8000 != 0);
        self.set_flag(PF, self.parity(r as u8));
        self.set_flag(OF, ((a ^ b) & (a ^ r) & 0x8000) != 0);
        self.set_flag(AF, (a & 0xF) < (b & 0xF) + bi);
    }
    fn flags_logic8(&mut self, r: u8) {
        self.set_flag(CF, false); self.set_flag(OF, false);
        self.set_flag(ZF, r == 0); self.set_flag(SF, r & 0x80 != 0);
        self.set_flag(PF, self.parity(r));
    }
    fn flags_logic16(&mut self, r: u16) {
        self.set_flag(CF, false); self.set_flag(OF, false);
        self.set_flag(ZF, r == 0); self.set_flag(SF, r & 0x8000 != 0);
        self.set_flag(PF, self.parity(r as u8));
    }

    // DAA: adjust packed BCD in AL after ADD/ADC
    fn daa(&mut self) {
        let old_cf = self.flag(CF);
        let old_af = self.flag(AF);
        if self.al() & 0x0F > 9 || old_af {
            self.set_al(self.al().wrapping_add(6));
            self.set_flag(AF, true);
        } else {
            self.set_flag(AF, false);
        }
        if self.al() > 0x9F || old_cf {
            self.set_al(self.al().wrapping_add(0x60));
            self.set_flag(CF, true);
        } else {
            self.set_flag(CF, false);
        }
        self.set_flag(ZF, self.al() == 0);
        self.set_flag(SF, self.al() & 0x80 != 0);
        self.set_flag(PF, self.parity(self.al()));
    }

    // DAS: adjust packed BCD in AL after SUB/SBB
    fn das(&mut self) {
        let old_cf = self.flag(CF);
        let old_af = self.flag(AF);
        if self.al() & 0x0F > 9 || old_af {
            self.set_al(self.al().wrapping_sub(6));
            self.set_flag(AF, true);
        } else {
            self.set_flag(AF, false);
        }
        if self.al() > 0x9F || old_cf {
            self.set_al(self.al().wrapping_sub(0x60));
            self.set_flag(CF, true);
        } else {
            self.set_flag(CF, false);
        }
        self.set_flag(ZF, self.al() == 0);
        self.set_flag(SF, self.al() & 0x80 != 0);
        self.set_flag(PF, self.parity(self.al()));
    }

    // AAA: ASCII adjust after ADD
    fn aaa(&mut self) {
        if self.al() & 0x0F > 9 || self.flag(AF) {
            self.set_al(self.al().wrapping_add(6));
            self.set_ah(self.ah().wrapping_add(1));
            self.set_flag(AF, true);
            self.set_flag(CF, true);
        } else {
            self.set_flag(AF, false);
            self.set_flag(CF, false);
        }
        self.set_al(self.al() & 0x0F);
    }

    // AAS: ASCII adjust after SUB
    fn aas(&mut self) {
        if self.al() & 0x0F > 9 || self.flag(AF) {
            self.set_al(self.al().wrapping_sub(6));
            self.set_ah(self.ah().wrapping_sub(1));
            self.set_flag(AF, true);
            self.set_flag(CF, true);
        } else {
            self.set_flag(AF, false);
            self.set_flag(CF, false);
        }
        self.set_al(self.al() & 0x0F);
    }

    fn jcc_taken(&self, cond: u8) -> bool {
        match cond {
            0x0 => self.flag(OF),
            0x1 => !self.flag(OF),
            0x2 => self.flag(CF),
            0x3 => !self.flag(CF),
            0x4 => self.flag(ZF),
            0x5 => !self.flag(ZF),
            0x6 => self.flag(CF) || self.flag(ZF),
            0x7 => !self.flag(CF) && !self.flag(ZF),
            0x8 => self.flag(SF),
            0x9 => !self.flag(SF),
            0xA => self.flag(PF),
            0xB => !self.flag(PF),
            0xC => self.flag(SF) != self.flag(OF),
            0xD => self.flag(SF) == self.flag(OF),
            0xE => self.flag(ZF) || (self.flag(SF) != self.flag(OF)),
            _ => self.flag(ZF) && (self.flag(SF) == self.flag(OF)),
        }
    }

    fn unimplemented(&mut self, op: u8) {
        self.fault = Some(format!("8086: unimplemented opcode {op:02X}h at CS:IP"));
        self.halted = true;
    }

    // ----- DOS/BIOS services -----
    /// Queue a key for INT 21h AH=01/06/07/08/0C reads. Clears the
    /// input-pending state so a blocked INT 21h can re-execute.
    pub fn push_key(&mut self, ch: u8) {
        self.keybuf.push_back(ch);
        self.input_pending = false;
    }

    pub fn waiting_input(&self) -> bool { self.input_pending }

    fn key_read(&mut self) -> Option<u8> { self.keybuf.pop_front() }

    fn port_in16(&self, p: usize) -> u16 {
        self.ports[p] as u16 | (self.ports[(p + 1) & 0xFF] as u16) << 8
    }
    fn port_out8(&mut self, p: usize, v: u8) {
        self.ports[p] = v;
        if p == 0x01 {
            self.out.put_char(v as char);
        }
    }
    fn port_out16(&mut self, p: usize, v: u16) {
        self.port_out8(p, v as u8);
        self.ports[(p + 1) & 0xFF] = (v >> 8) as u8;
    }

    /// INT 21h input read. With an empty buffer the IP is re-pointed at the
    /// INT 21h instruction and the CPU blocks until push_key() is called.
    fn int_read(&mut self, echo: bool) {
        match self.key_read() {
            Some(k) => {
                self.set_al(k);
                if echo { self.out.put_char(k as char); }
            }
            None => {
                self.input_pending = true;
                self.ip = self.ip.wrapping_sub(2); // re-execute INT 21h
            }
        }
    }

    fn int_service(&mut self, n: u8) {
        match (n, self.ah()) {
            (0x21, 0x01) => self.int_read(true),                 // read char, echo
            (0x21, 0x07) | (0x21, 0x08) => self.int_read(false), // read, no echo
            (0x21, 0x02) => { self.out.put_char(self.dl() as char); }
            (0x21, 0x06) => {
                if self.dl() == 0xFF { self.int_read(false); }   // direct read
                else { self.out.put_char(self.dl() as char); }   // direct write
            }
            (0x21, 0x09) => {
                let mut a = self.phys(self.ds, self.dx);
                loop {
                    let c = self.mem.read(a);
                    a += 1;
                    if c == b'$' { break; }
                    self.out.put_char(c as char);
                }
            }
            (0x21, 0x0C) => { // flush buffer, then optionally read (AL=01/06/07/08)
                self.keybuf.clear();
                match self.al() {
                    0x01 => self.int_read(true),
                    0x06..=0x08 => self.int_read(false),
                    _ => {}
                }
            }
            (0x21, 0x4C) => { self.halted = true; }
            (0x10, 0x0E) => { self.out.put_char(self.al() as char); }
            (0x10, 0x0F) => { self.set_ah(0x03); self.set_al(0x00); } // video mode
            _ => {} // other services: silently ignored
        }
    }
    #[inline] fn al(&self) -> u8 { self.ax as u8 }
    #[inline] fn set_al(&mut self, v: u8) { self.ax = (self.ax & 0xFF00) | v as u16; }
    #[inline] fn ah(&self) -> u8 { (self.ax >> 8) as u8 }
    #[inline] fn dl(&self) -> u8 { self.dx as u8 }
    #[inline] fn set_ah(&mut self, v: u8) { self.ax = (self.ax & 0x00FF) | ((v as u16) << 8); }

    // ----- one instruction -----
    pub fn exec(&mut self) -> bool {
        self.rep = None;
        self.seg_ov = None;
        // prefix scan
        loop {
            let p = self.mem.read(self.phys(self.cs, self.ip));
            match p {
                0x26 => { self.seg_ov = Some(self.es); self.ip = self.ip.wrapping_add(1); }
                0x2E => { self.seg_ov = Some(self.cs); self.ip = self.ip.wrapping_add(1); }
                0x36 => { self.seg_ov = Some(self.ss); self.ip = self.ip.wrapping_add(1); }
                0x3E => { self.seg_ov = Some(self.ds); self.ip = self.ip.wrapping_add(1); }
                0xF3 => { self.rep = Some(true); self.ip = self.ip.wrapping_add(1); }
                0xF2 => { self.rep = Some(false); self.ip = self.ip.wrapping_add(1); }
                _ => break,
            }
        }
        let op = self.fetch8();
        match op {
            0x00..=0x05 => self.op_group1(op),
            0x06 => self.push16(self.es),
            0x07 => { self.es = self.pop16(); }
            0x08..=0x0D => self.op_group1(op),
            0x0E => self.push16(self.cs),
            0x0F => { self.pop16(); }
            0x10..=0x15 => self.op_group1(op),
            0x16 => self.push16(self.ss),
            0x17 => { self.ss = self.pop16(); }
            0x18..=0x1D => self.op_group1(op),
            0x1E => self.push16(self.ds),
            0x1F => { self.ds = self.pop16(); }
            0x20..=0x25 => self.op_group1(op),
            0x26 | 0x2E | 0x36 | 0x3E | 0xF2 | 0xF3 => unreachable!(),
            0x27 => self.daa(),
            0x28..=0x2D => self.op_group1(op),
            0x2F => self.das(),
            0x30..=0x35 => self.op_group1(op),
            0x37 => self.aaa(),
            0x38..=0x3D => self.op_group1(op),
            0x3F => self.aas(),
            0x40..=0x47 => { // INC r16
                let i = op & 7;
                let v = self.reg16(i).wrapping_add(1);
                let a = self.reg16(i);
                self.set_flag(OF, a == 0x7FFF);
                self.set_flag(AF, (a & 0xF) == 0xF);
                self.set_flag(ZF, v == 0);
                self.set_flag(SF, v & 0x8000 != 0);
                self.set_flag(PF, self.parity(v as u8));
                self.set_reg16(i, v);
            }
            0x48..=0x4F => { // DEC r16
                let i = op & 7;
                let a = self.reg16(i);
                let v = a.wrapping_sub(1);
                self.set_flag(OF, a == 0x8000);
                self.set_flag(AF, (a & 0xF) == 0);
                self.set_flag(ZF, v == 0);
                self.set_flag(SF, v & 0x8000 != 0);
                self.set_flag(PF, self.parity(v as u8));
                self.set_reg16(i, v);
            }
            0x50..=0x57 => self.push16(self.reg16(op & 7)),
            0x58..=0x5F => { let v = self.pop16(); self.set_reg16(op & 7, v); }
            0x60 | 0x61 => {} // PUSHA/POPA: unsupported no-op
            0x62 => { // BOUND r16, m16
                let (m, r, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let v = self.reg16(r);
                if m != 3 {
                    let low = self.mem.read16(self.phys(seg, off));
                    let high = self.mem.read16(self.phys(seg, off.wrapping_add(2)));
                    if (v as i16) < (low as i16) || (v as i16) > (high as i16) {
                        self.int_vec(5);
                    }
                }
            }
            0x68 => { let v = self.fetch16(); self.push16(v); }
            0x69 => { // IMUL r16,r/m16,imm16
                let (m, _, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let b = self.rm16(m, rm, seg, off);
                let c = self.fetch16();
                let r = self.imul_word(b, c);
                self.set_reg16(m & 7, r);
            }
            0x6A => { let v = self.fetch8() as i8 as u16; self.push16(v); }
            0x6C..=0x6F => self.op_io_string(op),
            0x6B => { // IMUL r16,r/m16,imm8
                let (m, _, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let b = self.rm16(m, rm, seg, off);
                let c = self.fetch8() as i8 as i16 as u16;
                let r = self.imul_word(b, c);
                self.set_reg16(m & 7, r);
            }
            0x70..=0x7F => { // Jcc rel8
                let d = self.fetch8() as i8 as i16;
                if self.jcc_taken(op & 0xF) {
                    self.ip = self.ip.wrapping_add_signed(d);
                }
            }
            0x80..=0x83 => self.op_group1_imm(op),
            0x84 | 0x85 => self.op_test(),
            0x86 | 0x87 => self.op_xchg(op),
            0x88..=0x8B => self.op_mov_regmem(op),
            0x8C => { // MOV r/m16,seg
                let (m, r, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let v = match r { 0 => self.es, 1 => self.cs, 2 => self.ss, _ => self.ds };
                self.write_rm16(m, rm, seg, off, v);
            }
            0x8D => { // LEA
                let (m, r, rm) = self.modrm();
                let (_seg, off) = self.ea(m, rm, self.ds);
                self.set_reg16(r, off);
            }
            0x8E => { // MOV seg,r/m16
                let (m, r, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let v = self.rm16(m, rm, seg, off);
                match r {
                    0 => self.es = v, 1 => self.cs = v, 2 => self.ss = v, _ => self.ds = v,
                }
            }
            0x8F => { // POP r/m16
                let (m, _, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let v = self.pop16();
                self.write_rm16(m, rm, seg, off, v);
            }
            0x90 => {} // NOP / XCHG AX,AX
            0x91..=0x97 => { let t = self.reg16(op & 7); self.set_reg16(op & 7, self.ax); self.ax = t; }
            0x98 => self.ax = self.al() as i8 as i16 as u16, // CBW
            0x99 => { // CWD
                let sign = self.ax & 0x8000 != 0;
                self.dx = if sign { 0xFFFF } else { 0 };
            }
            0x9A => { // CALL far ptr16:16
                let off = self.fetch16();
                let seg = self.fetch16();
                self.push16(self.cs);
                self.push16(self.ip);
                self.cs = seg;
                self.ip = off;
            }
            0x9B => {} // WAIT/FWAIT: no FPU, no-op
            0x9C => self.push16(self.flags),
            0x9D => { self.flags = (self.pop16() & !(0x2000 | 0x4000)) | 0x0002; } // POPF: restores TF
            0x9E => { // SAHF
                let v = self.ah();
                self.set_flag(CF, v & 1 != 0); self.set_flag(PF, v & 4 != 0);
                self.set_flag(AF, v & 0x10 != 0); self.set_flag(ZF, v & 0x40 != 0);
                self.set_flag(SF, v & 0x80 != 0);
            }
            0x9F => { // LAHF
                let mut v = 0u8;
                if self.flag(CF) { v |= 1; }
                if self.flag(PF) { v |= 4; }
                if self.flag(AF) { v |= 0x10; }
                if self.flag(ZF) { v |= 0x40; }
                if self.flag(SF) { v |= 0x80; }
                self.set_ah(v);
            }
            0xA0 => { let off = self.fetch16(); let v = self.read_ea8(self.ds, off); self.set_al(v); }
            0xA1 => { let off = self.fetch16(); let v = self.read_ea16(self.ds, off); self.ax = v; }
            0xA2 => { let off = self.fetch16(); self.write_ea8(self.ds, off, self.al()); }
            0xA3 => { let off = self.fetch16(); self.write_ea16(self.ds, off, self.ax); }
            0xA4..=0xA7 => self.op_string(op),
            0xA8 => { // TEST AL,imm8
                let v = self.fetch8();
                let r = self.al() & v;
                self.flags_logic8(r);
            }
            0xA9 => { // TEST AX,imm16
                let v = self.fetch16();
                let r = self.ax & v;
                self.flags_logic16(r);
            }
            0xAA..=0xAF => self.op_string(op),
            0xB0..=0xB7 => { let v = self.fetch8(); self.set_reg8(op & 7, v); }
            0xB8..=0xBF => { let v = self.fetch16(); self.set_reg16(op & 7, v); }
            0xC0 | 0xC1 => { // shift r/m8/16,imm8 (186+)
                let (m, r, rm) = self.modrm();
                let n = (self.fetch8() & 0x1F) as usize;
                self.op_shift(m, r, rm, n);
            }
            0xC2 => { let _n = self.fetch16(); self.pop16(); } // RET imm16
            0xC3 => { self.ip = self.pop16(); } // RET
            0xC4 => { // LES
                let (m, r, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let lo = self.read_ea16(seg, off);
                let hi = self.read_ea16(seg, off + 2);
                self.set_reg16(r, lo);
                self.es = hi;
            }
            0xC5 => { // LDS
                let (m, r, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let lo = self.read_ea16(seg, off);
                let hi = self.read_ea16(seg, off + 2);
                self.set_reg16(r, lo);
                self.ds = hi;
            }
            0xC6 => { // MOV r/m8,imm8
                let (m, _, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let v = self.fetch8();
                self.write_rm8(m, rm, seg, off, v);
            }
            0xC7 => { // MOV r/m16,imm16
                let (m, _, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let v = self.fetch16();
                self.write_rm16(m, rm, seg, off, v);
            }
            0xC8 => { // ENTER: no-op
                let _ = self.fetch16(); let _ = self.fetch8();
            }
            0xC9 => {} // LEAVE
            0xCA => { let _n = self.fetch16(); let ip = self.pop16(); let cs = self.pop16(); self.ip = ip; self.cs = cs; }
            0xCB => { let ip = self.pop16(); let cs = self.pop16(); self.ip = ip; self.cs = cs; }
            0xCC => self.int_vec(3),
            0xCD => { let n = self.fetch8(); self.int_vec(n); }
            0xCE => { if self.flag(OF) { self.int_vec(4); } }
            0xCF => { let ip = self.pop16(); let cs = self.pop16(); let fl = self.pop16(); self.ip = ip; self.cs = cs; self.flags = fl; }
            0xD0..=0xD3 => { let (m, r, rm) = self.modrm();
                let n = if op & 3 == 1 { 1 } else if op & 3 == 3 { self.cx as usize } else { 1 };
                self.op_shift(m, r, rm, n);
            }
            0xD4 => { let base = self.fetch8(); // AAM: AL = AL % base, AH = AL / base
                let v = self.al();
                if base == 0 {
                    self.fault = Some("8086: AAM divide by zero".into());
                    self.halted = true;
                    return false;
                }
                self.set_ah(v / base);
                self.set_al(v % base);
                self.flags_logic8(self.al());
            }
            0xD5 => { let base = self.fetch8(); // AAD: AL = AH*base + AL, AH = 0
                let v = self.ah().wrapping_mul(base).wrapping_add(self.al());
                self.set_ah(0);
                self.set_al(v);
                self.flags_logic8(self.al());
            }
            0xD6 => {} // SALC
            0xD7 => { // XLAT
                let off = self.bx.wrapping_add(self.al() as u16);
                let v = self.read_ea8(self.ds, off);
                self.set_al(v);
            }
            0xD8..=0xDF => {} // ESC: no-op
            0xE0..=0xE3 => { // LOOP/LOOPZ/LOOPNZ/JCXZ rel8
                let d = self.fetch8() as i8 as i16;
                let cx = self.cx;
                match op {
                    0xE0 => { self.cx = cx.wrapping_sub(1); if cx != 0 { self.ip = self.ip.wrapping_add_signed(d); } }
                    0xE1 => { self.cx = cx.wrapping_sub(1); if cx != 0 && self.flag(ZF) { self.ip = self.ip.wrapping_add_signed(d); } }
                    0xE2 => { self.cx = cx.wrapping_sub(1); if cx != 0 && !self.flag(ZF) { self.ip = self.ip.wrapping_add_signed(d); } }
                    _ => { if cx == 0 { self.ip = self.ip.wrapping_add_signed(d); } }
                }
            }
            0xE4 => { let p = self.fetch8() as usize; self.set_al(self.ports[p]); } // IN AL,imm8
            0xE5 => { let p = self.fetch8() as usize; self.ax = self.port_in16(p); }
            0xE6 => { let p = self.fetch8() as usize; self.port_out8(p, self.al()); } // OUT imm8,AL
            0xE7 => { let p = self.fetch8() as usize; self.port_out16(p, self.ax); }
            0xE8 => { // CALL rel16
                let d = self.fetch16() as i16;
                self.push16(self.ip);
                self.ip = self.ip.wrapping_add_signed(d);
            }
            0xE9 => { let d = self.fetch16() as i16; self.ip = self.ip.wrapping_add_signed(d); }
            0xEA => { let off = self.fetch16(); let seg = self.fetch16(); self.cs = seg; self.ip = off; }
            0xEB => { let d = self.fetch8() as i8 as i16; self.ip = self.ip.wrapping_add_signed(d); }
            0xEC => { let p = self.dx as usize & 0xFF; self.set_al(self.ports[p]); } // IN AL,DX
            0xED => { let p = self.dx as usize & 0xFF; self.ax = self.port_in16(p); }
            0xEE => { let p = self.dx as usize & 0xFF; self.port_out8(p, self.al()); } // OUT DX,AL
            0xEF => { let p = self.dx as usize & 0xFF; self.port_out16(p, self.ax); }
            0xF0 => {} // LOCK
            0xF4 => { self.halted = true; }
            0xF5 => { self.flags ^= CF; }
            0xF6 | 0xF7 => self.op_f6f7(op),
            0xF8 => self.set_flag(CF, false),
            0xF9 => self.set_flag(CF, true),
            0xFA => self.set_flag(IF, false),
            0xFB => self.set_flag(IF, true),
            0xFC => self.set_flag(DF, false),
            0xFD => self.set_flag(DF, true),
            0xFE => { // INC/DEC r/m8
                let (m, r, rm) = self.modrm();
                let (seg, off) = self.ea(m, rm, self.ds);
                let v = self.rm8(m, rm, seg, off);
                match r {
                    0 => {
                        let nv = v.wrapping_add(1);
                        self.set_flag(OF, v == 0x7F);
                        self.set_flag(AF, (v & 0xF) == 0xF);
                        self.set_flag(ZF, nv == 0);
                        self.set_flag(SF, nv & 0x80 != 0);
                        self.set_flag(PF, self.parity(nv));
                        self.write_rm8(m, rm, seg, off, nv);
                    }
                    1 => {
                        let nv = v.wrapping_sub(1);
                        self.set_flag(OF, v == 0x80);
                        self.set_flag(AF, (v & 0xF) == 0);
                        self.set_flag(ZF, nv == 0);
                        self.set_flag(SF, nv & 0x80 != 0);
                        self.set_flag(PF, self.parity(nv));
                        self.write_rm8(m, rm, seg, off, nv);
                    }
                    _ => self.unimplemented(op),
                }
            }
            0xFF => self.op_ff(),
            _ => self.unimplemented(op),
        }
        self.halted
    }

    fn int_vec(&mut self, n: u8) {
        if n == 0x21 || n == 0x10 {
            self.int_service(n);
            return;
        }
        let addr = n as usize * 4;
        let ip = self.mem.read16(addr);
        let cs = self.mem.read16(addr + 2);
        self.push16(self.flags);
        self.push16(self.cs);
        self.push16(self.ip);
        self.set_flag(IF, false);
        self.set_flag(TF, false);
        self.cs = cs;
        self.ip = ip;
    }

    fn imul_word(&mut self, a: u16, b: u16) -> u16 {
        let r = (a as i16 as i32) * (b as i16 as i32);
        let lo = r as u16;
        let sign_ext_ok = (lo as i16 as i32) == r;
        self.set_flag(CF, !sign_ext_ok);
        self.set_flag(OF, !sign_ext_ok);
        lo
    }

    fn op_group1(&mut self, op: u8) {
        let base = op & 0x38; // 00,08,10,18,20,28,30,38
        let wide = op & 1 == 1;
        let acc = op & 4 == 4; // immediate-to-accumulator form (04/05, 0C/0D, ...)
        if acc {
            if !wide {
                let imm = self.fetch8();
                let a = self.al();
                match base {
                    0x00 => { self.set_al(a.wrapping_add(imm)); self.flags_add8(a, imm, false); }
                    0x08 => { self.set_al(a | imm); self.flags_logic8(self.al()); }
                    0x10 => { self.set_al(a.wrapping_add(imm).wrapping_add(self.flag(CF) as u8)); self.flags_add8(a, imm, self.flag(CF)); }
                    0x18 => { self.set_al(a.wrapping_sub(imm).wrapping_sub(self.flag(CF) as u8)); self.flags_sub8(a, imm, self.flag(CF)); }
                    0x20 => { self.set_al(a & imm); self.flags_logic8(self.al()); }
                    0x28 => { self.set_al(a.wrapping_sub(imm)); self.flags_sub8(a, imm, false); }
                    0x30 => { self.set_al(a ^ imm); self.flags_logic8(self.al()); }
                    _ => { let r = a.wrapping_sub(imm); self.flags_sub8(a, imm, false); let _ = r; }
                }
                return;
            }
            let imm = self.fetch16();
            let a = self.ax;
            match base {
                0x00 => { self.ax = a.wrapping_add(imm); self.flags_add16(a, imm, false); }
                0x08 => { self.ax = a | imm; self.flags_logic16(self.ax); }
                0x10 => { self.ax = a.wrapping_add(imm).wrapping_add(self.flag(CF) as u16); self.flags_add16(a, imm, self.flag(CF)); }
                0x18 => { self.ax = a.wrapping_sub(imm).wrapping_sub(self.flag(CF) as u16); self.flags_sub16(a, imm, self.flag(CF)); }
                0x20 => { self.ax = a & imm; self.flags_logic16(self.ax); }
                0x28 => { self.ax = a.wrapping_sub(imm); self.flags_sub16(a, imm, false); }
                0x30 => { self.ax = a ^ imm; self.flags_logic16(self.ax); }
                _ => { let r = a.wrapping_sub(imm); self.flags_sub16(a, imm, false); let _ = r; }
            }
            return;
        }
        let (m, r, rm) = self.modrm();
        let (seg, off) = self.ea(m, rm, self.ds);
        if wide {
            let a = self.rm16(m, rm, seg, off);
            let b = self.reg16(r);
            let mut nv = 0u16;
            match base {
                0x00 => { nv = a.wrapping_add(b); self.flags_add16(a, b, false); }
                0x08 => { nv = a | b; self.flags_logic16(nv); }
                0x10 => { nv = a.wrapping_add(b).wrapping_add(self.flag(CF) as u16); self.flags_add16(a, b, self.flag(CF)); }
                0x18 => { nv = a.wrapping_sub(b).wrapping_sub(self.flag(CF) as u16); self.flags_sub16(a, b, self.flag(CF)); }
                0x20 => { nv = a & b; self.flags_logic16(nv); }
                0x28 => { nv = a.wrapping_sub(b); self.flags_sub16(a, b, false); }
                0x30 => { nv = a ^ b; self.flags_logic16(nv); }
                _ => { let _ = a.wrapping_sub(b); self.flags_sub16(a, b, false); }
            }
            if base != 0x38 { self.write_rm16(m, rm, seg, off, nv); }
        } else {
            let a = self.rm8(m, rm, seg, off);
            let b = self.reg8(r);
            let mut nv = 0u8;
            match base {
                0x00 => { nv = a.wrapping_add(b); self.flags_add8(a, b, false); }
                0x08 => { nv = a | b; self.flags_logic8(nv); }
                0x10 => { nv = a.wrapping_add(b).wrapping_add(self.flag(CF) as u8); self.flags_add8(a, b, self.flag(CF)); }
                0x18 => { nv = a.wrapping_sub(b).wrapping_sub(self.flag(CF) as u8); self.flags_sub8(a, b, self.flag(CF)); }
                0x20 => { nv = a & b; self.flags_logic8(nv); }
                0x28 => { nv = a.wrapping_sub(b); self.flags_sub8(a, b, false); }
                0x30 => { nv = a ^ b; self.flags_logic8(nv); }
                _ => { let _ = a.wrapping_sub(b); self.flags_sub8(a, b, false); }
            }
            if base != 0x38 { self.write_rm8(m, rm, seg, off, nv); }
        }
    }

    fn op_group1_imm(&mut self, op: u8) {
        let (m, r, rm) = self.modrm();
        let (seg, off) = self.ea(m, rm, self.ds);
        let wide = (op & 1) == 1;
        let sign_ext = op == 0x83;
        let (a8, a16) = if wide {
            (0u8, self.rm16(m, rm, seg, off))
        } else {
            (self.rm8(m, rm, seg, off), 0u16)
        };
        let raw = self.fetch8();
        let imm8 = raw;
        let imm16 = if sign_ext { raw as i8 as i16 as u16 } else { self.fetch16() };
        if !wide {
            let b = imm8;
            let mut nv = 0u8;
            match r {
                0 => { nv = a8.wrapping_add(b); self.flags_add8(a8, b, false); }
                1 => { nv = a8 | b; self.flags_logic8(nv); }
                2 => { nv = a8.wrapping_add(b).wrapping_add(self.flag(CF) as u8); self.flags_add8(a8, b, self.flag(CF)); }
                3 => { nv = a8.wrapping_sub(b).wrapping_sub(self.flag(CF) as u8); self.flags_sub8(a8, b, self.flag(CF)); }
                4 => { nv = a8 & b; self.flags_logic8(nv); }
                5 => { nv = a8.wrapping_sub(b); self.flags_sub8(a8, b, false); }
                6 => { nv = a8 ^ b; self.flags_logic8(nv); }
                _ => { let _ = a8.wrapping_sub(b); self.flags_sub8(a8, b, false); }
            }
            if r != 7 { self.write_rm8(m, rm, seg, off, nv); }
        } else {
            let b = imm16;
            let mut nv = 0u16;
            match r {
                0 => { nv = a16.wrapping_add(b); self.flags_add16(a16, b, false); }
                1 => { nv = a16 | b; self.flags_logic16(nv); }
                2 => { nv = a16.wrapping_add(b).wrapping_add(self.flag(CF) as u16); self.flags_add16(a16, b, self.flag(CF)); }
                3 => { nv = a16.wrapping_sub(b).wrapping_sub(self.flag(CF) as u16); self.flags_sub16(a16, b, self.flag(CF)); }
                4 => { nv = a16 & b; self.flags_logic16(nv); }
                5 => { nv = a16.wrapping_sub(b); self.flags_sub16(a16, b, false); }
                6 => { nv = a16 ^ b; self.flags_logic16(nv); }
                _ => { let _ = a16.wrapping_sub(b); self.flags_sub16(a16, b, false); }
            }
            if r != 7 { self.write_rm16(m, rm, seg, off, nv); }
        }
    }

    fn op_test(&mut self) {
        let (m, r, rm) = self.modrm();
        let (seg, off) = self.ea(m, rm, self.ds);
        if m & 1 == 0 {
            // byte form 84
            let a = self.rm8(m, rm, seg, off);
            let b = self.reg8(r);
            self.flags_logic8(a & b);
        } else {
            let a = self.rm16(m, rm, seg, off);
            let b = self.reg16(r);
            self.flags_logic16(a & b);
        }
    }

    fn op_xchg(&mut self, op: u8) {
        let (m, r, rm) = self.modrm();
        let (seg, off) = self.ea(m, rm, self.ds);
        if op == 0x86 {
            let a = self.rm8(m, rm, seg, off);
            let b = self.reg8(r);
            self.write_rm8(m, rm, seg, off, b);
            self.set_reg8(r, a);
        } else {
            let a = self.rm16(m, rm, seg, off);
            let b = self.reg16(r);
            self.write_rm16(m, rm, seg, off, b);
            self.set_reg16(r, a);
        }
    }

    fn op_mov_regmem(&mut self, op: u8) {
        let (m, r, rm) = self.modrm();
        let (seg, off) = self.ea(m, rm, self.ds);
        match op {
            0x88 => { let v = self.reg8(r); self.write_rm8(m, rm, seg, off, v); }
            0x89 => { let v = self.reg16(r); self.write_rm16(m, rm, seg, off, v); }
            0x8A => { let v = self.rm8(m, rm, seg, off); self.set_reg8(r, v); }
            _ => { let v = self.rm16(m, rm, seg, off); self.set_reg16(r, v); }
        }
    }

    fn op_shift(&mut self, m: u8, r: u8, rm: u8, n: usize) {
        let wide = (m & 1) == 1;
        let (seg, off) = self.ea(m, rm, self.ds);
        if wide {
            let mut v = self.rm16(m, rm, seg, off);
            for i in 0..n {
                let cf_before = self.flag(CF);
                match r {
                    0 => { self.set_flag(CF, v & 0x8000 != 0); v = v.rotate_left(1); }
                    1 => { self.set_flag(CF, v & 1 != 0); v = v.rotate_right(1); }
                    2 => { let nc = v & 0x8000 != 0; v = (v << 1) | cf_before as u16; self.set_flag(CF, nc); }
                    3 => { let nc = v & 1 != 0; v = (v >> 1) | ((cf_before as u16) << 15); self.set_flag(CF, nc); }
                    4 => { self.set_flag(CF, v & 0x8000 != 0); v <<= 1; }
                    5 => { self.set_flag(CF, v & 1 != 0); v >>= 1; }
                    _ => { self.set_flag(CF, v & 1 != 0); v = ((v as i16) >> 1) as u16; }
                }
                if n == 1 {
                    self.set_flag(OF, match r {
                        0 | 1 => self.flag(CF) != (v & 0x8000 != 0) && r <= 1,
                        4 => self.flag(CF) != (v & 0x8000 != 0),
                        5 => false,
                        6 => false,
                        _ => self.flag(CF),
                    });
                }
                let _ = i;
            }
            if n >= 1 { self.set_flag(ZF, v == 0); self.set_flag(SF, v & 0x8000 != 0); self.set_flag(PF, self.parity(v as u8)); }
            self.write_rm16(m, rm, seg, off, v);
        } else {
            let mut v = self.rm8(m, rm, seg, off);
            for i in 0..n {
                let cf_before = self.flag(CF);
                match r {
                    0 => { self.set_flag(CF, v & 0x80 != 0); v = v.rotate_left(1); }
                    1 => { self.set_flag(CF, v & 1 != 0); v = v.rotate_right(1); }
                    2 => { let nc = v & 0x80 != 0; v = (v << 1) | cf_before as u8; self.set_flag(CF, nc); }
                    3 => { let nc = v & 1 != 0; v = (v >> 1) | ((cf_before as u8) << 7); self.set_flag(CF, nc); }
                    4 => { self.set_flag(CF, v & 0x80 != 0); v <<= 1; }
                    5 => { self.set_flag(CF, v & 1 != 0); v >>= 1; }
                    _ => { self.set_flag(CF, v & 1 != 0); v = ((v as i8) >> 1) as u8; }
                }
                let _ = i;
            }
            self.set_flag(ZF, v == 0);
            self.set_flag(SF, v & 0x80 != 0);
            self.set_flag(PF, self.parity(v));
            self.set_flag(OF, false);
            self.write_rm8(m, rm, seg, off, v);
        }
    }

    fn op_f6f7(&mut self, op: u8) {
        let wide = op == 0xF7;
        let (m, r, rm) = self.modrm();
        let (seg, off) = self.ea(m, rm, self.ds);
        if r == 0 {
            // TEST r/m,imm
            let a = if wide { self.rm16(m, rm, seg, off) as u32 } else { self.rm8(m, rm, seg, off) as u32 };
            let imm = if wide { self.fetch16() as u32 } else { self.fetch8() as u32 };
            if wide { self.flags_logic16((a & imm) as u16); }
            else { self.flags_logic8((a & imm) as u8); }
            return;
        }
        let v = if wide { self.rm16(m, rm, seg, off) } else { self.rm8(m, rm, seg, off) as u16 };
        match r {
            2 => { // NOT
                if wide { self.write_rm16(m, rm, seg, off, !v); } else { self.write_rm8(m, rm, seg, off, !(v as u8)); }
            }
            3 => { // NEG
                if wide {
                    let nv = v.wrapping_neg();
                    self.set_flag(CF, v != 0); self.set_flag(OF, v == 0x8000);
                    self.set_flag(ZF, nv == 0); self.set_flag(SF, nv & 0x8000 != 0);
                    self.set_flag(PF, self.parity(nv as u8));
                    self.write_rm16(m, rm, seg, off, nv);
                } else {
                    let b = v as u8;
                    let nv = b.wrapping_neg();
                    self.set_flag(CF, b != 0); self.set_flag(OF, b == 0x80);
                    self.set_flag(ZF, nv == 0); self.set_flag(SF, nv & 0x80 != 0);
                    self.set_flag(PF, self.parity(nv));
                    self.write_rm8(m, rm, seg, off, nv);
                }
            }
            4 => { // MUL
                if wide {
                    let a = self.ax as u32;
                    let b = v as u32;
                    let r = a * b;
                    self.set_flag(CF, (r >> 16) != 0); self.set_flag(OF, (r >> 16) != 0);
                    self.dx = (r >> 16) as u16; self.ax = r as u16;
                } else {
                    let r = (self.al() as u16) * (v as u8 as u16);
                    self.set_flag(CF, (r >> 8) != 0); self.set_flag(OF, (r >> 8) != 0);
                    self.ax = r;
                }
            }
            5 => { // IMUL
                if wide {
                    let r = (self.ax as i16 as i32) * (v as i16 as i32);
                    let hi = (r >> 16) as u16;
                    self.dx = hi; self.ax = r as u16;
                    let fits = ((r >> 16) == 0 && (r >> 15) == 0) || ((r >> 16) == -1 && (r >> 15) == 1);
                    self.set_flag(CF, !fits); self.set_flag(OF, !fits);
                } else {
                    let r = (self.al() as i8 as i16) * (v as u8 as i8 as i16);
                    self.ax = r as u16;
                    let fits = ((r >> 8) == 0 && (r >> 7) == 0) || ((r >> 8) == -1 && (r >> 7) == 1);
                    self.set_flag(CF, !fits); self.set_flag(OF, !fits);
                }
            }
            6 => { // DIV
                if wide {
                    let n = ((self.dx as u32) << 16) | self.ax as u32;
                    let d = v as u32;
                    if d == 0 || (n / d) > 0xFFFF { self.int_vec(0); return; }
                    self.ax = (n / d) as u16; self.dx = (n % d) as u16;
                } else {
                    let n = self.ax;
                    let d = v as u8 as u16;
                    if d == 0 || (n / d) > 0xFF { self.int_vec(0); return; }
                    self.set_al((n / d) as u8); self.set_ah((n % d) as u8);
                }
            }
            _ => { // IDIV
                if wide {
                    let n = ((self.dx as u32) << 16) | self.ax as u32;
                    let d = v as i16 as i32;
                    if d == 0 || n == 0x8000_0000 && d == -1 { self.int_vec(0); return; }
                    let q = (n as i32).wrapping_div(d);
                    let r = (n as i32).wrapping_rem(d);
                    self.ax = q as u16; self.dx = r as u16;
                } else {
                    let n = self.ax as i16;
                    let d = v as u8 as i8 as i16;
                    if d == 0 || n == -128 && d == -1 { self.int_vec(0); return; }
                    let q = n.wrapping_div(d);
                    let r = n.wrapping_rem(d);
                    self.set_al(q as u8); self.set_ah(r as u8);
                }
            }
        }
    }

    fn op_ff(&mut self) {
        let (m, r, rm) = self.modrm();
        let (seg, off) = self.ea(m, rm, self.ds);
        match r {
            0 => { let v = self.rm16(m, rm, seg, off).wrapping_add(1); self.write_rm16(m, rm, seg, off, v);
                self.set_flag(ZF, v == 0); self.set_flag(SF, v & 0x8000 != 0); self.set_flag(PF, self.parity(v as u8)); }
            1 => { let v = self.rm16(m, rm, seg, off).wrapping_sub(1); self.write_rm16(m, rm, seg, off, v);
                self.set_flag(ZF, v == 0); self.set_flag(SF, v & 0x8000 != 0); self.set_flag(PF, self.parity(v as u8)); }
            2 => { self.push16(self.ip); self.ip = self.rm16(m, rm, seg, off); }
            3 => { let off2 = self.rm16(m, rm, seg, off); let seg2 = self.rm16(m, rm, seg, off + 2);
                self.push16(self.cs); self.push16(self.ip); self.cs = seg2; self.ip = off2; }
            4 => { self.ip = self.rm16(m, rm, seg, off); }
            5 => { let off2 = self.rm16(m, rm, seg, off); let seg2 = self.rm16(m, rm, seg, off + 2);
                self.cs = seg2; self.ip = off2; }
            6 => { let v = self.rm16(m, rm, seg, off); self.push16(v); }
            _ => self.unimplemented(0xFF),
        }
    }

    fn op_string(&mut self, op: u8) {
        let word = op & 1 == 1;
        // REP with CX = 0 executes zero times (8086 semantics)
        if self.rep.is_some() && self.cx == 0 { self.rep = None; return; }
        // execute once, then repeat while rep active
        loop {
            let inc = if self.flag(DF) { -1i16 } else { 1 };
            match op {
                0xA4 | 0xA5 => { // MOVS
                    let b = self.mem.read(self.phys(self.ds, self.si));
                    self.mem.write(self.phys(self.es, self.di), b);
                    if word {
                        let b2 = self.mem.read(self.phys(self.ds, self.si.wrapping_add(1)));
                        self.mem.write(self.phys(self.es, self.di.wrapping_add(1)), b2);
                    }
                    self.si = self.si.wrapping_add_signed(inc);
                    self.di = self.di.wrapping_add_signed(if word { inc * 2 } else { inc });
                }
                0xA6 | 0xA7 => { // CMPS
                    let a = self.mem.read(self.phys(self.ds, self.si));
                    let b = self.mem.read(self.phys(self.es, self.di));
                    if word {
                        let a2 = self.mem.read(self.phys(self.ds, self.si.wrapping_add(1)));
                        let b2 = self.mem.read(self.phys(self.es, self.di.wrapping_add(1)));
                        let v = a as u16 | (a2 as u16) << 8;
                        let w = b as u16 | (b2 as u16) << 8;
                        self.flags_sub16(v, w, false);
                    } else {
                        self.flags_sub8(a, b, false);
                    }
                    self.si = self.si.wrapping_add_signed(inc);
                    self.di = self.di.wrapping_add_signed(if word { inc * 2 } else { inc });
                }
                0xAA | 0xAB => { // STOS
                    let v = if word { self.ax } else { self.al() as u16 };
                    self.mem.write(self.phys(self.es, self.di), v as u8);
                    if word { self.mem.write(self.phys(self.es, self.di.wrapping_add(1)), (v >> 8) as u8); }
                    self.di = self.di.wrapping_add_signed(if word { inc * 2 } else { inc });
                }
                0xAC | 0xAD => { // LODS
                    if word {
                        self.ax = self.mem.read16(self.phys(self.ds, self.si));
                        self.si = self.si.wrapping_add_signed(inc * 2);
                    } else {
                        self.set_al(self.mem.read(self.phys(self.ds, self.si)));
                        self.si = self.si.wrapping_add_signed(inc);
                    }
                }
                _ => { // SCAS
                    let a = if word { self.ax } else { self.al() as u16 };
                    let b = self.mem.read(self.phys(self.es, self.di));
                    if word {
                        let b2 = self.mem.read(self.phys(self.es, self.di.wrapping_add(1)));
                        self.flags_sub16(a, b as u16 | (b2 as u16) << 8, false);
                    } else {
                        self.flags_sub8(a as u8, b, false);
                    }
                    self.di = self.di.wrapping_add_signed(if word { inc * 2 } else { inc });
                }
            }
            if let Some(repe) = self.rep {
                if self.cx == 0 { self.rep = None; break; }
                self.cx -= 1;
                if self.cx == 0 { self.rep = None; break; }
                if (0xA6..=0xAF).contains(&op) {
                    let zf = self.flag(ZF);
                    if (repe && !zf) || (!repe && zf) { self.rep = None; break; }
                }
            } else {
                break;
            }
        }
    }

    fn op_io_string(&mut self, op: u8) {
        let word = op & 1 == 1;
        if self.rep.is_some() && self.cx == 0 { self.rep = None; return; }
        loop {
            let inc = if self.flag(DF) { -1i16 } else { 1 };
            match op {
                0x6C | 0x6D => { // INS: ES:[DI] = port(DX); no port model -> 0
                    let v = 0u16;
                    self.mem.write(self.phys(self.es, self.di), v as u8);
                    if word {
                        self.mem.write(self.phys(self.es, self.di.wrapping_add(1)), (v >> 8) as u8);
                    }
                    self.di = self.di.wrapping_add_signed(if word { inc * 2 } else { inc });
                }
                _ => { // OUTS: port(DX) = DS:[SI]; no port model -> no-op
                    self.si = self.si.wrapping_add_signed(if word { inc * 2 } else { inc });
                }
            }
            if self.rep.is_some() {
                if self.cx == 0 { self.rep = None; break; }
                self.cx -= 1;
                if self.cx == 0 { self.rep = None; break; }
            } else {
                break;
            }
        }
    }
}

impl Cpu for Cpu8086 {
    fn reset(&mut self) {
        self.ax = 0; self.bx = 0; self.cx = 0; self.dx = 0;
        self.si = 0; self.di = 0; self.bp = 0; self.sp = 0xFFFF;
        self.cs = 0; self.ds = 0; self.es = 0; self.ss = 0;
        self.ip = 0;
        self.flags = 0x0002;
        self.halted = false;
        self.fault = None;
        self.rep = None;
        self.seg_ov = None;
        self.keybuf.clear();
        self.input_pending = false;
        self.pending_nmi = false;
        self.pending_intr = false;
        self.intr_vector = 0;
    }

    fn step(&mut self) -> bool {
        if self.halted { return false; }
        if self.input_pending { return true; } // blocked on INT 21h input
        let trap = self.flag(TF);
        self.exec();
        if !self.halted {
            self.service_interrupts();
            if trap && self.flag(TF) {
                self.hardware_intr(1); // single-step trap: INT 1
            }
        }
        !self.halted
    }

    fn pc(&self) -> u32 {
        (self.cs as u32) << 4 | self.ip as u32
    }

    fn set_pc(&mut self, addr: u32) {
        self.cs = (addr >> 4) as u16;
        self.ip = (addr & 0xF) as u16;
    }

    fn regs(&self) -> Vec<Reg> {
        vec![
            Reg::new("AX", self.ax as u32),
            Reg::new("BX", self.bx as u32),
            Reg::new("CX", self.cx as u32),
            Reg::new("DX", self.dx as u32),
            Reg::new("SI", self.si as u32),
            Reg::new("DI", self.di as u32),
            Reg::new("BP", self.bp as u32),
            Reg::new("SP", self.sp as u32),
            Reg::new("CS", self.cs as u32),
            Reg::new("DS", self.ds as u32),
            Reg::new("ES", self.es as u32),
            Reg::new("SS", self.ss as u32),
            Reg::new("IP", self.ip as u32),
        ]
    }

    fn flags(&self) -> FlagSet {
        FlagSet {
            carry: self.flag(CF),
            zero: self.flag(ZF),
            sign: self.flag(SF),
            parity: self.flag(PF),
            aux: self.flag(AF),
            overflow: self.flag(OF),
            direction: self.flag(DF),
            interrupt: self.flag(IF),
            trap: self.flag(TF),
        }
    }

    fn mem_read(&self, addr: u32, len: usize) -> Vec<u8> {
        (0..len).map(|i| self.mem.read(addr as usize + i)).collect()
    }

    fn mem_write(&mut self, addr: u32, data: &[u8]) {
        for (i, b) in data.iter().enumerate() {
            self.mem.write(addr as usize + i, *b);
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(34 + MEM_SIZE + self.keybuf.len() + 256);
        v.push(3); // version
        for r in [self.ax, self.bx, self.cx, self.dx, self.si, self.di, self.bp, self.sp,
                  self.cs, self.ds, self.es, self.ss, self.ip, self.flags] {
            v.extend_from_slice(&r.to_le_bytes());
        }
        v.push(self.halted as u8);
        v.push(self.input_pending as u8);
        v.extend_from_slice(&self.mem.data);
        v.extend_from_slice(&(self.keybuf.len() as u16).to_le_bytes());
        v.extend_from_slice(&self.keybuf.iter().copied().collect::<Vec<_>>());
        v.extend_from_slice(&self.ports);
        v.push(self.pending_nmi as u8);
        v.push(self.pending_intr as u8);
        v.push(self.intr_vector);
        v
    }

    fn restore(&mut self, data: &[u8]) {
        if data.is_empty() { return; }
        let ver = data[0];
        if data.len() < 30 { return; }
        let mut it = data.iter().copied().skip(1);
        let mut rd = || -> u16 {
            let lo = it.next().unwrap_or(0) as u16;
            let hi = it.next().unwrap_or(0) as u16;
            lo | hi << 8
        };
        self.ax = rd(); self.bx = rd(); self.cx = rd(); self.dx = rd();
        self.si = rd(); self.di = rd(); self.bp = rd(); self.sp = rd();
        self.cs = rd(); self.ds = rd(); self.es = rd(); self.ss = rd();
        self.ip = rd(); self.flags = rd();
        self.halted = data.get(29).is_some_and(|b| *b != 0);
        self.input_pending = data.get(30).is_some_and(|b| *b != 0);
        let body = &data[31..];
        let n = body.len().min(MEM_SIZE);
        self.mem.data[..n].copy_from_slice(&body[..n]);
        self.keybuf.clear();
        self.ports = [0; 256];
        if body.len() > MEM_SIZE + 2 {
            let klen = (body[MEM_SIZE] as usize) | ((body[MEM_SIZE + 1] as usize) << 8);
            for &b in body[MEM_SIZE + 2..].iter().take(klen) {
                self.keybuf.push_back(b);
            }
            if ver >= 2 {
                let start = MEM_SIZE + 2 + klen;
                let n = body.len().saturating_sub(start).min(256);
                self.ports[..n].copy_from_slice(&body[start..start + n]);
            }
            if ver >= 3 {
                let start = MEM_SIZE + 2 + klen + 256;
                self.pending_nmi = body.get(start).is_some_and(|b| *b != 0);
                self.pending_intr = body.get(start + 1).is_some_and(|b| *b != 0);
                self.intr_vector = body.get(start + 2).copied().unwrap_or(0);
            }
        }
        self.rep = None;
        self.seg_ov = None;
    }

    fn is_halted(&self) -> bool { self.halted }

    fn waiting_input(&self) -> bool { self.input_pending }
}

impl RunResult {
    pub fn with_error(e: String) -> Self {
        RunResult { steps: 0, halted: true, error: Some(e) }
    }
}
