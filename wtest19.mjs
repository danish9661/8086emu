import { writeFileSync } from "node:fs";
const BASE = "http://127.0.0.1:8125";
const js = await (await fetch(`${BASE}/pkg/multi_cpu_emu.js`)).text();
writeFileSync("/tmp/opencode/pkgmod19.mjs", js);
const mod = await import("file:///tmp/opencode/pkgmod19.mjs?x=" + Date.now());
const wasmBytes = new Uint8Array(
  await (await fetch(`${BASE}/pkg/multi_cpu_emu_bg.wasm`)).arrayBuffer()
);
await mod.default(wasmBytes);
const { Emulator } = mod;

// 1) PUSHA/POPA via wasm
const e = new Emulator("8086");
let src = `ORG 100h
MOV AX, 1111h
MOV BX, 2222h
MOV SP, 8000h
PUSHA
MOV AX, 0AAAAh
POPA
HLT
END`;
e.load(e.assemble(src), 0);
e.set_pc(0x100);
e.run(100);
console.log("PUSHA/POPA: AX =", e.regs()[0], "(expect AX=1111)");

// 2) TF appears in flags() when trap set
e.reset();
src = `ORG 4
DW isr
ORG 100h
MOV AX, 0100h
PUSH AX
POPF
MOV BX, 1111h
MOV AH, 4Ch
INT 21h
isr:
INC SI
IRET
END`;
e.load(e.assemble(src), 0);
e.set_pc(0x100);
e.run(100);
console.log("TF in flags:", e.flags().includes("TF"), "(expect true)");

// 3) 8085 SID/SOD via wasm
const e85 = new Emulator("8085");
e85.set_sid(true);
src = `RIM
MVI A, 80h
SIM
HLT
END`;
e85.load(e85.assemble(src), 0);
e85.run(100);
console.log("SID->A =", e85.regs()[0], "sod =", e85.sod(), "(expect A=80 sod=1)");

// 4) snapshot save/restore round-trip
const e2 = new Emulator("8086");
src = `ORG 100h
MOV AX, 1234h
MOV BX, 5678h
HLT
END`;
e2.load(e2.assemble(src), 0);
e2.set_pc(0x100);
e2.run(100);
const snap = e2.snapshot();
const axBefore = e2.regs()[0];
e2.reset();
const e3 = new Emulator("8086");
e3.load(e3.assemble(`ORG 100h\nMOV AX, 9999h\nHLT\nEND`), 0);
e3.set_pc(0x100);
e3.run(100);
e3.restore(snap);
console.log("snapshot: AX before =", axBefore, "AX after restore =", e3.regs()[0], "(expect both 1234)");
