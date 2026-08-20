//! Common infrastructure shared by the three CPU cores.

/// 8-bit memory abstraction. The 8086 uses it as a 1 MiB flat image (20-bit
/// addresses resolved by its segment unit); the 8085 and 8051 use it directly.
#[derive(Clone)]
pub struct Mem {
    pub data: Vec<u8>,
}

impl Mem {
    pub fn new(size: usize) -> Self {
        Mem { data: vec![0u8; size] }
    }

    #[inline]
    pub fn read(&self, addr: usize) -> u8 {
        self.data[addr & (self.data.len() - 1)]
    }

    #[inline]
    pub fn write(&mut self, addr: usize, val: u8) {
        let len = self.data.len();
        self.data[addr & (len - 1)] = val;
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

    pub fn load(&mut self, addr: usize, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.write(addr + i, *b);
        }
    }

    pub fn slice(&self, addr: usize, len: usize) -> Vec<u8> {
        (0..len).map(|i| self.read(addr + i)).collect()
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
#[derive(Default, Clone)]
pub struct Output {
    pub buffer: String,
}

impl Output {
    pub fn put_char(&mut self, c: char) {
        self.buffer.push(c);
    }
    pub fn put_str(&mut self, s: &str) {
        self.buffer.push_str(s);
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
    fn regs(&self) -> Vec<Reg>;
    fn flags(&self) -> FlagSet;
    fn mem_read(&self, addr: u32, len: usize) -> Vec<u8>;
    fn mem_write(&mut self, addr: u32, data: &[u8]);
    fn snapshot(&self) -> Vec<u8>;
    fn restore(&mut self, data: &[u8]);
    fn is_halted(&self) -> bool;
    /// True while the CPU is blocked waiting for keyboard input
    /// (8086 INT 21h AH=01/06/07/08/0C with an empty input buffer).
    fn waiting_input(&self) -> bool { false }

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
