//! multi-cpu-emu: 8086 / 8085 / 8051 emulator cores in one crate.
//!
//! Compiles to a single WASM module (feature `wasm`) plus a native rlib.

pub mod cpu;
pub mod i8085;
pub mod i8086;
pub mod mcs51;
pub mod asm;

#[cfg(feature = "wasm")]
pub mod wasm;

use cpu::{Cpu, FlagSet, Output, Reg, RunResult};

/// Facade over the three cores. The WASM surface and CLI both use this.
pub enum Emulator {
    I8086(Box<i8086::Cpu8086>),
    I8085(i8085::Cpu8085),
    Mcs51(Box<mcs51::Cpu8051>),
}

pub fn make_emulator(isa: &str) -> Result<Emulator, String> {
    match isa.to_ascii_uppercase().as_str() {
        "8086" | "X86" => Ok(Emulator::I8086(Box::<i8086::Cpu8086>::default())),
        "8085" => Ok(Emulator::I8085(i8085::Cpu8085::new())),
        "8051" | "MCS51" | "MCS-51" => Ok(Emulator::Mcs51(Box::<mcs51::Cpu8051>::default())),
        other => Err(format!("unknown ISA '{other}'; expected 8086, 8085 or 8051")),
    }
}

impl Emulator {
    /// Assemble source for this ISA; returns machine code or the first error.
    pub fn assemble(&self, source: &str) -> Result<Vec<u8>, String> {
        let (code, errs) = match self {
            Emulator::I8086(_) => asm::parse_8086(source),
            Emulator::I8085(_) => asm::parse_8085(source),
            Emulator::Mcs51(_) => asm::parse_8051(source),
        };
        if let Some(e) = errs.first() {
            return Err(format!("line {}: {}", e.line, e.msg));
        }
        Ok(code)
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
            _ => Err(format!("interrupts are only supported on the 8085/8051 cores (got '{kind}')")),
        }
    }

    /// Read the SFR / internal-RAM byte at the given address (8051 only).
    pub fn sfr(&self, addr: u8) -> u8 {
        match self {
            Emulator::Mcs51(c) => c.sfr_byte(addr),
            _ => 0,
        }
    }

    fn cpu(&mut self) -> &mut dyn Cpu {
        match self {
            Emulator::I8086(c) => c.as_mut(),
            Emulator::I8085(c) => c,
            Emulator::Mcs51(c) => c.as_mut(),
        }
    }

    fn cpu_ref(&self) -> &dyn Cpu {
        match self {
            Emulator::I8086(c) => c.as_ref(),
            Emulator::I8085(c) => c,
            Emulator::Mcs51(c) => c.as_ref(),
        }
    }

    fn output_mut(&mut self) -> &mut Output {
        match self {
            Emulator::I8086(c) => &mut c.as_mut().out,
            Emulator::I8085(c) => &mut c.out,
            Emulator::Mcs51(c) => &mut c.as_mut().out,
        }
    }
}
