import { writeFileSync } from 'node:fs';
const js = await (await fetch('http://127.0.0.1:8124/pkg/multi_cpu_emu.js')).text();
writeFileSync('/tmp/opencode/pkgmod13.mjs', js);
const mod = await import('file:///tmp/opencode/pkgmod13.mjs?x=' + Date.now());
const wasmBytes = new Uint8Array(await (await fetch('http://127.0.0.1:8124/pkg/multi_cpu_emu_bg.wasm')).arrayBuffer());
await mod.default(wasmBytes);
const { Emulator } = mod;
const e = new Emulator("8086");
const src = `ORG 0
DW isrNmi
DW 0000h
DW isrIntr
DW 0000h
ORG 100h
STI
MOV CX, 0000h
spin:
INC CX
JMP spin
isrNmi:
MOV DX, 1111h
IRET
isrIntr:
MOV DX, 2222h
IRET
END`;
e.load(e.assemble(src), 0);
e.set_pc(0x100);
e.run(50);
console.log("mid run: pc =", e.pc().toString(16), "DX =", e.regs()[3]);
console.log("interrupt result:", e.interrupt("NMI", 0));
e.run(1000);
console.log("after NMI: DX =", e.regs()[3], "pc =", e.pc().toString(16));
