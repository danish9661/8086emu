import init, { Emulator } from './pkg/multi_cpu_emu.js';

const EXAMPLES = {
  '8086': [
    {
      name: 'Hello (INT 21h)',
      src: `; Print a message with DOS service INT 21h, AH=09
; then exit with AH=4Ch
ORG 100h
MOV DX, OFFSET msg
MOV AH, 09h
INT 21h
MOV AH, 4Ch
INT 21h
msg: DB 'Hello, 8086!$'
END
`,
    },
    {
      name: 'Arithmetic (5*3+2)',
      src: `; AX = 5*3 + 2 = 17
ORG 100h
MOV AX, 5
MOV BX, 3
MUL BX          ; AX = 15
ADD AX, 2       ; AX = 17
MOV CX, AX      ; save result
MOV AH, 4Ch
INT 21h
END
`,
    },
    {
      name: 'Loop & memory (sum)',
      src: `; Sum of 5 numbers in a table -> AX
ORG 100h
MOV CX, 5
MOV SI, 0
XOR AX, AX
again:
ADD AX, [table + SI]
INC SI
INC SI
LOOP again
MOV AH, 4Ch
INT 21h
table: DW 10, 20, 30, 40, 50   ; sum = 150
END
`,
    },
    {
      name: 'Flags demo (CMP/Jcc)',
      src: `; Compare two values and branch on the result
ORG 100h
MOV AX, 25
MOV BX, 100
CMP AX, BX      ; sets flags
JB less
MOV CX, 1       ; AX >= BX
JMP done
less:
MOV CX, 2       ; AX < BX
done:
MOV AH, 4Ch
INT 21h
END
`,
    },
  ],
  '8085': [
    {
      name: 'Hello (OUT 01h)',
      src: `; Print a string: OUT 01h prints the char in A
MVI C, 05h
LXI H, msg
loop:
MOV A, M
CPI '$'
JZ done
OUT 01h
INX H
JMP loop
done:
HLT
msg: DB 'Hello, 8085!$'
END
`,
    },
    {
      name: 'Arithmetic (A=25+5)',
      src: `; A = 25 + 5 = 30
MVI A, 25
ADI 05h
MOV B, A
HLT
END
`,
    },
    {
      name: 'Loop (count 1..9)',
      src: `; Print digits 1..9 via OUT 01h
MVI A, '1'
loop:
OUT 01h
CPI '9'
JZ done
INR A
JMP loop
done:
HLT
END
`,
    },
    {
      name: 'Memory (copy block)',
      src: `; Copy 4 bytes from src to dst (LDA/STA)
MVI C, 4
LXI H, src
LXI D, dst
copy:
MOV A, M
STAX D
INX H
INX D
DCR C
JNZ copy
HLT
src: DB 11h, 22h, 33h, 44h
dst: DB 0, 0, 0, 0
END
`,
    },
    {
      name: 'Interrupts (TRAP/RST)',
      src: `; Press the IRQ buttons above while running.
; TRAP prints 'T', RST 7.5 prints '7', RST 6.5 prints '6',
; RST 5.5 prints '5'. Handlers live at the hardware vectors.
EI
main:
JMP main
ORG 24h       ; TRAP vector (non-maskable)
MVI A, 'T'
OUT 01h
RET
ORG 2Ch       ; RST 5.5 vector
MVI A, '5'
OUT 01h
EI
RET
ORG 34h       ; RST 6.5 vector
MVI A, '6'
OUT 01h
EI
RET
ORG 3Ch       ; RST 7.5 vector
MVI A, '7'
OUT 01h
EI
RET
END
`,
    },
  ],
  '8051': [
    {
      name: 'Hello (SBUF)',
      src: `; Print a string: writing SBUF prints a char
MOV DPTR, #msg
MOV R1, #00h
loop:
MOV A, R1
MOVC A, @A+DPTR
JZ done
MOV SBUF, A
INC R1
SJMP loop
done:
SJMP done
msg: DB 'Hello, 8051!', 0
END
`,
    },
    {
      name: 'Arithmetic (A=40-7)',
      src: `; A = 40 - 7 = 33
MOV A, #40
SUBB A, #07
MOV R0, A
END
`,
    },
    {
      name: 'Timer counter demo',
      src: `; Count timer0 overflows into R0 (run in Step mode)
MOV TMOD, #01h   ; timer 0, mode 1 (16-bit)
MOV TH0, #0FFh
MOV TL0, #0FEh
SETB TR0         ; start timer
MOV R0, #00h
again:
JNB TF0, again   ; wait for overflow
CLR TF0
INC R0
MOV TH0, #0FFh   ; reload
MOV TL0, #0FEh
SJMP again
END
`,
    },
    {
      name: 'Interrupts (INT0/INT1)',
      src: `; Press the INT0 / INT1 IRQ buttons above while running.
; INT0 handler prints '0', INT1 handler prints '1'.
; Vectors live at the bottom of code space, so the main
; program sits at ORG 30h (canonical 8051 layout).
ORG 0
SJMP main
ORG 03h       ; INT0 vector
MOV SBUF, #'0'
RETI
ORG 13h       ; INT1 vector
MOV SBUF, #'1'
RETI
ORG 30h
main:
MOV IE, #85h  ; EA + EX0 + EX1
SETB IT0      ; edge-triggered
SETB IT1
loop:
SJMP loop
END
`,
    },
    {
      name: 'Bit operations',
      src: `; Flip a port bit pattern: P1.0..P1.3
SETB P1.0
SETB P1.3
CLR P1.1
CPL P1.2
END
`,
    },
  ],
};

const ISA_DEFAULTS = {
  '8086': EXAMPLES['8086'][0].src,
  '8085': EXAMPLES['8085'][0].src,
  '8051': EXAMPLES['8051'][0].src,
};

const ISA_INFO = {
  '8086': { origin: 0, entry: 0x100, pcLabel: (pc, regs) => {
    const cs = val(regs, 'CS'), ip = val(regs, 'IP');
    return `${cs.toString(16).toUpperCase().padStart(4, '0')}:${ip.toString(16).toUpperCase().padStart(4, '0')} (${(cs * 16 + ip).toString(16).toUpperCase()})`;
  }, memBase: (pc) => pc },
  '8085': { origin: 0, entry: 0, pcLabel: (pc) => pc.toString(16).toUpperCase(), memBase: (pc) => pc },
  '8051': { origin: 0, entry: 0, pcLabel: (pc) => pc.toString(16).toUpperCase(), memBase: (pc) => pc },
};

const FLAG_MAP = {
  '8086': [['carry','CF'],['zero','ZF'],['sign','SF'],['parity','PF'],['aux','AF'],['overflow','OF'],['direction','DF'],['interrupt','IF']],
  '8085': [['carry','CY'],['zero','Z'],['sign','S'],['parity','P'],['aux','AC'],['interrupt','IE']],
  '8051': [['carry','CY'],['aux','AC'],['overflow','OV'],['parity','P']],
};

function val(regs, name) {
  const r = regs.find((r) => r.startsWith(name + '='));
  return r ? parseInt(r.split('=')[1], 16) : 0;
}

await init();
let emu = null;
let isa = '8086';
let steps = 0;
let runTimer = null;
let stopRequested = false;
let accumOut = '';
let errLine = -1;

const $ = (id) => document.getElementById(id);
const editor = $('editor'), gutter = $('gutter'), errorsBox = $('errors'),
      regsBox = $('regs'), flagsBox = $('flags'), memView = $('memview'),
      outputBox = $('output'), errpop = $('errpop');

function newEmulator() {
  emu = new Emulator(isa);
  steps = 0; accumOut = '';
}

function entry() { return ISA_INFO[isa].entry; }

function fmt(v, w = 4) { return v.toString(16).toUpperCase().padStart(w, '0'); }

function refresh() {
  const regs = emu.regs();
  const flags = emu.flags();
  const pc = emu.pc();

  // --- registers ---
  let html = '';
  const pcPhys = ISA_INFO[isa].memBase(pc);
  if (isa === '8086') {
    const pairs = [['AX','AH','AL'],['BX','BH','BL'],['CX','CH','CL'],['DX','DH','DL']];
    for (const [r, h, l] of pairs) {
      const v = val(regs, r);
      html += chip(r, fmt(v), `${h}=${fmt(v >> 8, 2)} ${l}=${fmt(v & 0xFF, 2)}`);
    }
    for (const r of ['SI','DI','BP','SP','CS','DS','ES','SS']) {
      html += chip(r, fmt(val(regs, r)));
    }
    const fl = flags.join('').replace(/[A-Z]/g, '');
    let fv = 0;
    if (flags.includes('CF')) fv |= 0x001;
    if (flags.includes('PF')) fv |= 0x004;
    if (flags.includes('AF')) fv |= 0x010;
    if (flags.includes('ZF')) fv |= 0x040;
    if (flags.includes('SF')) fv |= 0x080;
    if (flags.includes('IF')) fv |= 0x200;
    if (flags.includes('DF')) fv |= 0x400;
    if (flags.includes('OF')) fv |= 0x800;
    html += chip('IP', fmt(val(regs, 'IP')));
    html += chip('FLAGS', fmt(fv));
  } else if (isa === '8085') {
    for (const r of ['A','B','C','D','E','H','L','SP']) html += chip(r, fmt(val(regs, r), 2));
    html += chip('PC', fmt(val(regs, 'PC')), null, true);
  } else {
    for (const r of ['A','B','DPTR','SP','PC','PSW']) html += chip(r, fmt(val(regs, r), r === 'B' || r === 'A' ? 2 : 4), null, r === 'PC');
    for (let i = 0; i < 8; i++) html += chip('R' + i, fmt(val(regs, 'R' + i), 2));
    html += chip('BANK', fmt(val(regs, 'BANK'), 1));
  }
  regsBox.innerHTML = html;

  // --- flags ---
  flagsBox.innerHTML = FLAG_MAP[isa].map(([key, label]) =>
    `<span class="flag ${flags.includes(label) ? 'on' : ''}">${label}</span>`).join('');

  // --- memory dump ---
  renderMem(pcPhys);

  // --- output ---
  const fresh = emu.out();
  if (fresh) accumOut += fresh;
  outputBox.textContent = accumOut;

  // --- status ---
  $('sbPc').textContent = ISA_INFO[isa].pcLabel(pc, regs);
  $('sbSteps').textContent = steps;
  $('sbState').textContent = emu.halted() ? 'halted' : (runTimer ? 'running…' : 'ready');
  $('stepBtn').disabled = runTimer || emu.halted();
  $('runBtn').disabled = runTimer || emu.halted();
  $('stopBtn').disabled = !runTimer;
}

function chip(name, value, sub = null, isPc = false) {
  return `<div class="rreg ${isPc ? 'pc' : ''}"><div class="n">${name}</div>` +
         `<div class="v">${value}</div>` +
         (sub ? `<div class="v sub">${sub}</div>` : '') + `</div>`;
}

// ---------- memory ----------
let memBase = 0;
const PAGE = 256;

function renderMem(pcPhys) {
  const input = $('memaddr');
  memBase = parseInt(input.value, 16);
  if (Number.isNaN(memBase)) memBase = 0;
  const len = PAGE;
  const bytes = new Uint8Array(emu.mem(memBase, len));
  const inRange = pcPhys >= memBase && pcPhys < memBase + len;
  const off = inRange ? pcPhys - memBase : -1;
  let html = '';
  for (let row = 0; row < len; row += 16) {
    const addr = memBase + row;
    html += `<span class="addr">${fmt(addr).padStart(6, '0')}</span>  `;
    for (let c = 0; c < 16; c++) {
      const i = row + c;
      const b = bytes[i];
      const isPc = i === off;
      html += isPc ? `<span class="hl">${fmt(b, 2)}</span> ` : `${fmt(b, 2)} `;
    }
    html += ' |';
    for (let c = 0; c < 16; c++) {
      const b = bytes[row + c];
      html += b >= 32 && b < 127 ? String.fromCharCode(b) : '.';
    }
    html += '|\n';
  }
  memView.innerHTML = html;
  $('meminfo').textContent = inRange
    ? `PC highlighted (${fmt(pcPhys)})` : `PC ${fmt(pcPhys)} outside view`;
}

$('mempgUp').onclick = () => { $('memaddr').value = fmt(Math.max(0, memBase - PAGE)); renderMem(emu.pc()); };
$('mempgDn').onclick = () => { $('memaddr').value = fmt(memBase + PAGE); renderMem(emu.pc()); };
$('memaddr').onchange = () => renderMem(emu.pc());

// ---------- editor ----------
function buildGutter() {
  const n = editor.value.split('\n').length;
  let html = '';
  for (let i = 1; i <= n; i++) html += `<div class="${i === errLine ? 'err' : ''}">${i}</div>`;
  gutter.innerHTML = html;
}
editor.oninput = () => { buildGutter(); saveSource(); };
editor.onscroll = () => { gutter.scrollTop = editor.scrollTop; };
editor.onkeydown = (e) => {
  if (e.key === 'Tab') {
    e.preventDefault();
    const s = editor.selectionStart;
    editor.setRangeText('    ', s, editor.selectionEnd, 'end');
    buildGutter();
  }
};

function showErrors(msg) {
  const m = /line (\d+): (.*)/.exec(msg || '');
  errLine = m ? parseInt(m[1], 10) : -1;
  if (m) {
    errorsBox.style.display = 'block';
    errorsBox.innerHTML = `<span class="errline">line ${m[1]}:</span> <span class="errmsg">${m[2]}</span>`;
    const lines = editor.value.split('\n');
    const ln = Math.min(parseInt(m[1], 10), lines.length) - 1;
    const y = ln * 20;
    editor.scrollTop = Math.max(0, y - editor.clientHeight / 2);
  } else {
    errorsBox.style.display = 'none';
    errorsBox.innerHTML = '';
  }
  buildGutter();
}

function toast(msg) {
  errpop.textContent = msg;
  errpop.style.display = 'block';
  clearTimeout(toast._t);
  toast._t = setTimeout(() => { errpop.style.display = 'none'; }, 3000);
}

// ---------- assemble / run ----------
function assemble() {
  stopRun();
  try {
    const src = editor.value;
    const code = emu.assemble(src);
    newEmulator();
    emu.load(emu.assemble(src), 0);
    emu.set_pc(entry());
    errorsBox.style.display = 'none';
    errorsBox.innerHTML = '';
    errLine = -1;
    buildGutter();
    const hex = Array.from(code).slice(0, 12).map((b) => b.toString(16).padStart(2, '0')).join(' ');
    $('sbCode').textContent = `${code.length} bytes${code.length > 12 ? '…' : ''} (${hex}${code.length > 12 ? '…' : ''})`;
    toast(`Assembled ${code.length} bytes, entry ${fmt(entry(), 4)}`);
  } catch (e) {
    showErrors(String(e));
    $('sbCode').textContent = '—';
    toast(String(e));
  }
  refresh();
}

function stepOnce() {
  if (emu.halted()) return;
  emu.step();
  steps++;
  refresh();
}

function startRun() {
  if (emu.halted()) return;
  stopRequested = false;
  runTimer = setInterval(() => {
    const n = emu.run(30000);
    steps += n;
    refresh();
    if (emu.halted() || stopRequested) stopRun();
  }, 16);
  $('stopBtn').disabled = false;
  refresh();
}

function stopRun() {
  if (runTimer) { clearInterval(runTimer); runTimer = null; }
  stopRequested = false;
  refresh();
}

$('asmBtn').onclick = assemble;
$('stepBtn').onclick = stepOnce;
$('runBtn').onclick = startRun;
$('stopBtn').onclick = stopRun;
$('resetBtn').onclick = () => { newEmulator(); refresh(); toast('CPU reset'); };
$('clearOutBtn').onclick = () => { accumOut = ''; outputBox.textContent = ''; };

// ---------- examples / persistence ----------
let exIndex = 0;
$('exampleBtn').onclick = () => {
  const list = EXAMPLES[isa];
  exIndex = (exIndex + 1) % list.length;
  editor.value = list[exIndex].src;
  errLine = -1;
  buildGutter();
  saveSource();
  toast(`Example: ${list[exIndex].name}`);
};

function loadSource() {
  const saved = localStorage.getItem('mcu_src_' + isa);
  editor.value = saved !== null ? saved : ISA_DEFAULTS[isa];
  errLine = -1;
  buildGutter();
}
function saveSource() {
  localStorage.setItem('mcu_src_' + isa, editor.value);
}

$('isa').onchange = () => {
  isa = $('isa').value;
  newEmulator();
  loadSource();
  $('memaddr').value = '0';
  memBase = 0;
  $('intrBar85').style.display = isa === '8085' ? '' : 'none';
  $('intrBar51').style.display = isa === '8051' ? '' : 'none';
  refresh();
};

$('irqTrap').onclick = () => { emu.interrupt('TRAP', 0); refresh(); };
$('irq75').onclick = () => { emu.interrupt('RST75', 0); refresh(); };
$('irq65').onclick = () => { emu.interrupt('RST65', 0); refresh(); };
$('irq55').onclick = () => { emu.interrupt('RST55', 0); refresh(); };
$('irqIntr').onclick = () => { emu.interrupt('INTR', 0x08); refresh(); };
$('irqInt0').onclick = () => { emu.interrupt('INT0', 0); refresh(); };
$('irqInt1').onclick = () => { emu.interrupt('INT1', 0); refresh(); };

// ---------- keys ----------
document.addEventListener('keydown', (e) => {
  if (e.ctrlKey && e.key.toLowerCase() === 's') {
    e.preventDefault();
    saveSource();
    toast('Saved to browser');
    return;
  }
  if (document.activeElement === editor && ['F5','F7','F8','Escape'].includes(e.key)) e.preventDefault();
  switch (e.key) {
    case 'F7': assemble(); break;
    case 'F8': stepOnce(); break;
    case 'F5': startRun(); break;
    case 'Escape': stopRun(); break;
  }
});

newEmulator();
loadSource();
$('intrBar85').style.display = 'none';
  $('intrBar51').style.display = 'none';
refresh();