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
    flags(): string[];
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
    mem(addr: number, len: number): Uint8Array;
    constructor(isa: string);
    /**
     * Drain the program output buffer.
     */
    out(): string;
    pc(): number;
    /**
     * Queue a key for the 8086's INT 21h keyboard reads (AH=01/06/07/08/0C).
     */
    push_key(ch: number): void;
    regs(): string[];
    reset(): void;
    restore(data: Uint8Array): void;
    /**
     * Run up to `max_steps` instructions; returns steps executed.
     */
    run(max_steps: number): number;
    /**
     * Run until `target` is the next instruction to execute (not executed),
     * or halt / blocked on input / max steps. Returns steps executed.
     */
    run_to(target_pc: number, max_steps: number): number;
    snapshot(): Uint8Array;
    /**
     * Execute one instruction.
     */
    step(): void;
    /**
     * True while the 8086 is blocked on an INT 21h read with an empty buffer.
     */
    waiting_input(): boolean;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_emulator_free: (a: number, b: number) => void;
    readonly emulator_assemble: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_assemble_info: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_flags: (a: number) => [number, number];
    readonly emulator_halted: (a: number) => number;
    readonly emulator_interrupt: (a: number, b: number, c: number, d: number) => [number, number];
    readonly emulator_load: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_mem: (a: number, b: number, c: number) => [number, number];
    readonly emulator_new: (a: number, b: number) => [number, number, number];
    readonly emulator_out: (a: number) => [number, number];
    readonly emulator_pc: (a: number) => number;
    readonly emulator_push_key: (a: number, b: number) => void;
    readonly emulator_regs: (a: number) => [number, number];
    readonly emulator_reset: (a: number) => void;
    readonly emulator_restore: (a: number, b: number, c: number) => void;
    readonly emulator_run: (a: number, b: number) => number;
    readonly emulator_run_to: (a: number, b: number, c: number) => number;
    readonly emulator_snapshot: (a: number) => [number, number];
    readonly emulator_step: (a: number) => void;
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
