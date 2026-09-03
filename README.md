# multi-cpu-emu

[![Pages](https://github.com/danish9661/8086emu/actions/workflows/pages.yml/badge.svg)](https://github.com/danish9661/8086emu/actions/workflows/pages.yml)
[![Publish](https://github.com/danish9661/8086emu/actions/workflows/publish.yml/badge.svg)](https://github.com/danish9661/8086emu/actions/workflows/publish.yml)
[![npm](https://img.shields.io/npm/v/8086emu.svg)](https://www.npmjs.com/package/8086emu)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A single Rust crate that emulates six classic microprocessors:

- **Intel 8086** — 16-bit, segmented, 1 MiB address space; includes an 8259 PIC
  and 8253 PIT so timer interrupts (IRQ0 → `INT 8`) fire end-to-end
- **Intel 8085** — 8-bit, 64 KiB, accumulator-centric
- **Intel 8051 (MCS-51)** — 8-bit, SFRs, bit-addressable RAM, timers
- **MOS 6502** — 8-bit, decimal mode, NMI/IRQ/BRK vectoring
- **Zilog Z80** — 8-bit, IM 0/1/2, NMI/INT, full 8080 + Z80 ops
- **RISC-V rv32i (+M)** — 32-bit, base integer ISA plus the M-extension

Each core has a matching assembler, and the whole crate compiles to **one WASM
module** (via `wasm-bindgen`, feature `wasm`) plus a native `rlib`/`cdylib`.
A full dependency-free web IDE for students lives in `docs/` and deploys to
GitHub Pages with zero config.

Design was inspired by https://github.com/abuXsarkar/modern8086 (MIT) — used
only as an architecture/scope reference; all code here is written from scratch.
See `AGENTS.md` for the full architecture and per-ISA coverage.

## Build & test

```bash
cargo test                         # 133 tests (13 unit + 120 integration across all 6 ISAs)
cargo clippy --all-targets         # should be warning-free

# wasm build (needs wasm-pack)
wasm-pack build --target web --out-dir docs/pkg --release --features wasm

# self-contained WASM smoke test (exercises all 6 ISAs + new features)
node tools/wasm-smoke.mjs

# serve the web demo
python3 -m http.server -d docs 8000   # then open http://localhost:8000
```

## Web IDE / GitHub Pages

The demo in `docs/` is a student-oriented IDE: ISA selector (8086/8085/8051/6502/Z80/RV32),
sample programs, line-numbered editor with assemble-error highlighting, step /
step-over / run / stop / reset, **click-in-gutter breakpoints** with Step-Back
time-travel, live register + flag panels, a memory dump with the PC highlighted,
a **live memory-map** (showing loaded ROM / external SRAM / 8051 EA state), an
**8051 SFR readout** (click a register to edit it live), and a program-output
console.

Deployment is handled by the workflow in `.github/workflows/pages.yml`: on every
push to `main` it builds the wasm pkg, runs the native tests, and deploys
`docs/` to GitHub Pages.

One-time setup in GitHub: **Settings → Pages → Source: "GitHub Actions"** (the
first workflow run may enable the site automatically). The site then appears at
`https://<user>.github.io/8086emu/`.

Alternative (no workflow): **Settings → Pages → Deploy from a branch → `main`,
folder `/docs`** — works because all asset paths in `docs/` are relative and the
prebuilt `docs/pkg/` is committed. After any Rust change, rebuild and commit it:
`wasm-pack build --target web --out-dir docs/pkg --release --features wasm`.
Root `index.html` redirects to `docs/` for local convenience.

## Quick start

### Run headless from a shell (CLI)

The CLI lives in `examples/run.rs`; it assembles source and runs the program,
printing registers, flags, and output.

```bash
# build once
cargo build --release --example run

# 8086 hello world
cargo run --example run -- examples/hello.asm

# other ISAs, with a step cap
cargo run --example run -- --isa 8051 --max-steps 1000 examples/hello51.asm

# trace every instruction + peripheral (port) write
cargo run --example run -- --isa 8085 --verbose examples/traffic.asm

# automate checks (exit 0 = pass, 1 = fail, 2 = usage error)
cargo run --example run -- --grade tests/spec.txt examples/prog.asm

# measure emulation throughput (native numbers)
cargo run --example run -- --bench            # default 10M steps
cargo run --example run -- --bench 2000000 --isa rv32
```

### Use it in the browser (WASM IDE)

```bash
# serve the demo (from repo root)
python3 -m http.server -d docs 8000
# open http://localhost:8000  (root redirects to /docs/)
```

In the IDE: pick an ISA → write code → `F7` assemble → `F5` run / `F8` step →
set breakpoints in the gutter → inspect registers, memory, and device panels.

**Browser throughput check** (open DevTools console on the IDE page; the
emulator is exposed as `window.emu`):

```js
let t = performance.now();
let s = emu.run(1_000_000);          // steps executed
let ms = performance.now() - t;
console.log(s, 'steps in', ms.toFixed(1), 'ms =>', Math.round(s / (ms/1000)), 'steps/sec');
```

Both the CLI and the browser run the **same Rust core** (native vs WASM), so
bulk `run()` throughput is comparable; only per-instruction single-stepping
from JS is slower because of the JS↔WASM call boundary.


## Examples

| File | ISA | Shows |
|---|---|---|
| `examples/hello.asm` | 8086 | `INT 21h` string output |
| `examples/hello85.asm` | 8085 | `OUT 01h` printing |
| `examples/hello51.asm` | 8051 | `SBUF` serial output |
| `examples/hello6502.asm` | 6502 | `STA $01` printing |
| `examples/helloz80.asm` | Z80 | `OUT (1),A` printing |
| `examples/hellorv32.asm` | rv32 | `ECALL` write/exit semihosting |
| `examples/8155.asm` | 8085 | 8155 external RAM/I/O |
| `examples/timer51.asm` | 8051 | timer + interrupt |
| `examples/ports86.asm` | 8086 | 8255 PPI + ADC0808 + LCD1602 + 8237 DMA via `OUT` |
| `examples/ports85.asm` | 8085 | same kit via `OUT`/`IN` |
| `examples/ports51.asm` | 8051 | same kit via `MOVX` to `FF00h`+port |
| `examples/ser.rs` | 8051 | native serial-RX injection |
| `examples/bios.asm` | 8086 | BIOS image that boots from the reset vector `FFFF:FFF0` |

## Layout

```
├── src/
│   ├── lib.rs          # Emulator facade over all 6 cores
│   ├── cpu.rs          # Cpu trait, Mem, Output, FlagSet, Reg, RunResult
│   ├── i8086.rs        # 8086 CPU core (segmented, INT 21h/10h subset)
│   ├── i8085.rs        # 8085 CPU core (full 8-bit ISA)
│   ├── mcs51.rs        # 8051 CPU core (SFRs, bit ops, timers)
│   ├── m6502.rs        # 6502 CPU core (decimal mode, NMI/IRQ/BRK)
│   ├── z80.rs          # Z80 CPU core (IM 0/1/2, full Z80 ISA)
│   ├── rv32.rs         # RV32I (+M) core (CSR, ECALL semihosting)
│   ├── asm/            # tokenizer + per-ISA assemblers (asm8086/8085/8051/6502/z80/rv32)
│   └── wasm.rs         # wasm-bindgen surface (feature = "wasm")
├── examples/run.rs     # native CLI runner (assemble + run + bench + grade)
├── tests/emulation.rs  # 120 integration tests across all 6 ISAs
├── tools/wasm-smoke.mjs # headless WASM smoke test (all ISAs)
├── docs/               # GitHub Pages IDE (index.html + app.js + style.css + pkg/)
└── index.html          # redirects to docs/
```

## WASM API

```js
const emu = new Emulator("8086");            // "8086" | "8085" | "8051" | "6502" | "z80" | "rv32"
const code = emu.assemble(src);              // throws on error
emu.load(code, 0x100);                       // write code + set PC
emu.set_pc(0x100);                           // (re)set the program counter
emu.run(1_000_000);                          // steps executed
emu.step();  emu.run_to(targetPc, 1_000_000); // step / run-to-line (Step-Over)
emu.pc();  emu.regs();  emu.flags();         // "AX=1234" / "ZF"
emu.mem(0, 64);                              // raw bytes
emu.out();                                   // program output (drains)
emu.halted();  emu.reset();
emu.snapshot();  emu.restore(bytes);         // deterministic time-travel

// External memory (write-protected ROM / external SRAM / 8051 EA):
emu.set_rom_region(0xF0000, 0x10000);        // mark ROM range
emu.load_rom(bytes, 0xF0000);                // place a firmware image
emu.set_ea(false);                           // 8051: fetch code from XDATA
emu.set_sram(0x9000, 0x2000);                // 8085: (re)map external SRAM
emu.rom_region();  emu.sram_region();        // live memory-map info
emu.ea_active();  emu.ext_code_region();

// 8051 peripheral registers:
emu.sfr(0xD0);  emu.set_sfr(0xD0, 0x00);     // read/write an SFR
```

## Program output conventions

- **8086** — `INT 21h` (AH=02, 06, 09, 4Ch) and `INT 10h` (AH=0Eh) write to the
  output buffer.
- **8085** — `OUT 01h` prints the char in A.
- **8051** — writing to `SBUF` prints the char.
- **6502/Z80** — memory-mapped or port-mapped I/O via `OUT`/`STA` (see `examples/` and `docs/doc.html`).
- **RV32** — `ECALL` semihosting (a7=64 write, a7=93 exit).

## Releases

Releases are manual via **Actions → Publish to npm & GitHub Packages → Run workflow**:
branch + version (`x.y` or `x.y.z`) + release notes → publishes `8086emu` to
[npm](https://www.npmjs.com/package/8086emu) and `@danish9661/8086emu` to
GitHub Packages, then creates a GitHub Release + `v<version>` tag.
Requires repo secret `NPM_TOKEN`. See `CHANGELOG.md`.