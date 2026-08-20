import init, { Emulator } from './pkg/multi_cpu_emu.js';

const EXAMPLES = {
  '8086': [
    {
      name: 'Keyboard input (AH=01)',
      src: `; Reads a key with INT 21h AH=01. While running, a
; dialog pops up and any text you type is queued as
; type-ahead (each INT 21h pops the next character).
ORG 100h
MOV AH, 01h
INT 21h         ; read char, echo it
MOV BL, AL
MOV AH, 09h
MOV DX, OFFSET msg
INT 21h         ; "You pressed: $"
MOV AH, 02h
MOV DL, BL
INT 21h
MOV AH, 4Ch
INT 21h
msg: DB 'You pressed: $'
END
`,
    },
    {
      name: 'Keyboard echo loop (AH=07)',
      src: `; Reads 5 characters with AH=07 (no echo) and prints
; them back with AH=02. Type 5 chars in the dialog and
; watch them appear when the program runs.
ORG 100h
MOV CX, 5
again:
MOV AH, 07h
INT 21h         ; read without echo
MOV DL, AL
MOV AH, 02h
INT 21h         ; print it
MOV AH, 02h
MOV DL, 20h     ; space
INT 21h
LOOP again
MOV AH, 4Ch
INT 21h
END
`,
    },
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
let codeMap = [];
let breakpoints = new Set();   // per source line: "ADDR  BYTES" or ''
let history = [];   // snapshots for Step-Back

const $ = (id) => document.getElementById(id);
const editor = $('editor'), gutter = $('gutter'), hl = $('hl'), errorsBox = $('errors'),
      regsBox = $('regs'), flagsBox = $('flags'), memView = $('memview'),
      outputBox = $('output'), errpop = $('errpop');

function newEmulator() {
  emu = new Emulator(isa);
  breakpoints = new Set();
  steps = 0; accumOut = '';
  history = [];
  closeInput();
}

// ---------- keyboard input (8086 INT 21h AH=01/06/07/08/0C) ----------
let inputOpen = false;
function maybePromptInput() {
  if (inputOpen || !emu.waiting_input() || emu.halted()) return;
  inputOpen = true;
  const m = $('inputModal'), f = $('inputField');
  m.style.display = 'flex';
  f.value = '';
  f.focus();
}
function submitInput() {
  const text = $('inputField').value;
  for (const ch of text) emu.push_key(ch.charCodeAt(0) & 0xFF);
  closeInput();
  refresh();
  if (emu.waiting_input()) maybePromptInput(); // program asked again immediately
}
function closeInput() {
  inputOpen = false;
  $('inputModal').style.display = 'none';
}
$('inputOk').onclick = submitInput;
$('inputCancel').onclick = () => { closeInput(); refresh(); };
$('inputField').onkeydown = (e) => { if (e.key === 'Enter') submitInput(); };

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

  // --- ports ---
  renderPorts();

  // --- status ---
  $('sbPc').textContent = ISA_INFO[isa].pcLabel(pc, regs);
  $('sbSteps').textContent = steps;
  $('sbState').textContent = emu.halted() ? 'halted' : (emu.waiting_input() ? 'waiting for input' : (runTimer ? 'running…' : 'ready'));
  $('stepBtn').disabled = runTimer || emu.halted();
  $('overBtn').disabled = runTimer || emu.halted();
  $('backBtn').disabled = runTimer || history.length === 0;
  $('runBtn').disabled = runTimer || emu.halted();
  $('stopBtn').disabled = !runTimer;
}

function renderPorts() {
  const box = $('ports');
  if (isa === '8051') {
    box.innerHTML = ['P0', 'P1', 'P2', 'P3'].map((n, i) =>
      `<span class="port" data-port="${i}" title="${n} pins — click to set">${n} ${fmt(emu.port_read(i), 2)}</span>`).join('');
    return;
  }
  let html = '';
  for (let i = 0; i < 16; i++) {
    const v = emu.port_read(i);
    html += `<span class="port ${v ? 'set' : ''}" data-port="${i}" title="Port ${fmt(i, 2)}h — click to set">${fmt(v, 2)}</span>`;
  }
  box.innerHTML = html;
}

$('ports').addEventListener('click', (e) => {
  const el = e.target.closest('.port');
  if (!el) return;
  const port = parseInt(el.dataset.port, 10);
  const name = isa === '8051' ? 'P' + port : fmt(port, 2) + 'h';
  const inp = prompt(name + ' value (hex, 00-FF)', fmt(emu.port_read(port), 2));
  if (inp === null) return;
  const v = parseInt(inp.trim(), 16);
  if (Number.isNaN(v) || v < 0 || v > 0xFF) { toast('Bad port value: ' + inp); return; }
  emu.port_write(port, v);
  renderPorts();
});

$('portsClearBtn').onclick = () => {
  const n = isa === '8051' ? 4 : 256;
  for (let i = 0; i < n; i++) emu.port_write(i, 0);
  renderPorts();
  toast('Ports cleared');
};

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
      const cls = isPc ? 'hl' : 'mb';
      html += `<span class="${cls}" data-addr="${memBase + i}" title="Click to edit [${fmt(memBase + i).padStart(6, '0')}]">${fmt(b, 2)}</span> `;
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

$('memview').addEventListener('click', (e) => {
  const el = e.target.closest('.mb');
  if (!el) return;
  const addr = parseInt(el.dataset.addr, 10);
  const inp = prompt('Memory [' + fmt(addr).padStart(6, '0') + '] byte (hex, 00-FF)', el.textContent);
  if (inp === null) return;
  const v = parseInt(inp.trim(), 16);
  if (Number.isNaN(v) || v < 0 || v > 0xFF) { toast('Bad byte: ' + inp); return; }
  emu.mem_write(addr, new Uint8Array([v]));
  renderMem(emu.pc());
});
$('mempgUp').onclick = () => { $('memaddr').value = fmt(Math.max(0, memBase - PAGE)); renderMem(emu.pc()); };
$('mempgDn').onclick = () => { $('memaddr').value = fmt(memBase + PAGE); renderMem(emu.pc()); };
$('memaddr').onchange = () => renderMem(emu.pc());

// ---------- editor ----------
const HL = {
  dirs: ['ORG', 'DB', 'DW', 'EQU', 'END'],
  regs: {
    '8086': 'AX BX CX DX AH AL BH BL CH CL DH DL SI DI BP SP CS DS ES SS IP FLAGS'.split(' '),
    '8085': 'A B C D E H L M SP PSW'.split(' '),
    '8051': 'A B R0 R1 R2 R3 R4 R5 R6 R7 ACC DPTR SP PC PSW P0 P1 P2 P3 TCON TMOD TH0 TL0 TH1 TL1 SCON SBUF IE IP DPL DPH'.split(' '),
  },
  mnem: {
    '8086': 'MOV PUSH POP ADD ADC SUB SBB AND OR XOR CMP INC DEC NEG NOT MUL IMUL DIV IDIV TEST XCHG LEA SHL SHR SAL SAR ROL ROR RCL RCR CBW CWD MOVS MOVSB MOVSW LODS LODSB LODSW STOS STOSB STOSW CMPS CMPSB CMPSW SCAS SCASB SCASW LAHF SAHF CLC STC CMC CLI STI CLD STD JA JAE JB JBE JC JE JG JGE JL JLE JNA JNAE JNB JNBE JNC JNE JNG JNGE JNL JNLE JNO JNP JNS JNZ JO JP JPE JPO JS JZ JMP CALL RET RETF LOOP LOOPZ LOOPNZ JCXZ INT INT3 INTO IRET NOP HLT'.split(' '),
    '8085': 'MOV MVI LXI LDA STA LDAX STAX LHLD SHLD XCHG ADD ADC SUB SBB ANA XRA ORA CMP ADI ACI SUI SBI ANI XRI ORI CPI INR DCR INX DCX DAD RLC RRC RAL RAR CMA CMC STC DAA JMP JNZ JZ JNC JC JPO JPE JP JM CALL CNZ CZ CNC CC CPO CPE CP CM RET RNZ RZ RNC RC RPO RPE RP RM RST PUSH POP XTHL SPHL PCHL IN OUT EI DI SIM RIM NOP HLT'.split(' '),
    '8051': 'MOV MOVC MOVX PUSH POP XCH XCHD SWAP ADD ADDC SUBB INC DEC MUL DIV DA ANL ORL XRL CLR CPL RL RR RLC RRC SETB SJMP AJMP LJMP JZ JNZ JC JNC JB JNB JBC CJNE DJNZ ACALL LCALL RET RETI NOP'.split(' '),
  },
};
const TOKEN_RE = /('[^'\n]*')|(;[^\n]*$)|(0[xX][0-9a-fA-F]+)|([0-9a-fA-F]+[hHbBqQ](?![0-9a-fA-F]))|([0-9]+)|([A-Za-z_][A-Za-z0-9_.@]*)|([\[\](),:+\-*/])/g;
function escHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
function renderHl() {
  const labels = new Set();
  const labRe = /^\s*([A-Za-z_][A-Za-z0-9_.@]*)(:|\s+EQU\b)/i;
  for (const line of editor.value.split('\n')) {
    const m = labRe.exec(line);
    if (m) labels.add(m[1].toUpperCase());
  }
  const regs = HL.regs[isa], mnems = HL.mnem[isa], dirs = HL.dirs;
  hl.textContent = '';
  const out = editor.value.split('\n').map((line) => {
    let r = '', last = 0, m;
    TOKEN_RE.lastIndex = 0;
    while ((m = TOKEN_RE.exec(line))) {
      r += escHtml(line.slice(last, m.index));
      const [tok, str, com, hx, suf, dec, word, sym] = m;
      let cls;
      if (str) cls = 's';
      else if (com) cls = 'c';
      else if (hx || suf || dec) cls = 'n';
      else if (sym) cls = 'y';
      else {
        const w = word.toUpperCase();
        if (dirs.includes(w)) cls = 'm';
        else if (mnems.includes(w)) cls = 'm';
        else if (regs.includes(w)) cls = 'r';
        else if (labels.has(w)) cls = 'l';
        else cls = 'y';
      }
      r += `<span class="${cls}">${escHtml(tok)}</span>`;
      last = m.index + tok.length;
    }
    return r + escHtml(line.slice(last));
  }).join('\n');
  hl.innerHTML = out;
}
function buildGutter() {
  const lines = editor.value.split('\n');
  let html = '';
  for (let i = 0; i < lines.length; i++) {
    const c = codeMap[i] || '';
    const addr = c ? parseInt(c, 16) : 0;
    const bp = addr && breakpoints.has(addr) ? ' bp' : '';
    html += `<div class="${i + 1 === errLine ? 'err' : ''}${bp}" ${addr ? `data-addr="${addr.toString(16)}"` : ''}><span class="n" ${addr ? `data-addr="${addr.toString(16)}"` : ''} title="${addr ? (breakpoints.has(addr) ? 'Remove breakpoint' : 'Toggle breakpoint') : ''}">${i + 1}</span>` +
            (c ? `<span class="c">${c}</span>` : '') + '</div>';
  }
  gutter.innerHTML = html;
}
gutter.addEventListener('click', (e) => {
  const el = e.target.closest('[data-addr]');
  if (!el || el.closest('.err')) return;
  const addr = parseInt(el.dataset.addr, 16);
  if (el.classList.contains('n')) toggleBreakpoint(addr);
  else runToLine(addr);
});

function toggleBreakpoint(addr) {
  if (breakpoints.has(addr)) { breakpoints.delete(addr); toast('Breakpoint removed'); }
  else { breakpoints.add(addr); toast('Breakpoint set at ' + fmt(addr).padStart(4, '0') + 'h'); }
  renderSource();
}
function renderSource() { buildGutter(); renderHl(); }
editor.oninput = () => { renderSource(); saveSource(); };
editor.onscroll = () => { gutter.scrollTop = editor.scrollTop; hl.scrollTop = editor.scrollTop; hl.scrollLeft = editor.scrollLeft; };
editor.onkeydown = (e) => {
  if (e.key === 'Tab') {
    e.preventDefault();
    const s = editor.selectionStart;
    editor.setRangeText('    ', s, editor.selectionEnd, 'end');
    renderSource();
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
    emu.load(code, 0);
    emu.set_pc(entry());
    codeMap = emu.assemble_info(src);
    errorsBox.style.display = 'none';
    errorsBox.innerHTML = '';
    errLine = -1;
    buildGutter();
    const hex = Array.from(code).slice(0, 12).map((b) => b.toString(16).padStart(2, '0')).join(' ');
    $('sbCode').textContent = `${code.length} bytes${code.length > 12 ? '…' : ''} (${hex}${code.length > 12 ? '…' : ''})`;
    toast(`Assembled ${code.length} bytes, entry ${fmt(entry(), 4)}`);
  } catch (e) {
    showErrors(String(e));
    codeMap = [];
    $('sbCode').textContent = '—';
    toast(String(e));
  }
  refresh();
}

function stepOnce() {
  if (emu.halted()) return;
  history.push(emu.snapshot());
  if (history.length > 50) history.shift();
  emu.step();
  steps++;
  refresh();
  maybePromptInput();
}

// Return address of the instruction at PC, or null if it is not a CALL.
function callRetAddr() {
  const b = emu.mem(emu.pc(), 6);
  const pc = emu.pc();
  const op = b[0];
  if (isa === '8086') {
    if (op === 0xE8) return pc + 3;        // CALL rel16
    if (op === 0x9A) return pc + 5;        // CALL far
    if (op === 0xFF) {                     // CALL r/m
      const modrm = b[1] & 0xC7;
      const mod = b[1] >> 6, rm = b[1] & 7;
      if ((modrm & 0x38) === 0x10) {       // mod=00 reg=010
        if (mod === 0 && rm === 6) return pc + 6;   // disp32
        return pc + 2;
      }
      if (mod === 1) return pc + 3;
      if (mod === 2) return pc + 4;
      return null;
    }
    return null;
  }
  if (isa === '8085') {
    return op === 0xCD ? pc + 3 : null;    // CALL (unconditional only)
  }
  // 8051
  if (op === 0x12) return pc + 3;          // LCALL
  if ((op & 0xF1) === 0x11) return pc + 2; // ACALL
  return null;
}

function stepOver() {
  if (emu.halted() || runTimer) return;
  history.push(emu.snapshot());
  if (history.length > 50) history.shift();
  const ret = callRetAddr();
  if (ret === null) {
    emu.step();
    steps++;
    refresh();
    maybePromptInput();
    return;
  }
  const active = [...breakpoints].filter(a => a !== emu.pc());
  const n = emu.run_bp(100000, active.concat([ret]));
  steps += n;
  refresh();
  maybePromptInput();
  if (!emu.halted() && !emu.waiting_input() && breakpoints.has(emu.pc()) && emu.pc() !== ret) {
    toast('Breakpoint at ' + fmt(emu.pc()).padStart(4, '0') + 'h');
  }
}

function runToLine(addr) {
  if (runTimer || emu.halted()) return;
  history.push(emu.snapshot());
  if (history.length > 50) history.shift();
  if (addr === emu.pc()) return;
  const active = [...breakpoints].filter(a => a !== emu.pc());
  const n = emu.run_bp(100000, active.concat([addr]));
  steps += n;
  refresh();
  maybePromptInput();
  if (!emu.halted() && !emu.waiting_input()) {
    if (breakpoints.has(emu.pc()) && emu.pc() !== addr) toast('Breakpoint at ' + fmt(emu.pc()).padStart(4, '0') + 'h');
    else if (emu.pc() !== addr) toast('Never reached that line (jumped over it?)');
  }
}

function startRun() {
  if (emu.halted()) return;
  stopRequested = false;
  const active = [...breakpoints].filter(a => a !== emu.pc()); // continue past the bp we are stopped at
  runTimer = setInterval(() => {
    const n = emu.run_bp(30000, active);
    steps += n;
    refresh();
    if (emu.waiting_input()) { stopRun(); maybePromptInput(); }
    else if (n < 30000 && !stopRequested && breakpoints.has(emu.pc())) {
      stopRun();
      toast('Breakpoint at ' + fmt(emu.pc()).padStart(4, '0') + 'h');
    }
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
$('overBtn').onclick = stepOver;
$('backBtn').onclick = () => {
  if (!history.length || runTimer) return;
  emu.restore(history.pop());
  steps = Math.max(0, steps - 1);
  refresh();
};
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
  codeMap = [];
  renderSource();
  saveSource();
  toast(`Example: ${list[exIndex].name}`);
};

function loadSource() {
  const saved = localStorage.getItem('mcu_src_' + isa);
  editor.value = saved !== null ? saved : ISA_DEFAULTS[isa];
  errLine = -1;
  codeMap = [];
  renderSource();
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
  renderSource();
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
  if (document.activeElement === editor && ['F5','F7','F8','F10','Escape'].includes(e.key)) e.preventDefault();
  switch (e.key) {
    case 'F7': assemble(); break;
    case 'F8': stepOnce(); break;
    case 'F10': e.preventDefault(); stepOver(); break;
    case 'F5': startRun(); break;
    case 'Escape': stopRun(); break;
  }
});

newEmulator();
loadSource();
$('intrBar85').style.display = 'none';
  $('intrBar51').style.display = 'none';
refresh();