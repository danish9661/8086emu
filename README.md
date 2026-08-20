# multi-cpu-emu

A single Rust crate that emulates three classic microprocessors:

- **Intel 8086** — 16-bit, segmented, 1 MiB address space
- **Intel 8085** — 8-bit, 64 KiB, accumulator-centric
- **Intel 8051 (MCS-51)** — 8-bit, SFRs, bit-addressable RAM, timers

Each core has a matching assembler, and the whole crate compiles to **one WASM
module** (via `wasm-bindgen`, feature `wasm`) plus a native `rlib`/`cdylib`.
A full dependency-free web IDE for students lives in `docs/` and deploys to
GitHub Pages with zero config.

Design was inspired by https://github.com/abuXsarkar/modern8086 (MIT) — used
only as an architecture/scope reference; all code here is written from scratch.
See `AGENTS.md` for the full architecture and per-ISA coverage.

## Build & test

```bash
cargo test                         # 8 integration tests across all three ISAs
cargo clippy --all-targets         # should be warning-free

# wasm build (needs wasm-pack)
wasm-pack build --target web --out-dir docs/pkg --release --features wasm

# serve the web demo
python3 -m http.server -d docs 8000   # then open http://localhost:8000
```

## Web IDE / GitHub Pages

The demo in `docs/` is a student-oriented IDE: ISA selector (8086/8085/8051),
sample programs, line-numbered editor with assemble-error highlighting, step /
run / stop / reset, live register + flag panels, memory dump with the PC
highlighted, and a program-output console.

To deploy: GitHub **Settings → Pages → Deploy from a branch → `main`, folder
`/docs`**. The site appears at `https://<user>.github.io/8086emu/`. After any
Rust change, rebuild and commit the pkg:
`wasm-pack build --target web --out-dir docs/pkg --release --features wasm`.
Root `index.html` redirects to `docs/` for local convenience.

## CLI runner

```bash
cargo run --example run -- examples/hello.asm          # 8086
cargo run --example run -- --isa 8085 examples/hello85.asm
cargo run --example run -- --isa 8051 examples/hello51.asm
```

## Layout

```
├── src/
│   ├── lib.rs          # Emulator facade over the three cores
│   ├── cpu.rs          # Cpu trait, Mem, Output, FlagSet, Reg, RunResult
│   ├── i8086.rs        # 8086 CPU core (segmented, INT 21h/10h subset)
│   ├── i8085.rs        # 8085 CPU core (full 8-bit ISA)
│   ├── mcs51.rs        # 8051 CPU core (SFRs, bit ops, timers)
│   ├── asm/            # tokenizer + per-ISA assemblers
│   └── wasm.rs         # wasm-bindgen surface (feature = "wasm")
├── examples/run.rs     # native CLI runner
├── tests/emulation.rs  # integration tests
├── docs/               # GitHub Pages IDE (index.html + app.js + style.css + pkg/)
└── index.html          # redirects to docs/
```

## WASM API

```js
const emu = new Emulator("8086");            // "8086" | "8085" | "8051"
const code = emu.assemble(src);              // throws on error
emu.load(code, 0x100);                       // write code + set PC
emu.run(1_000_000);                          // steps executed
emu.step();
emu.pc();  emu.regs();  emu.flags();         // "AX=1234" / "ZF"
emu.mem(0, 64);                              // raw bytes
emu.out();                                   // program output (drains)
emu.halted();  emu.reset();
emu.snapshot();  emu.restore(bytes);         // deterministic time-travel
```

## Program output conventions

- **8086** — `INT 21h` (AH=02, 06, 09, 4Ch) and `INT 10h` (AH=0Eh) write to the
  output buffer.
- **8085** — `OUT 01h` prints the char in A.
- **8051** — writing to `SBUF` prints the char.