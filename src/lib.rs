//! multi-cpu-emu: 8086 / 8085 / 8051 emulator cores in one crate.
//!
//! Compiles to a single WASM module (feature `wasm`) plus a native rlib.

pub mod cpu;
pub mod i8085;
pub mod i8086;
pub mod mcs51;
pub mod asm;
pub mod disasm8086;
pub mod disasm8085;
pub mod disasm8051;
pub mod m6502;
pub mod z80;
pub mod rv32;
pub mod pit;
pub mod pic8259;
pub mod i8155;

#[cfg(feature = "wasm")]
pub mod wasm;

use cpu::{Cpu, FlagSet, Output, Reg, RunResult};

/// Facade over the three cores. The WASM surface and CLI both use this.
pub enum Emulator {
    I8086(Box<i8086::Cpu8086>),
    I8085(Box<i8085::Cpu8085>),
    Mcs51(Box<mcs51::Cpu8051>),
    Rv32(Box<rv32::CpuRv32>),
    M6502(Box<m6502::Cpu6502>),
    Z80(Box<z80::CpuZ80>),
}

pub fn make_emulator(isa: &str) -> Result<Emulator, String> {
    match isa.to_ascii_uppercase().as_str() {
        "8086" | "X86" => Ok(Emulator::I8086(Box::<i8086::Cpu8086>::default())),
        "8085" => Ok(Emulator::I8085(Box::default())),
        "8051" | "MCS51" | "MCS-51" => Ok(Emulator::Mcs51(Box::<mcs51::Cpu8051>::default())),
        "RV32" | "RV32I" | "RISC-V" | "RISCV" => Ok(Emulator::Rv32(Box::<rv32::CpuRv32>::default())),
        "6502" | "65C02" | "R6502" | "M6502" | "MOS6502" => Ok(Emulator::M6502(Box::<m6502::Cpu6502>::default())),
        "Z80" | "ZILOG" => Ok(Emulator::Z80(Box::<z80::CpuZ80>::default())),
        other => Err(format!("unknown ISA '{other}'; expected 8086, 8085, 8051, rv32, 6502 or z80")),
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
            Emulator::Rv32(_) => asm::parse_rv32(source),
            Emulator::M6502(_) => asm::parse_6502(source),
            Emulator::Z80(_) => asm::parse_z80(source),
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

    /// Set a register by name (e.g. "AX", "PC", "R0"). Ignored if the ISA
    /// does not expose that register. Used by the IDE watch window.
    pub fn set_reg(&mut self, name: &str, val: u32) {
        self.cpu().set_reg(name, val);
    }

    pub fn regs(&self) -> Vec<Reg> {
        self.cpu_ref().regs()
    }

    pub fn flags(&self) -> FlagSet {
        self.cpu_ref().flags()
    }

    /// 8086 graphics framebuffer (base, width, height) when in a pixel mode,
    /// else None. Used by the IDE to draw the graphics screen canvas.
    pub fn gfx_framebuffer(&self) -> Option<(u32, u32, u32)> {
        match self {
            Emulator::I8086(c) => c.gfx_framebuffer(),
            _ => None,
        }
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

    /// 8086 current video mode number (0 when not 8086).
    pub fn video_mode(&self) -> u8 {
        match self {
            Emulator::I8086(c) => c.video_mode(),
            _ => 0,
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
            Emulator::Rv32(_) => Err("request_interrupt: rv32 has no interrupt model".into()),
            Emulator::M6502(c) => { if kind.eq_ignore_ascii_case("NMI") { c.request_nmi(); } else { c.request_irq(); } Ok(()) }
            Emulator::Z80(c) => { if kind.eq_ignore_ascii_case("NMI") { c.request_nmi(); } else { c.request_int(); } Ok(()) }
        }
    }

    /// Set the Z80 interrupt mode (0/1 -> 0x0038, 2 -> I*0x100 + data).
    pub fn set_interrupt_mode(&mut self, m: u8) -> Result<(), String> {
        match self {
            Emulator::Z80(c) => { c.set_im(m); Ok(()) }
            _ => Err("set_interrupt_mode is only supported for Z80".into()),
        }
    }

    /// Read an I/O port byte (8085/8086: port space 0-255; 8051: P0-P3 pins
    /// merged with the latch, quasi-bidirectional).
    pub fn port_read(&self, port: u8) -> u8 {
        match self {
            Self::I8085(c) => c.ports[port as usize],
            Self::I8086(c) => c.ports[port as usize],
            Self::Mcs51(c) => c.port_read(port),
            Self::Rv32(_) => 0,
            Self::M6502(_) => 0,
            Self::Z80(c) => c.port_read(port),
        }
    }

    /// Write an I/O port byte (8085/8086: port space; 8051: P0-P3 external
    /// pin state that port reads observe). No print side effects here.
    pub fn port_write(&mut self, port: u8, v: u8) {
        match self {
            Self::I8085(c) => c.ports[port as usize] = v,
            Self::I8086(c) => c.ports[port as usize] = v,
            Self::Mcs51(c) => c.port_write(port, v),
            Self::Rv32(_) => {}
            Self::M6502(_) => {}
            Self::Z80(c) => c.port_write(port, v),
        }
    }

    /// Total clock cycles executed (machine cycles / T-states). Drives the
    /// cycle-accurate timers (8086 PIT, 8051 timers, 8085 8155 timer).
    pub fn cycles(&self) -> u64 {
        match self {
            Self::I8085(c) => c.cycles(),
            Self::I8086(c) => c.cycles(),
            Self::Mcs51(c) => c.cycles(),
            Self::Rv32(_) => 0,
            Self::M6502(_) => 0,
            Self::Z80(_) => 0,
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
            Emulator::Rv32(c) => c.load_rom(data, addr),
            Emulator::M6502(c) => c.load_rom(data, addr),
            Emulator::Z80(c) => c.load_rom(data, addr),
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

    /// Write an 8051 SFR / IRAM byte (IDE peripheral-register editor).
    pub fn set_sfr(&mut self, addr: u8, v: u8) {
        if let Emulator::Mcs51(c) = self {
            c.set_sfr_byte(addr, v);
        }
    }

    /// Live memory-map info: write-protected ROM region, if configured.
    pub fn rom_region(&self) -> Option<(u32, u32)> {
        match self {
            Emulator::I8086(c) => {
                let (b, l) = c.mem.rom_range();
                if l > 0 { Some((b as u32, l as u32)) } else { None }
            }
            Emulator::I8085(c) => {
                let (b, l) = c.mem.rom_range();
                if l > 0 { Some((b as u32, l as u32)) } else { None }
            }
            Emulator::Mcs51(_) => None,
            Emulator::Rv32(c) => {
                let (b, l) = c.mem.rom_range();
                if l > 0 { Some((b as u32, l as u32)) } else { None }
            }
            Emulator::M6502(c) => {
                let (b, l) = c.mem.rom_range();
                if l > 0 { Some((b as u32, l as u32)) } else { None }
            }
            Self::Z80(c) => {
                let (b, l) = c.rom_region();
                if l > 0 { Some((b, l)) } else { None }
            }
        }
    }

    /// Live memory-map info: external SRAM window (8085), if configured.
    pub fn sram_region(&self) -> Option<(u32, u32)> {
        match self {
            Emulator::I8085(c) => {
                if c.sram_len > 0 { Some((c.sram_base, c.sram_len)) } else { None }
            }
            _ => None,
        }
    }

    /// 8051 EA pin state (true = internal code, false = external code via XDATA).
    pub fn ea_active(&self) -> bool {
        match self {
            Emulator::Mcs51(c) => c.ea,
            _ => true,
        }
    }

    /// 8051 external-code region (XDATA) bounds when EA is low, if loaded.
    pub fn ext_code_region(&self) -> Option<(u32, u32)> {
        match self {
            Emulator::Mcs51(c) => {
                if !c.ea && c.xdata.size() > 0 {
                    Some((0, c.xdata.size() as u32))
                } else {
                    None
                }
            }
            _ => None,
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

    /// Disassemble instructions from memory. 8086 uses the real decoder; the
    /// Decodes instructions starting at `start` for the active ISA. Works for
    /// 8086, 8085 and 8051.
    pub fn disassemble(&self, start: u32, count: usize) -> Vec<cpu::Disasm> {
        match self {
            Emulator::I8086(c) => disasm8086::disasm(&c.mem, start, count),
            Emulator::I8085(c) => disasm8085::disasm(&c.mem, start, count),
            // 8051 fetches code from internal `code` when EA=1, and from
            // external XDATA (the loaded ROM) when EA=0.
            Emulator::Mcs51(c) => disasm8051::disasm(if c.ea { &c.code } else { &c.xdata }, start, count),
            Emulator::Rv32(c) => c.disasm(start, count),
            Emulator::M6502(c) => c.disasm(start, count),
            Emulator::Z80(c) => c.disasm(start, count),
        }
    }

    fn cpu(&mut self) -> &mut dyn Cpu {
        match self {
            Emulator::I8086(c) => c.as_mut(),
            Emulator::I8085(c) => c.as_mut(),
            Emulator::Mcs51(c) => c.as_mut(),
            Emulator::Rv32(c) => c.as_mut(),
            Emulator::M6502(c) => c.as_mut(),
            Emulator::Z80(c) => c.as_mut(),
        }
    }

    fn cpu_ref(&self) -> &dyn Cpu {
        match self {
            Emulator::I8086(c) => c.as_ref(),
            Emulator::I8085(c) => c.as_ref(),
            Emulator::Mcs51(c) => c.as_ref(),
            Emulator::Rv32(c) => c.as_ref(),
            Emulator::M6502(c) => c.as_ref(),
            Emulator::Z80(c) => c.as_ref(),
        }
    }

    fn output_mut(&mut self) -> &mut Output {
        match self {
            Emulator::I8086(c) => &mut c.as_mut().out,
            Emulator::I8085(c) => &mut c.as_mut().out,
            Emulator::Mcs51(c) => &mut c.as_mut().out,
            Emulator::Rv32(c) => &mut c.as_mut().out,
            Emulator::M6502(c) => &mut c.as_mut().out,
            Emulator::Z80(c) => &mut c.as_mut().out,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(regs: &[Reg], name: &str, val: u32) -> bool {
        regs.iter().any(|r| r.name == name && r.value == val)
    }

    #[test]
    fn set_reg_round_trips() {
        // 8086
        let mut e = make_emulator("8086").unwrap();
        e.set_reg("AX", 0x1234);
        e.set_reg("IP", 0x0100);
        assert!(has(&e.regs(), "AX", 0x1234));
        assert!(has(&e.regs(), "IP", 0x0100));
        e.set_reg("BX", 0x00FF);
        assert!(has(&e.regs(), "BX", 0x00FF));

        // 8085
        let mut e = make_emulator("8085").unwrap();
        e.set_reg("A", 0x42);
        e.set_reg("PC", 0x1234);
        assert!(has(&e.regs(), "A", 0x42));
        assert!(has(&e.regs(), "PC", 0x1234));

        // 8051
        let mut e = make_emulator("8051").unwrap();
        e.set_reg("A", 0x11);
        e.set_reg("R0", 0x55);
        e.set_reg("DPTR", 0x1234);
        assert!(has(&e.regs(), "A", 0x11));
        assert!(has(&e.regs(), "R0", 0x55));
        assert!(has(&e.regs(), "DPTR", 0x1234));
    }
}
