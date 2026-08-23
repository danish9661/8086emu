//! RISC-V RV32I integer core (base ISA: RV32I).
//!
//! 32 general registers x0..x31 (x0 is hardwired zero), 32-bit PC, flat
//! little-endian address space (1 MiB). Only the integer base ISA is modeled
//! (no M/A/F/C extensions). `ECALL` implements a tiny semihosting ABI so
//! programs can print and exit: a7 = syscall number (64 = write fd/a1/a2,
//! 93 = exit), matching the Linux RISC-V convention.

use crate::cpu::{Cpu, Mem, Output, FlagSet, Reg, Disasm, RunResult};

#[derive(Clone, Copy)]
struct RvDec {
    opcode: u8,
    f3: u8,
    f7: u8,
    rd: usize,
    rs1: usize,
    rs2: usize,
    imm_i: u32,
}

#[derive(Clone)]
pub struct CpuRv32 {
    pub mem: Mem,
    pub x: [u32; 32],
    pub pc: u32,
    pub halt: bool,
    pub out: Output,
    pub halted_reason: Option<String>,
    /// Decode cache: `(pc, insn, decoded_fields)`. On a hit the raw bytes are
    /// *trusted* (the 4-byte `fetch` is skipped) when `pc` is in ROM (ROM writes
    /// are silently ignored, so the bytes are immutable during execution); for
    /// writable code the cached bytes are re-read and compared, so self-modifying
    /// code stays correct. Invalidated on `reset`/`restore`.
    dec: Option<(u32, u32, RvDec)>,
    /// Control/status registers (full 4 KiB space), plain storage.
    pub csr: [u32; 4096],
}

impl Default for CpuRv32 {
    fn default() -> Self {
        let mut c = CpuRv32 {
            mem: Mem::new(1 << 20),
            x: [0u32; 32],
            pc: 0,
            halt: false,
            out: Output::default(),
            halted_reason: None,
            dec: None,
            csr: [0u32; 4096],
        };
        c.reset();
        c
    }
}

impl CpuRv32 {
    fn rd(&self, i: usize) -> u32 {
        if i == 0 { 0 } else { self.x[i] }
    }
    /// Load a ROM image at `addr` (also marks that range read-only).
    pub fn load_rom(&mut self, data: &[u8], addr: u32) {
        self.mem.load(addr as usize, data);
        self.mem.set_rom(addr as usize, data.len());
    }
    fn wr(&mut self, i: usize, v: u32) {
        if i != 0 {
            self.x[i] = v;
        }
    }
    /// Read a 32-bit little-endian instruction at `a` *without* advancing `pc`.
    fn peek32(&self, a: u32) -> u32 {
        let a = a as usize;
        self.mem.read(a) as u32
            | (self.mem.read(a + 1) as u32) << 8
            | (self.mem.read(a + 2) as u32) << 16
            | (self.mem.read(a + 3) as u32) << 24
    }
    /// Split an instruction word into its base decode fields.
    fn fields(insn: u32) -> RvDec {
        RvDec {
            opcode: (insn & 0x7f) as u8,
            rd: ((insn >> 7) & 0x1f) as usize,
            f3: ((insn >> 12) & 0x7) as u8,
            rs1: ((insn >> 15) & 0x1f) as usize,
            rs2: ((insn >> 20) & 0x1f) as usize,
            f7: ((insn >> 25) & 0x7f) as u8,
            imm_i: ((insn as i32) >> 20) as u32,
        }
    }
    /// Fetch + decode at `addr`, using the decode cache. Returns the raw
    /// instruction word and its decoded fields. Advances are the caller's job
    /// (the cache must not move `pc`), so `step` sets the default next-PC itself.
    fn fetch_decode(&mut self, addr: u32) -> (u32, RvDec) {
        if let Some((cpc, cinsn, cd)) = self.dec {
            if cpc == addr {
                if self.mem.in_rom(addr as usize) {
                    return (cinsn, cd); // immutable: trust, skip the 4-byte fetch
                }
                let cur = self.peek32(addr);
                if cur == cinsn {
                    return (cinsn, cd); // writable but unchanged: reuse fields
                }
                let d = Self::fields(cur);
                self.dec = Some((addr, cur, d));
                return (cur, d);
            }
        }
        let cur = self.peek32(addr);
        let d = Self::fields(cur);
        self.dec = Some((addr, cur, d));
        (cur, d)
    }
    fn lb(&self, a: u32) -> u32 {
        (self.mem.read(a as usize) as i8) as i32 as u32
    }
    fn lh(&self, a: u32) -> u32 {
        (self.mem.read16(a as usize) as i16) as i32 as u32
    }
    fn lw(&self, a: u32) -> u32 {
        self.mem.read(a as usize) as u32
            | (self.mem.read(a as usize + 1) as u32) << 8
            | (self.mem.read(a as usize + 2) as u32) << 16
            | (self.mem.read(a as usize + 3) as u32) << 24
    }

    /// Decode one instruction into a human-readable string for the disassembler.
    pub fn decode(&self, insn: u32, addr: u32) -> String {
        let opcode = insn & 0x7f;
        let rd = ((insn >> 7) & 0x1f) as usize;
        let f3 = (insn >> 12) & 0x7;
        let rs1 = ((insn >> 15) & 0x1f) as usize;
        let rs2 = ((insn >> 20) & 0x1f) as usize;
        let f7 = (insn >> 25) & 0x7f;
        let imm_i = ((insn as i32) >> 20) as u32;
        let r = |i: usize| format!("x{i}");
        match opcode {
            0x37 => format!("lui     {},0x{:x}", r(rd), insn & 0xfffff000),
            0x17 => format!("auipc   {},0x{:x}", r(rd), insn & 0xfffff000),
            0x6f => format!("jal     {},{}", r(rd), rel_j(insn).wrapping_add(addr.wrapping_add(4))),
            0x67 => format!("jalr    {},{}({})", r(rd), imm_i as i32, r(rs1)),
            0x63 => {
                let tgt = rel_b(insn).wrapping_add(addr.wrapping_add(4));
                let nm = match f3 {
                    0 => "beq", 1 => "bne", 4 => "blt",
                    5 => "bge", 6 => "bltu", 7 => "bgeu", _ => "branch?",
                };
                format!("{nm}    {},{},{}", r(rs1), r(rs2), tgt)
            }
            0x03 => {
                let nm = match f3 { 0 => "lb", 1 => "lh", 2 => "lw", 4 => "lbu", 5 => "lhu", _ => "load?" };
                format!("{nm}     {},{}({})", r(rd), imm_i as i32, r(rs1))
            }
            0x23 => {
                let imm = rel_s(insn);
                let nm = match f3 { 0 => "sb", 1 => "sh", 2 => "sw", _ => "store?" };
                format!("{nm}     {},{}({})", r(rs2), imm as i32, r(rs1))
            }
            0x13 => {
                if f3 == 1 { format!("slli    {},{},{}", r(rd), r(rs1), (imm_i & 0x1f)) }
                else if f3 == 5 {
                    let sh = imm_i & 0x1f;
                    if f7 == 0x20 { format!("srai    {},{},{}", r(rd), r(rs1), sh) }
                    else { format!("srli    {},{},{}", r(rd), r(rs1), sh) }
                } else {
                    let (nm, val) = match f3 {
                        0 => ("addi", imm_i as i32),
                        2 => ("slti", imm_i as i32),
                        3 => ("sltiu", imm_i as i32),
                        4 => ("xori", imm_i as i32),
                        6 => ("ori", imm_i as i32),
                        7 => ("andi", imm_i as i32),
                        _ => ("opimm?", 0),
                    };
                    format!("{nm}    {},{},{}", r(rd), r(rs1), val)
                }
            }
            0x33 => {
                let sub = f7 == 0x20;
                let t = if f7 == 0x01 {
                    match f3 {
                        0 => "mul", 1 => "mulh", 2 => "mulhsu", 3 => "mulhu",
                        4 => "div", 5 => "divu", 6 => "rem", 7 => "remu", _ => "op?",
                    }
                } else {
                    match f3 {
                        0 => if sub { "sub" } else { "add" },
                        1 => "sll",
                        2 => "slt",
                        3 => "sltu",
                        4 => "xor",
                        5 => if sub { "sra" } else { "srl" },
                        6 => "or",
                        7 => "and",
                        _ => "op?",
                    }
                };
                format!("{t}     {},{},{}", r(rd), r(rs1), r(rs2))
            }
            0x0f => "fence".to_string(),
            0x73 => {
                if insn == 0x00000073 { "ecall".to_string() }
                else if insn == 0x00100073 { "ebreak".to_string() }
                else {
                    let csr = (insn >> 20) & 0xfff;
                    format!("csrrw  {},{},{}", r(rd), csr, r(rs1))
                }
            }
            _ => format!(".word 0x{insn:08x}"),
        }
    }
}

fn rel_b(insn: u32) -> u32 {
    let imm12 = (insn >> 31) & 1;
    let imm10_5 = (insn >> 25) & 0x3f;
    let imm4_1 = (insn >> 8) & 0xf;
    let imm11 = (insn >> 7) & 1;
    let imm = (imm12 << 12) | (imm11 << 11) | (imm10_5 << 5) | (imm4_1 << 1);
    (imm as i32).wrapping_sub(if imm12 != 0 { (1i32 << 13) } else { 0 }) as u32
}

fn rel_j(insn: u32) -> u32 {
    let imm20 = (insn >> 31) & 1;
    let imm10_1 = (insn >> 21) & 0x3ff;
    let imm11 = (insn >> 20) & 1;
    let imm19_12 = (insn >> 12) & 0xff;
    let imm = (imm20 << 20) | (imm19_12 << 12) | (imm11 << 11) | (imm10_1 << 1);
    (imm as i32).wrapping_sub(if imm20 != 0 { (1i32 << 21) } else { 0 }) as u32
}

fn rel_s(insn: u32) -> u32 {
    let imm11_5 = (insn >> 25) & 0x7f;
    let imm4_0 = (insn >> 7) & 0x1f;
    let imm = (imm11_5 << 5) | imm4_0;
    (imm as i32).wrapping_sub(if imm11_5 & 0x40 != 0 { (1i32 << 12) } else { 0 }) as u32
}

impl Cpu for CpuRv32 {
    fn reset(&mut self) {
        self.x = [0u32; 32];
        self.pc = 0;
        self.halt = false;
        self.halted_reason = None;
        self.out = Output::default();
        self.dec = None;
        self.csr = [0u32; 4096];
    }

    fn step(&mut self) -> bool {
        if self.halt {
            return false;
        }
        let addr = self.pc;
        // Default next-PC is addr+4 (what `fetch32` used to advance); branches,
        // jumps, and `ecall`/`ebreak`/halt override below.
        self.pc = addr.wrapping_add(4);
        let (insn, d) = self.fetch_decode(addr);
        if insn == 0 {
            self.halt = true;
            self.halted_reason = Some("zero instruction".into());
            return false;
        }
        let opcode = d.opcode;
        let rd = d.rd;
        let f3 = d.f3;
        let rs1 = d.rs1;
        let rs2 = d.rs2;
        let f7 = d.f7;
        let imm_i = d.imm_i;

        match opcode {
            0x37 => self.wr(rd, insn & 0xfffff000),
            0x17 => self.wr(rd, addr.wrapping_add(insn & 0xfffff000)),
            0x6f => {
                let t = addr.wrapping_add(4);
                self.pc = addr.wrapping_add(4).wrapping_add(rel_j(insn));
                self.wr(rd, t);
            }
            0x67 => {
                let t = addr.wrapping_add(4);
                let target = (self.rd(rs1).wrapping_add(imm_i)) & !1u32;
                self.wr(rd, t);
                self.pc = target;
            }
            0x63 => {
                let a = self.rd(rs1);
                let b = self.rd(rs2);
                let take = match f3 {
                    0 => a == b,
                    1 => a != b,
                    4 => (a as i32) < (b as i32),
                    5 => (a as i32) >= (b as i32),
                    6 => a < b,
                    7 => a >= b,
                    _ => false,
                };
                if take {
                    self.pc = addr.wrapping_add(4).wrapping_add(rel_b(insn));
                }
            }
            0x03 => {
                let a = self.rd(rs1).wrapping_add(imm_i);
                let v = match f3 {
                    0 => self.lb(a),
                    1 => self.lh(a),
                    2 => self.lw(a),
                    4 => self.mem.read(a as usize) as u32,
                    5 => self.mem.read16(a as usize) as u32,
                    _ => 0,
                };
                self.wr(rd, v);
            }
            0x23 => {
                let a = self.rd(rs1).wrapping_add(rel_s(insn));
                let v = self.rd(rs2);
                match f3 {
                    0 => self.mem.write(a as usize, v as u8),
                    1 => self.mem.write16(a as usize, v as u16),
                    2 => {
                        self.mem.write(a as usize, v as u8);
                        self.mem.write(a as usize + 1, (v >> 8) as u8);
                        self.mem.write(a as usize + 2, (v >> 16) as u8);
                        self.mem.write(a as usize + 3, (v >> 24) as u8);
                    }
                    _ => {}
                }
            }
            0x13 => {
                let a = self.rd(rs1);
                let v = match f3 {
                    0 => a.wrapping_add(imm_i),
                    1 => a << (imm_i & 0x1f),
                    2 => if (a as i32) < (imm_i as i32) { 1 } else { 0 },
                    3 => if a < imm_i { 1 } else { 0 },
                    4 => a ^ imm_i,
                    5 => {
                        if f7 == 0x20 { ((a as i32) >> (imm_i & 0x1f)) as u32 }
                        else { a >> (imm_i & 0x1f) }
                    }
                    6 => a | imm_i,
                    7 => a & imm_i,
                    _ => 0,
                };
                self.wr(rd, v);
            }
            0x33 => {
                let a = self.rd(rs1);
                let b = self.rd(rs2);
                let v = if f7 == 0x01 {
                    // M-extension: multiply / divide / remainder
                    match f3 {
                        0 => (a as u64 * b as u64) as u32,
                        1 => (((a as i64 as i128) * (b as i64 as i128)) >> 32) as u32,
                        2 => (((a as i64 as i128) * (b as u64 as i128)) >> 32) as u32,
                        3 => (((a as u64 as u128) * (b as u64 as u128)) >> 32) as u32,
                        4 => if b == 0 { 0xFFFF_FFFF } else { (a as i32).wrapping_div(b as i32) as u32 },
                        5 => if b == 0 { 0xFFFF_FFFF } else { a.wrapping_div(b) },
                        6 => if b == 0 { a } else { (a as i32).wrapping_rem(b as i32) as u32 },
                        7 => if b == 0 { a } else { a.wrapping_rem(b) },
                        _ => 0,
                    }
                } else {
                    match f3 {
                        0 => if f7 == 0x20 { a.wrapping_sub(b) } else { a.wrapping_add(b) },
                        1 => a << (b & 0x1f),
                        2 => if (a as i32) < (b as i32) { 1 } else { 0 },
                        3 => if a < b { 1 } else { 0 },
                        4 => a ^ b,
                        5 => {
                            if f7 == 0x20 { ((a as i32) >> (b & 0x1f)) as u32 }
                            else { a >> (b & 0x1f) }
                        }
                        6 => a | b,
                        7 => a & b,
                        _ => 0,
                    }
                };
                self.wr(rd, v);
            }
            0x0f => {} // fence: no-op
            0x73 => {
                let f3 = (insn >> 12) & 0x7;
                if insn == 0x00000073 {
                    // ECALL: tiny semihosting ABI
                    let num = self.x[17]; // a7
                    match num {
                        64 => {
                            // write: a0=fd, a1=buf, a2=len
                            let ptr = self.x[11];
                            let len = self.x[12];
                            for i in 0..len {
                                self.out.put_char(self.mem.read(ptr.wrapping_add(i) as usize) as char);
                            }
                        }
                        93 => {
                            self.halt = true;
                            self.halted_reason = Some("ecall exit".into());
                        }
                        _ => {}
                    }
                } else if insn == 0x00100073 {
                    self.halt = true;
                    self.halted_reason = Some("ebreak".into());
                } else if f3 != 0 {
                    // CSR instructions (CSRRW/CSRRS/CSRRC and immediate forms)
                    let rd = ((insn >> 7) & 0x1f) as usize;
                    let rs1 = ((insn >> 15) & 0x1f) as usize;
                    let csr = ((insn >> 20) & 0xfff) as usize;
                    let old = self.csr[csr];
                    match f3 {
                        1 => { self.csr[csr] = self.x[rs1]; }
                        2 => { if rs1 != 0 { self.csr[csr] |= self.x[rs1]; } }
                        3 => { if rs1 != 0 { self.csr[csr] &= !self.x[rs1]; } }
                        5 => { self.csr[csr] = rs1 as u32; }
                        6 => { if rs1 != 0 { self.csr[csr] |= rs1 as u32; } }
                        7 => { if rs1 != 0 { self.csr[csr] &= !(rs1 as u32); } }
                        _ => {}
                    }
                    if rd != 0 { self.x[rd] = old; }
                }
                // f3 == 0 but not ECALL/EBREAK => other privileged ops; ignored
            }
            _ => {
                self.halt = true;
                self.halted_reason = Some(format!("unknown opcode {opcode:#x} at {addr:#x}"));
                return false;
            }
        }
        true
    }

    fn pc(&self) -> u32 { self.pc }
    fn set_pc(&mut self, addr: u32) { self.pc = addr; }
    fn set_reg(&mut self, name: &str, val: u32) {
        let n = name.to_ascii_lowercase();
        if n == "pc" { self.pc = val; return; }
        let idx = if let Some(r) = n.strip_prefix('x').and_then(|s| s.parse::<usize>().ok()) {
            Some(r)
        } else {
            rvreg_index(&n)
        };
        if let Some(i) = idx {
            self.wr(i, val);
        }
    }
    fn regs(&self) -> Vec<Reg> {
        let mut v = Vec::with_capacity(33);
        for i in 0..32 {
            v.push(Reg::new(&format!("x{i}"), self.x[i]));
        }
        v.push(Reg::new("pc", self.pc));
        v
    }
    fn flags(&self) -> FlagSet { FlagSet::default() }
    fn mem_read(&self, addr: u32, len: usize) -> Vec<u8> {
        (0..len).map(|i| self.mem.read(addr as usize + i)).collect()
    }
    fn mem_write(&mut self, addr: u32, data: &[u8]) {
        // External write (loader / debugger poke): the code that was just
        // written may land on a cached PC, so drop the decode cache.
        self.dec = None;
        for (i, b) in data.iter().enumerate() {
            self.mem.write(addr as usize + i, *b);
        }
    }
    fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(4 + 128 + 1 + self.mem.size());
        v.extend_from_slice(&self.pc.to_le_bytes());
        for r in &self.x {
            v.extend_from_slice(&r.to_le_bytes());
        }
        v.push(if self.halt { 1 } else { 0 });
        v.extend_from_slice(&self.mem.data);
        for c in &self.csr {
            v.extend_from_slice(&c.to_le_bytes());
        }
        v
    }
    fn restore(&mut self, data: &[u8]) {
        let mut o = 0;
        self.dec = None; // memory/pc changed; force re-decode
        let get4 = |d: &[u8], p: &mut usize| {
            let v = u32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            v
        };
        self.pc = get4(data, &mut o);
        for r in &mut self.x {
            *r = get4(data, &mut o);
        }
        self.halt = data[o] != 0;
        o += 1;
        for b in &mut self.mem.data {
            *b = data[o];
            o += 1;
        }
        for c in &mut self.csr {
            *c = get4(data, &mut o);
        }
    }
    fn is_halted(&self) -> bool { self.halt }

    fn disasm(&self, addr: u32, count: usize) -> Vec<Disasm> {
        let mut out = Vec::new();
        let mut a = addr;
        for _ in 0..count {
            let b = self.lw(a);
            let text = self.decode(b, a);
            out.push(Disasm {
                addr: a,
                bytes: b.to_le_bytes().to_vec(),
                text,
            });
            a = a.wrapping_add(4);
        }
        out
    }
}

/// Map an ABI register name (a0, t0, sp, ...) to its x-number.
fn rvreg_index(name: &str) -> Option<usize> {
    let abi: &[(&str, usize)] = &[
        ("zero", 0), ("ra", 1), ("sp", 2), ("gp", 3), ("tp", 4),
        ("t0", 5), ("t1", 6), ("t2", 7), ("s0", 8), ("fp", 8), ("s1", 9),
        ("a0", 10), ("a1", 11), ("a2", 12), ("a3", 13), ("a4", 14), ("a5", 15), ("a6", 16), ("a7", 17),
        ("s2", 18), ("s3", 19), ("s4", 20), ("s5", 21), ("s6", 22), ("s7", 23),
        ("s8", 24), ("s9", 25), ("s10", 26), ("s11", 27),
        ("t3", 28), ("t4", 29), ("t5", 30), ("t6", 31),
    ];
    abi.iter().find(|(n, _)| *n == name).map(|(_, i)| *i)
}
