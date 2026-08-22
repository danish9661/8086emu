# Changelog

All notable changes to the `multi-cpu-emu` project are documented here.
Format loosely follows [Keep a Changelog](https://keepachangelog.com/);
versions are dated snapshots of `main`.

## [Unreleased]

### Added
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

## [2026-08-22] - Initial multi-CPU feature set
- Cores: 8086, 8085, 8051, Z80, 6502, rv32i (+M).
- 8086 INT 10h text/graphics framebuffer + x87 FPU subset.
- Hardware-interrupt UI bars (8085/8051/Z80/6502) with vector injection.
- IDE: assemble, step/run/step-over, breakpoints, watches, memory diff,
  share-by-URL, headless grader, i18n scaffolding.
- Native CLI `examples/run.rs` with `--grade` spec checking.
