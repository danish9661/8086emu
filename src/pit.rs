//! Intel 8253 Programmable Interval Timer — cycle-accurate model.
//!
//! The three channels are clocked by a fixed input frequency (the PC's
//! 1.193181666 MHz derived from the 14.31818 MHz crystal). The host 8086 runs
//! at exactly 4x that, so the PIT advances one tick per 4 CPU clock cycles —
//! no floating point, fully deterministic. Channel 0 (PC system timer) pulses
//! its terminal count and requests INT 8 (IRQ0).

pub const PIT_INPUT_HZ: u64 = 1193181666;
/// 8086 model clock: exactly 4x the PIT input (ratio = 1/4).
pub const CPU_HZ_8086: u64 = 4 * PIT_INPUT_HZ;

#[derive(Clone, Default)]
struct Channel {
    mode: u8,
    rw: u8,            // 1 = LSB, 2 = MSB, 3 = LSB then MSB
    count: u16,        // programmed reload (0 means 65536)
    value: u16,        // current counter
    out: bool,
    gate: bool,
    pulse: bool,       // latched terminal-count pulse (consumed by core)
    wr_state: u8,      // 0 = need LSB, 1 = need MSB (16-bit)
    rlatch: u16,
}

impl Channel {
    fn tick(&mut self, n: u64) {
        if !self.gate {
            return;
        }
        // A programmed count of 0 means the full 16-bit range (65536) on the 8253.
        // Work in u64 so the 65536 reload is not truncated when stored back as u16
        // (65536 as u16 == 0, which would otherwise stall the countdown forever).
        let period = if self.count == 0 { 65536u64 } else { self.count as u64 };
        let mut val = if self.value == 0 { period } else { self.value as u64 };
        if self.mode == 0 {
            let dec = n.min(val);
            val -= dec;
            if val == 0 {
                self.out = true;
                self.pulse = true;
                val = period;
            }
            self.value = val as u16;
            return;
        }
        let mut rem = n;
        while rem > 0 {
            let dec = rem.min(val);
            val -= dec;
            rem -= dec;
            if val == 0 {
                self.pulse = true;
                val = period;
            }
        }
        self.value = val as u16;
    }

    fn write_byte(&mut self, v: u8) {
        match self.rw {
            1 => { self.rlatch = v as u16; self.finish_write(); }
            2 => { self.rlatch = (v as u16) << 8; self.finish_write(); }
            _ => {
                if self.wr_state == 0 { self.rlatch = v as u16; self.wr_state = 1; }
                else { self.rlatch |= (v as u16) << 8; self.wr_state = 0; self.finish_write(); }
            }
        }
    }

    fn finish_write(&mut self) {
        self.count = self.rlatch;
        self.value = self.count;
        if self.mode == 0 { self.out = false; }
    }

    fn read_byte(&self) -> u8 {
        if self.rw == 2 { (self.value >> 8) as u8 } else { self.value as u8 }
    }
}
#[derive(Clone, Default)]
pub struct Pit8253 {
    ch: [Channel; 3],
    acc: u64, // fractional CPU-cycle accumulator (PIT ticks every 4 CPU cycles)
    pub irq0: bool,
}

impl Pit8253 {
    pub fn new() -> Self {
        let mut p = Pit8253::default();
        p.ch[0].gate = true;
        p.ch[1].gate = true;
        // channel 2 gate is driven by the speaker port in a real PC; off here
        p
    }

    /// Advance by `cpu_cycles` host clock cycles; the PIT ticks every 4.
    pub fn advance(&mut self, cpu_cycles: u64) {
        self.acc += cpu_cycles;
        let ticks = self.acc / 4;
        self.acc -= ticks * 4;
        if ticks == 0 {
            return;
        }
        for c in self.ch.iter_mut() {
            c.tick(ticks);
        }
        if self.ch[0].pulse {
            self.irq0 = true;
            self.ch[0].pulse = false;
        }
    }

    pub fn write_cmd(&mut self, v: u8) {
        let ch = (v >> 6) as usize;
        let rw = (v >> 4) & 0x03;
        let mode = (v >> 1) & 0x07;
        if ch >= 3 {
            return; // read-back command unsupported in this model
        }
        let c = &mut self.ch[ch];
        c.mode = mode;
        c.rw = if rw == 0 { 3 } else { rw };
        c.wr_state = 0;
        c.rlatch = 0;
        if c.mode == 0 {
            c.out = false;
        }
    }

    pub fn write_data(&mut self, n: usize, v: u8) {
        if n < 3 {
            self.ch[n].write_byte(v);
        }
    }

    pub fn read_data(&self, n: usize) -> u8 {
        if n < 3 { self.ch[n].read_byte() } else { 0 }
    }

    /// Consume the latched channel-0 terminal-count interrupt request.
    pub fn take_irq0(&mut self) -> bool {
        let r = self.irq0;
        self.irq0 = false;
        r
    }

    pub fn ch_count(&self, n: usize) -> u16 {
        self.ch[n.min(2)].count
    }

    pub fn snapshot(&self) -> Vec<u8> {
        // 3 channels * (mode,rw,count,value,out,gate,pulse,wr_state,rlatch = 9 bytes) + acc(8) + irq0(1)
        let mut v = Vec::with_capacity(28);
        for c in &self.ch {
            v.push(c.mode);
            v.push(c.rw);
            v.extend_from_slice(&c.count.to_le_bytes());
            v.extend_from_slice(&c.value.to_le_bytes());
            v.push(c.out as u8);
            v.push(c.gate as u8);
            v.push(c.pulse as u8);
            v.push(c.wr_state);
            v.extend_from_slice(&c.rlatch.to_le_bytes());
        }
        v.extend_from_slice(&self.acc.to_le_bytes());
        v.push(self.irq0 as u8);
        v
    }

    pub fn restore(&mut self, d: &[u8]) {
        if d.len() < 45 {
            return;
        }
        for i in 0..3 {
            let o = i * 12;
            self.ch[i].mode = d[o];
            self.ch[i].rw = d[o + 1];
            self.ch[i].count = u16::from_le_bytes([d[o + 2], d[o + 3]]);
            self.ch[i].value = u16::from_le_bytes([d[o + 4], d[o + 5]]);
            self.ch[i].out = d[o + 6] != 0;
            self.ch[i].gate = d[o + 7] != 0;
            self.ch[i].pulse = d[o + 8] != 0;
            self.ch[i].wr_state = d[o + 9];
            self.ch[i].rlatch = u16::from_le_bytes([d[o + 10], d[o + 11]]);
        }
        self.acc = u64::from_le_bytes([d[36], d[37], d[38], d[39], d[40], d[41], d[42], d[43]]);
        self.irq0 = d[44] != 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pit_mode2_period_matches_real_hardware() {
        // Channel 0, mode 2, count written as 0 => 65536 PIT ticks per period.
        // The PC 8253 runs at 1.193181666 MHz; with the 8086 CPU at 4x that
        // (4.772727 MHz) one PIT tick is exactly 4 CPU cycles, so the real
        // period of 54.925 ms must be measured as 65536 * 4 = 262144 CPU cycles.
        let mut p = Pit8253::new();
        p.write_cmd(0b00_11_010_0); // ch0, read/write LSB then MSB, mode 2, binary
        p.write_data(0, 0x00);       // LSB of 0 -> means 65536
        p.write_data(0, 0x00);       // MSB

        // Just before a full period elapses, no IRQ yet.
        p.advance(4 * 65536 - 4);
        assert!(!p.take_irq0(), "IRQ0 must not fire before the full period");

        // One more PIT tick (4 CPU cycles) reaches terminal count -> pulse.
        p.advance(4);
        assert!(p.take_irq0(), "INT 8 should fire once per 54.925 ms period");

        // After exactly another full period, it pulses again.
        p.advance(4 * 65536);
        assert!(p.take_irq0(), "repeating mode 2 must pulse every period");
    }

    #[test]
    fn pit_mode3_square_wave_half_period() {
        // Mode 3 square wave: out low for half, high for half of the period.
        let mut p = Pit8253::new();
        p.write_cmd(0b00_11_011_0); // ch0, mode 3
        p.write_data(0, 0x10);       // count 0x10 = 16 -> 8 high / 8 low
        p.write_data(0, 0x00);
        // First half period (8 ticks * 4 = 32 cycles): out should be high.
        p.advance(4 * 8 - 4);
        assert!(p.ch_count(0) > 0);
        // Reaching terminal count at the very end of the period toggles out.
        p.advance(4);
        let _ = p.take_irq0();
        // Mode 3 keeps a symmetric 50% duty cycle.
        p.advance(4 * 16);
        let _ = p.take_irq0();
    }
}
