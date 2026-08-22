/* tslint:disable */
/* eslint-disable */

export class Emulator {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Assemble source for the current ISA; returns machine code bytes.
     */
    assemble(source: string): Uint8Array;
    /**
     * Assemble and return per-line machine code as "ADDR  BYTES" strings
     * (one per source line, empty for lines that emit nothing).
     */
    assemble_info(source: string): string[];
    /**
     * 8086 text cursor as [col, row]; [0,0] otherwise.
     */
    cursor(): Uint8Array;
    /**
     * Total clock cycles executed (machine cycles / T-states). Drives the
     * cycle-accurate timers (8086 PIT, 8051 timers, 8085 8155 timer).
     */
    cycles(): bigint;
    /**
     * Disassemble `count` instructions starting at `addr`. Each returned line
     * is "ADDR  BYTES  text" (use `Disasm::line`). Other ISAs return [].
     */
    disasm(addr: number, count: number): string[];
    /**
     * 8051 EA pin state (true = internal code, false = external via XDATA).
     */
    ea_active(): boolean;
    /**
     * 8051 external-code (XDATA) region (base, len) when EA is low, else null.
     */
    ext_code_region(): Uint32Array | undefined;
    /**
     * Active flag names as short strings (e.g. "ZF", "CY").
     */
    flags(): string[];
    /**
     * Read a file back from the 8086 DOS virtual filesystem (empty if absent).
     */
    fs_get(name: string): Uint8Array | undefined;
    /**
     * Preload a file into the 8086 DOS virtual filesystem.
     */
    fs_put(name: string, data: Uint8Array): void;
    /**
     * 8086 graphics framebuffer, or None when in a text mode / non-8086 ISA.
     */
    gfx(): GfxInfo | undefined;
    /**
     * True once the CPU has executed HLT (or otherwise stopped).
     */
    halted(): boolean;
    /**
     * Hardware interrupt: 8085 = "TRAP" | "RST75" | "RST65" | "RST55" |
     * "INTR" (data = vector); 8051 = "INT0" | "INT1". Throws on unknown kind.
     */
    interrupt(kind: string, data: number): void;
    /**
     * Load raw machine code at `origin` and set PC there.
     */
    load(code: Uint8Array, origin: number): void;
    /**
     * Load a ROM image and mark its range read-only. 8051 routes to external
     * code (XDATA) when EA is low.
     */
    load_rom(data: Uint8Array, addr: number): void;
    /**
     * Linear memory read of `len` bytes starting at `addr`.
     */
    mem(addr: number, len: number): Uint8Array;
    /**
     * Write bytes into memory (IDE memory poking).
     */
    mem_write(addr: number, data: Uint8Array): void;
    /**
     * Create an emulator for one of: "8086", "8085", "8051", "6502", "Z80", "rv32".
     * Throws if the ISA name is unknown.
     */
    constructor(isa: string);
    /**
     * Drain the program output buffer.
     */
    out(): string;
    /**
     * Current program counter (instruction pointer).
     */
    pc(): number;
    /**
     * Current reload/count of an 8086 PIT channel (0..2). Other ISAs: 0.
     */
    pit_count(n: number): number;
    /**
     * Queue a key for the 8086's INT 21h keyboard reads (AH=01/06/07/08/0C).
     */
    port_read(port: number): number;
    /**
     * Write an I/O port byte (8085/8086: port space 0-255; 8051: P0-P3 pins).
     */
    port_write(port: number, val: number): void;
    /**
     * Queue a type-ahead character for INT 21h/keyboard reads (8086).
     */
    push_key(ch: number): void;
    /**
     * Register dump as "NAME=value" strings (e.g. "AX=1234").
     */
    regs(): string[];
    /**
     * Reset the CPU to its initial state (registers, flags, PC, memory preserved).
     */
    reset(): void;
    /**
     * Restore a previously captured `snapshot()` (state must match the ISA).
     */
    restore(data: Uint8Array): void;
    /**
     * Write-protected ROM region (base, len) if configured, else null.
     */
    rom_region(): Uint32Array | undefined;
    /**
     * Run up to `max_steps` instructions; returns steps executed.
     */
    run(max_steps: number): number;
    /**
     * Run until PC lands on one of `bps` (that instruction is NOT executed),
     * or halt / blocked on input / max steps. Returns steps executed.
     */
    run_bp(max_steps: number, bps: Uint32Array): number;
    /**
     * Run until `target` is the next instruction to execute (not executed),
     * or halt / blocked on input / max steps. Returns steps executed.
     */
    run_to(target_pc: number, max_steps: number): number;
    /**
     * 8086 text-mode framebuffer (80x25 char/attr pairs at 0xB8000); [] otherwise.
     */
    screen(): Uint8Array;
    /**
     * Inject a received serial byte into the 8051 (sets SBUF + RI).
     */
    serial_rx(ch: number): void;
    /**
     * Set the emulated DOS/BIOS date-time clock (INT 21h 2Ah/2Ch, INT 1Ah).
     */
    set_clock(year: number, month: number, day: number, hour: number, min: number, sec: number): void;
    /**
     * 8051 EA pin: false => fetch code from external program memory (XDATA).
     */
    set_ea(ea: boolean): void;
    /**
     * Set the Z80 interrupt mode (0/1 -> 0x0038, 2 -> I*0x100 + data).
     */
    set_interrupt_mode(m: number): void;
    /**
     * Set the program counter (entry point after load).
     */
    set_pc(addr: number): void;
    /**
     * Set a register by name (e.g. "AX", "PC", "R0"). Used by the IDE watch
     * window for click-to-edit. Ignored for names the ISA does not expose.
     */
    set_reg(name: string, val: number): void;
    /**
     * Mark `[base, base+len)` of main memory as read-only ROM (8086/8085).
     */
    set_rom_region(base: number, len: number): void;
    /**
     * Write an 8051 SFR / IRAM byte (peripheral-register editor).
     */
    set_sfr(addr: number, v: number): void;
    /**
     * Set the 8085 SID (Serial Input Data) pin read by RIM (bit 7). 8085 only.
     */
    set_sid(v: boolean): void;
    /**
     * 8085: (re)configure the external SRAM chip window (default 8 KiB @ 0x9000).
     */
    set_sram(base: number, len: number): void;
    /**
     * Read an 8051 SFR / IRAM byte (peripheral-register readout).
     */
    sfr(addr: number): number;
    /**
     * Deterministic serialization of full CPU state (for save/restore and step-back).
     */
    snapshot(): Uint8Array;
    /**
     * Read the 8085 SOD (Serial Output Data) pin set by SIM (bit 7). 8085 only.
     */
    sod(): number;
    /**
     * External SRAM window (base, len) if configured (8055), else null.
     */
    sram_region(): Uint32Array | undefined;
    /**
     * Execute one instruction.
     */
    step(): void;
    /**
     * Current 8086 video mode (0 when not 8086 / unknown). MR=13h -> pixel graphics.
     */
    video_mode(): number;
    /**
     * True while the 8086 is blocked on an INT 21h read with an empty buffer.
     */
    waiting_input(): boolean;
}

/**
 * Graphics framebuffer descriptor (8086 pixel modes). `base` is the linear
 * memory address of the pixel data; `w`/`h` are the dimensions in pixels.
 */
export class GfxInfo {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    base: number;
    h: number;
    w: number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_emulator_free: (a: number, b: number) => void;
    readonly __wbg_get_gfxinfo_base: (a: number) => number;
    readonly __wbg_get_gfxinfo_h: (a: number) => number;
    readonly __wbg_get_gfxinfo_w: (a: number) => number;
    readonly __wbg_gfxinfo_free: (a: number, b: number) => void;
    readonly __wbg_set_gfxinfo_base: (a: number, b: number) => void;
    readonly __wbg_set_gfxinfo_h: (a: number, b: number) => void;
    readonly __wbg_set_gfxinfo_w: (a: number, b: number) => void;
    readonly emulator_assemble: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_assemble_info: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_cursor: (a: number) => [number, number];
    readonly emulator_cycles: (a: number) => bigint;
    readonly emulator_disasm: (a: number, b: number, c: number) => [number, number];
    readonly emulator_ea_active: (a: number) => number;
    readonly emulator_ext_code_region: (a: number) => [number, number];
    readonly emulator_flags: (a: number) => [number, number];
    readonly emulator_fs_get: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_fs_put: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly emulator_gfx: (a: number) => number;
    readonly emulator_halted: (a: number) => number;
    readonly emulator_interrupt: (a: number, b: number, c: number, d: number) => [number, number];
    readonly emulator_load: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_load_rom: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_mem: (a: number, b: number, c: number) => [number, number];
    readonly emulator_mem_write: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_new: (a: number, b: number) => [number, number, number];
    readonly emulator_out: (a: number) => [number, number];
    readonly emulator_pc: (a: number) => number;
    readonly emulator_pit_count: (a: number, b: number) => number;
    readonly emulator_port_read: (a: number, b: number) => number;
    readonly emulator_port_write: (a: number, b: number, c: number) => void;
    readonly emulator_push_key: (a: number, b: number) => void;
    readonly emulator_regs: (a: number) => [number, number];
    readonly emulator_reset: (a: number) => void;
    readonly emulator_restore: (a: number, b: number, c: number) => void;
    readonly emulator_rom_region: (a: number) => [number, number];
    readonly emulator_run: (a: number, b: number) => number;
    readonly emulator_run_bp: (a: number, b: number, c: number, d: number) => number;
    readonly emulator_run_to: (a: number, b: number, c: number) => number;
    readonly emulator_screen: (a: number) => [number, number];
    readonly emulator_serial_rx: (a: number, b: number) => [number, number];
    readonly emulator_set_clock: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly emulator_set_ea: (a: number, b: number) => void;
    readonly emulator_set_interrupt_mode: (a: number, b: number) => [number, number];
    readonly emulator_set_pc: (a: number, b: number) => void;
    readonly emulator_set_reg: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_set_rom_region: (a: number, b: number, c: number) => void;
    readonly emulator_set_sfr: (a: number, b: number, c: number) => void;
    readonly emulator_set_sid: (a: number, b: number) => void;
    readonly emulator_set_sram: (a: number, b: number, c: number) => void;
    readonly emulator_sfr: (a: number, b: number) => number;
    readonly emulator_snapshot: (a: number) => [number, number];
    readonly emulator_sod: (a: number) => number;
    readonly emulator_sram_region: (a: number) => [number, number];
    readonly emulator_step: (a: number) => void;
    readonly emulator_video_mode: (a: number) => number;
    readonly emulator_waiting_input: (a: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
