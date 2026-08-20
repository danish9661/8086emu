# AGENTS.md — multi-cpu-emu (8086 / 8085 / 8051 emulator)

Guidance for AI agents and contributors working in this repository.

## Project overview

A single Rust crate (`multi-cpu-emu`) that emulates three classic microprocessors:

- **Intel 8086** — 16-bit, segmented, 1 MiB address space
- **Intel 8085** — 8-bit, 64 KiB, accumulator-centric
- **Intel 8051 (MCS-51)** — 8-bit, SFRs, bit-addressable RAM, timers

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
  TEST, XCHG, LEA, shifts/rotates (D0–D3), CBW/CWD, MOVS/LODS/STOS/CMPS/SCAS
  (byte+word, with REP prefixes), LAHF/SAHF, flag ops (CLC/STC/CMC/CLI/STI/
  CLD/STD), Jcc/JMP (short/near/far)/CALL/RET/RETF, LOOP/LOOPZ/LOOPNZ/JCXZ,
  INT n/INT3/INTO/IRET, NOP, HLT.
- DOS/BIOS service subset: INT 21h (AH=01, 02, 09, 4Ch), INT 10h (AH=0Eh).
  Output goes to the `Output` buffer.

### 8085 (`i8085.rs`)
- Registers: A, B, C, D, E, H, L, SP, PC; flags S/Z/AC/P/CY.
- Full 8-bit ISA: MOV/MVI/LXI/LDA/STA/LDAX/STAX/LHLD/SHLD/XCHG, ADD/ADC/SUB/
  SBB/ANA/XRA/ORA/CMP (+immediate ADI/ACI/SUI/SBI/ANI/XRI/ORI/CPI), INR/DCR/
  INX/DCX/DAD, RLC/RRC/RAL/RAR/CMA/CMC/STC/DAA, JMP/Jcc/CALL/Ccc/RET/Rcc/RST,
  PUSH/POP (regs + PSW), XTHL/SPHL/PCHL, IN/OUT, EI/DI, SIM/RIM, NOP, HLT.
- OUT to port 01h prints the char in A to `Output` (documented convention).
- Hardware interrupts (raised via `Cpu8085::request_interrupt(kind)` /
  `Emulator::request_interrupt(kind, data)` / wasm `interrupt()`): TRAP
  (vector 0x24, non-maskable, keeps IE), RST 7.5/6.5/5.5 (0x3C/0x34/0x2C,
  maskable via SIM, clear IE), INTR (external vector, clear IE). Priority:
  TRAP > 7.5 > 6.5 > 5.5 > INTR. An ISR pushes PSW then PC; 5.5/6.5 pending
  flags are latched and cleared on service (simplification). SIM (A: D0-D2
  masks 5.5/6.5/7.5, D3=MSE, D4=reset RST 7.5 latch, D7=SOD) and RIM
  (A: D7=SID, D6-D4 pending 7.5/6.5/5.5, D3=IE, D2-D0 masks) match the chip.
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
  calibration); writing SBUF emits a char to `Output`.

## Assembler design (src/asm)

- Line-oriented, case-insensitive, `;` comments, labels (`name:` or `name EQU
  expr`), directives: `ORG`, `DB`, `DW`, `EQU`, `END`.
- Numbers: decimal, `0x` hex, `h`/`H` hex suffix, `b`/`B` binary, `q`/`Q` octal,
  `'char'`. Simple label arithmetic (+/-) between labels is supported.
- `ORG` emits a complete memory image: forward `ORG` pads with zeros (use it
  to place ISRs at hardware vectors, e.g. `ORG 24h`), backward `ORG` is an
  error. Load the assembled image at address 0 (`load(code, 0)`); the entry
  point is 0 for 8085/8051 and 0x100 for 8086 (matching `ORG 100h`).
- Per-ISA parser produces `(Vec<u8>, Vec<AsmError{line, msg}>)`. For the 8086
  a two-pass approach resolves forward label references.
- Mnemonic coverage mirrors the CPU cores above (assembling what the core can
  execute). Errors must report line numbers and human-readable messages.

## WASM API (src/wasm.rs, feature "wasm")

Exposed class `Emulator`:

```
new(isa: "8086" | "8085" | "8051") -> Emulator
assemble(source: &str) -> Vec<u8>        // machine code (error via exception/result)
load(code: &[u8], origin: u32)           // place code in memory
step()                                   // one instruction
run(max_steps: u32) -> u32               // steps executed
pc() -> u32
regs() -> Vec<String>                    // e.g. "AX=1234"
flags() -> Vec<String>                   // e.g. "ZF"
mem(addr: u32, len: u32) -> Vec<u8>      // linear read
out() -> String                          // take program output
halted() -> bool
reset()
snapshot() -> Vec<u8>
restore(data: &[u8])
interrupt(kind: &str, data: u32)         // 8085 only: TRAP|RST75|RST65|RST55|INTR
```

## Conventions

- No `unsafe` except where `wasm-bindgen` requires it; keep core logic `safe`.
- No external crates for the cores (no emulation dependencies); `wasm-bindgen`
  only behind the `wasm` feature.
- Deterministic: same input → same execution; snapshot/restore must round-trip.
- Tests live in `tests/` (integration, per ISA) and `#[cfg(test)]` modules.
- Run `cargo test` and `cargo clippy --all-targets` before finishing a task.
- Keep the web demo dependency-free (vanilla JS, no bundler).
