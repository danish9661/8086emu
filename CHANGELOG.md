# Changelog

All notable changes to the `multi-cpu-emu` project are documented here.
Format loosely follows [Keep a Changelog](https://keepachangelog.com/);
versions are dated snapshots of `main`.

## [Unreleased]

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

### Changed
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

## [2026-08-22] - Initial multi-CPU feature set
- Cores: 8086, 8085, 8051, Z80, 6502, rv32i (+M).
- 8086 INT 10h text/graphics framebuffer + x87 FPU subset.
- Hardware-interrupt UI bars (8085/8051/Z80/6502) with vector injection.
- IDE: assemble, step/run/step-over, breakpoints, watches, memory diff,
  share-by-URL, headless grader, i18n scaffolding.
- Native CLI `examples/run.rs` with `--grade` spec checking.
