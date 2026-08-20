import { writeFileSync } from 'node:fs';
const js = await (await fetch('http://127.0.0.1:8124/pkg/multi_cpu_emu.js')).text();
writeFileSync('/tmp/opencode/pkgmod8.mjs', js);
const mod = await import('file:///tmp/opencode/pkgmod8.mjs?x=' + Date.now());
const wasmBytes = new Uint8Array(await (await fetch('http://127.0.0.1:8124/pkg/multi_cpu_emu_bg.wasm')).arrayBuffer());
await mod.default(wasmBytes);
const { Emulator } = mod;
for (const isa of ['8086', '8085', '8051']) {
  const e = new Emulator(isa);
  const src = isa === '8086' ? "ORG 100h\nMOV AX, 5\nMOV BX, 3\nMUL BX\nMOV AH, 4Ch\nINT 21h\nEND"
    : isa === '8085' ? "ORG 8000h\nMVI A, 05h\nADI 02h\nHLT\nEND"
    : "ORG 30h\nMOV A, #05h\nADD A, #02h\nEND";
  const code = e.assemble(src);
  e.load(code, 0);
  const info = e.assemble_info(src);
  const snap = e.snapshot();
  e.step();
  const after = e.pc();
  e.restore(snap);
  console.log(isa, 'info lines:', info.filter(s => s).length, 'restore->pc', e.pc().toString(16), 'expected 0', 'steps:', e.run(100));
}
