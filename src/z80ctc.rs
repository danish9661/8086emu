//! Zilog Z80 CTC (Counter/Timer Circuit) — teaching subset.
//!
//! Four channels (0-3) mapped to I/O ports 0x10-0x13. Each channel is either
//! a timer (counts system clocks divided by 16 or 256) or a counter (counts
//! external pulses injected via [`Z80Ctc::pulse`], e.g. the IDE pulse button).
//! A terminal count reloads the time constant and raises a maskable interrupt
//! (highest channel first: 0 > 1 > 2 > 3); the CPU vectors through its usual
//! maskable-INT path. In IM 2 the peripheral would supply
//! `(vector_base & 0xF8) | (channel * 2)` — see [`Z80Ctc::take_irq`].
//!
//! Programming (one byte at a time, same order as the real chip):
//! * channel control word (bit 0 = 1): D7 = interrupt enable, D6 = 1 timer /
//!   0 counter, D5 = 1 prescaler 256 / 0 prescaler 16 (timer only), D2 = 1
//!   time-constant byte follows, D1 = software reset.
//! * time-constant byte (when D2 was set): 1-256 (0 means 256); starts counting.
//! * interrupt vector word (bit 0 = 0, channel 0 only): base vector for IM 2.
//!
//! Simplifications vs hardware (documented for students): one [`Z80Ctc::tick`]
//! equals one CPU instruction (not one system-clock edge); counter mode has no
//! CLK/TRG pins (pulses come from `pulse()`); the daisy-chain IEI/IEO pins are
//! not modelled (priority is fixed 0 > 1 > 2 > 3); the channel-3 "timer on
//! channel 3 TRG" wiring is not modelled.

/// I/O base port of CTC channel 0 (channels live at 0x10-0x13).
pub const CTC_BASE: u8 = 0x10;

#[derive(Clone, Copy)]
struct Channel {
    /// Interrupt enabled (control D7).
    ie: bool,
    /// True = timer mode, false = counter mode (control D6).
    timer: bool,
    /// True = prescaler 256, false = 16 (control D5, timer only).
    pre256: bool,
    /// Time constant (0 stored as 256 counts).
    tc: u16,
    /// Current down-counter (0 stored as 256 remaining).
    counter: u16,
    /// Prescaler divider countdown.
    pre: u16,
    /// Counting right now (set once a time constant is loaded).
    running: bool,
    /// Interrupt latched (terminal count reached, not yet consumed).
    irq: bool,
    /// Next data byte is a time constant (control D2 was set).
    tc_follows: bool,
}

impl Default for Channel {
    fn default() -> Self {
        Channel {
            ie: false,
            timer: true,
            pre256: false,
            tc: 0,
            counter: 0,
            pre: 0,
            running: false,
            irq: false,
            tc_follows: false,
        }
    }
}

impl Channel {
    fn reload(&self) -> u16 {
        if self.tc == 0 { 256 } else { self.tc }
    }

    fn tick_timer(&mut self) {
        if !self.running || !self.timer {
            return;
        }
        let div = if self.pre256 { 256 } else { 16 };
        self.pre += 1;
        if self.pre < div {
            return;
        }
        self.pre = 0;
        if self.counter == 0 {
            self.counter = self.reload();
        }
        self.counter -= 1;
        if self.counter == 0 {
            self.counter = self.reload();
            self.irq = true;
        }
    }

    fn snapshot(&self) -> [u8; 6] {
        let mut flags = 0u8;
        if self.ie { flags |= 0x01; }
        if self.timer { flags |= 0x02; }
        if self.pre256 { flags |= 0x04; }
        if self.running { flags |= 0x08; }
        if self.irq { flags |= 0x10; }
        if self.tc_follows { flags |= 0x20; }
        [
            self.tc as u8,
            self.counter as u8,
            self.pre as u8,
            flags,
            (self.counter >> 8) as u8,
            (self.pre >> 8) as u8,
        ]
    }

    fn restore(&mut self, d: &[u8]) {
        if d.len() < 6 {
            return;
        }
        self.tc = d[0] as u16;
        self.counter = d[1] as u16 | ((d[4] as u16) << 8);
        self.pre = d[2] as u16 | ((d[5] as u16) << 8);
        let f = d[3];
        self.ie = f & 0x01 != 0;
        self.timer = f & 0x02 != 0;
        self.pre256 = f & 0x04 != 0;
        self.running = f & 0x08 != 0;
        self.irq = f & 0x10 != 0;
        self.tc_follows = f & 0x20 != 0;
    }
}

#[derive(Clone, Default)]
pub struct Z80Ctc {
    ch: [Channel; 4],
    /// Interrupt vector base (programmed on channel 0, used in IM 2).
    vector: u8,
}

impl Z80Ctc {
    pub fn new() -> Self {
        Self::default()
    }

    /// Write a control / time-constant / vector byte to channel `ch` (0-3).
    pub fn write(&mut self, ch: usize, v: u8) {
        if ch > 3 {
            return;
        }
        let c = &mut self.ch[ch];
        if c.tc_follows {
            c.tc = v as u16;
            c.counter = if v == 0 { 256 } else { v as u16 };
            c.pre = 0;
            c.tc_follows = false;
            c.running = true;
            return;
        }
        if v & 0x01 == 0 {
            // Interrupt vector word (only channel 0 carries it on hardware).
            if ch == 0 {
                self.vector = v;
            }
            return;
        }
        // Channel control word.
        if v & 0x02 != 0 {
            // Software reset: stop and clear the latch (time constant kept).
            c.running = false;
            c.irq = false;
            c.pre = 0;
            c.tc_follows = false;
        }
        c.ie = v & 0x80 != 0;
        c.timer = v & 0x40 != 0;
        c.pre256 = v & 0x20 != 0;
        if v & 0x04 != 0 {
            c.tc_follows = true;
        }
    }

    /// Read channel `ch`: the current down-counter value (low byte).
    pub fn read(&self, ch: usize) -> u8 {
        if ch > 3 {
            return 0;
        }
        (self.ch[ch].counter & 0xFF) as u8
    }

    /// Advance all timer-mode channels by one CPU instruction.
    pub fn tick(&mut self) {
        for c in self.ch.iter_mut() {
            c.tick_timer();
        }
    }

    /// Inject one external CLK/TRG pulse into channel `ch` (counter mode).
    /// Returns true if this pulse caused a terminal count.
    pub fn pulse(&mut self, ch: usize) -> bool {
        if ch > 3 {
            return false;
        }
        let c = &mut self.ch[ch];
        if !c.running || c.timer {
            return false;
        }
        if c.counter == 0 {
            c.counter = c.reload();
        }
        c.counter -= 1;
        if c.counter == 0 {
            c.counter = c.reload();
            c.irq = true;
            return true;
        }
        false
    }

    /// True when any interrupt-enabled channel has a latched interrupt.
    pub fn irq_pending(&self) -> bool {
        self.ch.iter().any(|c| c.irq && c.ie)
    }

    /// Consume the highest-priority latched interrupt (0 > 1 > 2 > 3).
    /// Returns `(channel, vector_byte)` for IM 2 dispatch, or None.
    pub fn take_irq(&mut self) -> Option<(usize, u8)> {
        for (i, c) in self.ch.iter_mut().enumerate() {
            if c.irq && c.ie {
                c.irq = false;
                let vec = (self.vector & 0xF8) | ((i as u8) * 2);
                return Some((i, vec));
            }
        }
        None
    }

    /// Programmed interrupt vector base (channel 0 vector word).
    pub fn vector(&self) -> u8 {
        self.vector
    }

    /// Live down-counter of channel `ch` (0-256, 256 shown for reload 0).
    pub fn count(&self, ch: usize) -> u16 {
        if ch > 3 {
            return 0;
        }
        let c = self.ch[ch].counter;
        if c == 0 { 256 } else { c }
    }

    /// (interrupt_enabled, timer_mode) of channel `ch` for the IDE panel.
    pub fn mode(&self, ch: usize) -> (bool, bool) {
        if ch > 3 {
            return (false, true);
        }
        (self.ch[ch].ie, self.ch[ch].timer)
    }

    pub fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(26);
        v.push(1); // layout version
        v.push(self.vector);
        for c in &self.ch {
            v.extend_from_slice(&c.snapshot());
        }
        v
    }

    pub fn restore(&mut self, d: &[u8]) {
        if d.len() < 26 || d[0] != 1 {
            return;
        }
        self.vector = d[1];
        for (i, c) in self.ch.iter_mut().enumerate() {
            c.restore(&d[2 + i * 6..2 + i * 6 + 6]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_fires_and_reload() {
        let mut c = Z80Ctc::new();
        // ch0: interrupt + timer + prescaler 16 + tc follows + control
        c.write(0, 0x80 | 0x40 | 0x04 | 0x01);
        c.write(0, 2); // tc = 2 -> fires every 2*16 ticks
        assert!(!c.irq_pending());
        for _ in 0..31 {
            c.tick();
        }
        assert!(!c.irq_pending(), "prescaler 16 x tc 2 needs 32 ticks");
        c.tick();
        assert!(c.irq_pending());
        let (ch, vec) = c.take_irq().unwrap();
        assert_eq!(ch, 0);
        assert_eq!(vec & 0x07, 0);
        assert!(!c.irq_pending(), "latch consumed");
    }

    #[test]
    fn counter_counts_pulses() {
        let mut c = Z80Ctc::new();
        // ch1: interrupt + counter + tc follows + control
        c.write(1, 0x80 | 0x04 | 0x01);
        c.write(1, 3);
        assert!(!c.pulse(1));
        assert!(!c.pulse(1));
        assert!(c.pulse(1), "third pulse is the terminal count");
        assert!(c.irq_pending());
    }

    #[test]
    fn reset_stops_channel() {
        let mut c = Z80Ctc::new();
        c.write(2, 0x80 | 0x40 | 0x04 | 0x01);
        c.write(2, 5);
        c.tick();
        c.write(2, 0x02 | 0x01); // software reset
        for _ in 0..1000 {
            c.tick();
        }
        assert!(!c.irq_pending(), "reset channel must stay silent");
    }
}
