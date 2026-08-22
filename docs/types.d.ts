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

/** One decoded instruction for the disassembler view. */
export interface DisasmLine {
  /** Linear address of the instruction. */
  addr: number;
  /** Raw machine-code bytes. */
  bytes: Uint8Array;
  /** Human-readable mnemonic + operands, e.g. "MOV AX, 1". */
  text: string;
}

/** 8086 graphics-mode framebuffer descriptor. */
export interface GfxInfo {
  /** Start of the framebuffer in linear memory. */
  base: number;
  /** Width in pixels. */
  w: number;
  /** Height in pixels. */
  h: number;
  /** Bytes per pixel (1 for mode 13h). */
  bpp: number;
}

export type IsaName = '8086' | '8085' | '8051' | '6502' | 'Z80' | 'rv32';

export class Emulator {
  /** Create an emulator. Throws on an unknown ISA name. */
  constructor(isa: IsaName | string);

  /** Assemble source into machine code. Rejects with a message on error. */
  assemble(source: string): Promise<Uint8Array> | Uint8Array;
  /** Per-source-line "ADDR  BYTES" strings for the IDE gutter. */
  assemble_info(source: string): Promise<string[]> | string[];

  /** Load machine code into memory at `origin` (0 for 8085/8051/Z80/6502/rv32, 0x100 for 8086). */
  load(code: Uint8Array, origin: number): void;

  /** Execute one instruction. Returns false if the CPU halted. */
  step(): void;
  /** Run up to `maxSteps` instructions; returns the number actually executed. */
  run(maxSteps: number): number;
  /** Run until `targetPc` is the next instruction (or `maxSteps` reached). */
  run_to(targetPc: number, maxSteps: number): number;
  /** Run until one of `bps` (addresses) is hit. */
  run_bp(maxSteps: number, bps: number[]): number;

  /** Current program counter. */
  pc(): number;
  /** Register dump as "NAME=value" strings. */
  regs(): string[];
  /** Active flag names, e.g. ["ZF", "CY"]. */
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
  /** 8086 graphics framebuffer descriptor, or null for non-graphics modes. */
  gfx(): GfxInfo | null;
  /** Text-mode cursor as [col, row]. */
  cursor(): Uint8Array;
  /** Current 8086 BIOS video mode number. */
  video_mode(): number;

  // ---- I/O ----
  /** Read a byte from the 256-entry port space (or P0-P3 for 8051). */
  port_read(port: number): number;
  /** Write a byte to the port space. */
  port_write(port: number, val: number): void;
  /** 8051: inject a received serial byte (SBUF + RI). */
  serial_rx(ch: number): void;
  /** 8085: drive the SID input pin (read by RIM bit 7). */
  set_sid(v: boolean): void;
  /** 8085: read the SOD output pin (set by SIM bit 7). */
  sod(): number;

  // ---- interrupts ----
  /** Raise a hardware interrupt. Kind depends on ISA (e.g. NMI/INT for Z80, TRAP/INT0 for 8051/8085, NMI/INTR for 6502). */
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
}

export function init(module_or_path?: unknown): Promise<unknown>;
