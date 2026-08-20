import { writeFileSync } from 'node:fs';
const js = await (await fetch('http://127.0.0.1:8124/pkg/multi_cpu_emu.js')).text();
writeFileSync('/tmp/opencode/pkgmod9.mjs', js);
const mod = await import('file:///tmp/opencode/pkgmod9.mjs?x=' + Date.now());
const wasmBytes = new Uint8Array(await (await fetch('http://127.0.0.1:8124/pkg/multi_cpu_emu_bg.wasm')).arrayBuffer());
await mod.default(wasmBytes);
const { Emulator } = mod;
// 8085: LXI SP + PUSH PSW / POP B (rp3 fix)
let e = new Emulator("8085");
e.load(e.assemble("ORG 0\nLXI SP, 9000h\nMVI A, 42h\nPUSH PSW\nPOP B\nHLT\nEND"), 0);
e.run(100);
console.log("8085 B =", e.regs().find(r => r.startsWith("B=")), "(want B=42)");
// 8051: mode 2 auto-reload counting (timer fix)
e = new Emulator("8051");
const tsrc = "ORG 30h\nMOV TMOD, #22h\nMOV TL0, #0FEh\nMOV TH0, #0FEh\nSETB TR0\nMOV A, #00h\nMOV B, #00h\nSJMP $\nEND";
e.load(e.assemble(tsrc), 0x30);
e.run(20);
const sfr = new Uint8Array(e.mem(0, 0)); // dummy
console.log("8051 TL0 =", e.regs().length > 0 ? "(use sfr)" : "");
