//! Intel 8051 (MCS-51) CPU core — 8-bit, SFRs, bit-addressable RAM, timers.
//!
//! Internal RAM (0x00–0x7F) plus SFR space (0x80–0xFF); 64 KiB code space;
//! 64 KiB XDATA. Timers 0/1 tick per instruction while TRx=1 (no real-time
//! calibration). Writing SBUF emits a character to the Output buffer.

use crate::cpu::{Cpu, FlagSet, Mem, Output, Reg};

const CODE_SIZE: usize = 64 * 1024;
const XDATA_SIZE: usize = 256 * 1024; // banked external RAM (0x00000..0x3FFFF)

// SFR indices (offset within the 128-byte SFR array)
const S_SP: usize = 0x01;
const S_DPL: usize = 0x02;
const S_DPH: usize = 0x03;
const S_PCON: usize = 0x07; // 0x87: IDL(b0), PD(b1), SMOD(b7)
const S_TCON: usize = 0x08;
const S_TMOD: usize = 0x09;
const S_TL0: usize = 0x0A;
const S_TL1: usize = 0x0B;
const S_TH0: usize = 0x0C;
const S_TH1: usize = 0x0D;
const S_SCON: usize = 0x18;
const S_SBUF: usize = 0x19;
const S_XPAGE: usize = 0x78; // 0xF8: external RAM bank (extension for >64 KiB XDATA)
const S_IP: usize = 0x38;
const S_IE: usize = 0x28;
const S_PSW: usize = 0x50;
const S_ACC: usize = 0x60;
const S_B: usize = 0x70;

pub struct Cpu8051 {
    pub iram: [u8; 128],
    pub sfr: [u8; 128],
    pub xdata: Mem,
    /// External pin state for P0-P3; reads of a port return `latch | pin`
    /// (quasi-bidirectional model). Set via `Emulator::port_write`.
    pub port_pins: [u8; 4],
    pub code: Mem,
    pub pc: u16,
    pub out: Output,
    pub halted: bool,
    pub fault: Option<String>,
    in_svc_low: bool,
    in_svc_high: bool,
    /// External INT0/INT1 line held low (level-triggered mode, ITx=0): the
    /// interrupt re-asserts after service until the line is released.
    ext_int0_held: bool,
    ext_int1_held: bool,
    /// External RAM bank for MOVX @DPTR/@Ri (extension beyond 64 KiB XDATA).
    xdata_bank: u8,
    /// Serial transmit in progress: steps remaining until TI is set and the
    /// character is emitted to Output (baud-rate modelled from Timer 1 / SMOD).
    tx_countdown: u32,
    tx_char: u8,
}

impl Default for Cpu8051 {
    fn default() -> Self { Self::new() }
}

impl Cpu8051 {
    pub fn new() -> Self {
        let mut c = Cpu8051 {
            iram: [0; 128],
            sfr: [0; 128],
            xdata: Mem::new(XDATA_SIZE),
            port_pins: [0; 4],
            code: Mem::new(CODE_SIZE),
            pc: 0,
            out: Output::default(),
            halted: false,
            fault: None,
            in_svc_low: false,
            in_svc_high: false,
            ext_int0_held: false,
            ext_int1_held: false,
            xdata_bank: 0,
            tx_countdown: 0,
            tx_char: 0,
        };
        c.reset();
        c
    }

    pub fn last_error(&self) -> Option<String> { self.fault.clone() }

    fn reset_core(&mut self) {
        self.iram = [0; 128];
        self.sfr = [0; 128];
        self.sfr[S_SP] = 0x07;
        self.pc = 0;
        self.halted = false;
        self.fault = None;
        self.in_svc_low = false;
        self.in_svc_high = false;
        self.ext_int0_held = false;
        self.ext_int1_held = false;
        self.xdata_bank = 0;
        self.tx_countdown = 0;
        self.tx_char = 0;
    }

    // ----- helpers -----
    #[inline] fn acc(&self) -> u8 { self.sfr[S_ACC] }
    #[inline] fn set_acc(&mut self, v: u8) {
        self.sfr[S_ACC] = v;
        let p = if (v.count_ones() & 1) == 1 { 1u8 } else { 0 };
        self.sfr[S_PSW] = (self.sfr[S_PSW] & 0xFE) | p;
    }
    #[inline] fn b_reg(&self) -> u8 { self.sfr[S_B] }
    #[inline] fn set_b(&mut self, v: u8) { self.sfr[S_B] = v; }
    #[inline] fn psw(&self) -> u8 { self.sfr[S_PSW] }
    #[inline] fn set_psw(&mut self, v: u8) { self.sfr[S_PSW] = v & 0xFE | (self.sfr[S_PSW] & 1); }
    #[inline] fn cy(&self) -> bool { self.sfr[S_PSW] & 0x80 != 0 }
    #[inline] fn set_cy(&mut self, v: bool) {
        if v { self.sfr[S_PSW] |= 0x80; } else { self.sfr[S_PSW] &= 0x7F; }
    }
    #[inline] fn ac(&self) -> bool { self.sfr[S_PSW] & 0x40 != 0 }
    #[inline] fn set_ac(&mut self, v: bool) {
        if v { self.sfr[S_PSW] |= 0x40; } else { self.sfr[S_PSW] &= 0xBF; }
    }
    #[inline] fn ov(&self) -> bool { self.sfr[S_PSW] & 0x04 != 0 }
    #[inline] fn set_ov(&mut self, v: bool) {
        if v { self.sfr[S_PSW] |= 0x04; } else { self.sfr[S_PSW] &= 0xFB; }
    }
    #[inline] fn dptr(&self) -> u16 { ((self.sfr[S_DPH] as u16) << 8) | self.sfr[S_DPL] as u16 }
    #[inline] fn set_dptr(&mut self, v: u16) { self.sfr[S_DPH] = (v >> 8) as u8; self.sfr[S_DPL] = v as u8; }

    #[inline] fn bank(&self) -> usize { ((self.psw() >> 3) & 3) as usize }
    #[inline] fn rn(&self, n: u8) -> u8 { self.iram[self.bank() * 8 + n as usize] }
    #[inline] fn set_rn(&mut self, n: u8, v: u8) { self.iram[self.bank() * 8 + n as usize] = v; }
    #[inline] fn ri(&self, i: u8) -> u8 { self.iram[(self.bank() * 8 + i as usize) & 0xFF] }

    fn read_direct(&self, addr: u8) -> u8 {
        if addr < 0x80 {
            self.iram[addr as usize]
        } else {
            let idx = addr as usize - 0x80;
            match addr {
                0x80 | 0x90 | 0xA0 | 0xB0 => self.sfr[idx] | self.port_pins[((addr - 0x80) / 0x10) as usize],
                _ => self.sfr[idx],
            }
        }
    }
    fn write_direct(&mut self, addr: u8, v: u8) {
        if addr < 0x80 {
            self.iram[addr as usize] = v;
        } else {
            let idx = addr as usize - 0x80;
            match idx {
                S_SBUF => { self.start_tx(v); } // schedule transmit (TI fires after baud delay)
                S_PCON => { self.sfr[idx] = v; } // IDL/PD/SMOD latched; effects read in step()
                S_XPAGE => { self.xdata_bank = v; self.sfr[idx] = v; }
                S_ACC => self.set_acc(v),
                S_PSW => self.set_psw(v),
                _ => self.sfr[idx] = v,
            }
        }
    }

    // bit address -> (byte addr, bit index)
    /// Inject external pin state for P0-P3 (quasi-bidirectional: a port read
    /// returns `latch | pin`).
    pub fn port_write(&mut self, port: u8, v: u8) {
        if port < 4 { self.port_pins[port as usize] = v; }
    }
    pub fn port_read(&self, port: u8) -> u8 {
        if port < 4 { self.read_direct(0x80 + port * 0x10) } else { 0 }
    }
    /// Inject a received serial byte: writes SBUF and sets RI (receive
    /// interrupt flag); the serial ISR must clear RI (as on the chip).
    pub fn serial_rx(&mut self, ch: u8) {
        self.sfr[S_SBUF] = ch;
        self.sfr[S_SCON] |= 0x01;
    }
    /// Begin a serial transmit. If Timer 1 is running as a baud-rate generator
    /// (mode 1/2), TI and the emitted character are deferred by the frame time
    /// derived from the timer period and SMOD; otherwise TI is set immediately
    /// (legacy behaviour, e.g. inside a serial ISR before TR1 is started).
    fn start_tx(&mut self, v: u8) {
        self.sfr[S_SBUF] = v;
        let tcon = self.sfr[S_TCON];
        let tmod = self.sfr[S_TMOD];
        let pcon = self.sfr[S_PCON];
        let tx_delay = if tcon & 0x40 != 0 { // TR1 running
            let mode = (tmod >> 4) & 3;
            let period: u32 = match mode {
                2 => 256u32.wrapping_sub(self.sfr[S_TH1] as u32),
                1 => 65536u32.wrapping_sub(((self.sfr[S_TH1] as u32) << 8) | self.sfr[S_TL1] as u32),
                0 => 8192u32.wrapping_sub(((self.sfr[S_TH1] as u32 & 0x1F) << 8) | self.sfr[S_TL1] as u32),
                _ => 0,
            };
            if period == 0 { 0 } else {
                let smod = if pcon & 0x80 != 0 { 16 } else { 32 };
                (10u32 * smod * period).min(200_000)
            }
        } else { 0 };
        if tx_delay == 0 {
            self.out.put_char(v as char);
            self.sfr[S_SCON] |= 0x02; // TI
        } else {
            self.tx_char = v;
            self.tx_countdown = tx_delay;
        }
    }

    fn bit_location(bit: u8) -> (u8, u8) {
        if bit < 0x80 {
            (0x20 + ((bit >> 3).saturating_sub(4)), bit & 7)
        } else {
            (bit & 0xF8, bit & 7)
        }
    }
    fn read_bit(&self, bit: u8) -> bool {
        let (b, i) = Self::bit_location(bit);
        self.read_direct(b) & (1 << i) != 0
    }
    fn write_bit(&mut self, bit: u8, v: bool) {
        let (b, i) = Self::bit_location(bit);
        let mut byte = self.read_direct(b);
        if v { byte |= 1 << i; } else { byte &= !(1 << i); }
        self.write_direct(b, byte);
    }

    #[inline] fn fetch8(&mut self) -> u8 {
        let b = self.code.read(self.pc as usize);
        self.pc = self.pc.wrapping_add(1);
        b
    }
    #[inline] fn fetch16(&mut self) -> u16 {
        let hi = self.fetch8() as u16;
        let lo = self.fetch8() as u16;
        hi << 8 | lo
    }

    fn push(&mut self, v: u8) {
        let sp = self.sfr[S_SP];
        self.sfr[S_SP] = sp.wrapping_add(1);
        self.write_direct(sp.wrapping_add(1), v);
    }
    fn pop(&mut self) -> u8 {
        let sp = self.sfr[S_SP];
        self.sfr[S_SP] = sp.wrapping_sub(1);
        self.read_direct(sp)
    }

    // ----- timers -----
    /// Effective external-RAM address: XDATA bank in the high 8 bits, so
    /// MOVX @DPTR/@Ri can reach up to 256 KiB when a bank is selected.
    #[inline] fn xdata_addr(&self, off: u32) -> usize {
        (((self.xdata_bank as u32) << 16) | off) as usize & (XDATA_SIZE - 1)
    }
    fn tick_timers(&mut self) {
        let tcon = self.sfr[S_TCON];
        let tmod = self.sfr[S_TMOD];
        if tcon & 0x10 != 0 { // TR0
            let mode = tmod & 3;
            let (v, reload) = self.timer_regs(mode, S_TH0, S_TL0);
            let nv = v.wrapping_add(1);
            let wrap = v == u16::MAX;
            let overflow = match mode {
                0 => v & 0x1FFF == 0x1FFF,
                1 => wrap,
                _ => nv as u8 == 0,
            };
            if overflow {
                self.sfr[S_TCON] |= 0x20; // TF0
            }
            if reload {
                // mode 2 (8-bit auto-reload): reload TH into TL only on overflow,
                // otherwise just count in TL
                if overflow {
                    self.sfr[S_TL0] = self.sfr[S_TH0];
                } else {
                    self.sfr[S_TL0] = nv as u8;
                }
            } else {
                self.sfr[S_TH0] = (nv >> 8) as u8;
                self.sfr[S_TL0] = nv as u8;
            }
        }
        if tcon & 0x40 != 0 { // TR1
            let mode = (tmod >> 4) & 3;
            let (v, reload) = self.timer_regs(mode, S_TH1, S_TL1);
            let nv = v.wrapping_add(1);
            let wrap = v == u16::MAX;
            let overflow = match mode {
                0 => v & 0x1FFF == 0x1FFF,
                1 => wrap,
                _ => nv as u8 == 0,
            };
            if overflow {
                self.sfr[S_TCON] |= 0x80; // TF1
            }
            if reload {
                // mode 2 (8-bit auto-reload): reload TH into TL only on overflow,
                // otherwise just count in TL
                if overflow {
                    self.sfr[S_TL1] = self.sfr[S_TH1];
                } else {
                    self.sfr[S_TL1] = nv as u8;
                }
            } else {
                self.sfr[S_TH1] = (nv >> 8) as u8;
                self.sfr[S_TL1] = nv as u8;
            }
        }
        // serial transmit baud-rate countdown: when it elapses, emit the char
        // and set TI (transmit-complete) — the serial ISR must clear TI.
        if self.tx_countdown > 0 {
            self.tx_countdown -= 1;
            if self.tx_countdown == 0 {
                self.out.put_char(self.tx_char as char);
                self.sfr[S_SCON] |= 0x02;
            }
        }
    }
    fn timer_regs(&self, mode: u8, th: usize, tl: usize) -> (u16, bool) {
        let v = ((self.sfr[th] as u16) << 8) | self.sfr[tl] as u16;
        match mode {
            0 => (v & 0x1FFF, false),
            2 => (self.sfr[tl] as u16, true),
            _ => (v, false),
        }
    }

    // ----- ALU -----
    fn add_a(&mut self, v: u8) {
        let a = self.acc();
        let r = a.wrapping_add(v);
        self.set_cy((a as u16) + (v as u16) > 0xFF);
        self.set_ac((a & 0xF) + (v & 0xF) > 0xF);
        self.set_ov(((a ^ r) & (v ^ r) & 0x80) != 0);
        self.set_acc(r);
    }
    fn addc_a(&mut self, v: u8) {
        let a = self.acc();
        let ci = self.cy() as u16;
        let r = a as u16 + v as u16 + ci;
        self.set_cy(r > 0xFF);
        self.set_ac((a & 0xF) + (v & 0xF) + ci as u8 > 0xF);
        let rr = r as u8;
        self.set_ov(((a ^ rr) & (v ^ rr) & 0x80) != 0);
        self.set_acc(rr);
    }
    fn subb_a(&mut self, v: u8) {
        let a = self.acc();
        let bi = self.cy() as u16;
        let r = a as u16;
        let rr = (r.wrapping_sub(v as u16).wrapping_sub(bi)) as u8;
        self.set_cy(r < v as u16 + bi);
        self.set_ac((a & 0xF) < (v & 0xF) + bi as u8);
        self.set_ov(((a ^ v) & (a ^ rr) & 0x80) != 0);
        self.set_acc(rr);
    }

    fn rel_addr(&mut self) -> u16 {
        let rel = self.fetch8() as i8;
        self.pc.wrapping_add_signed(rel as i16)
    }

    fn unimplemented(&mut self, op: u8) {
        self.fault = Some(format!("8051: unimplemented opcode {op:02X}h at PC {:04X}h", self.pc.wrapping_sub(1)));
        self.halted = true;
    }

    pub fn exec(&mut self) {
        let op = self.fetch8();
        match op {
            // ----- MOV A,<src> -----
            0xE8..=0xEF => { let v = self.rn(op & 7); self.set_acc(v); }
            0xE5 => { let d = self.fetch8(); let v = self.read_direct(d); self.set_acc(v); }
            0xE6 | 0xE7 => { let v = self.read_direct(self.ri(op & 1)); self.set_acc(v); }
            0x74 => { let v = self.fetch8(); self.set_acc(v); }
            // ----- MOV Rn,<src> -----
            0xF8..=0xFF => { let v = self.acc(); self.set_rn(op & 7, v); }
            0xA8..=0xAF => { let d = self.fetch8(); let v = self.read_direct(d); self.set_rn(op & 7, v); }
            0x78..=0x7F => { let v = self.fetch8(); self.set_rn(op & 7, v); }
            // ----- MOV direct,<src> -----
            0xF5 => { let d = self.fetch8(); let v = self.acc(); self.write_direct(d, v); }
            0x88..=0x8F => { let d = self.fetch8(); let v = self.rn(op & 7); self.write_direct(d, v); }
            0x85 => { let s = self.fetch8(); let d = self.fetch8(); let v = self.read_direct(s); self.write_direct(d, v); }
            0x86 | 0x87 => { let d = self.fetch8(); let v = self.read_direct(self.ri(op & 1)); self.write_direct(d, v); }
            0x75 => { let d = self.fetch8(); let v = self.fetch8(); self.write_direct(d, v); }
            0x90 => { let v = self.fetch16(); self.set_dptr(v); } // MOV DPTR,#imm16
            // ----- MOV @Ri,<src> -----
            0xF6 | 0xF7 => { let a = self.ri(op & 1); let v = self.acc(); self.write_direct(a, v); }
            0xA6 | 0xA7 => { let a = self.ri(op & 1); let d = self.fetch8(); let v = self.read_direct(d); self.write_direct(a, v); }
            0x76 | 0x77 => { let a = self.ri(op & 1); let v = self.fetch8(); self.write_direct(a, v); }
            // ----- MOVC / MOVX -----
            0x93 => {
                let a = self.acc() as u16;
                let addr = self.dptr().wrapping_add(a);
                let v = self.code.read(addr as usize);
                self.set_acc(v);
            }
            0x83 => {
                self.pc = self.pc.wrapping_add(1);
                let a = self.acc() as u16;
                let addr = self.pc.wrapping_add(a);
                let v = self.code.read(addr as usize);
                self.set_acc(v);
            }
            0xE2 | 0xE3 => { let a = self.xdata_addr(self.ri(op & 1) as u32); let v = self.xdata.read(a); self.set_acc(v); }
            0xE0 => { let a = self.xdata_addr(self.dptr() as u32); let v = self.xdata.read(a); self.set_acc(v); }
            0xF2 | 0xF3 => { let a = self.xdata_addr(self.ri(op & 1) as u32); let v = self.acc(); self.xdata.write(a, v); }
            0xF0 => { let a = self.xdata_addr(self.dptr() as u32); let v = self.acc(); self.xdata.write(a, v); }
            // ----- stack -----
            0xC0 => { let d = self.fetch8(); let v = self.read_direct(d); self.push(v); }
            0xD0 => { let d = self.fetch8(); let v = self.pop(); self.write_direct(d, v); }
            // ----- exchange -----
            0xC5 => { let d = self.fetch8(); let v = self.read_direct(d); let a = self.acc(); self.write_direct(d, a); self.set_acc(v); }
            0xC6 | 0xC7 => { let a = self.ri(op & 1); let v = self.read_direct(a); let acc = self.acc(); self.write_direct(a, acc); self.set_acc(v); }
            0xC8..=0xCF => { let v = self.rn(op & 7); let a = self.acc(); self.set_rn(op & 7, a); self.set_acc(v); }
            0xD6 | 0xD7 => { // XCHD
                let a = self.ri(op & 1);
                let v = self.read_direct(a);
                let acc = self.acc();
                self.write_direct(a, (v & 0xF0) | (acc & 0x0F));
                self.set_acc((acc & 0xF0) | (v & 0x0F));
            }
            0xC4 => { let v = self.acc(); self.set_acc(v.rotate_left(4)); }
            // ----- arithmetic -----
            0x25 => { let d = self.fetch8(); let v = self.read_direct(d); self.add_a(v); }
            0x26 | 0x27 => { let v = self.read_direct(self.ri(op & 1)); self.add_a(v); }
            0x28..=0x2F => { let v = self.rn(op & 7); self.add_a(v); }
            0x24 => { let v = self.fetch8(); self.add_a(v); }
            0x35 => { let d = self.fetch8(); let v = self.read_direct(d); self.addc_a(v); }
            0x36 | 0x37 => { let v = self.read_direct(self.ri(op & 1)); self.addc_a(v); }
            0x38..=0x3F => { let v = self.rn(op & 7); self.addc_a(v); }
            0x34 => { let v = self.fetch8(); self.addc_a(v); }
            0x95 => { let d = self.fetch8(); let v = self.read_direct(d); self.subb_a(v); }
            0x96 | 0x97 => { let v = self.read_direct(self.ri(op & 1)); self.subb_a(v); }
            0x98..=0x9F => { let v = self.rn(op & 7); self.subb_a(v); }
            0x94 => { let v = self.fetch8(); self.subb_a(v); }
            0x04 => { let v = self.acc().wrapping_add(1); self.set_acc(v); }
            0x08..=0x0F => { let v = self.rn(op & 7).wrapping_add(1); self.set_rn(op & 7, v); }
            0x05 => { let d = self.fetch8(); let v = self.read_direct(d).wrapping_add(1); self.write_direct(d, v); }
            0x06 | 0x07 => { let a = self.ri(op & 1); let v = self.read_direct(a).wrapping_add(1); self.write_direct(a, v); }
            0xA3 => { let v = self.dptr().wrapping_add(1); self.set_dptr(v); }
            0x14 => { let v = self.acc().wrapping_sub(1); self.set_acc(v); }
            0x18..=0x1F => { let v = self.rn(op & 7).wrapping_sub(1); self.set_rn(op & 7, v); }
            0x15 => { let d = self.fetch8(); let v = self.read_direct(d).wrapping_sub(1); self.write_direct(d, v); }
            0x16 | 0x17 => { let a = self.ri(op & 1); let v = self.read_direct(a).wrapping_sub(1); self.write_direct(a, v); }
            0xA4 => { // MUL AB
                let a = self.acc() as u16;
                let b = self.b_reg() as u16;
                let r = a * b;
                self.set_acc(r as u8);
                self.set_b((r >> 8) as u8);
                self.set_ov(r > 0xFF);
                self.set_cy(false);
            }
            0x84 => { // DIV AB
                let a = self.acc();
                let b = self.b_reg();
                match a.checked_div(b) {
                    None => {
                        self.set_ov(true);
                        self.set_cy(false);
                    }
                    Some(q) => {
                        self.set_acc(q);
                        self.set_b(a % b);
                        self.set_ov(false);
                        self.set_cy(false);
                    }
                }
            }
            0xD4 => { // DA A
                let a = self.acc();
                let mut r = a;
                let mut cy = self.cy();
                let ac = self.ac();
                if (a & 0x0F) > 9 || ac {
                    r = r.wrapping_add(0x06);
                }
                let hi = r >> 4;
                if hi > 9 || (hi == 9 && (r & 0x0F) > 9) || cy {
                    r = r.wrapping_add(0x60);
                    cy = true;
                }
                self.set_cy(cy);
                self.set_acc(r);
            }
            // ----- logical -----
            0x58..=0x5F => { let v = self.rn(op & 7); let a = self.acc() & v; self.set_acc(a); }
            0x55 => { let d = self.fetch8(); let v = self.read_direct(d); let a = self.acc() & v; self.set_acc(a); }
            0x56 | 0x57 => { let v = self.read_direct(self.ri(op & 1)); let a = self.acc() & v; self.set_acc(a); }
            0x54 => { let v = self.fetch8(); let a = self.acc() & v; self.set_acc(a); }
            0x52 => { let d = self.fetch8(); let v = self.read_direct(d) & self.acc(); self.write_direct(d, v); }
            0x53 => { let d = self.fetch8(); let m = self.fetch8(); let v = self.read_direct(d) & m; self.write_direct(d, v); }
            0x48..=0x4F => { let v = self.rn(op & 7); let a = self.acc() | v; self.set_acc(a); }
            0x45 => { let d = self.fetch8(); let v = self.read_direct(d); let a = self.acc() | v; self.set_acc(a); }
            0x46 | 0x47 => { let v = self.read_direct(self.ri(op & 1)); let a = self.acc() | v; self.set_acc(a); }
            0x44 => { let v = self.fetch8(); let a = self.acc() | v; self.set_acc(a); }
            0x42 => { let d = self.fetch8(); let v = self.read_direct(d) | self.acc(); self.write_direct(d, v); }
            0x43 => { let d = self.fetch8(); let m = self.fetch8(); let v = self.read_direct(d) | m; self.write_direct(d, v); }
            0x68..=0x6F => { let v = self.rn(op & 7); let a = self.acc() ^ v; self.set_acc(a); }
            0x65 => { let d = self.fetch8(); let v = self.read_direct(d); let a = self.acc() ^ v; self.set_acc(a); }
            0x66 | 0x67 => { let v = self.read_direct(self.ri(op & 1)); let a = self.acc() ^ v; self.set_acc(a); }
            0x64 => { let v = self.fetch8(); let a = self.acc() ^ v; self.set_acc(a); }
            0x62 => { let d = self.fetch8(); let v = self.read_direct(d) ^ self.acc(); self.write_direct(d, v); }
            0x63 => { let d = self.fetch8(); let m = self.fetch8(); let v = self.read_direct(d) ^ m; self.write_direct(d, v); }
            0xE4 => self.set_acc(0),
            0xF4 => { let v = !self.acc(); self.set_acc(v); }
            0x23 => { let v = self.acc().rotate_left(1); self.set_acc(v); }
            0x03 => { let v = self.acc().rotate_right(1); self.set_acc(v); }
            0x33 => { // RLC
                let v = self.acc();
                let nc = v & 0x80 != 0;
                let cy = self.cy() as u8;
                self.set_acc((v << 1) | cy);
                self.set_cy(nc);
            }
            0x13 => { // RRC
                let v = self.acc();
                let nc = v & 1 != 0;
                let cy = self.cy() as u8;
                self.set_acc((v >> 1) | (cy << 7));
                self.set_cy(nc);
            }
            // ----- bit ops -----
            0xD3 => self.set_cy(true),
            0xD2 => { let b = self.fetch8(); self.write_bit(b, true); }
            0xC3 => self.set_cy(false),
            0xC2 => { let b = self.fetch8(); self.write_bit(b, false); }
            0xB3 => { let v = !self.cy(); self.set_cy(v); }
            0xB2 => { let b = self.fetch8(); let v = !self.read_bit(b); self.write_bit(b, v); }
            0x82 => { let b = self.fetch8(); let v = self.cy() && self.read_bit(b); self.set_cy(v); }
            0xB0 => { let b = self.fetch8(); let v = self.cy() && !self.read_bit(b); self.set_cy(v); }
            0x72 => { let b = self.fetch8(); let v = self.cy() || self.read_bit(b); self.set_cy(v); }
            0xA0 => { let b = self.fetch8(); let v = self.cy() || !self.read_bit(b); self.set_cy(v); }
            0xA2 => { let b = self.fetch8(); let v = self.read_bit(b); self.set_cy(v); }
            0x92 => { let b = self.fetch8(); let c = self.cy(); self.write_bit(b, c); }
            // ----- branches -----
            0x73 => { let a = self.acc() as u16; self.pc = a + self.dptr(); } // JMP @A+DPTR
            0x80 => { let t = self.rel_addr(); self.pc = t; }
            0x02 => { let a = self.fetch16(); self.pc = a; }
            0x01 | 0x21 | 0x41 | 0x61 | 0x81 | 0xA1 | 0xC1 | 0xE1 => { // AJMP
                let target = self.fetch8() as u16;
                let a11 = (((op as u16) & 0xE0) << 3) | target;
                self.pc = (self.pc & 0xF800) | a11;
            }
            0x60 => { let t = self.rel_addr(); if self.acc() == 0 { self.pc = t; } }
            0x70 => { let t = self.rel_addr(); if self.acc() != 0 { self.pc = t; } }
            0x40 => { let t = self.rel_addr(); if self.cy() { self.pc = t; } }
            0x50 => { let t = self.rel_addr(); if !self.cy() { self.pc = t; } }
            0x20 => { let b = self.fetch8(); let t = self.rel_addr(); if self.read_bit(b) { self.pc = t; } }
            0x30 => { let b = self.fetch8(); let t = self.rel_addr(); if !self.read_bit(b) { self.pc = t; } }
            0x10 => { let b = self.fetch8(); let t = self.rel_addr(); if self.read_bit(b) { self.write_bit(b, false); self.pc = t; } }
            0xB4 => { let v = self.fetch8(); let t = self.rel_addr(); let a = self.acc(); self.cjne(v, t, a); }
            0xB5 => { let d = self.fetch8(); let v = self.read_direct(d); let t = self.rel_addr(); let a = self.acc(); self.cjne(v, t, a); }
            0xB8..=0xBF => { let v = self.fetch8(); let t = self.rel_addr(); let r = self.rn(op & 7); self.cjne(v, t, r); }
            0xB6 | 0xB7 => { let v = self.fetch8(); let t = self.rel_addr(); let r = self.read_direct(self.ri(op & 1)); self.cjne(v, t, r); }
            0xD8..=0xDF => { // DJNZ Rn,rel
                let t = self.rel_addr();
                let nv = self.rn(op & 7).wrapping_sub(1);
                self.set_rn(op & 7, nv);
                if nv != 0 { self.pc = t; }
            }
            0xD5 => { // DJNZ direct,rel
                let d = self.fetch8();
                let t = self.rel_addr();
                let nv = self.read_direct(d).wrapping_sub(1);
                self.write_direct(d, nv);
                if nv != 0 { self.pc = t; }
            }
            0x11 | 0x31 | 0x51 | 0x71 | 0x91 | 0xB1 | 0xD1 | 0xF1 => { // ACALL
                let target = self.fetch8() as u16;
                let a11 = (((op as u16) & 0xE0) << 3) | target;
                let pch = self.pc >> 8;
                let pcl = self.pc as u8;
                self.push(pcl);
                self.push(pch as u8);
                self.pc = (self.pc & 0xF800) | a11;
            }
            0x12 => { let a = self.fetch16(); let pch = self.pc >> 8; let pcl = self.pc as u8; self.push(pcl); self.push(pch as u8); self.pc = a; }
            0x22 => { let hi = self.pop() as u16; let lo = self.pop() as u16; self.pc = hi << 8 | lo; }
            0x32 => { // RETI: return and clear the in-service priority latch
                let hi = self.pop() as u16; let lo = self.pop() as u16; self.pc = hi << 8 | lo;
                if self.in_svc_high { self.in_svc_high = false; } else { self.in_svc_low = false; }
            }
            0x00 => {}
            _ => self.unimplemented(op),
        }
    }

    fn cjne(&mut self, v: u8, target: u16, cmp_a: u8) {
        self.set_cy(cmp_a < v);
        if cmp_a != v { self.pc = target; }
    }

    /// Raise an external interrupt: "INT0" or "INT1" (sets the IE0/IE1 latch
    /// in TCON, edge or level triggered per IT0/IT1 — level is treated like
    /// edge, the latch is cleared on service; document this simplification).
    pub fn request_interrupt(&mut self, kind: &str) -> Result<(), String> {
        match kind.to_ascii_uppercase().as_str() {
            "INT0" => {
                self.sfr[S_TCON] |= 0x02; // IE0 latch (edge mode)
                // Held only if level-triggered (IT0=0): re-asserts after RETI.
                self.ext_int0_held = self.sfr[S_TCON] & 0x01 == 0;
                Ok(())
            }
            "INT1" => {
                self.sfr[S_TCON] |= 0x08; // IE1 latch (edge mode)
                self.ext_int1_held = self.sfr[S_TCON] & 0x04 == 0;
                Ok(())
            }
            _ => Err(format!("unknown 8051 interrupt '{kind}' (use INT0 or INT1)")),
        }
    }

    /// Read a byte from the SFR space (address 0x80-0xFF) or internal RAM
    /// (address 0x00-0x7F) — used by the debugger UI and tests.
    pub fn sfr_byte(&self, addr: u8) -> u8 {
        if addr < 0x80 { self.iram[addr as usize] } else { self.sfr[addr as usize - 0x80] }
    }

    /// Check pending interrupt sources in natural priority order
    /// (INT0 > TF0 > INT1 > TF1 > serial) and vector if enabled and
    /// not blocked by an equal-or-higher in-service priority latch.
    fn service_interrupts(&mut self) {
        let ie = self.sfr[S_IE];
        if ie & 0x80 == 0 { return; } // EA
        let tcon = self.sfr[S_TCON];
        let scon = self.sfr[S_SCON];
        let ip = self.sfr[S_IP];
        // INT0/INT1: edge mode uses the IE0/IE1 latch; level mode (ITx=0) uses
        // the external line held low (re-asserts after the ISR returns).
        let ie0 = if tcon & 0x01 != 0 { tcon & 0x02 != 0 } else { self.ext_int0_held };
        let ie1 = if tcon & 0x04 != 0 { tcon & 0x08 != 0 } else { self.ext_int1_held };
        let sources: [(bool, u16, u8, bool); 5] = [
            (ie0, 0x03, ie & 0x01, ip & 0x01 != 0), // INT0
            (tcon & 0x20 != 0, 0x0B, ie & 0x02, ip & 0x02 != 0), // TF0
            (ie1, 0x13, ie & 0x04, ip & 0x04 != 0), // INT1
            (tcon & 0x80 != 0, 0x1B, ie & 0x08, ip & 0x08 != 0), // TF1
            (scon & 0x03 != 0, 0x23, ie & 0x10, ip & 0x10 != 0), // RI|TI
        ];
        for (flag, vector, en, high) in sources {
            if !flag || en == 0 { continue; }
            if high {
                if self.in_svc_high { continue; }
            } else if self.in_svc_low || self.in_svc_high {
                continue;
            }
            match vector {
                // Clear the IE0/IE1 latch only in edge mode (level mode is
                // driven by the held external line, not the latch).
                0x03 => if tcon & 0x01 != 0 { self.sfr[S_TCON] &= !0x02; }
                0x0B => self.sfr[S_TCON] &= !0x20, // TF0
                0x13 => if tcon & 0x04 != 0 { self.sfr[S_TCON] &= !0x08; }
                0x1B => self.sfr[S_TCON] &= !0x80, // TF1
                _ => {} // serial RI/TI are cleared by the ISR in software
            }
            let pcl = self.pc as u8;
            let pch = (self.pc >> 8) as u8;
            self.push(pcl);
            self.push(pch);
            if high { self.in_svc_high = true; } else { self.in_svc_low = true; }
            self.sfr[S_PCON] &= !0x01; // hardware clears IDL on interrupt entry
            self.pc = vector;
            return;
        }
    }
}

impl Cpu for Cpu8051 {
    fn reset(&mut self) {
        self.reset_core();
        self.xdata = Mem::new(XDATA_SIZE);
        self.code = Mem::new(CODE_SIZE);
        self.out = Output::default();
    }

    fn step(&mut self) -> bool {
        if self.halted { return false; }
        let pcon = self.sfr[S_PCON];
        if pcon & 0x02 != 0 { // PD (power-down): oscillator stopped, frozen
            return true; // only reset wakes it; not "halted"
        }
        if pcon & 0x01 != 0 { // IDL (idle): CPU sleeps, peripherals keep running
            self.tick_timers();
            if !self.halted { self.service_interrupts(); } // an interrupt wakes it (clears IDL)
            return true; // no user instruction executed this step
        }
        self.tick_timers();
        self.exec();
        if !self.halted {
            self.service_interrupts();
        }
        !self.halted
    }

    fn pc(&self) -> u32 { self.pc as u32 }

    fn set_pc(&mut self, addr: u32) { self.pc = addr as u16; }

    fn regs(&self) -> Vec<Reg> {
        let bank = self.bank();
        vec![
            Reg::new("A", self.acc() as u32),
            Reg::new("B", self.b_reg() as u32),
            Reg::new("DPTR", self.dptr() as u32),
            Reg::new("SP", self.sfr[S_SP] as u32),
            Reg::new("PC", self.pc as u32),
            Reg::new("PSW", self.psw() as u32),
            Reg::new("R0", self.rn(0) as u32),
            Reg::new("R1", self.rn(1) as u32),
            Reg::new("R2", self.rn(2) as u32),
            Reg::new("R3", self.rn(3) as u32),
            Reg::new("R4", self.rn(4) as u32),
            Reg::new("R5", self.rn(5) as u32),
            Reg::new("R6", self.rn(6) as u32),
            Reg::new("R7", self.rn(7) as u32),
            Reg::new("BANK", bank as u32),
        ]
    }

    fn flags(&self) -> FlagSet {
        FlagSet {
            carry: self.cy(),
            zero: false,
            sign: false,
            parity: self.psw() & 1 != 0,
            aux: self.ac(),
            overflow: self.ov(),
            direction: false,
            interrupt: false,
            trap: false,
        }
    }

    fn mem_read(&self, addr: u32, len: usize) -> Vec<u8> {
        (0..len).map(|i| self.code.read(addr as usize + i)).collect()
    }

    fn mem_write(&mut self, addr: u32, data: &[u8]) {
        for (i, b) in data.iter().enumerate() {
            self.code.write(addr as usize + i, *b);
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(9 + 128 + 128 + XDATA_SIZE + CODE_SIZE + 4 + 2);
        v.push(5);
        v.push(self.halted as u8);
        v.extend_from_slice(&self.pc.to_le_bytes());
        v.push(self.in_svc_low as u8);
        v.push(self.in_svc_high as u8);
        v.push(self.xdata_bank);
        v.extend_from_slice(&self.tx_countdown.to_le_bytes());
        v.extend_from_slice(&self.iram);
        v.extend_from_slice(&self.sfr);
        v.extend_from_slice(&self.xdata.data);
        v.extend_from_slice(&self.code.data);
        v.extend_from_slice(&self.port_pins);
        v.push(self.ext_int0_held as u8);
        v.push(self.ext_int1_held as u8);
        v
    }

    fn restore(&mut self, data: &[u8]) {
        if data.len() < 3 { return; }
        self.halted = data[1] != 0;
        self.pc = ((data[2] as u16) << 8) | data[3] as u16;
        self.in_svc_low = data.get(4).is_some_and(|b| *b != 0);
        self.in_svc_high = data.get(5).is_some_and(|b| *b != 0);
        let mut start = if data[0] >= 2 { 6 } else { 4 };
        if data[0] >= 5 {
            self.xdata_bank = data.get(6).copied().unwrap_or(0);
            self.tx_countdown = u32::from_le_bytes([data[7], data[8], data[9], data[10]]);
            start = 11;
        } else {
            self.xdata_bank = 0;
            self.tx_countdown = 0;
        }
        let mut off = start;
        let mut take = |n: usize, dst: &mut [u8]| {
            let n = n.min(dst.len()).min(data.len().saturating_sub(off));
            dst[..n].copy_from_slice(&data[off..off + n]);
            off += n;
        };
        take(128, &mut self.iram);
        take(128, &mut self.sfr);
        take(XDATA_SIZE, &mut self.xdata.data);
        take(CODE_SIZE, &mut self.code.data);
        self.port_pins = [0; 4];
        if data[0] >= 3 {
            take(4, &mut self.port_pins);
        }
        self.ext_int0_held = false;
        self.ext_int1_held = false;
        if data[0] >= 4 {
            self.ext_int0_held = data[data.len() - 2] != 0;
            self.ext_int1_held = data[data.len() - 1] != 0;
        }
    }

    fn is_halted(&self) -> bool { self.halted }
}
