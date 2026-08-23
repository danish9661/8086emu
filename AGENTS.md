# AGENTS.md — multi-cpu-emu (8086 / 8085 / 8051 / 6502 / Z80 / rv32i emulator)

Guidance for AI agents and contributors working in this repository.

## Project overview

A single Rust crate (`multi-cpu-emu`) that emulates six classic microprocessors:

- **Intel 8086** — 16-bit, segmented, 1 MiB address space
- **Intel 8085** — 8-bit, 64 KiB, accumulator-centric
- **Intel 8051 (MCS-51)** — 8-bit, SFRs, bit-addressable RAM, timers
- **MOS 6502** — 8-bit, 64 KiB, zero-page + stack, decimal mode
- **Zilog Z80** — 8-bit, 64 KiB, full 8080 + Z80 instruction set, IX/IY, R/I
- **RISC-V rv32i** — 32-bit, 1 MiB flat space, base integer ISA (+ M ext, CSR)

The crate compiles to **one WASM module** (via `wasm-bindgen`) plus native
`rlib`/`cdylib`. A full web IDE (`docs/`) consumes the WASM build. There is
also a small native CLI example (`examples/run.rs`) for headless testing.

Design reference (do NOT copy code; it is only a reference for architecture,
feature scope, and UX conventions):

- https://github.com/abuXsarkar/modern8086 — MIT-licensed 8086 (+8085 sibling)
  emulator/IDE. Look at its `packages/core`, `packages/assembler`,
  `packages/wasm-api` layout, its `ARCHITECTURE.md`, `BUILD_PLAN.md`, and
  `ROADMAP.md` for scope ideas. Everything here is written from scratch.

## Reference repo details

| Item | Value |
|---|---|
| URL | https://github.com/abuXsarkar/modern8086 |
| License | MIT |
| Live site | https://modern8086.com (8086) / https://modern8086.com/8085 (8085 sibling) |
| Rust core | `packages/core` — 8086 CPU (Rust → wasm), deterministic snapshot/restore |
| Assembler | `packages/assembler` — emu8086 dialect |
| WASM glue | `packages/wasm-api` — wasm-bindgen surface |
| Devices | `packages/devices` — traffic light, stepper, 7-seg, LED matrix, text screen, keyboard, printer, robot grid |
| Tests | `cargo test --workspace` (219 tests across Rust crates) |
| CLI | `m86` / `m85` (crates `packages/cli`, `packages/cli-8085`) |
| Docs | `ARCHITECTURE.md`, `BUILD_PLAN.md`, `ROADMAP.md`, `docs/emu8086-compatibility.md` |

Useful takeaways to mirror (in our own implementation):

1. One Rust core per ISA, compiled with `wasm-pack build --target web`.
2. Snapshot/restore for a "time-travel" debugger — our `Cpu::snapshot` /
   `Cpu::restore` exists for this.
3. Shared frontend (web UI) with per-ISA core swap. We expose one JS class
   `Emulator` that takes the ISA name, mirroring the same idea.

## Layout

```
.
├── Cargo.toml          # single crate; feature "wasm" enables wasm-bindgen
├── src/
│   ├── lib.rs          # facade: Emulator enum over the three cores
│   ├── cpu.rs          # Cpu trait, Mem, Output, FlagSet, Reg, RunResult
│   ├── i8086.rs        # 8086 CPU core (segmented, INT 21h/10h subset)
│   ├── i8085.rs        # 8085 CPU core (full 8-bit ISA)
│   ├── mcs51.rs        # 8051 CPU core (SFRs, bit ops, timers)
│   ├── asm/
│   │   ├── mod.rs      # asm entry: parse(source) -> (Vec<u8>, Vec<AsmErr>)
│   │   ├── common.rs   # tokenizer, labels, ORG/DB/DW/EQU/END, number parsing
│   │   ├── asm8086.rs  # 8086 mnemonics -> machine code
│   │   ├── asm8085.rs  # 8085 mnemonics -> machine code
│   │   └── asm8051.rs  # 8051 mnemonics -> machine code
│   └── wasm.rs         # wasm-bindgen surface (feature = "wasm")
├── examples/run.rs     # native CLI runner: assemble + run a file, print regs
 ├── tests/              # integration tests (hello world, arithmetic, flags)
 ├── docs/               # GitHub Pages demo (index.html + app.js + style.css + pkg/)
 └── AGENTS.md
 ```

## Build & test commands

```bash
# native test suite (all three cores)
cargo test

# wasm build
rustup target add wasm32-unknown-unknown
cargo install wasm-pack            # once
wasm-pack build --target web --out-dir docs/pkg --release --features wasm

# run a program headlessly (native)
cargo run --example run -- examples/hello.asm        # 8086
cargo run --example run -- --isa 8085 examples/hello85.asm
cargo run --example run -- --isa 8051 examples/hello51.asm

# serve the web demo
python3 -m http.server -d docs 8000   # then open http://localhost:8000
```

## GitHub Pages deployment

A workflow at `.github/workflows/pages.yml` handles deployment: on every push
to `main` it builds the wasm pkg (wasm-pack), runs `cargo test --test
emulation`, and deploys `docs/` via the Pages API. One-time repo setup:
**Settings → Pages → Source: "GitHub Actions"** (the first workflow run may
enable the site automatically). The site appears at
`https://<user>.github.io/8086emu/`.

Alternative (no workflow): **Settings → Pages → Source: "Deploy from a branch"**
with folder `/docs` — works because all asset paths in `docs/` are relative and
the prebuilt `docs/pkg/` is committed, so Pages needs no build step. After any
Rust change, rebuild and commit the pkg:
`wasm-pack build --target web --out-dir docs/pkg --release --features wasm`.
Root `index.html` redirects to `docs/` for local convenience.

## Core design (src/cpu.rs)

```rust
pub trait Cpu {
    fn reset(&mut self);
    fn step(&mut self) -> bool;          // false => CPU halted (HLT)
    fn pc(&self) -> u32;
    fn regs(&self) -> Vec<Reg>;          // display registers (name, u32)
    fn flags(&self) -> FlagSet;          // canonical flags for the UI
    fn mem_read(&self, addr: u32, len: usize) -> Vec<u8>;
    fn mem_write(&mut self, addr: u32, data: &[u8]);
    fn snapshot(&self) -> Vec<u8>;       // deterministic serialization
    fn restore(&mut self, data: &[u8]);
    fn is_halted(&self) -> bool;
    fn run(&mut self, max_steps: u32) -> RunResult;  // default impl
    fn run_to(&mut self, max_steps: u32, target: u32) -> RunResult;  // stop when
        target is the next instruction (target not executed); used by the IDE
        for Step-Over (target = return address) and run-to-line
}
```

- `Mem` is a power-of-two `Vec<u8>`; addresses are masked with
  `data.len() - 1`. All core sizes are powers of two (1 MiB, 64 KiB, 64 KiB).
- Every core owns an `Output` (string buffer) that collects printed characters
  so both the CLI and the WASM UI can display program output uniformly.
- `FlagSet` is the canonical flag representation; each core translates its
  internal flags into it for `flags()`.

## Core coverage (what must be implemented per ISA)

### 8086 (`i8086.rs`)
- Registers: AX/BX/CX/DX (AH/AL …), SI/DI/BP/SP, CS/DS/ES/SS, IP, FLAGS.
- Memory: 1 MiB flat; `ea = seg<<4 + off`.
- Instruction subset (mainline + lab programs): MOV (all forms incl.
  segment regs), PUSH/POP (reg, r/m, imm, seg), ADD/ADC/SUB/SBB/AND/OR/XOR/
  CMP (modrm + imm + accumulator forms), INC/DEC, NEG/NOT, MUL/IMUL/DIV/IDIV,
  TEST, XCHG, LEA, shifts/rotates (D0–D3), CBW/CWD, BCD/ASCII adjust
  (DAA/DAS/AAA/AAS/AAM/AAD; AAM with base 0 faults like divide-by-zero),
  MOVS/LODS/STOS/CMPS/SCAS
  (byte+word, with REP prefixes), LAHF/SAHF, flag ops (CLC/STC/CMC/CLI/STI/
  CLD/STD), Jcc/JMP (short/near/far)/CALL/RET/RETF, LOOP/LOOPZ/LOOPNZ/JCXZ,
  PUSHA/POPA (push/pop AX/CX/DX/BX/SP/BP/SI/DI; POPA discards the saved SP),
  INT n/INT3/INTO/IRET, NOP, HLT.
- DOS/BIOS service subset: INT 21h (AH=01, 02, 06, 07, 08, 09, 0C, 4Ch),
  INT 10h (AH=0Eh). Output goes to the `Output` buffer.
- Hardware interrupts (raised via `Cpu8086::request_interrupt` /
  `Emulator::request_interrupt` / wasm `interrupt()`): NMI (vector 02h,
  non-maskable) and INTR (maskable via IF, device-supplied vector).
  Latched, serviced at the end of `step()` (never while halted), priority
  NMI > INTR; service pushes FLAGS/CS/IP, clears IF+TF, jumps through the
  IVT (vector n at address 4n). Snapshot/restore covers the pending state.
  TF is a real flag: POPF restores it and, while set, every executed
  instruction traps into vector 1 (INT 1, single-step) — the trap fires
  after each instruction as long as TF is still set (IRET restores it, so
  a trapped program keeps single-stepping). `FlagSet.trap` exposes it.
- I/O ports: IN/OUT (imm8 and DX forms) over a 256-byte port space;
  OUT to port 01h also prints AL to `Output` (8085-style convention).
- Keyboard input: `Emulator::push_key(ch)` / wasm `push_key()` queue
  type-ahead characters; INT 21h AH=01 (echo)/06/07/08/0C pop the next char.
  With an empty buffer the CPU blocks: `waiting_input()` is true, IP is
  re-pointed at the INT 21h instruction, and `run()` stops early so the
  caller can push a key and resume. Snapshot/restore covers the buffer.

### 8085 (`i8085.rs`)
- Registers: A, B, C, D, E, H, L, SP, PC; flags S/Z/AC/P/CY.
- Full 8-bit ISA: MOV/MVI/LXI/LDA/STA/LDAX/STAX/LHLD/SHLD/XCHG, ADD/ADC/SUB/
  SBB/ANA/XRA/ORA/CMP (+immediate ADI/ACI/SUI/SBI/ANI/XRI/ORI/CPI), INR/DCR/
  INX/DCX/DAD, RLC/RRC/RAL/RAR/CMA/CMC/STC/DAA, JMP/Jcc/CALL/Ccc/RET/Rcc/RST,
  PUSH/POP (regs + PSW), XTHL/SPHL/PCHL, IN/OUT, EI/DI, SIM/RIM, NOP, HLT.
- OUT to port 01h prints the char in A to `Output` (documented convention);
  IN/OUT read/write a 256-byte port space (`Cpu8085::ports`).
- Hardware interrupts (raised via `Cpu8085::request_interrupt(kind)` /
  `Emulator::request_interrupt(kind, data)` / wasm `interrupt()`): TRAP
  (vector 0x24, non-maskable, keeps IE), RST 7.5/6.5/5.5 (0x3C/0x34/0x2C,
  maskable via SIM, clear IE), INTR (external vector, clear IE). Priority:
  TRAP > 7.5 > 6.5 > 5.5 > INTR. An ISR pushes PSW then PC; 5.5/6.5 pending
  flags are latched and cleared on service (simplification). SIM (A: D0-D2
  masks 5.5/6.5/7.5, D3=MSE, D4=reset RST 7.5 latch, D7=SOD) and RIM
  (A: D7=SID, D6-D4 pending 7.5/6.5/5.5, D3=IE, D2-D0 masks) match the chip.
  The SID input pin is injectable via `Emulator::set_sid` / wasm `set_sid(ch)`;
  the SOD output pin is readable via `Emulator::sod` / wasm `sod()`.
  Interrupts are serviced at the end of `step()` (so they take effect right
  after EI) and never while halted. Snapshot/restore covers all interrupt
  state.

### 8051 (`mcs51.rs`)
- Registers: A, B, R0–R7 (4 register banks), DPTR, PC, PSW, SP.
- Memory: 64 KiB code, 128 B internal RAM + SFRs, bit-addressable 0x20–0x2F,
  XDATA (MOVX).
- Full data-movement (MOV/MOVC/MOVX/PUSH/POP/XCH/XCHD/SWAP), arithmetic
  (ADD/ADDC/SUBB/INC/DEC/MUL/DIV/DA), logical (ANL/ORL/XRL/CLR/CPL/RL/RR/RLC/
  RRC), bit ops (SETB/CLR/CPL/ANL C/ORL C/MOV C), branches (SJMP/AJMP/LJMP/JZ/
  JNZ/JC/JNC/JB/JNB/JBC/CJNE/DJNZ/ACALL/LCALL/RET/RETI), NOP.
- SFRs: P0–P3, PSW, ACC, B, SP, DPL/DPH, TCON, TMOD, TH0/TL0/TH1/TL1, SCON,
  SBUF, IE, IP. Timer 0/1 count on each step while TRx=1 (no real-time
  calibration); writing SBUF emits a char to `Output` and sets TI
  (transmit-complete); `Emulator::serial_rx(ch)` / wasm `serial_rx(ch)`
  injects a received byte (SBUF + RI) so the serial ISR (vector 23h) fires
  when ES is enabled.
- Port model: P0–P3 SFRs are the port latches; `Emulator::port_write(port,
  0-3, v)` injects external pin state and a port read returns `latch | pin`
  (quasi-bidirectional). Bit ops on port bits observe the same merged value.
- Interrupts (checked at the end of `step()`, never while halted):
  INT0/INT1 (external, raised via `Emulator::request_interrupt("INT0|INT1")`
  / wasm `interrupt()`), TF0/TF1 (timers), serial (RI|TI). Vectors
  03h/0Bh/13h/1Bh/23h, scanned in natural priority order; per-source
  priority from IP (PX0/PT0/PX1/PT1/PS). A source vectors only if EA + its
  IE bit are set and no equal-or-higher priority ISR is in service (two
  in-service latches, low/high). The ISR gets PCL then PCH pushed (real
  8051 stack layout); hardware clears IE0/IE1/TF0/TF1, serial RI/TI are
  software-cleared (a serial ISR that forgets `CLR TI` re-fires). `RETI`
  clears the in-service latch; RET/ACALL/LCALL push/pop PCL-first/PCH-first.
  INT0/INT1 level-triggered mode (ITx=0, set via `CLR ITx`) is honored: the
  external line is treated as held low, so the interrupt re-asserts after the
  ISR returns (until released); edge mode (ITx=1) latches on `request_interrupt`
  and clears on service like the real chip.

### 6502 (`m6502.rs`)
- Registers: A, X, Y, SP, PC, P (N/V/B/D/I/Z/C). Memory: 64 KiB (`Mem`),
  256-byte zero page, 256-byte stack page at 0x0100.
- Full 8-bit ISA: LDA/STA/LDX/LDY/LDX/LDY, LDX/LDY, TAX/TAY/TSX/TXS/TXS/TYA,
  ADC/SBC (with decimal mode when D set), CMP/CPX/CPY, AND/ORA/EOR/BIT,
  ASL/LSR/ROL/ROR, INC/DEC, INX/INY/DEX/DEY, JMP (abs + indirect), JSR/RTS,
  branches (BCC/BCS/BEQ/BNE/BMI/BPL/BVC/BVS), PHP/PLP/PHA/PLA, CLC/SEC/CLI/SEI
  /CLV/CLD/SED, BRK, NOP. Zero-page and indexed addressing fully supported.
- `BRK` vectors through 0xFFFE/0xFFFF; `RTI` restores the pushed status.
- IRQ/NMI raised via `Emulator::request_interrupt` / wasm `interrupt()`.
- Output: programs print via a monitor convention; see `tests/` for examples.

### Z80 (`z80.rs`)
- Registers: A, B, C, D, E, H, L, A'/B'/C'/D'/E'/H'/L'/F' (shadow), IX, IY,
  SP, PC, I, R; flags S/Z/H/P/V/N/C.
- Memory: 64 KiB. Full 8-bit ISA: LD r,r / LD r,(HL) / LD (HL),r / LD r,n,
  LD rr,nn, LD (rr),A, LD A,(rr), LD (nn),A, LD A,(nn), LD rr,(nn),
  LD (nn),rr, PUSH/POP rr (incl. AF/IX/IY), EX/EXX, LDI/LDIR/LDD/LDDR,
  CPI/CPIR/CPD/CPDR, ADD/ADC/SUB/SBC/AND/OR/XOR/CP A,r/(HL)/n, INC/DEC
  r/(HL)/rr, RLCA/RRCA/RLA/RRA/RLC/RRC/RL/RR/SLA/SRA/SRL/BIT/RES/SET,
  DAA/CPL/CCF/SCF, ADD/ADC/SBC HL,rr (incl. SP), NEG, RLCA..., RLD/RRD,
  JP/JR/DJNZ/CALL/RET/RET cc/CALL cc/RST n (n = 0,8,…,56),
  IN r,(C) / OUT (C),r / IN A,(n) / OUT (n),A, INI/OUTI/IND/OUTD and blocks,
  EI/DI, IM 0/1/2, HALT.
- Port model: 256-byte I/O space (`CpuZ80::ports`); `OUT (C),r` writes the
  register to port (BC), `IN r,(C)` reads port (BC) — both via `out_port` /
  `in_port` (so `Emulator::port_read/write` observe them). `OUT (n),A` writes
  A to port ((A<<8)|n); `IN A,(n)` reads it back.
- Interrupts: maskable (IM 0/1/2) and NMI; `Emulator::request_interrupt` /
  wasm `interrupt()`. `RETI`/`RETN` restore IFF state.
- Assembler (`asmz80.rs`) covers the above; `ORG` places code (forward ORG
  pads with zeros), `RST n` assembles to `0xC7 | (n & 0x38)`.

### rv32i (`rv32.rs`)
- Registers: x0–x31 (x0 hardwired zero), PC (32-bit), `csr[0..4096]` (CSR file,
  plain storage). Flat little-endian 1 MiB address space.
- Base ISA (RV32I) + M extension: LUI/AUIPC, JAL/JALR, branches, loads/stores
  (LB/LH/LW/LBU/LHU, SB/SH/SW), ADDI/SLTI*/XORI/ORI/ANDI/SLLI/SRLI/SRAI,
  ADD/SUB/SLL/SLT*/XOR/OR/AND/SRL/SRA, MUL/DIV/REM (signed + unsigned).
- CSR instructions (opcode 0x73, funct3≠0): CSRRW/CSRRS/CSRRC and immediate
  forms CSRRWI/CSRRSI/CSRRCI (assembler also accepts shorthand CSRWI/CSRSI/
  CSRCI), plus pseudos `CSRR rd, csr` (read) and `CSRW csr, rs` (write).
  Reads return the old value into `rd`; immediate forms take a 5-bit zimm.
  CSRs are modeled as plain storage (privileged side effects such as mstatus
  interrupt masking and mtvec redirection are NOT modeled).
- `ECALL` implements a tiny semihosting ABI (a7 = syscall; 64 = write
  fd/a1/a2, 93 = exit); `EBREAK` halts. Assembler (`asmrv32.rs`) covers all
  of the above.

## Assembler design (src/asm)

- Line-oriented, case-insensitive, `;` comments, labels (`name:` or `name EQU
  expr`), directives: `ORG`, `DB`, `DW`, `EQU`, `END`.
- Numbers: decimal, `0x` hex, `h`/`H` hex suffix, `b`/`B` binary, `q`/`Q` octal,
  `'char'`. Simple label arithmetic (+/-) between labels is supported.
- `ORG` emits a complete memory image: forward `ORG` pads with zeros (use it
  to place ISRs at hardware vectors, e.g. `ORG 24h`), backward `ORG` is an
  error. Load the assembled image at address 0 (`load(code, 0)`); the entry
  point is 0 for 8085/8051 and 0x100 for 8086 (matching `ORG 100h`).
- Per-ISA parser produces `(Vec<u8>, Vec<AsmError{line, msg}>, Vec<LineInfo{line,
  addr, bytes}>)` — `LineInfo` gives per-line machine code for the IDE gutter
  (wasm `assemble_info()` returns one "ADDR  BYTES" string per source line).
  For the 8086 a multi-pass approach resolves forward label references.
- Mnemonic coverage mirrors the CPU cores above (assembling what the core can
  execute). Errors must report line numbers and human-readable messages.

## WASM API (src/wasm.rs, feature "wasm")

Exposed class `Emulator`:

```
new(isa: "8086" | "8085" | "8051" | "6502" | "z80" | "rv32") -> Emulator
assemble(source: &str) -> Vec<u8>        // machine code (error via exception/result)
assemble_info(source: &str) -> Vec<String> // per-line "ADDR  BYTES" strings (IDE gutter)
load(code: &[u8], origin: u32)           // place code in memory
step()                                   // one instruction
run(max_steps: u32) -> u32               // steps executed
run_to(target_pc: u32, max_steps: u32) -> u32 // steps until target is next (IDE Step-Over/run-to-line)
pc() -> u32
regs() -> Vec<String>                    // e.g. "AX=1234"
flags() -> Vec<String>                   // e.g. "ZF"
mem(addr: u32, len: u32) -> Vec<u8>      // linear read
out() -> String                          // take program output
halted() -> bool
waiting_input() -> bool                  // 8086 blocked on INT 21h read
push_key(ch: u8)                         // 8086 keyboard input (type-ahead)
reset()
snapshot() -> Vec<u8>
restore(data: &[u8])
port_read(port: u8) -> u8             // 8085/8086 port space; 8051 P0-P3 (latch|pin)
port_write(port: u8, val: u8)        // 8085/8086 port space; 8051 pin injection
serial_rx(ch: u8)                     // 8051: inject received byte (SBUF + RI)
set_sid(ch: bool)                      // 8085: inject SID input pin (read by RIM bit 7)
sod() -> u8                             // 8085: read SOD output pin (set by SIM bit 7)
interrupt(kind: &str, data: u32)     // 8085: TRAP|RST75|RST65|RST55|INTR; 8051: INT0|INT1; 8086: NMI|INTR(data=vector)
```

## Conventions

- No `unsafe` except where `wasm-bindgen` requires it; keep core logic `safe`.
- No external crates for the cores (no emulation dependencies); `wasm-bindgen`
  only behind the `wasm` feature.
- Deterministic: same input → same execution; snapshot/restore must round-trip.
- Tests live in `tests/` (integration, per ISA) and `#[cfg(test)]` modules.
- Run `cargo test` and `cargo clippy --all-targets` before finishing a task.
- Keep the web demo dependency-free (vanilla JS, no bundler).
