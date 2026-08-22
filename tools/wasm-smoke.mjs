// Self-contained WASM smoke test: loads the built pkg directly (no server) and
// exercises all three ISAs plus the new external-memory / time-travel features.
// Run with: node tools/wasm-smoke.mjs   (after: wasm-pack build --target web --out-dir docs/pkg --release --features wasm)
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const pkgJs = pathToFileURL(new URL('../docs/pkg/multi_cpu_emu.js', import.meta.url).pathname).href;
const wasmPath = new URL('../docs/pkg/multi_cpu_emu_bg.wasm', import.meta.url);
const mod = await import(pkgJs);
await mod.default({ module_or_path: readFileSync(wasmPath) });
const { Emulator } = mod;

function assert(cond, msg) {
  if (!cond) { console.error('FAIL:', msg); process.exit(1); }
  console.log('ok -', msg);
}

// 8086: INT 21h / OUT printing + time-travel (snapshot/restore).
{
  const e = new Emulator('8086');
  const code = e.assemble('ORG 100h\nMOV CX, 3\nMOV AL, 41h\nlp: OUT 01h, AL\nINC AL\nLOOP lp\nHLT\nEND');
  e.load(code, 0x100);
  const snap = e.snapshot();
  e.run(1000);
  assert(e.out() === 'ABC', '8086 prints ABC via OUT 01h');
  e.restore(snap);
  assert(e.out() === '' && e.pc() === 0x100, '8086 snapshot/restore round-trips');
}

// 8085: OUT 01h prints A; external SRAM is reachable.
{
  const e = new Emulator('8085');
  const code = e.assemble('MVI A, 42h\nOUT 01h\nLXI H, 9000h\nMVI M, 0AAh\nHLT\nEND');
  e.load(code, 0);
  e.run(1000);
  assert(e.out() === 'B', '8085 prints B via OUT 01h');
  assert(e.mem(0x9000, 1)[0] === 0xAA, '8085 external SRAM write at 9000h');
}

// 8051: EA low -> code fetched from external XDATA; SBUF prints.
{
  const e = new Emulator('8051');
  e.set_ea(false);
  const code = e.assemble('ORG 0\nMOV A, #58h\nMOV SBUF, A\nSJMP $\nEND');
  e.load_rom(code, 0);
  e.set_pc(0);
  e.run(1000);
  assert(e.out() === 'X', '8051 runs external code (EA=0) and prints X via SBUF');
}

// 8086: load a ROM image and boot from the reset vector FFFF:FFF0.
{
  const e = new Emulator('8086');
  e.set_rom_region(0xF0000, 0x10000);
  e.mem_write(0xFFFF0, [0xEA, 0x00, 0xF0, 0x00, 0xF0]); // JMP FAR F000:F000
  e.mem_write(0xFF000, [0xB0, 0x41, 0xE7, 0x01, 0xF4]); // MOV AL,'A'; OUT 01h,AL; HLT
  e.reset();
  assert(e.pc() === 0xFFFF0, '8086 reset vectors to FFFF0 when ROM reaches top');
  e.run(100);
  assert(e.out() === 'A', '8086 BIOS boots from top ROM');
}

console.log('WASM smoke test passed for all three ISAs.');
