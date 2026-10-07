/**
 * Hand-written, consumer-facing type surface for the `multi_cpu_emu` WASM module.
 *
 * The auto-generated `pkg/multi_cpu_emu.d.ts` is authoritative for the raw JS
 * bindings, but it carries wasm-bindgen internals (`__wbg_*`, `GfxInfo`
 * internals, etc.). This file is a clean, documented view of the public API and
 * is the file an npm package should point `package.json#types` at.
 *
 * Usage (after `wasm-pack` build into `pkg/`):
 *   import init, { Emulator } from './pkg/multi_cpu_emu.js';
 *   await init();
 *   const emu = new Emulator('8086');
 */

/** 8086 graphics-mode framebuffer descriptor (matches the live `GfxInfo` class:
 *  `base`/`w`/`h` fields only — there is no `bpp`, and it is `undefined`
 *  (not `null`) when no pixel mode is active). */
export interface GfxInfo {
  /** Start of the framebuffer in linear memory. */
  base: number;
  /** Width in pixels. */
  w: number;
  /** Height in pixels. */
  h: number;
}

/** ISA names accepted by `new Emulator(isa)` (case-insensitive; `"8088"` is the 8086 core). */
export type IsaName = '8086' | '8088' | '8085' | '8051' | '6502' | 'Z80' | 'rv32';

export class Emulator {
  /** Create an emulator. Throws on an unknown ISA name. */
  constructor(isa: IsaName | string);

  /** Assemble source into machine code. Rejects with a message on error. */
  assemble(source: string): Promise<Uint8Array> | Uint8Array;
  /** Per-source-line "ADDR  BYTES" strings for the IDE gutter. */
  assemble_info(source: string): Promise<string[]> | string[];

  /** Load machine code into memory at `origin` (0 for 8085/8051/Z80/6502/rv32, 0x100 for 8086). */
  load(code: Uint8Array, origin: number): void;

  /** Execute one instruction. */
  step(): void;
  /** Run up to `maxSteps` instructions; returns the number actually executed. */
  run(maxSteps: number): number;
  /** Run until `targetPc` is the next instruction (or `maxSteps` reached). */
  run_to(targetPc: number, maxSteps: number): number;
  /** Run until one of `bps` (addresses) is hit. */
  run_bp(maxSteps: number, bps: number[]): number;

  /** Current program counter. */
  pc(): number;
  /** Register dump as "NAME=value" strings (4-digit hex; rv32 prints 8 digits). */
  regs(): string[];
  /** Active flag names in 8086 naming, e.g. ["ZF", "CF"] regardless of ISA. */
  flags(): string[];
  /** Set a register by name (e.g. "AX", "PC"). */
  set_reg(name: string, val: number): void;
  /** Set the program counter. */
  set_pc(addr: number): void;

  /** Linear memory read. */
  mem(addr: number, len: number): Uint8Array;
  /** Linear memory write (used by the IDE memory editor / pokes). */
  mem_write(addr: number, data: Uint8Array): void;
  /** Disassemble `count` instructions starting at `addr`. */
  disasm(addr: number, count: number): string[];

  // ---- 8086 graphics ----
  /** 80x25 text framebuffer bytes (char,attr pairs) or [] for other ISAs. */
  screen(): Uint8Array;
  /** 8086 graphics framebuffer descriptor, or undefined for non-graphics modes. */
  gfx(): GfxInfo | undefined;
  /** Text-mode cursor as [col, row]. */
  cursor(): Uint8Array;
  /** Current 8086 BIOS video mode number. */
  video_mode(): number;

  // ---- I/O ----
  /** Read a byte from the port space (8085/8086 ports; 8051 P0-P3; Z80 latch/CTC; rv32 0xE0 = GPIO DATA, 0xE1 = DIR; 6502: use via_read). */
  port_read(port: number): number;
  /** Write a byte to the port space (rv32 0xE0 injects GPIO input pins; 6502: use via_write/via_pins). */
  port_write(port: number, val: number): void;
  /** 8086/8085: inject 8255 PPI external input levels (0xE0/0xE1/0xE2 = A/B/C) without touching the output latch. */
  ppi_set_input(port: number, val: number): void;
  /** 8051: inject a received serial byte (SBUF + RI). */
  serial_rx(ch: number): void;
  /** Universal serial-RX hook, routed per ISA: 8086/8085/Z80 kit USART, 6502 board ACIA ($5000), 8051 SBUF, rv32 board UART (0xF0000). */
  usart_rx(v: number): void;
  /** Serial status for the same routing (bit0 TxRDY, bit1 RxRDY, bit2 TxEMPTY). */
  usart_status(): number;
  /** 8085: drive the SID input pin (read by RIM bit 7). */
  set_sid(v: boolean): void;
  /** 8085: read the SOD output pin (set by SIM bit 7). */
  sod(): number;
  /** Cycle clock, nonzero on all six ISAs (host sim-time; timers derive from it). */
  cycles(): bigint;

  // ---- interrupts ----
  /** Raise a hardware interrupt. 8086: NMI/INTR+vector; 8085: TRAP/RST75/RST65/RST55/INTR+vector; 8051: INT0/INT1; 6502: NMI (else IRQ); Z80: NMI (else INT); rv32: throws. */
  interrupt(kind: string, data: number): void;
  /** Z80: set the interrupt mode (0/1/2). */
  set_interrupt_mode(m: number): void;

  // ---- misc ----
  /** Take and clear the accumulated program output (INT 21h / OUT 01h / SBUF). */
  out(): string;
  /** True if the CPU has halted. */
  halted(): boolean;
  /** Reset registers/flags/PC (memory preserved). */
  reset(): void;
  /** 8086: queue a type-ahead character for INT 21h reads. */
  push_key(ch: number): void;
  /** True if the CPU is blocked waiting for keyboard input. */
  waiting_input(): boolean;
  /** Full deterministic state snapshot (for save/step-back). */
  snapshot(): Uint8Array;
  /** Restore a snapshot captured by `snapshot()`. */
  restore(data: Uint8Array): void;

  // ---- circuit-board helpers (OpenHW-style platforms) ----
  /** Total code/main memory in bytes (8086: 1 MiB, 8085/8051-code/6502/Z80: 64 KiB, rv32: 1 MiB). */
  mem_size(): number;
  /** Inject external pin levels on 6502 VIA port A (0) / B (1). */
  via_pins(port: number, v: number): void;
  /** Full 256-byte 8051 I2C EEPROM image (undefined for other ISAs). */
  i2c_dump(): Uint8Array | undefined;
  /** Write an RTC register via CMOS ports 0x70/0x71 (8086/8085). */
  rtc_write(reg: number, val: number): void;
}

export function init(module_or_path?: unknown): Promise<unknown>;
