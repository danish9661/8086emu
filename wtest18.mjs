import { writeFileSync } from "node:fs";
const BASE = "http://127.0.0.1:8124";
const js = await (await fetch(`${BASE}/pkg/multi_cpu_emu.js`)).text();
writeFileSync("/tmp/opencode/pkgmod18.mjs", js);
const mod = await import("file:///tmp/opencode/pkgmod18.mjs?x=" + Date.now());
const wasmBytes = new Uint8Array(
  await (await fetch(`${BASE}/pkg/multi_cpu_emu_bg.wasm`)).arrayBuffer()
);
await mod.default(wasmBytes);
const { Emulator } = mod;

const e = new Emulator("8086");
let src = `ORG 100h
MOV AL, 99h
ADD AL, 01h
DAA
MOV AH, 4Ch
INT 21h
END`;
e.load(e.assemble(src), 0);
e.set_pc(0x100);
e.run(100);
console.log("DAA 99+1: AX =", e.regs()[0], "| CF =", e.flags().includes("CF"), "(expect AX=4C00: AH=4Ch exit code, AL=00, CF)");

e.reset();
src = `ORG 4
DW isr
DW 0000h
ORG 100h
MOV AX, 0100h
PUSH AX
POPF
MOV BX, 1111h
MOV CX, 2222h
MOV AH, 4Ch
INT 21h
isr:
INC SI
IRET
END`;
e.load(e.assemble(src), 0);
e.set_pc(0x100);
e.run(100);
console.log("TF trap: SI =", e.regs()[4], "(expect 3: MOV BX, MOV CX, MOV AH)");

const e51 = new Emulator("8051");
src = `ORG 0
SJMP main
ORG 23h
LJMP isr
ORG 30h
main:
MOV IE, #90h
start:
SJMP start
isr:
MOV R7, SBUF
CLR RI
RETI
END`;
e51.load(e51.assemble(src), 0);
e51.serial_rx(0x58);
e51.run(100);
console.log("8051 serial RX: R7 =", e51.regs()[13], "(expect 88)");