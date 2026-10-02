/* tslint:disable */
/* eslint-disable */

export class Emulator {
    free(): void;
    [Symbol.dispose](): void;
    adc_get(ch: number): number;
    /**
     * ADC0808: set channel voltage (0..255) or read it.
     */
    adc_set(ch: number, val: number): void;
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
     * Live Z80 CTC down-counter of channel `ch` (0 for other ISAs).
     */
    ctc_count(ch: number): number;
    /**
     * Inject one CLK/TRG pulse into Z80 CTC channel `ch` (counter mode).
     */
    ctc_pulse(ch: number): number;
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
     * 8237 DMA status register.
     */
    dma_status(): number;
    /**
     * 8051 EA pin state (true = internal code, false = external via XDATA).
     */
    ea_active(): boolean;
    /**
     * 8051 external-code (XDATA) region (base, len) when EA is low, else null.
     */
    ext_code_region(): Uint32Array | undefined;
    /**
     * Active flag names in the 8086 naming ("CF","ZF","SF","PF","AF","OF",
     * "DF","IF","TF"). Every ISA's state is translated into this canonical
     * set, so e.g. an 8085 carry shows as "CF" and its interrupt-enable as
     * "IF" (see `docs/app.js` FLAG_MAP for the per-ISA display labels).
     */
    flags(): string[];
    /**
     * External Flash/EEPROM window (base, len) if configured.
     */
    flash_region(): Uint32Array | undefined;
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
     * Full 256-byte I2C EEPROM image (8051 only, `undefined` otherwise).
     * Needed by circuit boards to render/save attached EEPROM state.
     */
    i2c_dump(): Uint8Array | undefined;
    i2c_read(addr: number): number;
    /**
     * 8051 I2C EEPROM
     */
    i2c_write(addr: number, data: number): void;
    /**
     * Hardware interrupt: 8086 = "NMI" | "INTR" (data = vector);
     * 8085 = "TRAP" | "RST75" | "RST65" | "RST55" | "INTR" (data = vector);
     * 8051 = "INT0" | "INT1"; 6502 = "NMI" (else IRQ); Z80 = "NMI" (else INT).
     * rv32 has no interrupt model (throws).
     */
    interrupt(kind: string, data: number): void;
    /**
     * 8279 display RAM (8 bytes) + push key.
     */
    kb_disp(): Uint8Array | undefined;
    kb_push(k: number): void;
    /**
     * HD44780 LCD text (two 16-char lines) if present.
     */
    lcd_text(): string[] | undefined;
    /**
     * Load raw machine code at `origin` and set PC there.
     */
    load(code: Uint8Array, origin: number): void;
    load_flash(data: Uint8Array, addr: number): void;
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
     * Total code/main memory for the current ISA in bytes
     * (8086: 1 MiB, 8085/8051-code/6502/Z80/rv32 from the core's `Mem`).
     * Lets circuit boards size memory views without hardcoding per ISA.
     */
    mem_size(): number;
    /**
     * Write bytes into memory (IDE memory poking).
     */
    mem_write(addr: number, data: Uint8Array): void;
    /**
     * Create an emulator for one of: "8086" (or "8088", same core), "8085", "8051", "6502", "Z80", "rv32".
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
     * 8255 PPI state [PA, PB, PC, ctrl] if present.
     */
    ppi(): Uint8Array | undefined;
    /**
     * Queue a type-ahead character for INT 21h/keyboard reads (8086).
     */
    push_key(ch: number): void;
    /**
     * Register dump as "NAME=value" strings (e.g. "AX=1234"). rv32 registers
     * print full 8-digit hex; 16-bit ISAs print 4 digits.
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
     * RTC register read via CMOS ports 0x70/0x71.
     */
    rtc_reg(reg: number): number;
    /**
     * Write an RTC register via CMOS ports 0x70/0x71 (8086/8085).
     * The read half already exists as `rtc_reg`; boards need the write half
     * to set the clock without going through port I/O.
     */
    rtc_write(reg: number, val: number): void;
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
    set_flash(base: number, len: number): void;
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
     * PC speaker output level (8086 port 61h gate+enable AND channel 2 OUT).
     */
    speaker(): number;
    /**
     * Raw port 61h speaker latch (8086 only, 0 otherwise).
     */
    speaker_ctrl(): number;
    spi_read(addr: number): number;
    spi_write(addr: number, data: number): void;
    /**
     * External SRAM window (base, len) if configured (8055), else null.
     */
    sram_region(): Uint32Array | undefined;
    /**
     * Execute one instruction.
     */
    step(): void;
    usart_rx(v: number): void;
    /**
     * 8251 USART status / Rx push.
     */
    usart_status(): number;
    /**
     * Drive a VIA handshake input (`line`: 0 = CA1, 1 = CB1). 6502 only.
     */
    via_handshake(line: number, high: boolean): void;
    /**
     * Current 6522 VIA IRQ line level (0/1).
     */
    via_irq(): number;
    /**
     * Inject external pin levels seen on VIA port A (0) / B (1). 6502 only.
     * Lets a circuit board drive the VIA inputs (quasi-bidirectional merge).
     */
    via_pins(port: number, v: number): void;
    /**
     * Read a MOS 6522 VIA register `rs` (0-15, mapped at $6000). 6502 only.
     */
    via_read(rs: number): number;
    /**
     * Write a MOS 6522 VIA register `rs` (0-15). 6502 only.
     */
    via_write(rs: number, v: number): void;
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
    readonly emulator_adc_get: (a: number, b: number) => number;
    readonly emulator_adc_set: (a: number, b: number, c: number) => void;
    readonly emulator_assemble: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_assemble_info: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_ctc_count: (a: number, b: number) => number;
    readonly emulator_ctc_pulse: (a: number, b: number) => number;
    readonly emulator_cursor: (a: number) => [number, number];
    readonly emulator_cycles: (a: number) => bigint;
    readonly emulator_disasm: (a: number, b: number, c: number) => [number, number];
    readonly emulator_dma_status: (a: number) => number;
    readonly emulator_ea_active: (a: number) => number;
    readonly emulator_ext_code_region: (a: number) => [number, number];
    readonly emulator_flags: (a: number) => [number, number];
    readonly emulator_flash_region: (a: number) => [number, number];
    readonly emulator_fs_get: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_fs_put: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly emulator_gfx: (a: number) => number;
    readonly emulator_halted: (a: number) => number;
    readonly emulator_i2c_dump: (a: number) => [number, number];
    readonly emulator_i2c_read: (a: number, b: number) => number;
    readonly emulator_i2c_write: (a: number, b: number, c: number) => void;
    readonly emulator_interrupt: (a: number, b: number, c: number, d: number) => [number, number];
    readonly emulator_kb_disp: (a: number) => [number, number];
    readonly emulator_kb_push: (a: number, b: number) => void;
    readonly emulator_lcd_text: (a: number) => [number, number];
    readonly emulator_load: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_load_flash: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_load_rom: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_mem: (a: number, b: number, c: number) => [number, number];
    readonly emulator_mem_size: (a: number) => number;
    readonly emulator_mem_write: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_new: (a: number, b: number) => [number, number, number];
    readonly emulator_out: (a: number) => [number, number];
    readonly emulator_pc: (a: number) => number;
    readonly emulator_pit_count: (a: number, b: number) => number;
    readonly emulator_port_read: (a: number, b: number) => number;
    readonly emulator_port_write: (a: number, b: number, c: number) => void;
    readonly emulator_ppi: (a: number) => [number, number];
    readonly emulator_push_key: (a: number, b: number) => void;
    readonly emulator_regs: (a: number) => [number, number];
    readonly emulator_reset: (a: number) => void;
    readonly emulator_restore: (a: number, b: number, c: number) => void;
    readonly emulator_rom_region: (a: number) => [number, number];
    readonly emulator_rtc_reg: (a: number, b: number) => number;
    readonly emulator_rtc_write: (a: number, b: number, c: number) => void;
    readonly emulator_run: (a: number, b: number) => number;
    readonly emulator_run_bp: (a: number, b: number, c: number, d: number) => number;
    readonly emulator_run_to: (a: number, b: number, c: number) => number;
    readonly emulator_screen: (a: number) => [number, number];
    readonly emulator_serial_rx: (a: number, b: number) => [number, number];
    readonly emulator_set_clock: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly emulator_set_ea: (a: number, b: number) => void;
    readonly emulator_set_flash: (a: number, b: number, c: number) => void;
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
    readonly emulator_speaker: (a: number) => number;
    readonly emulator_speaker_ctrl: (a: number) => number;
    readonly emulator_spi_read: (a: number, b: number) => number;
    readonly emulator_spi_write: (a: number, b: number, c: number) => void;
    readonly emulator_sram_region: (a: number) => [number, number];
    readonly emulator_step: (a: number) => void;
    readonly emulator_usart_rx: (a: number, b: number) => void;
    readonly emulator_usart_status: (a: number) => number;
    readonly emulator_via_handshake: (a: number, b: number, c: number) => void;
    readonly emulator_via_irq: (a: number) => number;
    readonly emulator_via_pins: (a: number, b: number, c: number) => void;
    readonly emulator_via_read: (a: number, b: number) => number;
    readonly emulator_via_write: (a: number, b: number, c: number) => void;
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
