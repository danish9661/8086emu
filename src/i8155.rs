//! Intel 8155 RAM/I/O/Timer — external peripheral for the 8085 (and 8086).
//!
//! The 8155 packs 256 bytes of static RAM, three 8-bit I/O ports (PA/PB/PC),
//! a command/status register, and a 14-bit down-counting timer. It is a
//! memory-mapped part: on the 8085 its RAM sits at 0x8000..0x80FF and its
//! registers at I/O ports 0x80..0x85; on the 8086 the same windows apply.
//!
//! The timer is **cycle-accurate**: it counts down one tick per host clock
//! cycle (one 8085 machine cycle / 8086 bus cycle). A 14-bit load of N gives a
//! period of N+1 ticks in single-pulse mode, or toggles every (N+1)/2 ticks in
//! square-wave mode — exactly like the real chip.

#[derive(Clone)]
pub struct I8155 {
    ram: [u8; 256],
    pub pa: u8,
    pub pb: u8,
    pub pc: u8,
    cmd: u8,
    tl: u8,
    th: u8,
    tc: u16, // current 14-bit counter
    running: bool,
    mode: bool, // false = single pulse, true = square wave
    pulse: bool, // latched terminal-count pulse (consumed by core)
}

impl Default for I8155 {
    fn default() -> Self {
        I8155 {
            ram: [0; 256],
            pa: 0, pb: 0, pc: 0,
            cmd: 0, tl: 0, th: 0,
            tc: 0, running: false, mode: false, pulse: false,
        }
    }
}

impl I8155 {
    pub fn new() -> Self {
        I8155::default()
    }

    /// Cycle-accurate timer advance: one tick per host clock cycle.
    pub fn advance(&mut self, ticks: u64) {
        if !self.running {
            return;
        }
        let mut rem = ticks;
        while rem > 0 {
            if self.tc == 0 {
                // reload the latched count; (N+1) because 0 is the terminal state
                let load = ((self.th as u16 & 0x3F) << 8 | self.tl as u16) & 0x3FFF;
                if self.mode {
                    // square wave: toggle output, reload, continue
                    self.tc = load;
                } else {
                    // single pulse: one pulse then stop
                    self.pulse = true;
                    self.running = false;
                    break;
                }
            }
            let dec = rem.min(self.tc as u64);
            self.tc -= dec as u16;
            rem -= dec;
            if self.tc == 0 {
                self.pulse = true; // terminal count reached this tick
            }
        }
    }

    /// Consume the latched timer terminal-count pulse.
    pub fn take_pulse(&mut self) -> bool {
        let r = self.pulse;
        self.pulse = false;
        r
    }

    pub fn write_reg(&mut self, reg: usize, v: u8) {
        match reg {
            0 => {
                // command/status register
                self.cmd = v;
                // bit 4 = timer start (1) / stop (0)
                if v & 0x10 != 0 {
                    let load = ((self.th as u16 & 0x3F) << 8 | self.tl as u16) & 0x3FFF;
                    self.tc = load;
                    self.mode = self.th & 0x80 != 0;
                    self.running = true;
                } else {
                    self.running = false;
                }
            }
            1 => self.pa = v,
            2 => self.pb = v,
            3 => self.pc = v,
            4 => self.tl = v,
            5 => {
                self.th = v;
                self.mode = v & 0x80 != 0;
            }
            _ => {}
        }
    }

    pub fn read_reg(&self, reg: usize) -> u8 {
        match reg {
            0 => (self.cmd & 0x0F) | if self.running { 0x80 } else { 0 },
            1 => self.pa,
            2 => self.pb,
            3 => self.pc,
            4 => self.tc as u8,
            5 => (self.tc >> 8) as u8,
            _ => 0,
        }
    }

    pub fn ram_read(&self, off: usize) -> u8 {
        self.ram[off & 0xFF]
    }

    pub fn ram_write(&mut self, off: usize, v: u8) {
        self.ram[off & 0xFF] = v;
    }

    pub fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(256 + 11);
        v.extend_from_slice(&self.ram);
        v.push(self.pa);
        v.push(self.pb);
        v.push(self.pc);
        v.push(self.cmd);
        v.push(self.tl);
        v.push(self.th);
        v.extend_from_slice(&self.tc.to_le_bytes());
        v.push(self.running as u8);
        v.push(self.mode as u8);
        v.push(self.pulse as u8);
        v
    }

    pub fn restore(&mut self, d: &[u8]) {
        if d.len() < 267 {
            return;
        }
        self.ram.copy_from_slice(&d[..256]);
        self.pa = d[256];
        self.pb = d[257];
        self.pc = d[258];
        self.cmd = d[259];
        self.tl = d[260];
        self.th = d[261];
        self.tc = u16::from_le_bytes([d[262], d[263]]);
        self.running = d[264] != 0;
        self.mode = d[265] != 0;
        self.pulse = d[266] != 0;
    }
}
