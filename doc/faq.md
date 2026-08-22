# FAQ

**Q: Why are there two folders, `docs/` and `doc/`?**
`docs/` (with an **s**) is the built web demo — `index.html` plus the
`wasm-pack` output in `docs/pkg/`. `doc/` (this folder) is the human-readable
documentation. Keep the built artifacts in `docs/`, the prose in `doc/`.

**Q: Does the disassembler work for all three ISAs?**
Yes. `Emulator::disasm(addr, count)` (WASM) / `disassemble` (Rust) decodes
8086, 8085, and 8051. Unrecognized bytes become `DB xxh`. The 8051 disasm
reads the code space the CPU actually fetches from (internal `code` when `EA=1`,
external XDATA when `EA=0`).

**Q: How do I set a breakpoint?**
Click a line in the **Disassembly** view, or click a line number in the editor
gutter. Breakpoints are stored as linear addresses for all ISAs, so the
disassembler and the source gutter share the same set.

**Q: What does "run-to-cursor" do?**
Double-click a line in the Disassembly view: the CPU runs until that address is
the next instruction to execute (or hits another breakpoint/halts).

**Q: How does Step-Back (time travel) work?**
Every Step / Step-Over / Run pushes a `snapshot()` onto a 200-deep ring. Back
restores the most recent snapshot. Snapshots are deterministic and round-trip.

**Q: My 8086 program reads a key but nothing happens.**
INT 21h AH=01/06/07/08/0C block when the input buffer is empty. A dialog pops
up in the IDE — type the characters and they are queued as type-ahead. From
code, call `emu.push_key(code)`.

**Q: Why does OUT 01h print a character?**
That is a convention shared by the 8085 and 8086 cores: writing port `01h`
sends `AL` to the `Output` buffer. It is handy for minimal "hello" programs
without DOS/BIOS services.

**Q: The web demo shows old behavior after I changed Rust.**
Rebuild the package — `wasm-pack build --target web --out-dir docs/pkg
--release --features wasm` — and commit `docs/pkg/`. Pages serves the committed
pkg, so it needs a rebuild after any core change.

**Q: Can I use the emulator from Node, not just the browser?**
Yes. Rebuild with `--target nodejs` (or `--target bundler`) and `require`/import
the module. The CPU logic is identical; only the wasm instantiation differs.

**Q: How deterministic is execution?**
Fully deterministic for a given input. `snapshot`/`restore` round-trips, so the
same state always produces the same next state. There are no external
emulation dependencies and no `unsafe` outside `wasm-bindgen`.
