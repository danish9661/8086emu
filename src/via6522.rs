//! MOS 6522 Versatile Interface Adapter — teaching subset for the 6502.
//!
//! Memory-mapped at $6000-$600F (16 registers RS0-3). Provides two 8-bit
//! bidirectional ports (A/B with data-direction registers), two 16-bit timers
//! (T1 one-shot/free-run with PB7 square output, T2 timed interrupt), an
//! interrupt flag/enable pair (IFR/IER driving the 6502 IRQ line), control
//! registers (ACR/PCR), and CA1/CB1 handshake inputs.
//!
//! NOTE: $6000-$600F is reserved for VIA registers. The bulk image loader
//! (`Cpu::mem_write`) writes backing RAM directly so a padded 64 KiB image
//! (e.g. vectors at $FFFE) cannot program the device with zero fill; only
//! executed STA instructions (and `Emulator::via_write`) reach the registers.
//! Keep code and data clear of $6000-$600F.
//!
//! The CPU ticks the VIA once per executed instruction (not wall-clock time).
//! Simplifications vs hardware (documented for students):
//! * shift register (SR) is storage only (no shifting);
//! * reading T1C-L does NOT clear the T1 interrupt flag (on hardware it does)
//!   — clear it by writing IFR with bit 7 = 0 and bit 6 = 1;
//! * CA2/CB2 handshake/pulse output modes are simplified to manual levels
//!   (PCR mode 110 = low, 111 = high, otherwise the last level is held);
//! * T2 pulse-counting mode (ACR5 = 1) counts pulses injected via
//!   [`Via6522::pulse_pb6`] instead of the PB6 pin;
//! * CA1/CB1 edges come from [`Via6522::set_ca1`]/[`Via6522::set_cb1`]
//!   (the IDE pulse buttons), not physical pins.

/// Base address of the VIA register block.
pub const VIA_BASE: u32 = 0x6000;
/// Number of VIA registers (RS0-3).
pub const VIA_SIZE: u32 = 16;

// IFR/IER bit numbers.
const B_T1: u8 = 6;
const B_T2: u8 = 5;
const B_CA1: u8 = 1;
const B_CA2: u8 = 0;
const B_CB1: u8 = 4;
const B_CB2: u8 = 3;

#[derive(Clone)]
pub struct Via6522 {
    ora: u8,
    orb: u8,
    ddra: u8,
    ddrb: u8,
    /// External pin levels driven into ports A/B (IDE pin injection).
    pa_pins: u8,
    pb_pins: u8,
    t1l: u16,
    t1c: u16,
    t2c: u16,
    t1_run: bool,
    t2_run: bool,
    /// PB7 square-wave state (T1 free-run mode, ACR7 = 1).
    pb7: bool,
    sr: u8,
    acr: u8,
    pcr: u8,
    ier: u8, // bit 7 reads back 1
    ca1: bool,
    cb1: bool,
    ca2_out: bool,
    cb2_out: bool,
    irq_latch: u8, // raw interrupt flags (bits 0-6)
}

impl Default for Via6522 {
    fn default() -> Self {
        Via6522 {
            ora: 0,
            orb: 0,
            ddra: 0,
            ddrb: 0,
            pa_pins: 0xFF,
            pb_pins: 0xFF,
            t1l: 0,
            t1c: 0,
            t2c: 0,
            t1_run: false,
            t2_run: false,
            pb7: false,
            sr: 0,
            acr: 0,
            pcr: 0,
            ier: 0,
            ca1: false,
            cb1: false,
            ca2_out: true,
            cb2_out: true,
            irq_latch: 0,
        }
    }
}

impl Via6522 {
    pub fn new() -> Self {
        Self::default()
    }

    fn set_flag(&mut self, bit: u8) {
        self.irq_latch |= 1 << bit;
    }

    /// Current IRQ line level (any latched flag that is also enabled).
    pub fn irq(&self) -> bool {
        self.irq_latch & self.ier & 0x7F != 0
    }

    /// Read register `rs` (0-15).
    pub fn read(&self, rs: u8) -> u8 {
        match rs & 0x0F {
            0 => (self.orb & self.ddrb) | (self.pb_pins & !self.ddrb),
            1 => (self.ora & self.ddra) | (self.pa_pins & !self.ddra),
            2 => self.ddrb,
            3 => self.ddra,
            4 => self.t1c as u8,
            5 => (self.t1c >> 8) as u8,
            6 => self.t1l as u8,
            7 => (self.t1l >> 8) as u8,
            8 => self.t2c as u8,
            9 => (self.t2c >> 8) as u8,
            10 => self.sr,
            11 => self.acr,
            12 => self.pcr,
            13 => {
                let mut v = self.irq_latch & 0x7F;
                if self.irq() {
                    v |= 0x80;
                }
                v
            }
            14 => self.ier | 0x80,
            _ => (self.ora & self.ddra) | (self.pa_pins & !self.ddra),
        }
    }

    /// Write register `rs` (0-15).
    pub fn write(&mut self, rs: u8, v: u8) {
        match rs & 0x0F {
            0 => self.orb = v,
            1 | 15 => self.ora = v,
            2 => self.ddrb = v,
            3 => self.ddra = v,
            4 => self.t1l = (self.t1l & 0xFF00) | v as u16,
            5 => {
                // T1C-H: latch high, transfer latch to counter, restart.
                self.t1l = (self.t1l & 0x00FF) | ((v as u16) << 8);
                self.t1c = self.t1l;
                self.irq_latch &= !(1 << B_T1);
                self.t1_run = true;
                self.pb7 = false;
            }
            6 => self.t1l = (self.t1l & 0xFF00) | v as u16,
            7 => self.t1l = (self.t1l & 0x00FF) | ((v as u16) << 8),
            8 => {
                // T2C-L: low byte of the T2 latch (stored in t2c until H written).
                self.t2c = (self.t2c & 0xFF00) | v as u16;
            }
            9 => {
                // T2C-H: transfer to counter, restart timed mode.
                self.t2c = (self.t2c & 0x00FF) | ((v as u16) << 8);
                self.irq_latch &= !(1 << B_T2);
                if self.acr & 0x20 == 0 {
                    // Timed-interrupt mode counts down from here.
                    self.t2_run = true;
                }
                // Pulse-counting mode (ACR5 = 1) waits for pulse_pb6().
            }
            10 => self.sr = v,
            11 => self.acr = v,
            12 => {
                self.pcr = v;
                // Manual CA2/CB2 levels (modes 110/111); other modes hold.
                match (v >> 1) & 0x07 {
                    0x06 => self.ca2_out = false,
                    0x07 => self.ca2_out = true,
                    _ => {}
                }
                match (v >> 5) & 0x07 {
                    0x06 => self.cb2_out = false,
                    0x07 => self.cb2_out = true,
                    _ => {}
                }
            }
            13 => {
                // IFR: bit 7 = 0 clears the indicated flags.
                if v & 0x80 == 0 {
                    self.irq_latch &= !(v & 0x7F);
                }
            }
            14 => {
                // IER: bit 7 = 1 sets, 0 clears the indicated enables.
                if v & 0x80 != 0 {
                    self.ier |= v & 0x7F;
                } else {
                    self.ier &= !(v & 0x7F);
                }
            }
            _ => self.ora = v,
        }
    }

    /// Advance both timers by one CPU instruction.
    pub fn tick(&mut self) {
        if self.t1_run {
            if self.t1c == 0 {
                self.t1_timeout();
            } else {
                self.t1c -= 1;
                if self.t1c == 0 {
                    self.t1_timeout();
                }
            }
        }
        if self.t2_run && self.acr & 0x20 == 0 {
            if self.t2c == 0 {
                self.t2_timeout();
            } else {
                self.t2c -= 1;
                if self.t2c == 0 {
                    self.t2_timeout();
                }
            }
        }
    }

    fn t1_timeout(&mut self) {
        self.set_flag(B_T1);
        if self.acr & 0x80 != 0 {
            // Free-run: reload and toggle PB7.
            self.t1c = self.t1l;
            self.pb7 = !self.pb7;
        } else {
            // One-shot: reload for reads but stop counting.
            self.t1c = self.t1l;
            self.t1_run = false;
        }
    }

    fn t2_timeout(&mut self) {
        self.set_flag(B_T2);
        self.t2_run = false;
    }

    /// Count one PB6 pulse (T2 pulse-counting mode, ACR5 = 1).
    pub fn pulse_pb6(&mut self) {
        if self.acr & 0x20 == 0 {
            return;
        }
        if self.t2c == 0 {
            self.t2_timeout();
        } else {
            self.t2c -= 1;
            if self.t2c == 0 {
                self.t2_timeout();
            }
        }
    }

    /// Drive the CA1 input line (edge per PCR bit 0: 0 = falling, 1 = rising).
    pub fn set_ca1(&mut self, high: bool) {
        let edge = if self.pcr & 0x01 != 0 { !self.ca1 && high } else { self.ca1 && !high };
        self.ca1 = high;
        if edge {
            self.set_flag(B_CA1);
        }
    }

    /// Drive the CB1 input line (edge per PCR bit 4).
    pub fn set_cb1(&mut self, high: bool) {
        let edge = if self.pcr & 0x10 != 0 { !self.cb1 && high } else { self.cb1 && !high };
        self.cb1 = high;
        if edge {
            self.set_flag(B_CB1);
        }
    }

    /// Inject external pin levels seen on input port bits.
    /// `port`: 0 = A, 1 = B.
    pub fn set_pins(&mut self, port: u8, v: u8) {
        if port == 0 {
            self.pa_pins = v;
        } else {
            self.pb_pins = v;
        }
    }

    /// PB7 square-wave state (T1 free-run output, for the IDE panel).
    pub fn pb7(&self) -> bool {
        self.pb7
    }

    pub fn snapshot(&self) -> Vec<u8> {
        vec![
            1, // layout version
            self.ora,
            self.orb,
            self.ddra,
            self.ddrb,
            self.pa_pins,
            self.pb_pins,
            (self.t1l & 0xFF) as u8,
            (self.t1l >> 8) as u8,
            (self.t1c & 0xFF) as u8,
            (self.t1c >> 8) as u8,
            (self.t2c & 0xFF) as u8,
            (self.t2c >> 8) as u8,
            self.sr,
            self.acr,
            self.pcr,
            self.irq_latch,
            self.ier,
            ((self.t1_run as u8) << 1) | (self.t2_run as u8) | ((self.pb7 as u8) << 2),
            ((self.ca1 as u8) << 1)
                | (self.cb1 as u8)
                | ((self.ca2_out as u8) << 2)
                | ((self.cb2_out as u8) << 3),
        ]
    }

    pub fn restore(&mut self, d: &[u8]) {
        // `snapshot()` emits exactly 20 bytes; accept that (older callers
        // passed a 21-byte window whose last byte was already ignored).
        if d.len() < 20 || d[0] != 1 {
            return;
        }
        self.ora = d[1];
        self.orb = d[2];
        self.ddra = d[3];
        self.ddrb = d[4];
        self.pa_pins = d[5];
        self.pb_pins = d[6];
        self.t1l = d[7] as u16 | ((d[8] as u16) << 8);
        self.t1c = d[9] as u16 | ((d[10] as u16) << 8);
        self.t2c = d[11] as u16 | ((d[12] as u16) << 8);
        self.sr = d[13];
        self.acr = d[14];
        self.pcr = d[15];
        self.irq_latch = d[16] & 0x7F;
        self.ier = d[17] & 0x7F;
        self.t1_run = d[18] & 0x02 != 0;
        self.t2_run = d[18] & 0x01 != 0;
        self.pb7 = d[18] & 0x04 != 0;
        self.ca1 = d[19] & 0x02 != 0;
        self.cb1 = d[19] & 0x01 != 0;
        self.ca2_out = d[19] & 0x04 != 0;
        self.cb2_out = d[19] & 0x08 != 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1_oneshot_irqs_once() {
        let mut v = Via6522::new();
        v.write(6, 3); // T1L-L = 3
        v.write(7, 0); // T1L-H = 0
        v.write(14, 0x80 | 0x40); // IER: enable T1
        v.write(5, 0); // T1C-H: start (latch 3)
        assert!(!v.irq());
        v.tick();
        v.tick();
        assert!(!v.irq(), "2 ticks of 3 remaining");
        v.tick();
        assert!(v.irq(), "T1 timeout raises IRQ");
        v.write(13, 0x40); // IFR clear T1 (bit7=0)
        assert!(!v.irq(), "IFR write clears the flag");
        for _ in 0..100 {
            v.tick();
        }
        assert!(!v.irq(), "one-shot stays silent until restarted");
    }

    #[test]
    fn t1_freerun_repeats_and_toggles_pb7() {
        let mut v = Via6522::new();
        v.write(11, 0x80); // ACR: T1 free-run
        v.write(6, 2);
        v.write(7, 0);
        v.write(14, 0x80 | 0x40);
        v.write(5, 0);
        v.tick();
        v.tick();
        assert!(v.irq());
        let pb = v.pb7();
        v.write(13, 0x40);
        v.tick();
        v.tick();
        assert!(v.irq(), "free-run re-arms");
        assert_ne!(v.pb7(), pb, "PB7 toggles each period");
    }

    #[test]
    fn ports_follow_ddr() {
        let mut v = Via6522::new();
        v.write(3, 0xF0); // DDRA: high nibble out
        v.write(1, 0xA0); // ORA
        v.set_pins(0, 0x0F);
        assert_eq!(v.read(1), 0xAF);
    }

    #[test]
    fn ca1_edge_sets_flag() {
        let mut v = Via6522::new();
        v.write(12, 0x01); // PCR: CA1 rising edge
        v.set_ca1(false);
        v.set_ca1(true);
        assert_eq!(v.read(13) & 0x02, 0x02);
    }
}
