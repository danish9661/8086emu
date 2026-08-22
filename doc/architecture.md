# Architecture

The crate is a single Rust library (`multi-cpu-emu`) that compiles to native and
to one WebAssembly module. It is organized as a **facade over three independent
CPU cores** plus a shared assembler and a shared wasm glue layer.

## Component map

```
src/
├── lib.rs          facade: Emulator enum (I8086 / I8085 / Mcs51) over the cores
├── cpu.rs          Cpu trait, Mem, Output, FlagSet, Reg, Disasm, RunResult
├── i8086.rs        8086 core (segmented, 1 MiB, INT 21h/10h subset, PIC/PIT)
├── i8085.rs        8085 core (flat 64 KiB, accumulator-centric, SIM/RIM)
├── mcs51.rs        8051 core (SFRs, bit-addressable RAM, timers, serial)
├── asm/
│   ├── mod.rs      parse(source) -> (Vec<u8>, Vec<AsmError>, Vec<LineInfo>)
│   ├── asm8086.rs  emu8086-dialect assembler
│   ├── asm8085.rs  8085 assembler
│   └── asm8051.rs  8051 assembler
├── disasm8086.rs   8086 disassembler (new)
├── disasm8085.rs   8085 disassembler (new)
├── disasm8051.rs   8051 disassembler (new)
└── wasm.rs         wasm-bindgen surface (feature = "wasm")
```

## Core design

Every core implements the `Cpu` trait (`src/cpu.rs`):

- `step()` runs exactly one instruction and returns `false` if the CPU halted.
- `Mem` is a power-of-two `Vec<u8>`; addresses are masked with `len - 1`. All
  core sizes are powers of two (1 MiB, 64 KiB, 64 KiB).
- Each core owns an `Output` buffer that collects printed characters so the CLI
  and the WASM UI display program output uniformly.
- `FlagSet` is the canonical flag representation; each core translates its
  internal flags into it for `flags()`.
- `snapshot()` / `restore()` serialize and reload the full CPU state
  deterministically — this powers the IDE's time-travel **Step-Back**.
- `disasm(addr, count)` returns decoded instructions; a default implementation
  returns an empty list, and each core reaches it via the free `disasm*`
  functions selected in `Emulator::disassemble`.

## Facade

`Emulator` is an enum over `Box<dyn Cpu>`. WASM and native code both talk to the
single `Emulator` type, so the frontend/UI is ISA-agnostic. Adding a fourth ISA
means: a new core, a new assembler module, a new disassembler module, and one
more arm in `Emulator`'s match statements.

## Determinism

Same input → same execution. `snapshot`/`restore` round-trips. There is no
`unsafe` outside what `wasm-bindgen` requires, and no external emulation
dependencies — the cores are written from scratch.

## Assembler

Line-oriented, case-insensitive, `;` comments, labels (`name:` or `name EQU`),
directives `ORG` / `DB` / `DW` / `EQU` / `END`. Numbers support decimal,
`0x`, `h`/`H`, `b`/`B`, `q`/`Q`, and `'char'`. `ORG` emits a complete memory
image (forward `ORG` pads with zeros, backward is an error). Load at address 0;
entry is 0 for 8085/8051 and 0x100 for 8086 (`ORG 100h`).
