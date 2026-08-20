# progress.md — what's done, what's left

Status snapshot for the multi-cpu-emu project (8086 / 8085 / 8051).

Legend:
- `[x]` done and tested (integration test or verified by example/CLI run)
- `[~]` partial — works for the common case, missing edge cases
- `[ ]` not implemented

## Shared infrastructure

- [x] `Cpu` trait, `Mem` (power-of-two linear memory), `Output` buffer,
      `FlagSet`, `Reg`, `RunResult` (src/cpu.rs)
- [x] `Emulator` facade over the three cores (src/lib.rs)
- [x] Snapshot / restore on every core (deterministic round-trip, tested)
- [x] Line-oriented assembler: labels, `ORG/DB/DW/EQU/END`, `;` comments,
      decimal/hex/`h`/binary/`b`/octal/`q`/char literals, label arithmetic,
      per-line error reporting with line numbers
- [x] WASM surface (`src/wasm.rs`, feature `wasm`): `Emulator` class with
      assemble / load / step / run / pc / regs / flags / mem / out / halted /
      reset / snapshot / restore / interrupt (8085)
- [x] Native CLI runner (examples/run.rs)
- [x] 15 integration tests (tests/emulation.rs), `cargo clippy --all-targets`
      warning-free
- [x] `ORG` emits a complete memory image: forward `ORG` pads with zeros
      (place code at hardware vectors), backward `ORG` is an error; load code
      at 0 and start the 8086 at entry 0x100 (run.rs / tests / web demo all
      updated)
- [x] Web IDE for students in `docs/` (editor, registers, flags, memory dump,
      output, per-ISA examples) deployed on GitHub Pages
- [x] GitHub Actions workflow (build wasm pkg + run tests + deploy Pages)
- [x] Output conventions: 8086 `INT 21h/10h`, 8085 `OUT 01h`, 8051 `SBUF`

## Intel 8086 (src/i8086.rs)

### CPU core

- [x] Registers AX–DX (+AH/AL…), SI/DI/BP/SP, CS/DS/ES/SS, IP, FLAGS
- [x] Segmented addressing, 1 MiB flat memory, segment-override prefixes
      (26/2E/36/3E)
- [x] MOV (reg/reg, reg/mem, imm, segment registers), PUSH/POP (reg, r/m,
      imm, seg), XCHG, LEA
- [x] ADD/ADC/SUB/SBB/AND/OR/XOR/CMP — ModRM + accumulator + immediate forms
- [x] INC/DEC, NEG/NOT, MUL/IMUL/DIV/IDIV (8/16-bit, signed/unsigned)
- [x] TEST, shifts/rotates (D0–D3), CBW/CWD
- [x] String ops MOVS/LODS/STOS/CMPS/SCAS (byte+word) with REP/REPE/REPNE
- [x] LAHF/SAHF, flag ops CLC/STC/CMC/CLI/STI/CLD/STD
- [x] Jcc/JMP short/near/far, CALL/RET/RETF, LOOP/LOOPZ/LOOPNZ/JCXZ
- [x] INT n, INT3, INTO, IRET, NOP, HLT
- [x] BOUND r16, m16 (traps via INT 5 on range violation)
- [x] INS/OUTS (byte/word, REP, DF, port model returns 0 / no-op)
- [x] WAIT/FWAIT (no-op), LOCK prefix (no-op)
- [x] DOS/BIOS subset: INT 21h (AH=01, 07, 02, 06, 09, 0C, 4Ch),
      INT 10h (AH=0Eh, 0Fh)

### Assembler (src/asm/asm8086.rs)

- [x] All mnemonics matching the core subset
- [x] Two-pass with forward label resolution, `OFFSET`, `[base+idx+disp]`
      memory operands, byte/word size hints, `DW` data

### Left / known gaps

- [x] Full 8086 opcode coverage: only 386+-only instructions remain
      unimplemented (BOUND/INS/OUTS/WAIT now done; 0x60/0x61 PUSHA/POPA and
      0x64-0x67 prefixes are no-ops)
- [ ] INT 21h input services return a placeholder (no keyboard) — AH=01/07
      set AL=0 instead of reading a key
- [ ] PUSHA/POPA, 386+ instructions (ARPL, 0x64-0x67 prefixes) — no-ops
- [ ] No hardware-interrupt/timer simulation (8259/8253/PIT not modeled)
- [ ] FPU, 386+ extensions, DOS file/date services — out of scope
- [ ] Flags: TF (trap) tracked internally but not exposed in `FlagSet`
- [ ] I/O port space (IN/OUT instructions) not modeled (all DOS output goes
      through INT 21h)
- [ ] `case` sensitivity, macros, `INCLUDE`, structured directives
      (IF/WHILE) from emu8086 dialect — not supported

## Intel 8085 (src/i8085.rs)

### CPU core

- [x] Registers A, B, C, D, E, H, L, SP, PC; flags S/Z/AC/P/CY
- [x] Data movement: MOV/MVI/LXI/LDA/STA/LDAX/STAX/LHLD/SHLD/XCHG
- [x] Arithmetic: ADD/ADC/SUB/SBB (+ imm ADI/ACI/SUI/SBI), INR/DCR/INX/DCX,
      DAD
- [x] Logical: ANA/XRA/ORA/CMP (+ ANI/XRI/ORI/CPI), RLC/RRC/RAL/RAR, CMA,
      CMC, STC, DAA
- [x] Branches: JMP/Jcc, CALL/Ccc/RET/Rcc/RST
- [x] Stack: PUSH/POP (regs + PSW), XTHL, SPHL, PCHL
- [x] IN/OUT (OUT 01h prints char in A), EI/DI, SIM/RIM, NOP, HLT
- [x] Hardware interrupts: TRAP (non-maskable, keeps IE), RST 7.5 (edge
      latched, cleared on service), RST 6.5, RST 5.5, INTR (external vector);
      priority TRAP > 7.5 > 6.5 > 5.5 > INTR; ISR pushes PSW + PC, clears IE
      (except TRAP), vectors through 0x24/0x2C/0x34/0x3C
- [x] SIM masks RST 5.5/6.5/7.5 (MSE), resets the RST 7.5 latch (R7.5), sets
      SOD; RIM reports SID, pending 7.5/6.5/5.5, IE, masks — A register layout
      matches the real chip
- [x] `Emulator::request_interrupt(kind, data)` + wasm `interrupt()`; web demo
      IRQ buttons (TRAP/7.5/6.5/5.5/INTR@08h) with an interactive 8085
      interrupt example
- [x] Full 8-bit ISA — no known missing opcodes

### Assembler (src/asm/asm8085.rs)

- [x] Full mnemonic coverage matching the core
- [x] `DB/DW`, labels, arithmetic — 2-pass with labels computed in pass 1

### Left / known gaps

- [ ] SID/SOD pins are emulator-side state (no real serial I/O attached)
- [ ] I/O ports beyond 01h are no-ops (no peripheral model)
- [ ] Timing/clock cycles not modeled (step = 1 instruction)

## Intel 8051 / MCS-51 (src/mcs51.rs)

### CPU core

- [x] Registers: A, B, R0–R7 (4 register banks via PSW), DPTR, PC, PSW, SP
- [x] Memory: 64 KiB code, 128 B internal RAM, bit-addressable 0x20–0x2F,
      SFR bit-addressable area (0x80–0xFF), XDATA via MOVX
- [x] Data movement: MOV (Rn/direct/@Ri/#imm), MOVC @A+DPTR/@A+PC, MOVX
      @DPTR/@Ri, PUSH/POP, XCH/XCHD, SWAP
- [x] Arithmetic: ADD/ADDC/SUBB, INC/DEC (A/Rn/direct/@Ri/DPTR), MUL AB,
      DIV AB, DA A
- [x] Logical: ANL/ORL/XRL (A,Rn,direct,#imm,@Ri + direct-target forms),
      CLR/CPL A, RL/RR/RLC/RRC
- [x] Bit ops: SETB/CLR/CPL (bit + C), ANL C/ORL C/MOV C, bit-addressable
      RAM and SFR bits (P0–P3, TCON, PSW, ACC, B)
- [x] Branches: SJMP/AJMP/LJMP, JZ/JNZ/JC/JNC/JB/JNB/JBC, CJNE, DJNZ,
      ACALL/LCALL/RET/RETI
- [x] JMP @A+DPTR (table jump)
- [x] SFRs: P0–P3, PSW, ACC, B, SP, DPL/DPH, TCON, TMOD, TH0/TL0/TH1/TL1,
      SCON, SBUF, IE, IP
- [x] Timer 0/1 (mode 0/1/2) count while TRx=1; TFx set on overflow;
      mode-2 auto-reload
- [x] Writing SBUF emits a char to the output buffer
- [x] Full opcode coverage: only reserved 0xA5 remains unassigned

### Assembler (src/asm/asm8051.rs)

- [x] Full mnemonic coverage matching the core, SFR names, port-bit names
      (P0.0–P3.7), TCON bit names (IT0/IE0/IT1/IE1/TR0/TF0/TR1/TF1),
      C/OV/AC/P, direct bit addresses, `DB`
- [x] Relative branch patching (SJMP/JZ/…/CJNE/DJNZ) with range checks

### Left / known gaps

- [ ] No interrupt handling — IE/IP/TCON interrupt-enable bits are storage
      only; no vector dispatch, no RETI semantics beyond return
- [ ] Serial port (SCON/SBUF) transmit only; no receive, no baud-rate
      generation (timer-based baud not modeled)
- [ ] External interrupts (INT0/INT1), INTO/INT1 pins — not simulated
- [ ] Timer counting is per-emulator-step (no real-time calibration /
      machine-cycle accuracy)
- [ ] External memory beyond 64 KiB (up to 256 KiB via ports) — not modeled
- [ ] Watchdog, power-down modes (PCON) — storage only

## Web IDE / deployment

- [x] docs/ demo: ISA selector, 13 sample programs (incl. 8085 interrupts),
      line-numbered editor with error line highlighting,
      Assemble/Step/Run/Stop/Reset, IRQ buttons (8085), keyboard shortcuts,
      live registers/flags, pageable memory dump with PC marker, output
      console, localStorage persistence, Ctrl+S save
- [x] GitHub Pages workflow (.github/workflows/pages.yml) — builds wasm pkg,
      runs tests, deploys docs/
- [x] Live at https://danish9661.github.io/8086emu/ (verified 200 on
      index/wasm/js)
- [ ] Only remaining: enable Pages source "GitHub Actions" in repo settings
      if the workflow's auto-enable didn't take effect

## Suggested next steps (priority order)

1. [x] 8085: RST 5.5/6.5/7.5 + INTR interrupt simulation (done, tested)
2. 8051: interrupt vector dispatch for timers/serial/external
3. 8086: real keyboard input for INT 21h AH=01/07 via the web UI
4. Web IDE: syntax highlighting, per-line machine-code column, step-back
   (time-travel via snapshot/restore)
5. More integration tests per ISA (flags, string ops, stack, timers)