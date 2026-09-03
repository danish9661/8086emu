//! Common infrastructure shared by the three CPU cores.

/// 8-bit memory abstraction. The 8086 uses it as a 1 MiB flat image (20-bit
/// addresses resolved by its segment unit); the 8085 and 8051 use it directly.
///
/// A contiguous range may be marked read-only (ROM). CPU store instructions go
/// through `write`/`write16` and are silently ignored inside ROM; the loader
/// and debug poke use `poke`/`poke16`/`load` which bypass the protection.
#[derive(Clone)]
pub struct Mem {
    pub data: Vec<u8>,
    rom_base: usize,
    rom_len: usize,
}

impl Mem {
    pub fn new(size: usize) -> Self {
        Mem { data: vec![0u8; size], rom_base: 0, rom_len: 0 }
    }

    /// Total size of the memory array in bytes.
    pub fn size(&self) -> usize { self.data.len() }

    #[inline]
    pub fn read(&self, addr: usize) -> u8 {
        self.data[addr & (self.data.len() - 1)]
    }

    /// CPU store path: ignored inside the ROM range.
    #[inline]
    pub fn write(&mut self, addr: usize, val: u8) {
        let m = self.data.len() - 1;
        let i = addr & m;
        if self.rom_len > 0 && i >= self.rom_base && i < self.rom_base.wrapping_add(self.rom_len) {
            return;
        }
        self.data[i] = val;
    }

    #[inline]
    pub fn read16(&self, addr: usize) -> u16 {
        self.read(addr) as u16 | ((self.read(addr + 1) as u16) << 8)
    }

    #[inline]
    pub fn write16(&mut self, addr: usize, val: u16) {
        self.write(addr, val as u8);
        self.write(addr + 1, (val >> 8) as u8);
    }

    /// Unprotected store (loader / debug poke).
    #[inline]
    pub fn poke(&mut self, addr: usize, val: u8) {
        let m = self.data.len() - 1;
        self.data[addr & m] = val;
    }

    #[inline]
    pub fn poke16(&mut self, addr: usize, val: u16) {
        self.poke(addr, val as u8);
        self.poke(addr + 1, (val >> 8) as u8);
    }

    /// Load an image, bypassing ROM protection (used by the program/ROM loader).
    pub fn load(&mut self, addr: usize, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.poke(addr + i, *b);
        }
    }

    /// Mark `[base, base+len)` as read-only ROM (address is masked to the size).
    /// The length is clamped to the memory size so a wrap-around range can
    /// never mark unintended bytes read-only.
    pub fn set_rom(&mut self, base: usize, len: usize) {
        let m = self.data.len() - 1;
        self.rom_base = base & m;
        self.rom_len = len.min(self.data.len());
    }

    /// Current used length of the memory image.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Always false: memory is a fixed-size image, never empty.
    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn rom_range(&self) -> (usize, usize) {
        (self.rom_base, self.rom_len)
    }

    /// True if `addr` falls inside the read-only ROM range. CPU store
    /// instructions (`write`) silently ignore writes here, so the bytes at a
    /// ROM address are provably immutable during execution — a decoder may
    /// safely cache (trust) them without re-reading.
    pub fn in_rom(&self, addr: usize) -> bool {
        if self.rom_len == 0 {
            return false;
        }
        let m = self.data.len() - 1;
        let i = addr & m;
        i >= self.rom_base && i < self.rom_base.wrapping_add(self.rom_len)
    }

    pub fn slice(&self, addr: usize, len: usize) -> Vec<u8> {
        (0..len).map(|i| self.read(addr + i)).collect()
    }
}

/// Well-known I/O port numbers shared by the 8086/8085 lab kits.
/// Centralised here so cores, the facade, and the IDE agree on addresses.
pub const PORT_CONSOLE: u8 = 0x01;
pub const PORT_PPI_PA: u8 = 0xE0;
pub const PORT_PPI_PB: u8 = 0xE1;
pub const PORT_PPI_PC: u8 = 0xE2;
pub const PORT_PPI_CTRL: u8 = 0xE3;
pub const PORT_FLASH_STATUS: u8 = 0xE8;
pub const PORT_FLASH_CMD: u8 = 0xE9;
pub const PORT_RTC_SEL: u8 = 0x70;
pub const PORT_RTC_DATA: u8 = 0x71;
pub const PORT_ADC_CTRL: u8 = 0x28;
pub const PORT_ADC_DATA: u8 = 0x29;
pub const PORT_LCD_CMD: u8 = 0x38;
pub const PORT_LCD_DATA: u8 = 0x39;
pub const PORT_USART_DATA: u8 = 0x50;
pub const PORT_USART_CTRL: u8 = 0x51;
pub const PORT_KBD_CMD: u8 = 0x68;
pub const PORT_KBD_DATA: u8 = 0x69;

/// One decoded instruction for the disassembler view.
#[derive(Clone, Debug)]
pub struct Disasm {
    pub addr: u32,
    pub bytes: Vec<u8>,
    pub text: String,
}

impl Disasm {
    /// Render as "ADDR  BYTES  text" for the IDE gutter.
    pub fn line(&self) -> String {
        let hex: String = self.bytes.iter().map(|b| format!("{b:02X}")).collect();
        format!("{:05X}  {:<12} {}", self.addr, hex, self.text)
    }
}

/// All flags each CPU exposes; cores translate their internal flag state into
/// this canonical set so the frontend/UI can be shared.
#[derive(Default, Clone)]
pub struct FlagSet {
    pub carry: bool,
    pub zero: bool,
    pub sign: bool,
    pub parity: bool,
    pub aux: bool,
    pub overflow: bool,
    pub direction: bool,
    pub interrupt: bool,
    pub trap: bool,
}

/// A single captured register for display.
#[derive(Clone)]
pub struct Reg {
    pub name: String,
    pub value: u32,
}

impl Reg {
    pub fn new(name: &str, value: u32) -> Self {
        Reg { name: name.to_string(), value }
    }
}

/// Result of running the CPU for a batch of steps.
#[derive(Default, Clone)]
pub struct RunResult {
    pub steps: u32,
    pub halted: bool,
    pub error: Option<String>,
}

/// Anything the CPU "prints" goes here so both the WASM UI and the CLI can
/// show program output (INT 21h on 8086, OUT on 8085, SBUF on 8051).
/// The buffer is capped (1 MiB) so a print-loop program cannot grow WASM
/// linear memory without bound; oldest output is dropped once capped.
/// Call `take()` regularly to drain it.
#[derive(Default, Clone)]
pub struct Output {
    pub buffer: String,
}

/// Maximum buffered program output (chars). Oldest data is dropped past this.
pub const OUTPUT_CAP: usize = 1 << 20;

impl Output {
    pub fn put_char(&mut self, c: char) {
        if self.buffer.len() >= OUTPUT_CAP {
            // Drop oldest data in chunks to keep per-step cost O(1) amortised.
            let drop = OUTPUT_CAP / 4;
            self.buffer.drain(..drop);
        }
        self.buffer.push(c);
    }
    pub fn put_str(&mut self, s: &str) {
        for c in s.chars() {
            self.put_char(c);
        }
    }
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.buffer)
    }
}

pub trait Cpu {
    fn reset(&mut self);
    /// Execute exactly one instruction. Returns false if the CPU halted.
    fn step(&mut self) -> bool;
    fn pc(&self) -> u32;
    fn set_pc(&mut self, addr: u32);
    /// Set a register by canonical name (e.g. "AX", "PC", "R0", "FLAGS").
    /// Cores implement the names they expose; the default ignores unknown ones.
    fn set_reg(&mut self, name: &str, val: u32) { let _ = (name, val); }
    fn regs(&self) -> Vec<Reg>;
    fn flags(&self) -> FlagSet;
    fn mem_read(&self, addr: u32, len: usize) -> Vec<u8>;
    fn mem_write(&mut self, addr: u32, data: &[u8]);
    /// Drop any cached decoder state. Called after an external store (debugger
    /// / loader poke) so the next step re-decodes, keeping self-modifying code
    /// correct even for ROM-loaded images that trust their decode cache.
    fn invalidate_icache(&mut self) {}
    fn snapshot(&self) -> Vec<u8>;
    fn restore(&mut self, data: &[u8]);
    fn is_halted(&self) -> bool;
    /// Total clock cycles executed (machine cycles for 8051/8085, host cycles
    /// for 8086). Drives the cycle-accurate timers / PIT; 0 if the core does
    /// not model a clock.
    fn cycles(&self) -> u64 { 0 }
    /// True while the CPU is blocked waiting for keyboard input
    /// (8086 INT 21h AH=01/06/07/08/0C with an empty input buffer).
    fn waiting_input(&self) -> bool { false }

    /// Disassemble up to `count` instructions starting at `addr`. Cores provide
    /// an ISA-specific decoder; the default returns an empty list.
    fn disasm(&self, addr: u32, count: usize) -> Vec<Disasm> {
        let _ = (addr, count);
        Vec::new()
    }

    fn run(&mut self, max_steps: u32) -> RunResult {
        let mut r = RunResult::default();
        while r.steps < max_steps && !self.is_halted() {
            if self.waiting_input() {
                break; // blocked on input: caller should push a key and resume
            }
            if !self.step() {
                r.halted = true;
                break;
            }
            r.steps += 1;
        }
        r.halted = self.is_halted();
        r
    }

    /// Run until PC lands on one of `bps` (that instruction is NOT executed),
    /// or halted/blocked on input/max steps. Used by the IDE for breakpoints.
    fn run_to_bp(&mut self, max_steps: u32, bps: &[u32]) -> RunResult {
        let mut r = RunResult::default();
        while r.steps < max_steps && !self.is_halted() {
            if self.waiting_input() || bps.contains(&self.pc()) {
                break;
            }
            if !self.step() {
                r.halted = true;
                break;
            }
            r.steps += 1;
        }
        r.halted = self.is_halted();
        r
    }

    /// Run until `target` becomes the next instruction to execute (target not
    /// executed), or halted/blocked on input/max steps. Used for step-over
    /// (target = return address) and run-to-line in the debugger.
    fn run_to(&mut self, max_steps: u32, target: u32) -> RunResult {
        let mut r = RunResult::default();
        while r.steps < max_steps && !self.is_halted() {
            if self.pc() == target || self.waiting_input() {
                break;
            }
            if !self.step() {
                r.halted = true;
                break;
            }
            r.steps += 1;
        }
        r.halted = self.is_halted();
        r
    }
}
