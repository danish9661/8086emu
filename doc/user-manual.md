# User Manual — multi-cpu-emu IDE

A browser-based emulator + assembler for the **Intel 8086**, **Intel 8085**, and
**Intel 8051 (MCS-51)**. One WASM core, three ISAs, a shared debugger UI.

## Picking an ISA

Use the **ISA** dropdown in the toolbar. Each ISA has its own register set,
memory model, and assembly dialect:

| ISA | Entry point | Memory | Notes |
|---|---|---|---|
| 8086 | `0x100` (as if `ORG 100h`) | 1 MiB, segmented | DOS/BIOS subset |
| 8085 | `0x0000` | 64 KiB, flat | PSW + IN/OUT ports |
| 8051 | `0x0000` | 64 KiB code, 128 B IRAM + SFR | bit-addressable, timers, serial |

The editor resets to a per-ISA starter program when you switch ISA.

## Writing & assembling

1. Type assembly into the editor (line comments with `;`).
2. Press **Assemble** (F7) to assemble + load the program into memory.
   Assembler errors show the offending **line number** in the gutter (red) and
   in the error strip below the editor.
3. The **Disassembly** panel lists every assembled instruction with its address
   and machine bytes.

### Autocomplete

While typing, a popup suggests mnemonics and registers for the current ISA.
Press **↓ / ↑** to navigate, **Enter** or **Tab** to accept, **Esc** to dismiss.
Shift-clicking a suggestion (or hovering a disassembly mnemonic) shows a short
description.

### Share links

**Share Link** encodes the current source into the URL fragment
(`#isa=…&src=…`) and copies it to the clipboard. Opening the link restores the
program automatically. Handy for classroom snippets and bug reports.

## Running

| Button | Shortcut | Action |
|---|---|---|
| Step | F8 | execute one instruction |
| Step-Over | F10 | run a `CALL` as a single step |
| Step-Back | — | restore the previous snapshot (time-travel) |
| Run | F5 | run until halt / Stop / breakpoint |
| Stop | Esc | stop a running program |
| Reset | F4 | reset CPU, devices, and memory |

The status bar shows the live **PC**, **step count**, and **CPU state**.

### Breakpoints

- Click a gutter line (or a disassembly line) to toggle an **unconditional**
  breakpoint (red marker).
- **Shift-click** to attach a **condition** such as `CX==0`, `AX>10`, or
  `mem[0x200]==5`. Conditional breakpoints (amber marker) only stop the CPU
  when the expression is true; the run loop single-steps while any conditional
  breakpoint exists so the condition is checked every instruction.

### Run-to-cursor

Double-click a source or disassembly line to run until that address.

## Watching state

- **Watch**: add `AX`, `AL`, `[0x200]`, `100h`, or `FLAGS` to track values.
  Changed values flash. Entries persist in `localStorage`. Click a watch row to
  edit its value live; it is written back via `set_reg` / `mem_write`.
- **Memory**: set a hex base address and page through 256-byte views. Bytes
  that changed since the last refresh are highlighted. Click a byte to edit it.
- **I/O Ports** / **Peripherals**: inspect and toggle port pins and on-chip
  peripheral registers (8051 ports, 8086 PIC/PIT).
- **Program output**: text printed by `INT 21h`/`INT 10h` (8086), `OUT 01h`
  (8085), or `SBUF` (8051).

## Input & interrupts

- 8086 programs that read from the keyboard (`INT 21h`/`INT 16h`) block until a
  key is provided. Use the **input** dialog (or push keys programmatically).
- Hardware interrupts (NMI/INTR for 8086, TRAP/RST/INTR for 8085, INT0/INT1 for
  8051) can be raised from the toolbar; they are latched and serviced at the end
  of the next `step()`.

## Saving state

**Save State** downloads a deterministic snapshot of the CPU; **Load State**
restores it exactly. Snapshots are used internally for Step-Back (time-travel).

## Command-line (headless)

See `examples/run.rs` for a native runner and autograder:

```bash
cargo run --example run -- examples/hello.asm
cargo run --example run -- --grade grade/add.spec grade/add.asm
```
