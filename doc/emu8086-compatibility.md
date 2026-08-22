# emu8086 compatibility

This project is inspired by [modern8086](https://github.com/abuXsarkar/modern8086)
(an 8086 + 8085 IDE). It is a **from-scratch** implementation and adds two
extra ISAs (8085 and 8051). This page tracks how closely the 8086 dialect matches
the emu8086 assembler so existing emu8086 programs port with minimal edits.

## Assembler

| Feature | emu8086 | multi-cpu-emu |
|---|---|---|
| `;` line comments | ✅ | ✅ |
| Labels (`name:` / `EQU`) | ✅ | ✅ |
| `ORG` / `DB` / `DW` | ✅ | ✅ (forward `ORG` pads, backward errors) |
| Number bases `0x`, `h`, `b`, `q`, `'c'` | ✅ | ✅ |
| Label arithmetic (`loop+2`) | ✅ | ✅ |
| Multi-pass forward references | ✅ | ✅ |
| `END` directive | ✅ | ✅ |

### Instruction coverage (8086)

Implemented: `MOV`, `PUSH`/`POP` (reg, r/m, imm, seg), `PUSHA`/`POPA`,
`ADD`/`ADC`/`SUB`/`SBB`/`AND`/`OR`/`XOR`/`CMP` (modrm + imm + acc forms),
`INC`/`DEC`, `NEG`/`NOT`, `MUL`/`IMUL`/`DIV`/`IDIV`, `TEST`, `XCHG`, `LEA`,
shifts/rotates (`D0`–`D3`), `CBW`/`CWD`, BCD/ASCII adjust (`DAA`/`DAS`/`AAA`/
`AAS`/`AAM`/`AAD`), string ops `MOVS`/`LODS`/`STOS`/`CMPS`/`SCAS` + `REP`,
`LAHF`/`SAHF`, flag ops `CLC`/`STC`/`CMC`/`CLI`/`STI`/`CLD`/`STD`, branches
`Jcc`/`JMP`/`CALL`/`RET`/`RETF`, `LOOP`/`LOOPZ`/`LOOPNZ`/`JCXZ`, `INT n`/
`INT3`/`INTO`/`IRET`, `NOP`, `HLT`, `IN`/`OUT`.

**Not yet** modeled (treated as unimplemented/halt): x87 FPU opcodes are
stubbed (`FPU` state exists but no arithmetic), and a few obscure system
instructions. Most teaching/lab programs run unchanged.

## DOS / BIOS services

| Service | emu8086 | multi-cpu-emu |
|---|---|---|
| `INT 21h` AH=01 (read, echo) | ✅ | ✅ |
| AH=02 (write char) | ✅ | ✅ |
| AH=06 (direct console) | ✅ | ✅ |
| AH=07/08 (read, no echo) | ✅ | ✅ |
| AH=09 (write `$`-string) | ✅ | ✅ |
| AH=0A (buffered line input) | ✅ | ✅ |
| AH=0Ch (flush + read) | ✅ | ✅ |
| AH=2A/2C (date/time) | ✅ | ✅ |
| AH=3C/3D/3E/3F/40/41/42 (files) | ✅ | in-memory files |
| AH=4Ch (terminate) | ✅ | ✅ |
| `INT 10h` 00/01/02/03/06/07/08/09/0A/0E/0F/13 | ✅ | 00/01/02/03 + 09/0E added |
| `INT 16h` 00/01/02 (keyboard) | ✅ | ✅ (added) |

emu8086's larger video-mode surface (`INT 10h` graphics) is not emulated; the
text-mode framebuffer at `0xB8000` and `INT 10h` teletype/`AH=09` are.

## Differences to expect when porting

- **Entry point**: emu8086 defaults to `ORG 100h`; this emulator loads 8086
  code at `0` and sets `IP=0x100`, matching `ORG 100h` programs exactly.
- **Memory size**: 1 MiB flat (not the 1 MiB segmented limit modeled
  identically). `ea = seg<<4 + off`.
- **`OUT 01h`**: prints AL (8085-style convention) in addition to the real port
  write — handy for minimal "hello world" programs.
- **FPU**: x87 instructions are not executed (structure reserved for later).
