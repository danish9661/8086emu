/* @ts-self-types="./multi_cpu_emu.d.ts" */

export class Emulator {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EmulatorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_emulator_free(ptr, 0);
    }
    /**
     * Assemble source for the current ISA; returns machine code bytes.
     * @param {string} source
     * @returns {Uint8Array}
     */
    assemble(source) {
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_assemble(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Assemble and return per-line machine code as "ADDR  BYTES" strings
     * (one per source line, empty for lines that emit nothing).
     * @param {string} source
     * @returns {string[]}
     */
    assemble_info(source) {
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_assemble_info(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayJsValueFromWasm0(ret[0], ret[1]);
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * 8086 text-mode cursor as a 2-byte vector [col, row]. Other ISAs: [0, 0].
     * @returns {Uint8Array}
     */
    cursor() {
        const ret = wasm.emulator_cursor(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Total clock cycles executed (machine cycles / T-states). Drives the
     * cycle-accurate timers (8086 PIT, 8051 timers, 8085 8155 timer).
     * @returns {bigint}
     */
    cycles() {
        const ret = wasm.emulator_cycles(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Disassemble `count` instructions starting at `addr`. Each returned line
     * is "ADDR  BYTES  text" (use `Disasm::line`). Other ISAs return [].
     * @param {number} addr
     * @param {number} count
     * @returns {string[]}
     */
    disasm(addr, count) {
        const ret = wasm.emulator_disasm(this.__wbg_ptr, addr, count);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]);
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * 8051 EA pin state (true = internal code, false = external via XDATA).
     * @returns {boolean}
     */
    ea_active() {
        const ret = wasm.emulator_ea_active(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 8051 external-code (XDATA) region (base, len) when EA is low, else null.
     * @returns {Uint32Array | undefined}
     */
    ext_code_region() {
        const ret = wasm.emulator_ext_code_region(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        }
        return v1;
    }
    /**
     * @returns {string[]}
     */
    flags() {
        const ret = wasm.emulator_flags(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]);
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Read a file back from the 8086 DOS virtual filesystem (empty if absent).
     * @param {string} name
     * @returns {Uint8Array | undefined}
     */
    fs_get(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_fs_get(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        let v2;
        if (ret[0] !== 0) {
            v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v2;
    }
    /**
     * Preload a file into the 8086 DOS virtual filesystem.
     * @param {string} name
     * @param {Uint8Array} data
     */
    fs_put(name, data) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_fs_put(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {boolean}
     */
    halted() {
        const ret = wasm.emulator_halted(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Hardware interrupt: 8085 = "TRAP" | "RST75" | "RST65" | "RST55" |
     * "INTR" (data = vector); 8051 = "INT0" | "INT1". Throws on unknown kind.
     * @param {string} kind
     * @param {number} data
     */
    interrupt(kind, data) {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_interrupt(this.__wbg_ptr, ptr0, len0, data);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Load raw machine code at `origin` and set PC there.
     * @param {Uint8Array} code
     * @param {number} origin
     */
    load(code, origin) {
        const ptr0 = passArray8ToWasm0(code, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_load(this.__wbg_ptr, ptr0, len0, origin);
    }
    /**
     * Load a ROM image and mark its range read-only. 8051 routes to external
     * code (XDATA) when EA is low.
     * @param {Uint8Array} data
     * @param {number} addr
     */
    load_rom(data, addr) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_load_rom(this.__wbg_ptr, ptr0, len0, addr);
    }
    /**
     * @param {number} addr
     * @param {number} len
     * @returns {Uint8Array}
     */
    mem(addr, len) {
        const ret = wasm.emulator_mem(this.__wbg_ptr, addr, len);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Write bytes into memory (IDE memory poking).
     * @param {number} addr
     * @param {Uint8Array} data
     */
    mem_write(addr, data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_mem_write(this.__wbg_ptr, addr, ptr0, len0);
    }
    /**
     * @param {string} isa
     */
    constructor(isa) {
        const ptr0 = passStringToWasm0(isa, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        EmulatorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Drain the program output buffer.
     * @returns {string}
     */
    out() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.emulator_out(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    pc() {
        const ret = wasm.emulator_pc(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Current reload/count of an 8086 PIT channel (0..2). Other ISAs: 0.
     * @param {number} n
     * @returns {number}
     */
    pit_count(n) {
        const ret = wasm.emulator_pit_count(this.__wbg_ptr, n);
        return ret;
    }
    /**
     * Queue a key for the 8086's INT 21h keyboard reads (AH=01/06/07/08/0C).
     * @param {number} port
     * @returns {number}
     */
    port_read(port) {
        const ret = wasm.emulator_port_read(this.__wbg_ptr, port);
        return ret;
    }
    /**
     * Write an I/O port byte (8085/8086: port space 0-255; 8051: P0-P3 pins).
     * @param {number} port
     * @param {number} val
     */
    port_write(port, val) {
        wasm.emulator_port_write(this.__wbg_ptr, port, val);
    }
    /**
     * @param {number} ch
     */
    push_key(ch) {
        wasm.emulator_push_key(this.__wbg_ptr, ch);
    }
    /**
     * @returns {string[]}
     */
    regs() {
        const ret = wasm.emulator_regs(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]);
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    reset() {
        wasm.emulator_reset(this.__wbg_ptr);
    }
    /**
     * @param {Uint8Array} data
     */
    restore(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_restore(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Write-protected ROM region (base, len) if configured, else null.
     * @returns {Uint32Array | undefined}
     */
    rom_region() {
        const ret = wasm.emulator_rom_region(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        }
        return v1;
    }
    /**
     * Run up to `max_steps` instructions; returns steps executed.
     * @param {number} max_steps
     * @returns {number}
     */
    run(max_steps) {
        const ret = wasm.emulator_run(this.__wbg_ptr, max_steps);
        return ret >>> 0;
    }
    /**
     * Run until PC lands on one of `bps` (that instruction is NOT executed),
     * or halt / blocked on input / max steps. Returns steps executed.
     * @param {number} max_steps
     * @param {Uint32Array} bps
     * @returns {number}
     */
    run_bp(max_steps, bps) {
        const ptr0 = passArray32ToWasm0(bps, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_run_bp(this.__wbg_ptr, max_steps, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Run until `target` is the next instruction to execute (not executed),
     * or halt / blocked on input / max steps. Returns steps executed.
     * @param {number} target_pc
     * @param {number} max_steps
     * @returns {number}
     */
    run_to(target_pc, max_steps) {
        const ret = wasm.emulator_run_to(this.__wbg_ptr, target_pc, max_steps);
        return ret >>> 0;
    }
    /**
     * 8086 text-mode screen: 4000 bytes (80x25 cells of char, attr). Other ISAs
     * return an empty vector.
     * @returns {Uint8Array}
     */
    screen() {
        const ret = wasm.emulator_screen(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Inject a received serial byte into the 8051 (sets SBUF + RI).
     * @param {number} ch
     */
    serial_rx(ch) {
        const ret = wasm.emulator_serial_rx(this.__wbg_ptr, ch);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set the emulated DOS/BIOS date-time clock (INT 21h 2Ah/2Ch, INT 1Ah).
     * @param {number} year
     * @param {number} month
     * @param {number} day
     * @param {number} hour
     * @param {number} min
     * @param {number} sec
     */
    set_clock(year, month, day, hour, min, sec) {
        const ret = wasm.emulator_set_clock(this.__wbg_ptr, year, month, day, hour, min, sec);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * 8051 EA pin: false => fetch code from external program memory (XDATA).
     * @param {boolean} ea
     */
    set_ea(ea) {
        wasm.emulator_set_ea(this.__wbg_ptr, ea);
    }
    /**
     * Set the program counter (entry point after load).
     * @param {number} addr
     */
    set_pc(addr) {
        wasm.emulator_set_pc(this.__wbg_ptr, addr);
    }
    /**
     * Set a register by name (e.g. "AX", "PC", "R0"). Used by the IDE watch
     * window for click-to-edit. Ignored for names the ISA does not expose.
     * @param {string} name
     * @param {number} val
     */
    set_reg(name, val) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_set_reg(this.__wbg_ptr, ptr0, len0, val);
    }
    /**
     * Mark `[base, base+len)` of main memory as read-only ROM (8086/8085).
     * @param {number} base
     * @param {number} len
     */
    set_rom_region(base, len) {
        wasm.emulator_set_rom_region(this.__wbg_ptr, base, len);
    }
    /**
     * Write an 8051 SFR / IRAM byte (peripheral-register editor).
     * @param {number} addr
     * @param {number} v
     */
    set_sfr(addr, v) {
        wasm.emulator_set_sfr(this.__wbg_ptr, addr, v);
    }
    /**
     * Set the 8085 SID (Serial Input Data) pin read by RIM (bit 7). 8085 only.
     * @param {boolean} v
     */
    set_sid(v) {
        wasm.emulator_set_sid(this.__wbg_ptr, v);
    }
    /**
     * 8085: (re)configure the external SRAM chip window (default 8 KiB @ 0x9000).
     * @param {number} base
     * @param {number} len
     */
    set_sram(base, len) {
        wasm.emulator_set_sram(this.__wbg_ptr, base, len);
    }
    /**
     * Read an 8051 SFR / IRAM byte (peripheral-register readout).
     * @param {number} addr
     * @returns {number}
     */
    sfr(addr) {
        const ret = wasm.emulator_sfr(this.__wbg_ptr, addr);
        return ret;
    }
    /**
     * @returns {Uint8Array}
     */
    snapshot() {
        const ret = wasm.emulator_snapshot(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Read the 8085 SOD (Serial Output Data) pin set by SIM (bit 7). 8085 only.
     * @returns {number}
     */
    sod() {
        const ret = wasm.emulator_sod(this.__wbg_ptr);
        return ret;
    }
    /**
     * External SRAM window (base, len) if configured (8055), else null.
     * @returns {Uint32Array | undefined}
     */
    sram_region() {
        const ret = wasm.emulator_sram_region(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        }
        return v1;
    }
    /**
     * Execute one instruction.
     */
    step() {
        wasm.emulator_step(this.__wbg_ptr);
    }
    /**
     * True while the 8086 is blocked on an INT 21h read with an empty buffer.
     * @returns {boolean}
     */
    waiting_input() {
        const ret = wasm.emulator_waiting_input(this.__wbg_ptr);
        return ret !== 0;
    }
}
if (Symbol.dispose) Emulator.prototype[Symbol.dispose] = Emulator.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_bb96b2010945f0bc: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./multi_cpu_emu_bg.js": import0,
    };
}

const EmulatorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_emulator_free(ptr, 1));

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(wasm.__wbindgen_externrefs.get(mem.getUint32(i, true)));
    }
    wasm.__externref_drop_slice(ptr, len);
    return result;
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('multi_cpu_emu_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
