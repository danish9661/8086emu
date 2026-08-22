# API Reference

The public surface is the `Emulator` facade. The **WebAssembly** build
(`feature = "wasm"`) exposes a `wasm-bindgen` class; the **native** build
exposes the same methods on `multi_cpu_emu::Emulator` (returns `Vec<u8>` /
`String` instead of `Result<…, JsValue>`, and `assemble` returns
`Result<Vec<u8>, String>` on error).

## Construction & assembly

| Method | Signature | Notes |
|--------|-----------|-------|
| `new` / `make_emulator` | `(isa: &str) -> Emulator` | `isa` ∈ `"8086"` \| `"8085"` \| `"8051"` |
| `assemble` | `(source: &str) -> bytes` | error on bad syntax (line-numbered) |
| `assemble_info` | `(source: &str) -> Vec<String>` | one `"ADDR  BYTES"` line per source line (IDE gutter) |

## Loading & execution

| Method | Signature | Notes |
|--------|-----------|-------|
| `load` | `(code: &[u8], origin: u32)` | place code in memory, set PC to `origin` |
| `load_rom` | `(data: &[u8], addr: u32)` | mark external ROM (8051: also forces `EA=0`) |
| `set_pc` | `(addr: u32)` | set entry point |
| `step` | `()` | one instruction |
| `run` | `(max_steps: u32) -> u32` | steps executed |
| `run_to` | `(target_pc, max_steps) -> u32` | stop when `target` is next (step-over / run-to-line) |
| `run_bp` | `(max_steps, bps: Vec<u32>) -> u32` | stop when PC hits a breakpoint (not executed) |
| `set_reg` | `(name: &str, val: u32)` | set AX/PC/R0/… (ignored if ISA lacks it) |
| `reset` | `()` | hard reset to power-on state |

## Inspection

| Method | Signature | Notes |
|--------|-----------|-------|
| `pc` | `() -> u32` | 8086 returns linear (CS<<4+IP) |
| `regs` | `() -> Vec<String>` | e.g. `"AX=0008"` |
| `flags` | `() -> Vec<String>` | active flag labels, e.g. `"ZF"` |
| `mem` | `(addr, len) -> Vec<u8>` | linear read |
| `disasm` | `(addr, count) -> Vec<String>` | `"ADDR  BYTES  text"` lines for all 3 ISAs |
| `out` | `() -> String` | take program output (clears buffer) |
| `halted` / `waiting_input` | `() -> bool` | run-state queries |
| `screen` / `cursor` | `() -> Vec<u8>` | 8086 text-mode screen buffer (80×25×2) |

## Memory & devices

| Method | Signature | Notes |
|--------|-----------|-------|
| `mem_write` | `(addr, data)` | IDE memory poke |
| `set_rom_region` / `set_sram` | `(base, len)` | mark read-only ROM / external SRAM |
| `port_read` / `port_write` | `(port: u8, val: u8)` | 8085/8086 I/O space; 8051 P0–P3 |
| `sfr` / `set_sfr` | `(addr: u8, val: u8)` | 8051 special function registers |
| `set_ea` | `(bool)` | 8051 external-code select |
| `interrupt` | `(kind, data)` | 8085 TRAP/RST75/RST65/RST55/INTR; 8051 INT0/INT1; 8086 NMI/INTR |
| `push_key` | `(ch: u8)` | 8086 type-ahead keyboard input |
| `serial_rx` | `(ch: u8)` | 8051 received byte (SBUF + RI) |
| `set_sid` / `sod` | `(bool)` / `() -> u8` | 8085 SID input / SOD output pins |
| `cycles` / `pit_count` | `() -> u64` / `(n) -> u16` | cycle counter / 8253 channel count |

## State save / load

| Method | Signature |
|--------|-----------|
| `snapshot` | `() -> Vec<u8>` |
| `restore` | `(data: &[u8])` |

## Host services (8086 DOS)

| Method | Signature | Notes |
|--------|-----------|-------|
| `fs_put` / `fs_get` | `(name, data)` | simple virtual filesystem for INT 21h file ops |
| `set_clock` | `(year, month, day, hour, min, sec)` | DOS date/time |

### Example (WASM)

```js
const emu = new Emulator('8051');
const code = emu.assemble('MOV A, #05h\nADD A, #03h\nEND');
emu.load(code, 0);
emu.step();
console.log(emu.disasm(emu.pc(), 8)); // ["00003  ...  ADD A,#$03", ...]
```
