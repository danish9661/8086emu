//! Intel 8255A Programmable Peripheral Interface — Mode 0 model.
//!
//! Covers the common lab use-case: 3× 8-bit ports (A/B/C) + control register.
//! Only Mode 0 is emulated; Mode 1/2 control words are accepted but treated as
//! Mode 0 (the IDE never needs handshaking). BSR (Bit Set/Reset) mode is fully
//! emulated for Port C bit manipulation.
//!
//! Port mapping used in this emulator (both 8086 and 8085):
//!   0xE0  Port A
//!   0xE1  Port B
//!   0xE2  Port C
//!   0xE3  Control word
//! A second instance may be placed at 0xE4..0xE7 (same decode) if desired.
//! The WASM/JS side reads these ports through `port_read(0xE0..0xE3)`.

#[derive(Clone)]
pub struct Ppi8255 {
    /// Output latches (what OUT writes)
    pub pa: u8,
    pub pb: u8,
    pub pc: u8,
    /// External input pins injected via `set_input` / Emulator::port_write external
    pa_in: u8,
    pb_in: u8,
    pc_in: u8,
    ctrl: u8,
    // direction bits derived from control word (1 = input, 0 = output)
    dir_a: bool,
    dir_b: bool,
    dir_c_upper: bool,
    dir_c_lower: bool,
}

impl Default for Ppi8255 {
    fn default() -> Self {
        let mut s = Ppi8255 {
            pa: 0,
            pb: 0,
            pc: 0,
            pa_in: 0,
            pb_in: 0,
            pc_in: 0,
            ctrl: 0x9B, // power-up: all ports input (10011011b)
            dir_a: true,
            dir_b: true,
            dir_c_upper: true,
            dir_c_lower: true,
        };
        s.decode_ctrl(0x9B);
        s
    }
}

impl Ppi8255 {
    pub fn new() -> Self { Self::default() }

    fn decode_ctrl(&mut self, v: u8) {
        self.ctrl = v;
        if v & 0x80 != 0 {
            // Mode set (D7=1)
            // D6-D5 Group A mode, D4 PA dir, D3 PC upper dir, D2 Group B mode, D1 PB dir, D0 PC lower dir
            // We force Mode 0: ignore mode bits, just extract directions
            self.dir_a = v & 0x10 != 0;
            self.dir_c_upper = v & 0x08 != 0;
            self.dir_b = v & 0x02 != 0;
            self.dir_c_lower = v & 0x01 != 0;
        }
    }

    /// Write control register (0xE3). Handles both mode-set and BSR.
    pub fn write_ctrl(&mut self, v: u8) {
        if v & 0x80 != 0 {
            self.decode_ctrl(v);
        } else {
            // BSR: D7=0, D6-D4 X, D3-D1 bit select (0..7), D0 set/reset
            let bit = ((v >> 1) & 0x07) as usize;
            let set = v & 0x01 != 0;
            if set {
                self.pc |= 1 << bit;
            } else {
                self.pc &= !(1 << bit);
            }
        }
    }

    pub fn write_pa(&mut self, v: u8) {
        if !self.dir_a {
            self.pa = v;
        } else {
            // In input mode, OUT still updates the latch on real hardware? No-op for our model
            // Keep latch but it won't be visible on read
            self.pa = v;
        }
    }
    pub fn write_pb(&mut self, v: u8) {
        if !self.dir_b {
            self.pb = v;
        } else {
            self.pb = v;
        }
    }
    pub fn write_pc(&mut self, v: u8) {
        // PC is split into upper/lower nibbles with independent directions
        let mut out = self.pc;
        // lower nibble (PC0-3)
        if !self.dir_c_lower {
            out = (out & 0xF0) | (v & 0x0F);
        }
        // upper nibble (PC4-7)
        if !self.dir_c_upper {
            out = (out & 0x0F) | (v & 0xF0);
        }
        // If both halves input, still latch but not visible — store anyway
        if self.dir_c_lower && self.dir_c_upper {
            out = v;
        }
        self.pc = out;
    }

    pub fn read_pa(&self) -> u8 {
        if self.dir_a { self.pa_in } else { self.pa }
    }
    pub fn read_pb(&self) -> u8 {
        if self.dir_b { self.pb_in } else { self.pb }
    }
    pub fn read_pc(&self) -> u8 {
        // Return merged value: input bits from pins, output bits from latch
        let mut v = 0u8;
        // lower nibble
        if self.dir_c_lower { v |= self.pc_in & 0x0F; } else { v |= self.pc & 0x0F; }
        // upper nibble
        if self.dir_c_upper { v |= self.pc_in & 0xF0; } else { v |= self.pc & 0xF0; }
        v
    }

    pub fn read_ctrl(&self) -> u8 { self.ctrl }

    /// Inject external pin state for input ports (called by Emulator::port_write external / set_input)
    pub fn set_input(&mut self, port: u8, v: u8) {
        match port {
            0 => self.pa_in = v,
            1 => self.pb_in = v,
            2 => self.pc_in = v,
            _ => {}
        }
    }

    pub fn snapshot(&self) -> Vec<u8> {
        vec![self.pa, self.pb, self.pc, self.pa_in, self.pb_in, self.pc_in, self.ctrl,
             self.dir_a as u8, self.dir_b as u8, self.dir_c_upper as u8, self.dir_c_lower as u8]
    }
    pub fn restore(&mut self, d: &[u8]) {
        if d.len() < 11 { return; }
        self.pa = d[0]; self.pb = d[1]; self.pc = d[2];
        self.pa_in = d[3]; self.pb_in = d[4]; self.pc_in = d[5];
        self.ctrl = d[6];
        self.dir_a = d[7]!=0; self.dir_b = d[8]!=0;
        self.dir_c_upper = d[9]!=0; self.dir_c_lower = d[10]!=0;
    }
}
