# Changelog

All notable changes to the `multi-cpu-emu` project are documented here.
Format loosely follows [Keep a Changelog](https://keepachangelog.com/);
versions are dated snapshots of `main`.

## [Unreleased]

### Performance
- 8086 (and rv32) cores now **trust their decode cache for ROM-loaded images**:
  `exec` skips the per-step prefix+opcode re-read/verify when the instruction
  address is inside the read-only ROM range (`Mem::in_rom`). Programs loaded via
  `load_rom` (the web IDE now does this) therefore decode once and never
  re-fetch, giving a measurable step-rate gain (release: 8086 ~128M→~136M
  steps/s, larger in debug where the re-read was unoptimized). External
  `mem_write` pokes call `invalidate_icache` so self-modifying code — including
  edits to a ROM-loaded image — stays correct.
- The earlier hypothesis that an explicit bounds-check / `mem.len()-1` mask
  field would speed up `Mem` was **invalidated by benchmarking**: run-to-run
  variance (±5–10%) dwarfs any delta, confirming LLVM already elides the
  bounds check. A const-generic `Mem<N>` (the only way to structurally remove
  the check) is not viable because `i8085`/`mcs51`/`disasm` build `Mem` from
  runtime sizes (`i8085::reset(size)`). The cores are at the safe-Rust speed
  ceiling; further gains would require algorithmic work (e.g. extending the
  decode cache to 8085/6502/8051/z80).

### Added
- `audit.md`: security, memory/leakage, per-step overhead, performance
  benchmarks, determinism, and known-limitation audit of the cores.
- CLI: `--verbose`/`-v` debug trace that prints each decoded instruction and
  every peripheral (port) write, for tracing hardware access.
- CLI: `--help`/`-h` and a clean `print_usage()`; running with no arguments now
  prints usage and exits 0 instead of panicking.
- CLI: `--bench [N]` measures steady-state emulation throughput (steps/sec) for
  any ISA; the IDE also exposes `window.emu` for browser-side benchmarking.
- Hand-written, consumer-facing TypeScript types at `docs/types.d.ts`.
- 6502 decimal mode (ADC/SBC BCD under the D flag) and NMI/IRQ/BRK vectoring.
- rv32i M-extension (`MUL`/`MULH`/`MULHSU`/`MULHU`/`DIV`/`DIVU`/`REM`/`REMU`)
  in the core and assembler.
- IDE: 8085 SID/SOD indicators, 8051 serial RX injector, Z80 memory hex editor.
- IDE: **About** dialog (button in the header) documenting the supported CPUs,
  quick-start, assembler syntax, I/O/device ports, interrupts, keyboard
  shortcuts, save/load/share, and the source/repo. Closes on `Esc` or
  outside-click.
- IDE: **richer watch / breakpoint expression language**. A small expression
  evaluator (`evalExpr` in `docs/app.js`) supports registers (`AX`, `AH`, …),
  CPU flags (`ZF`, `CF`, …), arithmetic (`+ - * / %`), bitwise (`& | ^ ~`),
  shifts (`<< >>`), parentheses, unary minus/complement (`-X`, `~X`), and
  computed memory reads (`[0x200]`, `[BX+2]`). The watch input placeholder now
  reads `[0x200]`. The same evaluator powers conditional breakpoints, so
  `AX+BX == 3` and `[BX+2] != 0` work as expected.
- IDE: **device pop-out floaters**. Each device panel (traffic light, 7-segment,
  stepper, printer, robot, LED matrix, clock/timers) has a `↗` button that
  detaches it into a draggable, floating window (à la the reference
  `modern8086` IDE). Floater position persists in `localStorage`, the floater
  stays live during Run, and re-opening the panel returns it.
- IDE: verified the 8086 framebuffer render paths with a DOM-mock smoke test —
  `renderScreen` (INT 10h text mode) emits the 80×25 character/attribute spans
  with the cursor highlight and VGA colours, and `renderGfx` (mode 13h) paints
  the 320×200×4 pixel buffer to the canvas.

### Changed
- Web IDE (`docs/`): the right-hand column is no longer a single 12-panel
  stack that forces endless scrolling. The panels are now organized into tabs
  (Registers, Disassembly, Memory, I/O, Output, Devices) with only the active
  tab shown. This also fixes a performance problem: `refresh()` previously
  rebuilt *every* panel's DOM (registers, flags, 256-byte memory dump, text
  screen, graphics, all devices, memory map, peripherals, ports, 40-line
  disassembly, watch) on every single step/run tick. It now renders only the
  visible tab, so interactive stepping and the run loop do far less work per
  refresh. The Devices tab auto-hides for ISAs that have no device view.
- rv32: added a re-read-verified decode cache in `step()`. The decoded
  instruction word is *trusted* (the 4-byte `fetch` is skipped) when the PC is
  inside the read-only ROM range — `Mem::write` silently ignores ROM writes, so
  the bytes are provably immutable during execution; for writable code the cached
  bytes are re-read and compared, keeping self-modifying code correct. The cache
  is invalidated on `reset`/`restore`. Busy-loop throughput (debug native, ROM
  image) rose from ~33 M to ~52 M steps/sec (**~+57%**). `Mem::in_rom` added to
  support the fast path.
- CLI `--bench`: rv32 now loads its loop as ROM (representative of real
  read-only RISC-V code) instead of plain `mem_write`, so the reported rv32
  throughput reflects the decode-cache trust fast path.
- CLI: assembly/file/ISA errors now print actionable messages to stderr and use
  conventional exit codes (0 success, 1 runtime failure, 2 usage error).
- `renderPorts()` function header restored in `docs/app.js` (the missing header
  had broken the entire IDE script).
- Z80 `NEG` half-carry now computed correctly (was an always-zero erasing op).
- Richer JSDoc on the WASM `Emulator` surface for better generated `.d.ts`.
- 8086 performance: cached segment bases (`cs_base`..`gs_base`) avoid a `seg<<4`
  per memory access; `PIT`/`PIC` servicing is skipped in `step()` when no timer
  is counting and no IRQ can fire; a conservative, re-read-verified decode cache
  in `exec()` reuses decoded prefixes/opcode across iterations. The redundant
  `mem_read(pc())` used only for cycle counting was removed (`~2x` faster busy
  loop, ~16M steps/sec in a debug native build).
- 8086 core bug fix: `op_group1` `reg, r/m` forms (e.g. `ADD AX, [SI]`,
  `ADD AX, BX`) now write the result to the **register** operand instead of the
  `r/m` operand (the previous behaviour corrupted the source register/memory).
- CLI: `--bench` for Z80 now assembles the correct `JR again` loop (the
  case-sensitive `bench_loop` arm was matching `"Z80"` while `--isa` passes
  `"z80"`, which silently fell through to the 8086 loop and was rejected).
- Cross-core performance pass: 8085 and 8051 no longer re-read the opcode at
  `step()` just to compute cycle counts (the decoded opcode is reused via a
  `last_op` field, matching the 8086 optimization); 8051 also skips
  `tick_timers_n` / `service_interrupts` when no timer is running and no
  interrupt source is enabled. **8085 ~+22%, 8051 ~+64%** on busy-loop
  throughput (debug native). 6502 / Z80 / rv32 `step()` were already lean
  (single fetch, no per-step timer, cheap interrupt check) and are unchanged.

### Fixed
- Z80 `IN r,(C)` now actually loads the port (BC) value into the register /
  `(HL)`; previously it discarded the read and left the register unchanged.
- Z80 `OUT (C),r` now writes the register / `(HL)` to the port (BC); previously
  it *read* the port (swapped with `IN`). The two `exec_ed` arms were
  transposed.
- Z80 `ADD HL,BC/DE/HL/SP` (`0x09/0x19/0x29/0x39`) now execute (they were a
  silent NOP in the main decoder — only the IX/IY variant existed); a new
  `add_hl` helper sets H/N/V/C flags and leaves S/Z untouched.
- Z80 `RST n` (`0x00`–`0x38`) now pushes the return address and jumps to the
  vector; it was previously a NOP and had no assembler support. The assembler
  now encodes `RST n` as `0xC7 | (n & 0x38)` for `n ∈ {0,8,…,56}`.
- Z80 assembler `ORG` now emits padding for forward jumps (it previously only
  adjusted the first-pass address and never produced the gap), so interrupt
  vectors and `ORG`-placed ISRs land at the correct addresses.
- rv32 CSR instructions (`CSRRW/CSRRS/CSRRC` and immediate forms, opcode
  `0x73`, funct3 ≠ 0) now execute against a `csr[0..4096]` storage array
  (read-old / write-new with correct `rs1 = 0` no-write semantics) instead of
  being silently ignored. Snapshot/restore now covers the CSR file. The
  assembler gained `CSRRW/CSRRS/CSRRC/CSRRWI/CSRRSI/CSRRCI` (plus shorthand
  `CSRWI/CSRSI/CSRCI`) and the `CSRR rd,csr` / `CSRW csr,rs` pseudos.
- 8086 `INT 21h AH=09` (print `$`-terminated string) now bounds the scan to the
  end of physical memory, so a program that forgets the `$` terminator
  terminates instead of trapping the wasm runtime with an unbounded read loop.
- IDE: conditional-breakpoint expressions (`evalCond`) previously routed both
  sides through `evalWatch`, which treats a bare numeric constant (e.g. `0x10`)
  as a **memory address** and dereferences it. `CX == 0x10` therefore compared
  against `mem[0x10]` rather than the value `0x10`. Bare numeric constants are
  now treated as literal values; `[addr]` is still a memory read, so
  `[0x200] == 0xAB` continues to read memory as before.
- IDE: the watch / breakpoint **expression evaluator was completely broken** —
  an internal parse helper was named `expr`, which shadowed the `expr` parameter
  of `evalExpr`, so every evaluation saw the string argument as `undefined` and
  returned `NaN` (watches showed `?`, conditional breakpoints never triggered).
  The helper is renamed to `parseExpr`.
- IDE: **visual refresh** to match the cleaner `modern8086` aesthetic. UI chrome
  now uses a system sans-serif typeface with monospace reserved for code surfaces
  (editor, disassembly, registers, memory, text/graphics screen, ports); a
  cohesive token palette (panel / line / radius / shadow), gradient accent
  buttons, elevated "plugin-card" device panels with port-badge headers and
  pop-out buttons, refined tabs, `:focus-visible` outlines, custom scrollbars,
  and a CRT-glow text screen.
- IDE: shift operators `<<` / `>>` were never tokenized (the operator scanner
  omitted `<` / `>`), so `AX << 1` silently dropped the shift and returned `AX`.
  The tokenizer now recognizes `<<` / `>>` as single operators.

## [2026-08-22] - Initial multi-CPU feature set
- Cores: 8086, 8085, 8051, Z80, 6502, rv32i (+M).
- 8086 INT 10h text/graphics framebuffer + x87 FPU subset.
- Hardware-interrupt UI bars (8085/8051/Z80/6502) with vector injection.
- IDE: assemble, step/run/step-over, breakpoints, watches, memory diff,
  share-by-URL, headless grader, i18n scaffolding.
- Native CLI `examples/run.rs` with `--grade` spec checking.
