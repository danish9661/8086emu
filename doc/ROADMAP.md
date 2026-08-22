# Roadmap

Planned and in-progress work, tracked against the reference
[modern8086](https://github.com/abuXsarkar/modern8086) project.

## Done

- [x] One WASM core exposing all three ISAs (8086 / 8085 / 8051)
- [x] Assemblers for all three ISAs (multi-pass 8086, single-pass 8085/8051)
- [x] Live debugger: step / step-over / step-back (snapshot time-travel) / run
- [x] Disassembly panel for all three ISAs
- [x] Watch window (registers + memory) with click-to-edit and persistence
- [x] Memory view with byte editing and change highlighting, memory map, ROM load
- [x] I/O ports / peripheral register inspection
- [x] Keyboard shortcuts (F4/F5/F7/F8/F10/Esc), run-to-cursor
- [x] 8086 `INT 16h` keyboard + `INT 21h AH=0A` line input + broader `INT 10h`
- [x] 8051 disassembler external-ROM (EA=0) fix
- [x] `set_reg` test coverage
- [x] Conditional breakpoints + memory-diff highlight
- [x] Editor autocomplete + mnemonic hover tooltips
- [x] Share-by-URL (source in fragment)
- [x] Headless `m86`-style CLI runner + autograder (`examples/run.rs`, `grade/`)
- [x] Grading GitHub Action (`.github/workflows/grade.yml`)
- [x] i18n scaffold (EN/ES/DE/FR/HI) driven by `data-i18n`
- [x] `doc/` handbook (architecture, API, ISA matrix, packaging, FAQ, this file)

## Next

- [ ] x87 FPU execution (currently stubbed)
- [ ] Richer `INT 10h` graphics (pixel modes, scrolling windows)
- [ ] Visual breakpoint/condition editor (modal instead of `prompt()`)
- [ ] Autograder web UI: paste a spec, get PASS/FAIL inline
- [ ] Device library parity: traffic light, stepper, 7-seg, LED matrix, robot
      grid, printer (8086-style) exposed as pluggable peripherals
- [ ] Share-by-URL with full CPU state (snapshot in fragment) for reproducible bugs
- [ ] Monaco/CodeMirror editor upgrade with richer hover, go-to-definition,
      inline diagnostics
- [ ] PWA offline support + desktop build (Tauri/Electron)
- [ ] Expanded i18n coverage (all 13 languages from the reference) + RTL
- [ ] More ISAs: Z80, 6502, RISC-V (rv32i) as additional cores

## Non-goals

- Cycle-accurate timing (timers count steps, not real time)
- Full MS-DOS filesystem emulation (in-memory handles only)
- Binary (non-assembly) program loading beyond ROM images
