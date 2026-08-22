# Getting Started

`multi-cpu-emu` emulates three classic microprocessors in a single Rust crate
that also compiles to WebAssembly: **Intel 8086**, **Intel 8085**, and
**Intel 8051 (MCS-51)**.

## Prerequisites

- Rust (stable) + `cargo`
- For the web build: `wasm-pack` and the `wasm32-unknown-unknown` target
- For the web demo: any static file server (e.g. `python3`)

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## 1. Run the native test suite

```bash
cargo test
```

This exercises all three cores plus the disassemblers and the wasm smoke test.

## 2. Run a program headlessly (native CLI)

```bash
# 8086
cargo run --example run -- examples/hello.asm
# 8085
cargo run --example run -- --isa 8085 examples/hello85.asm
# 8051
cargo run --example run -- --isa 8051 examples/hello51.asm
```

The example runner assembles the source, loads it, runs to `HLT`, and prints
the final register state and any program output.

## 3. Build the WebAssembly module

```bash
wasm-pack build --target web --out-dir docs/pkg --release --features wasm
```

This produces `docs/pkg/` (the `.js` glue + `.wasm` binary) that the web IDE
imports. After any Rust change, **rebuild and commit `docs/pkg/`** so the
GitHub Pages demo stays current.

## 4. Run the web IDE

```bash
python3 -m http.server -d docs 8000
# open http://localhost:8000
```

The IDE gives you an assembler editor, a **Registers** panel (with change
highlighting), a **Flags** panel, a **Memory** dump (click to poke bytes),
a **Disassembly** view (click a line to toggle a breakpoint, double-click to
run-to-cursor), a **Watch** window (registers or `[addr]` memory, click to
edit), peripheral/device panels, and a time-travel **Step-Back** history.

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| F7  | Assemble |
| F8  | Step |
| F10 | Step over (CALL) |
| F5  | Run |
| F4  | Reset |
| Esc | Stop |

## 5. Use the wasm module from JavaScript

```js
import init, { Emulator } from './pkg/multi_cpu_emu.js';
await init();

const emu = new Emulator('8086');
const code = emu.assemble('ORG 100h\nMOV AX, 5\nADD AX, 3\nHLT\nEND');
emu.load(code, 0x100);
emu.run(1000);
console.log(emu.regs());   // ["AX=0008", ...]
console.log(emu.out());    // program output
```

See [api-reference.md](api-reference.md) for the full surface.
