//! wasm-bindgen surface (feature = "wasm").
//!
//! Exposes a single `Emulator` class that swaps between the 8086/8085/8051
//! cores, mirroring the JS-facing API documented in AGENTS.md.

use crate::{cpu::RunResult, Emulator as Core};
use wasm_bindgen::prelude::*;

fn to_js<T>(r: Result<T, String>) -> Result<T, JsValue> {
    r.map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub struct Emulator {
    inner: Core,
}

#[wasm_bindgen]
impl Emulator {
    #[wasm_bindgen(constructor)]
    pub fn new(isa: &str) -> Result<Emulator, JsValue> {
        to_js(crate::make_emulator(isa).map(|inner| Emulator { inner }))
    }

    /// Assemble source for the current ISA; returns machine code bytes.
    pub fn assemble(&self, source: &str) -> Result<Vec<u8>, JsValue> {
        to_js(self.inner.assemble(source))
    }

    /// Assemble and return per-line machine code as "ADDR  BYTES" strings
    /// (one per source line, empty for lines that emit nothing).
    pub fn assemble_info(&self, source: &str) -> Result<Vec<String>, JsValue> {
        let (_, info) = self.inner.assemble_info(source).map_err(|e| JsValue::from_str(&e))?;
        let n = source.lines().count();
        let mut out: Vec<String> = vec![String::new(); n];
        for li in info {
            if li.bytes.is_empty() { continue; }
            let hex: Vec<String> = li.bytes.iter().map(|b| format!("{b:02X}")).collect();
            out[li.line as usize - 1] = format!("{:04X}  {}", li.addr, hex.join(" "));
        }
        Ok(out)
    }

    /// Load raw machine code at `origin` and set PC there.
    pub fn load(&mut self, code: &[u8], origin: u32) {
        self.inner.mem_write(origin, code);
        self.inner.set_pc(origin);
    }

    /// Execute one instruction.
    pub fn step(&mut self) {
        self.inner.step();
    }

    /// Set the program counter (entry point after load).
    pub fn set_pc(&mut self, addr: u32) {
        self.inner.set_pc(addr);
    }

    /// Run up to `max_steps` instructions; returns steps executed.
    pub fn run(&mut self, max_steps: u32) -> u32 {
        let r: RunResult = self.inner.run(max_steps);
        r.steps
    }

    /// Run until `target` is the next instruction to execute (not executed),
    /// or halt / blocked on input / max steps. Returns steps executed.
    pub fn run_to(&mut self, target_pc: u32, max_steps: u32) -> u32 {
        let r: RunResult = self.inner.run_to(max_steps, target_pc);
        r.steps
    }

    /// Run until PC lands on one of `bps` (that instruction is NOT executed),
    /// or halt / blocked on input / max steps. Returns steps executed.
    pub fn run_bp(&mut self, max_steps: u32, bps: Vec<u32>) -> u32 {
        let r: RunResult = self.inner.run_to_bp(max_steps, &bps);
        r.steps
    }

    pub fn pc(&self) -> u32 {
        self.inner.pc()
    }

    pub fn regs(&self) -> Vec<String> {
        self.inner
            .regs()
            .iter()
            .map(|r| format!("{}={:04X}", r.name, r.value & 0xFFFF))
            .collect()
    }

    pub fn flags(&self) -> Vec<String> {
        let f = self.inner.flags();
        let mut v = Vec::new();
        if f.carry { v.push("CF".to_string()); }
        if f.zero { v.push("ZF".to_string()); }
        if f.sign { v.push("SF".to_string()); }
        if f.parity { v.push("PF".to_string()); }
        if f.aux { v.push("AF".to_string()); }
        if f.overflow { v.push("OF".to_string()); }
        if f.direction { v.push("DF".to_string()); }
        if f.interrupt { v.push("IF".to_string()); }
        if f.trap { v.push("TF".to_string()); }
        v
    }

    pub fn mem(&self, addr: u32, len: u32) -> Vec<u8> {
        self.inner.mem_read(addr, len as usize)
    }

    /// Write bytes into memory (IDE memory poking).
    pub fn mem_write(&mut self, addr: u32, data: &[u8]) {
        self.inner.mem_write(addr, data);
    }

    /// Mark `[base, base+len)` of main memory as read-only ROM (8086/8085).
    pub fn set_rom_region(&mut self, base: u32, len: u32) {
        self.inner.set_rom_region(base, len);
    }

    /// Load a ROM image and mark its range read-only. 8051 routes to external
    /// code (XDATA) when EA is low.
    pub fn load_rom(&mut self, data: &[u8], addr: u32) {
        self.inner.load_rom(data, addr);
    }

    /// 8051 EA pin: false => fetch code from external program memory (XDATA).
    pub fn set_ea(&mut self, ea: bool) {
        self.inner.set_ea(ea);
    }

    /// 8085: (re)configure the external SRAM chip window (default 8 KiB @ 0x9000).
    pub fn set_sram(&mut self, base: u32, len: u32) {
        self.inner.set_sram(base, len);
    }

    /// Drain the program output buffer.
    pub fn out(&mut self) -> String {
        self.inner.take_output()
    }

    pub fn halted(&self) -> bool {
        self.inner.is_halted()
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Hardware interrupt: 8085 = "TRAP" | "RST75" | "RST65" | "RST55" |
    /// "INTR" (data = vector); 8051 = "INT0" | "INT1". Throws on unknown kind.
    pub fn interrupt(&mut self, kind: &str, data: u32) -> Result<(), JsValue> {
        to_js(self.inner.request_interrupt(kind, data))
    }

    /// Queue a key for the 8086's INT 21h keyboard reads (AH=01/06/07/08/0C).
    pub fn port_read(&self, port: u8) -> u8 {
        self.inner.port_read(port)
    }

    /// Write an I/O port byte (8085/8086: port space 0-255; 8051: P0-P3 pins).
    pub fn port_write(&mut self, port: u8, val: u8) {
        self.inner.port_write(port, val)
    }

    /// Total clock cycles executed (machine cycles / T-states). Drives the
    /// cycle-accurate timers (8086 PIT, 8051 timers, 8085 8155 timer).
    pub fn cycles(&self) -> u64 {
        self.inner.cycles()
    }

    /// Current reload/count of an 8086 PIT channel (0..2). Other ISAs: 0.
    pub fn pit_count(&self, n: usize) -> u16 {
        self.inner.pit_count(n)
    }

    /// Inject a received serial byte into the 8051 (sets SBUF + RI).
    pub fn serial_rx(&mut self, ch: u8) -> Result<(), JsValue> {
        to_js(self.inner.serial_rx(ch))
    }

    /// Set the 8085 SID (Serial Input Data) pin read by RIM (bit 7). 8085 only.
    pub fn set_sid(&mut self, v: bool) {
        self.inner.set_sid(v);
    }

    /// Read the 8085 SOD (Serial Output Data) pin set by SIM (bit 7). 8085 only.
    pub fn sod(&self) -> u8 {
        self.inner.sod()
    }

    pub fn push_key(&mut self, ch: u8) {
        self.inner.push_key(ch);
    }

    /// True while the 8086 is blocked on an INT 21h read with an empty buffer.
    pub fn waiting_input(&self) -> bool {
        self.inner.waiting_input()
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.inner.snapshot()
    }

    pub fn restore(&mut self, data: &[u8]) {
        self.inner.restore(data);
    }

    /// Preload a file into the 8086 DOS virtual filesystem.
    pub fn fs_put(&mut self, name: &str, data: &[u8]) -> Result<(), JsValue> {
        to_js(self.inner.fs_put(name, data))
    }
    /// Read a file back from the 8086 DOS virtual filesystem (empty if absent).
    pub fn fs_get(&self, name: &str) -> Result<Option<Vec<u8>>, JsValue> {
        to_js(self.inner.fs_get(name))
    }
    /// Set the emulated DOS/BIOS date-time clock (INT 21h 2Ah/2Ch, INT 1Ah).
    pub fn set_clock(&mut self, year: u16, month: u8, day: u8, hour: u8, min: u8, sec: u8) -> Result<(), JsValue> {
        to_js(self.inner.set_clock(year, month, day, hour, min, sec))
    }

    /// 8086 text-mode screen: 4000 bytes (80x25 cells of char, attr). Other ISAs
    /// return an empty vector.
    pub fn screen(&self) -> Vec<u8> {
        self.inner.screen()
    }
    /// 8086 text-mode cursor as a 2-byte vector [col, row]. Other ISAs: [0, 0].
    pub fn cursor(&self) -> Vec<u8> {
        let (c, r) = self.inner.cursor();
        vec![c, r]
    }
}