//! Intel 8259A Programmable Interrupt Controller — minimal PC-compatible model.
//!
//! Routes device IRQ lines (e.g. 8253 PIT channel 0 → IRQ0) to the 8086 INTR
//! pin. Supports the PC BIOS initialization sequence (ICW1→ICW2→ICW3→ICW4),
//! the interrupt mask register (OCW1 / port 0x21), end-of-interrupt (OCW2 /
//! port 0x20), and OCW3 register-read selection. Priority is fixed (IRQ0
//! highest … IRQ7 lowest) with no preemption of an equal/higher in-service IRQ.

#[derive(Clone, Default)]
pub struct Pic8259 {
    base: u8,        // ICW2 vector base (IRQ0 -> base+0); default 0x08
    imr: u8,         // OCW1 mask: 1 = masked
    irr: u8,         // interrupt request register
    isr: u8,         // in-service register
    init_step: u8,   // 0 = normal; 1 = await ICW2; 2 = await ICW3; 3 = await ICW4
    expect_icw4: bool,
    cascade: bool,   // ICW3 expected (SNGL=0)
    auto_eoi: bool,  // ICW4 bit1
    read_isr: bool,  // OCW3 RIS: port 0x20 read returns ISR
    icw3: u8,
}

impl Pic8259 {
    pub fn new() -> Self {
        // Default base 0x08 so the system timer fires INT 8 even before a BIOS
        // initializes the controller (common in small bare-metal demos).
        Pic8259 { base: 0x08, ..Default::default() }
    }

    pub fn base(&self) -> u8 { self.base }

    /// A device asserts its IRQ line (level). Latched into IRR unless that IRQ
    /// is already in service (waiting for an EOI).
    pub fn request(&mut self, irq: u8) {
        if irq > 7 {
            return;
        }
        if self.isr & (1u8 << irq) == 0 {
            self.irr |= 1u8 << irq;
        }
    }

    /// Highest-priority unmasked, non-preempting IRQ vector, if any.
    pub fn output_vector(&self) -> Option<u8> {
        let req = self.irr & !self.imr;
        if req == 0 {
            return None;
        }
        let p = req.trailing_zeros() as u8; // lowest set bit = highest priority
        if self.isr != 0 {
            let ip = self.isr.trailing_zeros() as u8;
            if ip <= p {
                return None; // equal/higher priority ISR already in service
            }
        }
        Some(self.base.wrapping_add(p))
    }

    /// CPU acknowledged the vector: move IRR→ISR (unless auto-EOI).
    pub fn acknowledge(&mut self, irq: u8) {
        if irq > 7 {
            return;
        }
        self.irr &= !(1u8 << irq);
        if !self.auto_eoi {
            self.isr |= 1u8 << irq;
        }
    }

    /// Non-specific EOI: clear the highest-priority in-service bit.
    pub fn eoi(&mut self) {
        if self.isr != 0 {
            let ip = self.isr.trailing_zeros() as u8;
            self.isr &= !(1u8 << ip);
        }
    }

    pub fn write_cmd(&mut self, v: u8) {
        if v & 0x10 != 0 {
            // ICW1: begin initialization.
            self.init_step = 1;
            self.cascade = (v & 0x02) == 0; // SNGL=0 => cascade (ICW3 follows)
            self.expect_icw4 = (v & 0x01) != 0; // 8086 always sets this
            self.irr = 0;
            self.isr = 0;
            return;
        }
        if v & 0x18 == 0x00 {
            // OCW2.
            if v & 0x20 != 0 {
                // EOI (bit5).
                if v & 0x80 != 0 {
                    self.isr &= !(1u8 << (v & 0x07)); // specific EOI
                } else {
                    self.eoi(); // non-specific EOI
                }
            }
            // priority rotation bits ignored in this model
        } else if v & 0x18 == 0x08 {
            // OCW3: register-read selection.
            self.read_isr = (v & 0x02) != 0;
        }
    }

    pub fn write_data(&mut self, v: u8) {
        match self.init_step {
            1 => {
                self.base = v & 0xF8;
                self.init_step = if self.cascade { 2 } else if self.expect_icw4 { 3 } else { 0 };
            }
            2 => {
                self.icw3 = v;
                self.init_step = if self.expect_icw4 { 3 } else { 0 };
            }
            3 => {
                self.auto_eoi = (v & 0x02) != 0;
                self.init_step = 0;
            }
            _ => self.imr = v, // OCW1
        }
    }

    pub fn read_cmd(&self) -> u8 {
        if self.read_isr { self.isr } else { self.irr }
    }

    pub fn read_data(&self) -> u8 {
        self.imr
    }

    pub fn snapshot(&self) -> Vec<u8> {
        let mut f = 0u8;
        if self.expect_icw4 { f |= 1; }
        if self.cascade { f |= 2; }
        if self.auto_eoi { f |= 4; }
        if self.read_isr { f |= 8; }
        vec![self.base, self.imr, self.irr, self.isr, self.init_step, f, self.icw3, 0]
    }

    pub fn restore(&mut self, d: &[u8]) {
        if d.len() < 8 {
            return;
        }
        self.base = d[0];
        self.imr = d[1];
        self.irr = d[2];
        self.isr = d[3];
        self.init_step = d[4];
        self.expect_icw4 = d[5] & 1 != 0;
        self.cascade = d[5] & 2 != 0;
        self.auto_eoi = d[5] & 4 != 0;
        self.read_isr = d[5] & 8 != 0;
        self.icw3 = d[6];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pic_priority_and_eoi() {
        let mut p = Pic8259::new();
        p.request(0); // IRQ0 -> INT 8
        p.request(3); // IRQ3 -> INT B
        assert_eq!(p.output_vector(), Some(0x08), "IRQ0 outranks IRQ3");
        // Acknowledge IRQ0 (moves to ISR).
        p.acknowledge(0);
        // IRQ3 still pending but lower priority; with IRQ0 in service it is blocked.
        assert_eq!(p.output_vector(), None, "lower IRQ preempted by in-service IRQ0");
        p.eoi(); // ISR cleared
        assert_eq!(p.output_vector(), Some(0x0B), "IRQ3 fires after EOI");
    }

    #[test]
    fn pic_init_sequence_sets_base() {
        let mut p = Pic8259::new();
        p.write_cmd(0x11); // ICW1: init, cascade, expect ICW4
        p.write_data(0x08); // ICW2: base 0x08
        p.write_data(0x04); // ICW3: master has slave on IRQ2
        p.write_data(0x01); // ICW4: 8086 mode
        assert_eq!(p.init_step, 0, "init done after ICW4");
        p.request(0);
        assert_eq!(p.output_vector(), Some(0x08));
    }

    #[test]
    fn pic_imr_masks() {
        let mut p = Pic8259::new();
        p.write_data(0x01); // OCW1: mask IRQ0
        p.request(0);
        assert_eq!(p.output_vector(), None, "masked IRQ0 must not assert INTR");
    }
}
