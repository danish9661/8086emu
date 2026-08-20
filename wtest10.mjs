import { writeFileSync } from 'node:fs';
const js = await (await fetch('http://127.0.0.1:8124/pkg/multi_cpu_emu.js')).text();
writeFileSync('/tmp/opencode/pkgmod6.mjs', js);
const mod = await import('file:///tmp/opencode/pkgmod6.mjs?x=' + Date.now());
const wasmBytes = new Uint8Array(await (await fetch('http://127.0.0.1:8124/pkg/multi_cpu_emu_bg.wasm')).arrayBuffer());
await mod.default(wasmBytes);
const { Emulator } = mod;
const e = new Emulator("8086");
const src = "ORG 100h\nMOV AH, 01h\nINT 21h\nMOV AH, 4Ch\nINT 21h\nEND";
const code = e.assemble(src);
e.load(code, 0);
const r1 = e.run(1000);
console.log("run1 steps:", r1, "waiting:", e.waiting_input(), "out:", JSON.stringify(e.out()));
e.push_key(0x51); // 'Q'
const r2 = e.run(1000);
console.log("run2 steps:", r2, "halted:", e.halted(), "out:", JSON.stringify(e.out()), "al:", e.regs()[0]);
