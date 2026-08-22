# ISA Coverage Matrix

What each core can execute (and what the assembler accepts). "✓" = implemented,
"·" = not applicable.

## 8086 (1 MiB, segmented)

| Group | Coverage |
|-------|----------|
| Data move | MOV (reg/r/m/seg), PUSH/POP (reg, r/m, imm, seg), XCHG, LEA, LDS/LES |
| Arithmetic | ADD/ADC/SUB/SBB/CMP, INC/DEC, NEG/NOT, MUL/IMUL/DIV/IDIV, AAA/AAS/DAA/DAS/AAM/AAD |
| Logic | AND/OR/XOR/TEST, shifts & rotates (D0–D3), CBW/CWD |
| Strings | MOVS/LODS/STOS/CMPS/SCAS (+ REP/REPNE) |
| Control | Jcc, JMP (short/near/far), CALL/RET/RETF, LOOP/LOOPZ/LOOPNZ/JCXZ |
| Stack frame | PUSHA/POPA, ENTER/LEAVE |
| Flags | LAHF/SAHF, CLC/STC/CMC/CLI/STI/CLD/STD, POPF restores TF (single-step trap) |
| Interrupts | INT n/INT3/INTO/IRET; hardware NMI + INTR via IVT (priority NMI>INTR) |
| I/O | IN/OUT (imm8 + DX), port 01h also prints AL |
| Misc | NOP, HLT |
| DOS/BIOS | INT 21h (01/02/06/07/08/09/0C/4Ch), INT 10h (0Eh) |
| Peripherals | 8253 PIT → 8259 PIC (IRQ0→INT 8), ports 0x20/0x21 |

## 8085 (64 KiB, flat)

| Group | Coverage |
|-------|----------|
| Data move | MOV/MVI/LXI/LDA/STA/LDAX/STAX/LHLD/SHLD/XCHG |
| Arithmetic | ADD/ADC/SUB/SBB/ANA/XRA/ORA/CMP (+ immediate forms), INR/DCR/INX/DCX/DAD, DAA |
| Rotate | RLC/RRC/RAL/RAR, CMA/CMC/STC |
| Control | JMP/Jcc, CALL/Ccc/RET/Rcc, RST, PUSH/POP (regs + PSW), XTHL/SPHL/PCHL |
| Interrupts | TRAP (0x24), RST 7.5/6.5/5.5 (0x3C/0x34/0x2C), INTR; SIM/RIM masks & pending flags |
| I/O | IN/OUT (port 01h prints A), 256-byte port space |
| Misc | EI/DI, NOP, HLT |

## 8051 / MCS-51 (64 KiB code, SFRs)

| Group | Coverage |
|-------|----------|
| Data move | MOV/MOVC/MOVX, PUSH/POP, XCH/XCHD/SWAP |
| Arithmetic | ADD/ADDC/SUBB, INC/DEC, MUL/DIV, DA |
| Logic | ANL/ORL/XRL, CLR/CPL, RL/RR/RLC/RRC |
| Bit ops | SETB/CLR/CPL, ANL C/ORL C, MOV C |
| Branches | SJMP/AJMP/LJMP, JZ/JNZ/JC/JNC, JB/JNB/JBC, CJNE/DJNZ, ACALL/LCALL, RET/RETI |
| SFRs | P0–P3, PSW, ACC, B, SP, DPL/DPH, TCON, TMOD, TH0/TL0/TH1/TL1, SCON, SBUF, IE, IP |
| Timers | T0/T1 count per step while TRx=1; SBUF write → char out + TI |
| Interrupts | INT0/INT1, TF0/TF1, serial; vectors 03/0B/13/1B/23h; priority via IP |
| I/O | MOVX @DPTR/A to top 256 B of XDATA = I/O ports; P0–P3 quasi-bidirectional |

## Disassembler

The `Disasm` view works for **all three ISAs**. Unrecognized opcodes fall back
to `DB xxh`. 8051 16-bit addresses (LJMP/LCALL/MOV DPTR) are big-endian; code is
fetched from internal `code` when `EA=1` and from external XDATA when `EA=0`.
