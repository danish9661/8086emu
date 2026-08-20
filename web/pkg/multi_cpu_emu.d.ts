/* tslint:disable */
/* eslint-disable */

export class Emulator {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Assemble source for the current ISA; returns machine code bytes.
     */
    assemble(source: string): Uint8Array;
    flags(): string[];
    halted(): boolean;
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
    regs(): string[];
    reset(): void;
    restore(data: Uint8Array): void;
    /**
     * Run up to `max_steps` instructions; returns steps executed.
     */
    run(max_steps: number): number;
    snapshot(): Uint8Array;
    /**
     * Execute one instruction.
     */
    step(): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_emulator_free: (a: number, b: number) => void;
    readonly emulator_assemble: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_flags: (a: number) => [number, number];
    readonly emulator_halted: (a: number) => number;
    readonly emulator_load: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_mem: (a: number, b: number, c: number) => [number, number];
    readonly emulator_new: (a: number, b: number) => [number, number, number];
    readonly emulator_out: (a: number) => [number, number];
    readonly emulator_pc: (a: number) => number;
    readonly emulator_regs: (a: number) => [number, number];
    readonly emulator_reset: (a: number) => void;
    readonly emulator_restore: (a: number, b: number, c: number) => void;
    readonly emulator_run: (a: number, b: number) => number;
    readonly emulator_snapshot: (a: number) => [number, number];
    readonly emulator_step: (a: number) => void;
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
