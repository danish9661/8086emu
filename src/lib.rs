//! multi-cpu-emu: 8086 / 8085 / 8051 emulator cores in one crate.
//!
//! Compiles to a single WASM module (feature `wasm`) plus a native rlib.

pub mod cpu;
pub mod i8085;
pub mod i8086;
pub mod mcs51;
pub mod asm;
pub mod pit;
pub mod i8155;

#[cfg(feature = "wasm")]
pub mod wasm;

use cpu::{Cpu, FlagSet, Output, Reg, RunResult};

/// Facade over the three cores. The WASM surface and CLI both use this.
pub enum Emulator {
    I8086(Box<i8086::Cpu8086>),
    I8085(Box<i8085::Cpu8085>),
    Mcs51(Box<mcs51::Cpu8051>),
}

pub fn make_emulator(isa: &str) -> Result<Emulator, String> {
    match isa.to_ascii_uppercase().as_str() {
        "8086" | "X86" => Ok(Emulator::I8086(Box::<i8086::Cpu8086>::default())),
        "8085" => Ok(Emulator::I8085(Box::default())),
        "8051" | "MCS51" | "MCS-51" => Ok(Emulator::Mcs51(Box::<mcs51::Cpu8051>::default())),
        other => Err(format!("unknown ISA '{other}'; expected 8086, 8085 or 8051")),
    }
}

impl Emulator {
    /// Assemble source for this ISA; returns machine code or the first error.
    pub fn assemble(&self, source: &str) -> Result<Vec<u8>, String> {
        let (code, errs, _) = self.assemble_full(source);
        if let Some(e) = errs.first() {
            return Err(format!("line {}: {}", e.line, e.msg));
        }
        Ok(code)
    }

    /// Assemble and also return per-line machine-code info (line, address, bytes).
    pub fn assemble_info(&self, source: &str) -> Result<(Vec<u8>, Vec<asm::LineInfo>), String> {
        let (code, errs, info) = self.assemble_full(source);
        if let Some(e) = errs.first() {
            return Err(format!("line {}: {}", e.line, e.msg));
        }
        Ok((code, info))
    }

    fn assemble_full(&self, source: &str) -> (Vec<u8>, Vec<asm::AsmErr>, Vec<asm::LineInfo>) {
        match self {
            Emulator::I8086(_) => asm::parse_8086(source),
            Emulator::I8085(_) => asm::parse_8085(source),
            Emulator::Mcs51(_) => asm::parse_8051(source),
        }
    }

    pub fn reset(&mut self) {
        self.cpu().reset();
    }

    pub fn step(&mut self) -> bool {
        self.cpu().step()
    }

    pub fn run(&mut self, max_steps: u32) -> RunResult {
        self.cpu().run(max_steps)
    }

    pub fn run_to(&mut self, max_steps: u32, target: u32) -> RunResult {
        self.cpu().run_to(max_steps, target)
    }

    pub fn run_to_bp(&mut self, max_steps: u32, bps: &[u32]) -> RunResult {
        self.cpu().run_to_bp(max_steps, bps)
    }

    pub fn pc(&self) -> u32 {
        self.cpu_ref().pc()
    }

    pub fn set_pc(&mut self, addr: u32) {
        self.cpu().set_pc(addr);
    }

    pub fn regs(&self) -> Vec<Reg> {
        self.cpu_ref().regs()
    }

    pub fn flags(&self) -> FlagSet {
        self.cpu_ref().flags()
    }

    pub fn mem_read(&self, addr: u32, len: usize) -> Vec<u8> {
        self.cpu_ref().mem_read(addr, len)
    }

    /// 8086 text-mode screen: 4000 bytes (80x25 cells of char, attr) from the
    /// 0xB8000 text framebuffer. Empty for the other ISAs.
    pub fn screen(&self) -> Vec<u8> {
        match self {
            Emulator::I8086(c) => c.mem_read(0xB8000, 80 * 25 * 2),
            _ => Vec::new(),
        }
    }

    /// 8086 text-mode cursor (col, row); (0,0) for other ISAs.
    pub fn cursor(&self) -> (u8, u8) {
        match self {
            Emulator::I8086(c) => c.text_cursor(),
            _ => (0, 0),
        }
    }

    pub fn mem_write(&mut self, addr: u32, data: &[u8]) {
        self.cpu().mem_write(addr, data);
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.cpu_ref().snapshot()
    }

    pub fn restore(&mut self, data: &[u8]) {
        self.cpu().restore(data);
    }

    pub fn is_halted(&self) -> bool {
        self.cpu_ref().is_halted()
    }

    pub fn take_output(&mut self) -> String {
        self.output_mut().take()
    }

    /// Raise a hardware interrupt. 8085: kind = "TRAP" | "RST75" | "RST65" |
    /// "RST55" | "INTR" (data = vector). 8051: kind = "INT0" | "INT1".
    pub fn request_interrupt(&mut self, kind: &str, data: u32) -> Result<(), String> {
        match self {
            Emulator::I8085(c) => {
                if kind.eq_ignore_ascii_case("INTR") {
                    c.request_intr(data as u8);
                    Ok(())
                } else {
                    c.request_interrupt(kind)
                }
            }
            Emulator::Mcs51(c) => c.request_interrupt(kind),
            Emulator::I8086(c) => c.request_interrupt(kind, data),
        }
    }

    /// Read an I/O port byte (8085/8086: port space 0-255; 8051: P0-P3 pins
    /// merged with the latch, quasi-bidirectional).
    pub fn port_read(&self, port: u8) -> u8 {
        match self {
            Self::I8085(c) => c.ports[port as usize],
            Self::I8086(c) => c.ports[port as usize],
            Self::Mcs51(c) => c.port_read(port),
        }
    }

    /// Write an I/O port byte (8085/8086: port space; 8051: P0-P3 external
    /// pin state that port reads observe). No print side effects here.
    pub fn port_write(&mut self, port: u8, v: u8) {
        match self {
            Self::I8085(c) => c.ports[port as usize] = v,
            Self::I8086(c) => c.ports[port as usize] = v,
            Self::Mcs51(c) => c.port_write(port, v),
        }
    }

    /// Total clock cycles executed (machine cycles / T-states). Drives the
    /// cycle-accurate timers (8086 PIT, 8051 timers, 8085 8155 timer).
    pub fn cycles(&self) -> u64 {
        match self {
            Self::I8085(c) => c.cycles(),
            Self::I8086(c) => c.cycles(),
            Self::Mcs51(c) => c.cycles(),
        }
    }

    /// Mark `[base, base+len)` of main memory as read-only ROM (8086/8085).
    pub fn set_rom_region(&mut self, base: u32, len: u32) {
        match self {
            Emulator::I8086(c) => c.set_rom_region(base, len),
            Emulator::I8085(c) => c.set_rom_region(base, len),
            _ => {}
        }
    }

    /// Load a ROM image and mark its range read-only. 8051 routes to external
    /// code (XDATA) when EA is low, otherwise to the internal code image.
    pub fn load_rom(&mut self, data: &[u8], addr: u32) {
        match self {
            Emulator::I8086(c) => c.load_rom(data, addr),
            Emulator::I8085(c) => c.load_rom(data, addr),
            Emulator::Mcs51(c) => c.load_rom(data, addr),
        }
    }

    /// 8051 EA pin: false => fetch code from external program memory (XDATA).
    pub fn set_ea(&mut self, ea: bool) {
        if let Emulator::Mcs51(c) = self { c.set_ea(ea); }
    }

    /// 8085: (re)configure the external SRAM chip window (default 8 KiB @ 0x9000).
    pub fn set_sram(&mut self, base: u32, len: u32) {
        if let Emulator::I8085(c) = self { c.set_sram(base, len); }
    }

    /// Current reload/count of an 8086 PIT channel (0..2), for the IDE timer
    /// view. Returns 0 for other ISAs.
    pub fn pit_count(&self, n: usize) -> u16 {
        match self {
            Self::I8086(c) => c.pit_count(n),
            _ => 0,
        }
    }

    /// Preload a file into the 8086 DOS virtual filesystem (name matched
    /// case-insensitively; '@' not required). 8086 only.
    pub fn fs_put(&mut self, name: &str, data: &[u8]) -> Result<(), String> {
        match self {
            Emulator::I8086(c) => { c.fs_put(name, data); Ok(()) }
            _ => Err("fs_put: 8086 only".into()),
        }
    }
    /// Read a file's bytes back from the 8086 DOS virtual filesystem.
    pub fn fs_get(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        match self {
            Emulator::I8086(c) => Ok(c.fs_get(name)),
            _ => Err("fs_get: 8086 only".into()),
        }
    }
    /// Set the emulated DOS/BIOS date-time clock (INT 21h 2Ah/2Ch, INT 1Ah).
    pub fn set_clock(&mut self, year: u16, month: u8, day: u8, hour: u8, min: u8, sec: u8) -> Result<(), String> {
        match self {
            Emulator::I8086(c) => { c.set_clock(year, month, day, hour, min, sec); Ok(()) }
            _ => Err("set_clock: 8086 only".into()),
        }
    }

    /// Inject a received serial byte into the 8051 (sets SBUF + RI).
    pub fn serial_rx(&mut self, ch: u8) -> Result<(), String> {
        match self {
            Emulator::Mcs51(c) => { c.serial_rx(ch); Ok(()) }
            _ => Err("serial_rx: 8051 only".into()),
        }
    }

    /// Set the 8085 SID (Serial Input Data) pin that the RIM instruction reads
    /// (bit 7). 8085 only.
    pub fn set_sid(&mut self, v: bool) {
        if let Emulator::I8085(c) = self {
            c.sid = v;
        }
    }

    /// Read the 8085 SOD (Serial Output Data) pin set by the SIM instruction
    /// (bit 7, returned as 0/1). 8085 only.
    pub fn sod(&self) -> u8 {
        match self {
            Emulator::I8085(c) => c.sod as u8,
            _ => 0,
        }
    }

    /// Read the SFR / internal-RAM byte at the given address (8051 only).
    pub fn sfr(&self, addr: u8) -> u8 {
        match self {
            Emulator::Mcs51(c) => c.sfr_byte(addr),
            _ => 0,
        }
    }

    /// Queue a key for the 8086's INT 21h keyboard reads (AH=01/06/07/08/0C).
    pub fn push_key(&mut self, ch: u8) {
        if let Emulator::I8086(c) = self {
            c.push_key(ch);
        }
    }

    /// True while the 8086 is blocked on an INT 21h read with an empty buffer.
    pub fn waiting_input(&self) -> bool {
        match self {
            Emulator::I8086(c) => c.waiting_input(),
            _ => false,
        }
    }

    fn cpu(&mut self) -> &mut dyn Cpu {
        match self {
            Emulator::I8086(c) => c.as_mut(),
            Emulator::I8085(c) => c.as_mut(),
            Emulator::Mcs51(c) => c.as_mut(),
        }
    }

    fn cpu_ref(&self) -> &dyn Cpu {
        match self {
            Emulator::I8086(c) => c.as_ref(),
            Emulator::I8085(c) => c.as_ref(),
            Emulator::Mcs51(c) => c.as_ref(),
        }
    }

    fn output_mut(&mut self) -> &mut Output {
        match self {
            Emulator::I8086(c) => &mut c.as_mut().out,
            Emulator::I8085(c) => &mut c.as_mut().out,
            Emulator::Mcs51(c) => &mut c.as_mut().out,
        }
    }
}
