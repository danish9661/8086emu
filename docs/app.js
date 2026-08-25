import init, { Emulator } from './pkg/multi_cpu_emu.js';
import { renderDevices, resetDevices, renderMemMap, renderPeripherals } from './devices.js';

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
      name: 'Hardware interrupts (NMI/INTR)',
      src: `; 8086 hardware interrupts. The IVT (interrupt vector table)
; starts at address 0; each vector is 4 bytes (IP, CS).
; NMI is vector 02h -> IVT entry at 08h, INTR here uses
; device vector 08h -> IVT entry at 20h (ORG pads with zeros).
;
; While this runs, press NMI or INTR in the IRQ bar:
;   NMI   -> non-maskable, works even with CLI
;   INTR  -> maskable via IF (needs STI)
; Each IRQ sets DX to a marker, then IRET resumes the loop.
ORG 8
DW isrNmi        ; vector 02h: IP
DW 0000h         ; vector 02h: CS
ORG 20h
DW isrIntr       ; vector 08h: IP
DW 0000h         ; vector 08h: CS

ORG 100h
STI              ; enable INTR (NMI ignores IF anyway)
MOV CX, 0000h
spin:
INC CX
JMP spin         ; loop forever - press NMI/INTR while it runs

isrNmi:
MOV DX, 1111h
IRET

isrIntr:
MOV DX, 2222h
IRET
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
     {
      name: 'Graphics (INT 10h mode 13h)',
      src: `; Mode 13h: 320x200, 256 colours. Plot a diagonal line + boxes.
ORG 100h
     MOV AX, 0013h
     INT 10h          ; set graphics mode
     MOV CX, 0        ; x
loop:
     MOV DX, CX       ; y = x  (diagonal)
     MOV AL, 14       ; colour (yellow)
     MOV AH, 0Ch
     INT 10h          ; write pixel (CX,DX)
     INC CX
     CMP CX, 200
     JB loop
     ; draw a coloured bar across the top
     MOV CX, 0
bar:
     MOV DX, 10
     MOV AL, CL
     MOV AH, 0Ch
     INT 10h
     INC CX
     CMP CX, 320
     JB bar
     MOV AX, 4C00h
     INT 21h
END
`,
    },
    {
      name: 'Peripherals demo (ports 10h-27h)',
      src: `; Drive the peripheral devices panel via OUT.
; Traffic light: red+yellow+green (bits 0/1/2 of port 10h)
; 7-seg: write hex digit to 11h (low) / 12h (high)
; Stepper: 4-bit coil pattern to 13h
; Printer: write chars to 14h (read status 15h)
; Robot: write X to 16h, Y to 17h
; LED matrix: 8 rows at 20h-27h
ORG 100h
MOV AL, 001b
OUT 10h, AL          ; red on
MOV AL, 0Ch
OUT 11h, AL          ; 7-seg low = 'C'
MOV AL, 05h
OUT 12h, AL          ; 7-seg high = '5'
MOV AL, 0011b
OUT 13h, AL          ; stepper pos 1
MOV AL, 'H'
OUT 14h, AL
MOV AL, 'i'
OUT 14h, AL
MOV AL, 2
OUT 16h, AL          ; robot X = 2
MOV AL, 5
OUT 17h, AL          ; robot Y = 5
MOV AL, 10000000b
OUT 20h, AL          ; LED row 0 leftmost lit
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
    {
      name: 'Peripherals (MOVX to 0FF00h+ I/O)',
      src: `; 8051 has no OUT; devices live in the top 256 bytes of XDATA,
; so MOVX @DPTR,A with DPTR = 0FF00h+ writes I/O port (0..FF).
; Traffic light (10h) bits 0/1/2; 7-seg (11h/12h); stepper (13h);
; printer (14h); robot X/Y (16h/17h); LED matrix rows (20h-27h).
ORG 0
    MOV DPTR, #0FF10h
    MOV A, #001b
    MOVX @DPTR, A        ; red on
    MOV DPTR, #0FF11h
    MOV A, #0Ch
    MOVX @DPTR, A        ; 7-seg low = 'C'
    MOV DPTR, #0FF12h
    MOV A, #05h
    MOVX @DPTR, A        ; 7-seg high = '5'
    MOV DPTR, #0FF13h
    MOV A, #0011b
    MOVX @DPTR, A        ; stepper pos 1
    MOV DPTR, #0FF14h
    MOV A, #'H'
    MOVX @DPTR, A
    MOV A, #'i'
    MOVX @DPTR, A        ; printer "Hi"
    MOV DPTR, #0FF16h
    MOV A, #2
    MOVX @DPTR, A        ; robot X = 2
    MOV DPTR, #0FF17h
    MOV A, #5
    MOVX @DPTR, A        ; robot Y = 5
    MOV DPTR, #0FF20h
    MOV A, #10000000b
    MOVX @DPTR, A        ; LED row 0 leftmost lit
    SJMP $
END
`,
    },
  ],
  'rv32': [
    {
      name: 'Hello (ECALL write)',
      src: `; RV32I base ISA. Print "Hi\\n" via the tiny ECALL ABI
; (a7 = 64 write fd/a1/a2, a7 = 93 exit), then halt.
ORG 0
    ADDI a1, x0, 0x100   ; pointer to message
    ADDI a2, x0, 3       ; length
    ADDI a7, x0, 64      ; syscall: write
    ECALL
    ADDI a7, x0, 93      ; syscall: exit
    ECALL
ORG 0x100
    DB 'H','i',10
END
`,
    },
    {
      name: 'Arithmetic loop',
      src: `; Sum 1..10 into x3 using a BLT loop.
ORG 0
    ADDI x1, x0, 0       ; i = 0
    ADDI x2, x0, 10      ; limit
    ADDI x3, x0, 0       ; sum = 0
loop:
    ADDI x1, x1, 1       ; i++
    ADD  x3, x3, x1      ; sum += i
    BLT  x1, x2, loop
END
`,
    },
  ],
  '6502': [
    {
      name: 'Hello (STA $01)',
      src: `; MOS 6502. Print "Hi\\n" by writing each char to I/O port $01
; (the IDE maps STA $01 to the output console), then BRK.
ORG 0
    LDX #0
loop:
    LDA msg,X
    BEQ done
    STA $01
    INX
    JMP loop
done:
    BRK
msg: DB 'H','i',10,0
END
`,
    },
    {
      name: 'Sum 1..10',
      src: `; Sum 1..10 into $20 (zero page), using X as the counter.
ORG 0
    LDX #0
    LDA #0
loop:
    INX
    STX $21       ; temp = i
    CLC
    ADC $21       ; A = A + i
    CPX #10
    BNE loop
        STA $20
        BRK
    END
    `,
    },
  ],
  'Z80': [
    {
      name: 'Hello (OUT (1),A)',
      src: `; Zilog Z80. Print "Hi\\n" by OUT to port 1 (mapped to console), then HALT.
ORG 0
    LD A, 'H'
    OUT (1), A
    LD A, 'i'
    OUT (1), A
    LD A, 10
    OUT (1), A
    HALT
END
`,
    },
    {
      name: 'Sum 1..10',
      src: `; Sum 1..10 into $20 using B as counter and C as the running value.
ORG 0
    LD B, 10
    LD A, 0
    LD C, 0
loop:
    INC C
    ADD A, C
    DEC B
    JP NZ, loop
    LD ($20), A
    HALT
END
`,
    },
    {
      name: 'Interrupts (NMI/INT)',
      src: `; Z80 interrupt demo. Click the NMI / INT buttons in the IRQ bar.
; NMI jumps to 0066h, INT (when IFF1 set) to 0038h.
ORG 0
    JP main
    ORG 0x38
int_handler:
    LD A, 'I'
    OUT (1), A
    RETI
    ORG 0x66
nmi_handler:
    LD A, 'N'
    OUT (1), A
    RETI
main:
    EI
loop:
    LD A, '.'
    OUT (1), A
    JR loop
END
`,
    },
  ],
};

  const ISA_DEFAULTS = {
    '8086': EXAMPLES['8086'][0].src,
    '8085': EXAMPLES['8085'][0].src,
    '8051': EXAMPLES['8051'][0].src,
    'rv32': EXAMPLES['rv32'][0].src,
    '6502': EXAMPLES['6502'][0].src,
    'Z80': EXAMPLES['Z80'][0].src,
  };
  const ISA_LIST = Object.keys(ISA_DEFAULTS);

const ISA_INFO = {
  '8086': { origin: 0, entry: 0x100, pcLabel: (pc, regs) => {
    const cs = val(regs, 'CS'), ip = val(regs, 'IP');
    return `${cs.toString(16).toUpperCase().padStart(4, '0')}:${ip.toString(16).toUpperCase().padStart(4, '0')} (${(cs * 16 + ip).toString(16).toUpperCase()})`;
  }, memBase: (pc) => pc },
  '8085': { origin: 0, entry: 0, pcLabel: (pc) => pc.toString(16).toUpperCase(), memBase: (pc) => pc },
  '8051': { origin: 0, entry: 0, pcLabel: (pc) => pc.toString(16).toUpperCase(), memBase: (pc) => pc },
  'rv32': { origin: 0, entry: 0, pcLabel: (pc) => pc.toString(16).toUpperCase(), memBase: (pc) => pc },
  '6502': { origin: 0, entry: 0, pcLabel: (pc) => pc.toString(16).toUpperCase(), memBase: (pc) => pc },
  'Z80': { origin: 0, entry: 0, pcLabel: (pc) => pc.toString(16).toUpperCase(), memBase: (pc) => pc },
};

const FLAG_MAP = {
  '8086': [['carry','CF'],['zero','ZF'],['sign','SF'],['parity','PF'],['aux','AF'],['overflow','OF'],['direction','DF'],['interrupt','IF'],['trap','TF']],
  '8085': [['carry','CY'],['zero','Z'],['sign','S'],['parity','P'],['aux','AC'],['interrupt','IE']],
  '8051': [['carry','CY'],['aux','AC'],['overflow','OV'],['parity','P']],
  'rv32': [],
  '6502': [['carry','C'],['zero','Z'],['interrupt','I'],['decimal','D'],['overflow','V'],['sign','N']],
  'Z80': [['carry','CF'],['zero','ZF'],['sign','SF'],['half','AF'],['parity','PF'],['interrupt','IF']],
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
let rafId = null;          // handle for the requestAnimationFrame run loop
let stopRequested = false;
let accumOut = '';
let errLine = -1;
let codeMap = [];
let breakpoints = new Map();   // addr -> condition string ('' == unconditional)

// breakpoint helpers (Map-backed: addr -> condition expr)
function bpHas(addr) { return breakpoints.has(addr); }
function bpAdd(addr, cond) { breakpoints.set(addr, cond || ''); }
function bpDel(addr) { breakpoints.delete(addr); }
function bpCond(addr) { return breakpoints.get(addr) || ''; }
function bpAddrs() { return [...breakpoints.keys()]; }
function bpUncondAddrs() { return bpAddrs().filter(a => !bpCond(a)); }
function bpHit(addr) { return bpHas(addr) && (!bpCond(addr) || evalCond(bpCond(addr))); }
let history = [];   // snapshots for Step-Back (time-travel debugger)
const MAX_HISTORY = 200;        // bounded ring of CPU states
let prevRegMap = {};            // previous register values for change highlighting
let watches = loadWatches();    // watch expressions (registers / memory), persisted
let watchPrev = [];             // previous values for watch change highlighting
let currentTab = 'regs';        // active right-column tab (perf: only it is rendered)

function loadWatches() {
  try {
    const s = localStorage.getItem('mcu_watches');
    if (!s) return [];
    const a = JSON.parse(s);
    return Array.isArray(a) ? a : [];
  } catch { return []; }
}
function saveWatches() {
  try { localStorage.setItem('mcu_watches', JSON.stringify(watches)); } catch {}
}

// Register name sets per ISA, for the watch window.
const WATCH_REGS = {
  '8086': ['AX','BX','CX','DX','AH','AL','BH','BL','CH','CL','DH','DL','SI','DI','BP','SP','CS','DS','ES','SS','FS','GS','IP','FLAGS'],
  '8085': ['A','B','C','D','E','H','L','SP','PC','PSW'],
  '8051': ['A','B','DPTR','SP','PC','PSW','BANK','R0','R1','R2','R3','R4','R5','R6','R7'],
  'rv32': ['x0','x1','x2','x3','x4','x5','x6','x7','x8','x9','x10','x11','x12','x13','x14','x15','x16','x17','x18','x19','x20','x21','x22','x23','x24','x25','x26','x27','x28','x29','x30','x31','pc'],
  '6502': ['A','X','Y','PC','SP','P'],
  'Z80': ['A','F','B','C','D','E','H','L','IX','IY','SP','PC','I','R'],
};

const $ = (id) => document.getElementById(id);
const editor = $('editor'), gutter = $('gutter'), hl = $('hl'), errorsBox = $('errors'),
      regsBox = $('regs'), flagsBox = $('flags'), memView = $('memview'),
      outputBox = $('output'), errpop = $('errpop');

function newEmulator() {
  emu = new Emulator(isa);
  window.emu = emu; // exposed for console benchmarking, e.g. time a run()
    breakpoints = new Map();
  steps = 0; accumOut = '';
  history = [];
  prevRegMap = {};
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

// ---------- About modal (overview, usage, shortcuts) ----------
const ABOUT = `
<section>
  <h4>Overview</h4>
  <p><b>multi-cpu-emu</b> is a single Rust crate that emulates six classic
  microprocessors and compiles to one WebAssembly module, powering this
  in-browser IDE. No server, no install — assemble, run, and debug entirely
  in your browser.</p>
</section>
<section>
  <h4>Supported CPUs</h4>
  <table class="ab-tbl">
    <tr><th>ISA</th><th>Bits</th><th>Space</th><th>Highlights</th></tr>
    <tr><td>Intel 8086</td><td>16</td><td>1 MiB, segmented</td><td>DOS INT 21h / BIOS INT 10h, IVT, hardware IRQs</td></tr>
    <tr><td>Intel 8085</td><td>8</td><td>64 KiB</td><td>flags S/Z/AC/P/CY, TRAP/RST 7.5/6.5/5.5, SID/SOD</td></tr>
    <tr><td>Intel 8051</td><td>8</td><td>64 KiB code</td><td>SFRs, bit-ops, timers, serial, INT0/INT1</td></tr>
    <tr><td>MOS 6502</td><td>8</td><td>64 KiB</td><td>zero-page, decimal mode, BRK/RTI</td></tr>
    <tr><td>Zilog Z80</td><td>8</td><td>64 KiB</td><td>8080 + Z80 set, IX/IY, IM 0/1/2, block ops</td></tr>
    <tr><td>RISC-V rv32i</td><td>32</td><td>1 MiB flat</td><td>RV32I + M, CSR, ECALL semihosting</td></tr>
  </table>
</section>
<section>
  <h4>Quick start</h4>
  <ol>
    <li>Pick an <b>ISA</b> from the dropdown (top-left).</li>
    <li>Write assembly in the editor (or click <b>Example</b> to cycle samples).</li>
    <li>Press <b>Assemble</b> (F7) — errors show in the gutter and the bar below.</li>
    <li><b>Step</b> (F8) one instruction, or <b>Run</b> (F5) until halt/stop.</li>
    <li>Inspect registers, flags, disassembly, memory, ports, and output in the tabs.</li>
    <li><b>Reset</b> returns to the initial state; <b>Step-Back</b> undoes a step.</li>
  </ol>
</section>
<section>
  <h4>Assembler syntax</h4>
  <ul>
    <li><code>; comment</code> — inline comments.</li>
    <li>Labels: <code>name:</code> or <code>name EQU expr</code>.</li>
    <li>Directives: <code>ORG</code>, <code>DB</code>, <code>DW</code>, <code>EQU</code>, <code>END</code>.</li>
    <li>Numbers: decimal, <code>0x</code> hex, <code>h</code> suffix, <code>b</code> binary, <code>q</code> octal, <code>'c'</code> char.</li>
    <li>Programs load at <code>ORG 100h</code> (8086) / <code>ORG 0</code> (others).</li>
  </ul>
</section>
<section>
  <h4>I/O &amp; devices</h4>
  <p>Peripherals are driven purely by <code>OUT</code>/<code>IN</code> to fixed ports
  (see the <b>Devices</b> tab for 8086/8085/8051):</p>
  <table class="ab-tbl">
    <tr><th>Port</th><th>Device</th></tr>
    <tr><td>10h</td><td>Traffic light (bits 0/1/2 = red/yellow/green)</td></tr>
    <tr><td>11h, 12h</td><td>7-segment display (digits 0–15)</td></tr>
    <tr><td>13h</td><td>Stepper motor (4-bit coil pattern)</td></tr>
    <tr><td>14h</td><td>Printer (writes a byte)</td></tr>
    <tr><td>16h, 17h</td><td>Robot grid (X / Y position)</td></tr>
    <tr><td>20h–27h</td><td>8×8 LED matrix</td></tr>
  </table>
  <p>Text output conventions per ISA: <b>8086</b> → DOS <code>INT 21h</code>;
  <b>8085</b> → <code>OUT 01h</code>; <b>8051</b> → <code>SBUF</code>.</p>
</section>
<section>
  <h4>Interrupts</h4>
  <p>Use the IRQ bar for the selected ISA: 8086 NMI/INTR, 8085 TRAP/RST 7.5/6.5/5.5/INTR
  (+ SID/SOD), 8051 INT0/INT1 (+ serial RX), Z80 NMI/INT (+ IM 0/1/2).</p>
</section>
<section>
  <h4>Keyboard shortcuts</h4>
  <table class="ab-tbl ab-keys">
    <tr><th>Key</th><th>Action</th></tr>
    <tr><td>F7</td><td>Assemble &amp; load</td></tr>
    <tr><td>F8</td><td>Step one instruction</td></tr>
    <tr><td>F10</td><td>Step over a CALL</td></tr>
    <tr><td>F5</td><td>Run</td></tr>
    <tr><td>Esc</td><td>Stop running / close dialogs</td></tr>
    <tr><td>Ctrl+S</td><td>Save source to browser</td></tr>
    <tr><td>click disasm</td><td>toggle a breakpoint</td></tr>
  </table>
</section>
<section>
  <h4>Save · Load · Share</h4>
  <p><b>Save State</b> downloads a snapshot of the full CPU; <b>Load State</b>
  restores it (deterministic, works across refreshes). <b>Share Link</b> encodes
  the current source in the URL so others can open your program directly.</p>
</section>
<section>
  <h4>Source &amp; repo</h4>
  <p>Single Rust crate <code>multi-cpu-emu</code> → WASM. Reference design inspired by
  the MIT-licensed <code>modern8086</code> emulator. Built with
  <code>wasm-pack --target web</code>.</p>
</section>
`;
function showAbout() {
  const m = $('aboutModal');
  $('aboutBody').innerHTML = ABOUT;
  m.style.display = 'flex';
}
function closeAbout() { $('aboutModal').style.display = 'none'; }
$('aboutBtn').onclick = showAbout;
$('aboutClose').onclick = closeAbout;
$('aboutModal').addEventListener('click', (e) => { if (e.target === $('aboutModal')) closeAbout(); });

function entry() { return ISA_INFO[isa].entry; }

function fmt(v, w = 4) { return v.toString(16).toUpperCase().padStart(w, '0'); }

 function refresh() {
   const regs = emu.regs();
   const flags = emu.flags();
   const pc = emu.pc();

   // Status bar + run-control button states are always visible, so update them
   // on every refresh regardless of which tab is active.
   $('sbPc').textContent = ISA_INFO[isa].pcLabel(pc, regs);
   $('sbSteps').textContent = steps;
   $('sbState').textContent = emu.halted() ? 'halted' : (emu.waiting_input() ? 'waiting for input' : (runTimer ? 'running…' : 'ready'));
   $('stepBtn').disabled = runTimer || emu.halted();
   $('overBtn').disabled = runTimer || emu.halted();
   $('backBtn').disabled = runTimer || history.length === 0;
   $('runBtn').disabled = runTimer || emu.halted();
   $('stopBtn').disabled = !runTimer;

   // 8085 SOD output pin lives in the header IRQ bar, so update it every refresh.
   if (isa === '8085') {
     try {
       const sod = emu.sod();
       const el = $('sodLed');
       if (el) { el.textContent = 'SOD:' + sod; el.classList.toggle('on', sod !== 0); }
     } catch (e) {}
   }

   // The output buffer is drained on every refresh (so output is never lost
   // while another tab is showing); its DOM is only repainted on the Output tab.
   const fresh = emu.out();
   if (fresh) accumOut += fresh;

   renderTab(currentTab, regs, flags, pc);
 }

 // Render only the panels belonging to the active tab. This is the core perf
 // fix: a single step no longer rebuilds every panel's DOM, only the visible
 // one. Switching tabs calls showTab() which re-renders the newly shown group.
 function renderTab(name, regs, flags, pc) {
   if (name === 'regs') {
     renderRegs(regs, flags);
   } else if (name === 'code') {
     renderDisasm(pc);
     renderWatch(regs);
   } else if (name === 'mem') {
     renderMem(ISA_INFO[isa].memBase(pc));
     renderMemMap(emu, isa);
   } else if (name === 'io') {
     renderPorts();
     renderPeripherals(emu, isa);
   } else if (name === 'out') {
     outputBox.textContent = accumOut;
     renderScreen();
     renderGfx();
   } else if (name === 'dev') {
     renderDevices(emu, isa);
   }
 }

 function showTab(name) {
   currentTab = name;
   document.querySelectorAll('.tabgroup').forEach(g =>
     g.classList.toggle('active', g.dataset.tab === name));
   document.querySelectorAll('.tabs .tab').forEach(t =>
     t.classList.toggle('active', t.dataset.tab === name));
   // Re-render the now-visible group so it is never stale.
   renderTab(name, emu.regs(), emu.flags(), emu.pc());
 }

 function renderRegs(regs, flags) {
   const markChanged = (n, v) => {
     const ch = prevRegMap[n] !== undefined && prevRegMap[n] !== v;
     prevRegMap[n] = v;
     return ch;
   };
   let html = '';
   if (isa === '8086') {
     const pairs = [['AX','AH','AL'],['BX','BH','BL'],['CX','CH','CL'],['DX','DH','DL']];
     for (const [r, h, l] of pairs) {
       const v = val(regs, r);
       html += chip(r, fmt(v), `${h}=${fmt(v >> 8, 2)} ${l}=${fmt(v & 0xFF, 2)}`, false, markChanged(r, v));
     }
     for (const r of ['SI','DI','BP','SP','CS','DS','ES','SS']) {
       const v = val(regs, r);
       html += chip(r, fmt(v), null, false, markChanged(r, v));
     }
     let fv = 0;
     if (flags.includes('CF')) fv |= 0x001;
     if (flags.includes('PF')) fv |= 0x004;
     if (flags.includes('AF')) fv |= 0x010;
     if (flags.includes('ZF')) fv |= 0x040;
     if (flags.includes('SF')) fv |= 0x080;
     if (flags.includes('IF')) fv |= 0x200;
     if (flags.includes('DF')) fv |= 0x400;
     if (flags.includes('OF')) fv |= 0x800;
     const ipv = val(regs, 'IP');
     html += chip('IP', fmt(ipv), null, false, markChanged('IP', ipv));
     html += chip('FLAGS', fmt(fv), null, false, markChanged('FLAGS', fv));
   } else if (isa === '8085') {
     for (const r of ['A','B','C','D','E','H','L','SP']) { const v = val(regs, r); html += chip(r, fmt(v, 2), null, false, markChanged(r, v)); }
     const pcv = val(regs, 'PC');
     html += chip('PC', fmt(pcv), null, true, markChanged('PC', pcv));
   } else {
     for (const r of ['A','B','DPTR','SP','PC','PSW']) { const v = val(regs, r); html += chip(r, fmt(v, r === 'B' || r === 'A' ? 2 : 4), null, r === 'PC', markChanged(r, v)); }
     for (let i = 0; i < 8; i++) { const v = val(regs, 'R' + i); html += chip('R' + i, fmt(v, 2), null, false, markChanged('R' + i, v)); }
     const bk = val(regs, 'BANK');
     html += chip('BANK', fmt(bk, 1), null, false, markChanged('BANK', bk));
   }
   regsBox.innerHTML = html;
   renderFlags(flags);
 }

 function renderFlags(flags) {
   flagsBox.innerHTML = FLAG_MAP[isa].map(([key, label]) =>
     `<span class="flag ${flags.includes(label) ? 'on' : ''}">${label}</span>`).join('');
 }

function renderScreen() {
  const box = $('screen');
  const panel = $('screenPanel');
  if (!box || !panel) return;
  if (isa !== '8086') { panel.style.display = 'none'; return; }
  panel.style.display = '';
  const buf = emu.screen();
  const cur = emu.cursor();
  let html = '';
  for (let row = 0; row < 25; row++) {
    let line = '';
    for (let col = 0; col < 80; col++) {
      const i = (row * 80 + col) * 2;
      const ch = buf[i] || 0x20;
      const attr = buf[i + 1] || 0x07;
      const fg = VGA16(attr & 0x0F), bg = VGA16((attr >> 4) & 0x0F);
      const c = (ch >= 32 && ch < 127) ? String.fromCharCode(ch) : '·';
      const atCur = (cur[0] === col && cur[1] === row);
      const style = `color:${fg};background:${bg}`;
      line += atCur ? `<span class="cur" style="${style}">${c}</span>` : `<span style="${style}">${c}</span>`;
    }
    html += line + '\n';
  }
   box.innerHTML = html;
}

// VGA 16-colour text palette (matches the mode-13h base16).
function VGA16(n) {
  const p = ['#000','#0000aa','#00aa00','#00aaaa','#aa0000','#aa00aa','#aa5500','#aaaaaa','#555555','#5555ff','#55ff55','#55ffff','#ff5555','#ff55ff','#ffff55','#ffffff'];
  return p[n & 15];
}

// Standard VGA 256-colour palette (mode 13h)
const VGA_PAL = (function () {
  const p = [];
  const base16 = [[0,0,0],[0,0,170],[0,170,0],[0,170,170],[170,0,0],[170,0,170],[170,85,0],[170,170,170],[85,85,85],[85,85,255],[85,255,85],[85,255,255],[255,85,85],[255,85,255],[255,255,85],[255,255,255]];
  for (let i = 0; i < 16; i++) p[i] = base16[i];
  let n = 16;
  for (let r = 0; r < 6; r++) for (let g = 0; g < 6; g++) for (let b = 0; b < 6; b++) p[n++] = [r * 51, g * 51, b * 51];
  for (let i = 0; i < 24; i++) { const v = 8 + i * 10; p[232 + i] = [v, v, v]; }
  return p;
})();

function renderGfx() {
  const panel = $('gfxPanel');
  const cv = $('gfx');
  if (!panel || !cv) return;
  const info = emu.gfx();
  if (!info || isa !== '8086') { panel.style.display = 'none'; return; }
  panel.style.display = '';
  const w = info.w, h = info.h;
  const data = new Uint8Array(emu.mem(info.base, w * h));
  const ctx = cv.getContext('2d');
  const img = ctx.createImageData(w, h);
  for (let i = 0; i < w * h; i++) {
    const c = VGA_PAL[data[i]] || [0, 0, 0];
    img.data[i * 4] = c[0]; img.data[i * 4 + 1] = c[1]; img.data[i * 4 + 2] = c[2]; img.data[i * 4 + 3] = 255;
  }
  ctx.putImageData(img, 0, 0);
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

$('peripherals').addEventListener('click', (e) => {
  const el = e.target.closest('.sfr');
  if (!el) return;
  const addr = parseInt(el.dataset.sfr, 10);
  const cur = emu.sfr(addr);
  const inp = prompt('SFR ' + addr.toString(16).toUpperCase() + 'h value (hex, 00-FF)', fmt(cur, 2));
  if (inp === null) return;
  const v = parseInt(inp.trim(), 16);
  if (Number.isNaN(v) || v < 0 || v > 0xFF) { toast('Bad SFR value: ' + inp); return; }
  emu.set_sfr(addr, v);
  renderPeripherals(emu, isa);
});

$('portsClearBtn').onclick = () => {
  const n = isa === '8051' ? 4 : 256;
  for (let i = 0; i < n; i++) emu.port_write(i, 0);
  renderPorts();
  toast('Ports cleared');
};

function chip(name, value, sub = null, isPc = false, changed = false) {
  return `<div class="rreg ${isPc ? 'pc' : ''} ${changed ? 'changed' : ''}"><div class="n">${name}</div>` +
         `<div class="v">${value}</div>` +
         (sub ? `<div class="v sub">${sub}</div>` : '') + `</div>`;
}

// ---------- memory ----------
let memBase = 0;
let memPrev = null;            // previous memory page for diff highlighting
let memPrevBase = 0;
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
       const changed = memPrev && memPrevBase === memBase && memPrev[i] !== b;
       const cls = isPc ? 'hl' : (changed ? 'mb ch' : 'mb');
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
   memPrev = bytes.slice();
   memPrevBase = memBase;
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

// ---------- watch window ----------
$('watchAdd').onclick = () => {
  const v = $('watchInput').value.trim();
  if (v && !watches.includes(v)) { watches.push(v); watchPrev.push(undefined); $('watchInput').value = ''; saveWatches(); renderWatch(emu.regs()); }
};
$('watchInput').addEventListener('keydown', (e) => { if (e.key === 'Enter') $('watchAdd').click(); });
$('watchList').addEventListener('click', (e) => {
  const d = e.target.closest('.wdel');
  if (d) { watches.splice(+d.dataset.i, 1); watchPrev.splice(+d.dataset.i, 1); saveWatches(); renderWatch(emu.regs()); return; }
  const wv = e.target.closest('.wv');
  if (wv) editWatch(+wv.dataset.i);
});

// Click a watch value to edit it (registers via set_reg, memory via mem_write).
function editWatch(i) {
  const expr = watches[i];
  if (expr === undefined) return;
  const up = expr.trim().toUpperCase();
  const SUB = { AH:['AX',8], AL:['AX',0], BH:['BX',8], BL:['BX',0], CH:['CX',8], CL:['CX',0], DH:['DX',8], DL:['DX',0] };
  const isReg = !!SUB[up] || up === 'FLAGS' || WATCH_REGS[isa].includes(up);
  const cur = evalWatch(expr, emu.regs());
  const inp = prompt('New value for ' + cur.text + ' (hex):', Number.isFinite(cur.num) ? fmt(cur.num) : '0');
  if (inp === null) return;
  const v = parseInt(inp.trim(), 16);
  if (Number.isNaN(v) || v < 0 || v > 0xFFFFFFFF) { toast('Bad value: ' + inp); return; }
  if (isReg) {
    if (SUB[up]) {
      const [p, sh] = SUB[up];
      const parent = val(emu.regs(), p);
      emu.set_reg(p, (parent & ~(0xFF << sh)) | ((v & 0xFF) << sh));
    } else {
      emu.set_reg(up, v);
    }
  } else {
    const inner = (expr.startsWith('[') && expr.endsWith(']')) ? expr.slice(1, -1) : expr;
    const addr = evalExpr(inner, emu.regs(), false);
    if (Number.isNaN(addr) || addr < 0) { toast('Not editable'); return; }
    emu.mem_write(addr & 0xFFFFF, new Uint8Array([v & 0xFF, (v >> 8) & 0xFF]));
  }
  refresh();
}


// ---------- disassembly: click a line to toggle a breakpoint ----------
$('disasmView').addEventListener('click', (e) => {
  const row = e.target.closest('.drow');
  if (!row || !row.dataset.addr) return;
   const addr = parseInt(row.dataset.addr, 16);
   toggleBreakpoint(addr, e.shiftKey);
   renderDisasm(emu.pc());
});
// double-click a line to run-to-cursor
$('disasmView').addEventListener('dblclick', (e) => {
  const row = e.target.closest('.drow');
  if (!row || !row.dataset.addr) return;
  runToLine(parseInt(row.dataset.addr, 16));
});

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

// ---------- disassembly view (all ISAs) ----------
function renderDisasm(pc) {
  const view = $('disasmView');
  if (!view) return;
  let lines;
  try { lines = emu.disasm(pc, 40); } catch (e) { view.innerHTML = '<span class="muted">disasm unavailable</span>'; return; }
  let html = '';
  for (const ln of lines) {
    const m = ln.match(/^([0-9A-Fa-f]+)\s+(\S*)\s*(.*)$/);
    if (!m) { html += `<div class="drow"><span class="dtext">${escHtml(ln)}</span></div>`; continue; }
    const addr = parseInt(m[1], 16);
    const cur = (addr === pc) ? ' cur' : '';
    const bp = bpHas(addr) ? (bpCond(addr) ? ' cbp' : ' bp') : '';
    const tip = (DESC[m[3].trim().split(/\s+/)[0].toUpperCase()] || '').replace(/"/g, '&quot;');
    const titleAttr = tip ? ` title="${tip}"` : '';
    html += `<div class="drow${cur}${bp}" data-addr="${addr.toString(16)}">` +
            `<span class="daddr">${m[1]}</span>  <span class="dbytes">${m[2]}</span>  <span class="dtext"${titleAttr}>${escHtml(m[3])}</span></div>`;
  }
  view.innerHTML = html;
  // keep the current instruction in view
  const cur = view.querySelector('.drow.cur');
  if (cur && cur.scrollIntoView) cur.scrollIntoView({ block: 'nearest' });
}

// ---------- watch window ----------
function parseAddr(s) {
  s = (s || '').trim();
  if (/^0x/i.test(s)) return parseInt(s.slice(2), 16);
  if (/^0b/i.test(s)) return parseInt(s.slice(2), 2);
  if (/^0o/i.test(s)) return parseInt(s.slice(2), 8);
  if (/h$/i.test(s)) return parseInt(s.slice(0, -1), 16);
  if (/^%/.test(s)) return parseInt(s.slice(1), 2);
  if (/^\d+$/.test(s)) return parseInt(s, 10);
  return null;
}

// ---- richer watch / breakpoint expression language ----
// A watch or breakpoint side may be: a register (AX, IP), a sub-register
// (AH/AL), an individual flag (ZF, CF, …), the FLAGS word, a memory
// dereference [expr], a number (0x.., h/b/q suffix, decimal), or any
// arithmetic/bitwise combination (+, -, *, /, %, &, |, ^, ~, <<, >>, ()).
// In a *watch*, a bare numeric (e.g. `100h`) is treated as a memory address
// (legacy behaviour, matching "[addr]"); in breakpoint sides and inside
// "[...]" a bare number is a literal value.

const EXPR_FLAG_NAMES = ['CF','PF','AF','ZF','SF','TF','IF','DF','OF'];
const EXPR_FLAG_585 = ['S','Z','AC','P','CY'];

function flagsWordOf() {
  const fl = emu.flags();
  let fv = 0;
  if (fl.includes('CF')) fv |= 0x001;
  if (fl.includes('PF')) fv |= 0x004;
  if (fl.includes('AF')) fv |= 0x010;
  if (fl.includes('ZF')) fv |= 0x040;
  if (fl.includes('SF')) fv |= 0x080;
  if (fl.includes('TF')) fv |= 0x100;
  if (fl.includes('IF')) fv |= 0x200;
  if (fl.includes('DF')) fv |= 0x400;
  if (fl.includes('OF')) fv |= 0x800;
  return fv;
}

function tokenizeExpr(s) {
  const toks = [];
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (/\s/.test(c)) { i++; continue; }
    if (c === '0' && (s[i+1] === 'x' || s[i+1] === 'X')) {
      let j = i + 2; while (j < s.length && /[0-9a-fA-F]/.test(s[j])) j++;
      toks.push({ t: 'num', v: parseInt(s.slice(i + 2, j), 16) }); i = j; continue;
    }
    if (/[0-9]/.test(c)) {
      let j = i; while (j < s.length && /[0-9a-fA-F]/.test(s[j])) j++;
      const numStr = s.slice(i, j); let k = j;
      if (s[k] === 'h' || s[k] === 'H') { toks.push({ t: 'num', v: parseInt(numStr, 16) }); i = k + 1; continue; }
      if (s[k] === 'b' || s[k] === 'B') { toks.push({ t: 'num', v: parseInt(numStr, 2) }); i = k + 1; continue; }
      if (s[k] === 'q' || s[k] === 'Q') { toks.push({ t: 'num', v: parseInt(numStr, 8) }); i = k + 1; continue; }
      toks.push({ t: 'num', v: parseInt(numStr, 10) }); i = j; continue;
    }
    if (/[A-Za-z_.@]/.test(c)) {
      let j = i; while (j < s.length && /[A-Za-z0-9_.@]/.test(s[j])) j++;
      toks.push({ t: 'id', v: s.slice(i, j).toUpperCase() }); i = j; continue;
    }
    if ('+-*/%&|^~()[]<>'.includes(c)) {
      if ((c === '<' || c === '>') && s[i + 1] === c) { toks.push({ t: 'op', v: c + c }); i += 2; continue; }
      toks.push({ t: 'op', v: c }); i++; continue;
    }
    i++;
  }
  return toks;
}

function exprResolveId(id, regs) {
  const SUB = { AH:['AX',8], AL:['AX',0], BH:['BX',8], BL:['BX',0], CH:['CX',8], CL:['CX',0], DH:['DX',8], DL:['DX',0] };
  if (SUB[id]) { const [p, sh] = SUB[id]; return (val(regs, p) >> sh) & 0xFF; }
  if (EXPR_FLAG_NAMES.includes(id) || EXPR_FLAG_585.includes(id)) return emu.flags().includes(id) ? 1 : 0;
  if (id === 'FLAGS') return flagsWordOf();
  if (WATCH_REGS[isa].includes(id)) return val(regs, id);
  return NaN;
}

function exprReadMem(addr) {
  const b = new Uint8Array(emu.mem(addr & 0xFFFFF, 2));
  return b[0] | (b[1] << 8);
}

function evalExpr(expr, regs, bareMemory) {
  const toks = tokenizeExpr(expr);
  if (bareMemory && toks.length === 1 && toks[0].t === 'num') return exprReadMem(toks[0].v);
  let pos = 0;
  const peek = () => toks[pos];
  const next = () => toks[pos++];
  const expect = (v) => { if (toks[pos] && toks[pos].v === v) pos++; };
  function primary() {
    const tk = peek();
    if (!tk) return NaN;
    if (tk.t === 'op' && tk.v === '(') { next(); const v = parseExpr(); expect(')'); return v; }
    if (tk.t === 'op' && tk.v === '[') { next(); const a = parseExpr(); expect(']'); return exprReadMem(a); }
    if (tk.t === 'op' && (tk.v === '-' || tk.v === '~')) { next(); const v = primary(); return tk.v === '-' ? -v : ~v; }
    if (tk.t === 'num') { next(); return tk.v; }
    if (tk.t === 'id') { next(); return exprResolveId(tk.v, regs); }
    return NaN;
  }
  function factor() {
    let v = primary();
    while (peek() && peek().t === 'op' && ['*','/','%','<<','>>'].includes(peek().v)) {
      const op = next().v; const r = primary();
      v = op === '*' ? v*r : op === '/' ? (r === 0 ? 0 : (v / r) | 0)
        : op === '%' ? (r === 0 ? 0 : v % r)
        : op === '<<' ? (v << r) : (v >> r);
    }
    return v;
  }
  function term() {
    let v = factor();
    while (peek() && peek().t === 'op' && ['+','-','|','^','&'].includes(peek().v)) {
      const op = next().v; const r = factor();
      v = op === '+' ? v+r : op === '-' ? v-r : op === '|' ? (v|r) : op === '^' ? (v^r) : (v&r);
    }
    return v;
  }
  function parseExpr() { return term(); }
  const r = parseExpr();
  return Number.isFinite(r) ? (r >>> 0) : NaN;
}

function evalWatch(expr, regs) {
  const e = expr.trim();
  const num = evalExpr(e, regs, true);
  if (Number.isNaN(num)) return { text: e.toUpperCase(), value: '?', num: NaN };
  return { text: e.toUpperCase(), value: fmt(num & 0xFFFF, 4), num };
}

// conditional breakpoint expression: "LHS op RHS" (op: == != <= >= < >)
function evalCond(expr) {
  expr = expr.trim();
  if (!expr) return true;
  const m = expr.match(/^(.*?)\s*(==|!=|<=|>=|<|>)\s*(.*)$/);
  if (!m) return true;
  // Both sides are full expressions; brackets mean memory, bare numbers are
  // literal values, so "CX == 0x10" compares to the value and "[BX+2] != 0"
  // reads memory.
  const l = evalExpr(m[1].trim(), emu.regs(), false);
  const r = evalExpr(m[3].trim(), emu.regs(), false);
  if (Number.isNaN(l) || Number.isNaN(r)) return false;
  switch (m[2]) {
    case '==': return l === r;
    case '!=': return l !== r;
    case '<':  return l < r;
    case '>':  return l > r;
    case '<=': return l <= r;
    case '>=': return l >= r;
  }
  return false;
}

function renderWatch(regs) {
  const box = $('watchList');
  if (!box) return;
  if (watches.length === 0) {
    box.innerHTML = '<span class="muted">Add a register (AX) or memory ([0x200], 100h) to watch.</span>';
    return;
  }
  let html = '';
  for (let i = 0; i < watches.length; i++) {
    const { text, value, num } = evalWatch(watches[i], regs);
    const ch = (watchPrev[i] !== undefined && watchPrev[i] !== num) ? ' changed' : '';
    watchPrev[i] = num;
    html += `<div class="watchrow"><span class="wn">${escHtml(text)}</span>` +
            `<span class="wv${ch}" data-i="${i}" title="click to edit">${escHtml(value)}</span>` +
            `<span class="wdel" data-i="${i}" title="remove">✕</span></div>`;
  }
  box.innerHTML = html;
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
    const bp = addr && bpHas(addr) ? (bpCond(addr) ? ' cbp' : ' bp') : '';
    html += `<div class="${i + 1 === errLine ? 'err' : ''}${bp}" ${addr ? `data-addr="${addr.toString(16)}"` : ''}><span class="n" ${addr ? `data-addr="${addr.toString(16)}"` : ''} title="${addr ? (bpHas(addr) ? 'Breakpoint (Shift-click: edit condition)' : 'Toggle breakpoint (Shift-click: condition)') : ''}">${i + 1}</span>` +
            (c ? `<span class="c">${c}</span>` : '') + '</div>';
  }
  gutter.innerHTML = html;
}
gutter.addEventListener('click', (e) => {
  const el = e.target.closest('[data-addr]');
  if (!el || el.closest('.err')) return;
   const addr = parseInt(el.dataset.addr, 16);
   if (el.classList.contains('n')) toggleBreakpoint(addr, e.shiftKey);
   else runToLine(addr);
});

function toggleBreakpoint(addr, shift) {
  if (shift) {
    const cur = bpCond(addr);
    const c = prompt('Breakpoint condition (e.g. CX==0, mem[0x200]==5, AX>10). Empty = unconditional:', cur);
    if (c === null) return; // cancelled
    if (c.trim() === '') { bpDel(addr); toast('Breakpoint removed'); }
    else { bpAdd(addr, c.trim()); toast('Conditional breakpoint @' + fmt(addr).padStart(4, '0') + 'h: ' + c.trim()); }
    renderSource(); return;
  }
  if (bpHas(addr)) { bpDel(addr); toast('Breakpoint removed'); }
  else { bpAdd(addr, ''); toast('Breakpoint set @' + fmt(addr).padStart(4, '0') + 'h'); }
  renderSource();
}
function renderSource() { buildGutter(); renderHl(); }
editor.oninput = () => { renderSource(); saveSource(); updateAc(); };
editor.onscroll = () => { gutter.scrollTop = editor.scrollTop; hl.scrollTop = editor.scrollTop; hl.scrollLeft = editor.scrollLeft; hideAc(); };
editor.onkeydown = (e) => {
  if (acEl && acEl.style.display !== 'none' && acMatches.length) {
    if (e.key === 'ArrowDown') { e.preventDefault(); acIdx = (acIdx + 1) % acMatches.length; refreshAcSel(); return; }
    if (e.key === 'ArrowUp')   { e.preventDefault(); acIdx = (acIdx - 1 + acMatches.length) % acMatches.length; refreshAcSel(); return; }
    if (e.key === 'Enter' || e.key === 'Tab') { e.preventDefault(); applyAc(); return; }
    if (e.key === 'Escape') { e.preventDefault(); hideAc(); return; }
  }
  if (e.key === 'Tab') {
    e.preventDefault();
    const s = editor.selectionStart;
    editor.setRangeText('    ', s, editor.selectionEnd, 'end');
    renderSource();
  }
};
editor.addEventListener('blur', () => setTimeout(hideAc, 150));
/*AC*/
const ISA_MNEM = {
  8086: ['MOV','ADD','ADC','SUB','SBB','AND','OR','XOR','CMP','INC','DEC','MUL','IMUL','DIV','IDIV','NOT','NEG','TEST','XCHG','LEA','PUSH','POP','PUSHA','POPA','CALL','RET','RETF','JMP','JE','JZ','JNE','JNZ','JC','JB','JNC','JNB','JA','JAE','JBE','JG','JGE','JL','JLE','JO','JNO','JS','JNS','JP','JPE','JNP','JPO','LOOP','LOOPZ','LOOPNZ','JCXZ','INT','IRET','INT3','INTO','CLC','STC','CMC','CLI','STI','CLD','STD','NOP','HLT','SHL','SHR','SAL','SAR','ROL','ROR','RCL','RCR','CBW','CWD','DAA','DAS','AAA','AAS','AAM','AAD','LAHF','SAHF','MOVS','LODS','STOS','CMPS','SCAS','IN','OUT'],
  8085: ['MOV','MVI','LXI','LDA','STA','LHLD','SHLD','LDAX','STAX','XCHG','ADD','ADC','SUB','SBB','ANA','XRA','ORA','CMP','ADI','ACI','SUI','SBI','ANI','XRI','ORI','CPI','INR','DCR','INX','DCX','DAD','RLC','RRC','RAL','RAR','CMA','CMC','STC','DAA','JMP','JZ','JNZ','JC','JNC','JP','JM','JPE','JPO','CALL','CC','CNC','CZ','CNZ','CP','CM','CPE','CPO','RET','RNZ','RZ','RNC','RP','RM','RPE','RPO','RST','PUSH','POP','XTHL','SPHL','PCHL','EI','DI','SIM','RIM','IN','OUT','NOP','HLT'],
  8051: ['MOV','MOVC','MOVX','PUSH','POP','XCH','XCHD','SWAP','ADD','ADDC','SUBB','INC','DEC','MUL','DIV','DA','ANL','ORL','XRL','CLR','CPL','RL','RR','RLC','RRC','SETB','SJMP','AJMP','LJMP','JZ','JNZ','JC','JNC','JB','JNB','JBC','CJNE','DJNZ','ACALL','LCALL','RET','RETI','NOP'],
  rv32: ['LUI','AUIPC','JAL','JALR','BEQ','BNE','BLT','BGE','BLTU','BGEU','LB','LH','LW','LBU','LHU','SB','SH','SW','ADDI','SLTI','SLTIU','XORI','ORI','ANDI','SLLI','SRLI','SRAI','ADD','SUB','SLL','SLT','SLTU','XOR','SRL','SRA','OR','AND','FENCE','ECALL','EBREAK'],
  '6502': ['LDA','LDX','LDY','STA','STX','STY','ADC','SBC','INC','DEC','AND','ORA','EOR','ASL','LSR','ROL','ROR','CMP','CPX','CPY','BIT','JMP','JSR','RTS','RTI','BCC','BCS','BEQ','BNE','BMI','BPL','BVC','BVS','CLC','SEC','CLI','SEI','CLV','CLD','SED','TAX','TAY','TSX','TXA','TXS','TYA','DEX','DEY','INX','INY','PHA','PHP','PLA','PLP','BRK','NOP'],
  'Z80': ['LD','LDIR','LDI','LDD','PUSH','POP','ADD','ADC','SUB','SBC','AND','OR','XOR','CP','INC','DEC','RLCA','RRCA','RLA','RRA','RLC','RRC','RL','RR','SLA','SRA','SRL','EX','EXX','JP','JR','CALL','RET','DJNZ','NOP','HALT','DI','EI','CPL','SCF','CCF','DAA','BIT','RES','SET','IN','OUT','RST','NZ','Z','NC','C','PO','PE','P','M'],
};
const REG_WORDS = {
  8086: ['AX','BX','CX','DX','AH','AL','BH','BL','CH','CL','DH','DL','SI','DI','BP','SP','CS','DS','ES','SS','IP','FLAGS'],
  8085: ['A','B','C','D','E','H','L','PSW','SP','PC'],
  8051: ['A','B','R0','R1','R2','R3','R4','R5','R6','R7','DPTR','PSW','SP','PC','ACC','DPH','DPL'],
  rv32: ['x0','x1','x2','x3','x4','x5','x6','x7','x8','x9','x10','x11','x12','x13','x14','x15','x16','x17','x18','x19','x20','x21','x22','x23','x24','x25','x26','x27','x28','x29','x30','x31','pc'],
  'Z80': ['A','F','B','C','D','E','H','L','AF','BC','DE','HL','IX','IY','SP','PC','I','R'],
};
const DESC = {
  MOV:'Move data', ADD:'Add', ADC:'Add with carry', SUB:'Subtract', SBB:'Subtract with borrow',
  AND:'Logical AND', OR:'Logical OR', XOR:'Logical XOR', CMP:'Compare (sets flags only)', INC:'Increment', DEC:'Decrement',
  MUL:'Unsigned multiply', IMUL:'Signed multiply', DIV:'Unsigned divide', IDIV:'Signed divide',
  NOT:'Ones complement', NEG:'Twos complement', TEST:'Bitwise test (flags only)', XCHG:'Exchange',
  LEA:'Load effective address', PUSH:'Push onto stack', POP:'Pop from stack', CALL:'Call subroutine', RET:'Return from subroutine',
  JMP:'Unconditional jump', JE:'Jump if equal', JZ:'Jump if zero', JNE:'Jump if not equal', JNZ:'Jump if not zero',
  JC:'Jump if carry', JNC:'Jump if no carry', LOOP:'Loop CX times', INT:'Software interrupt', IRET:'Return from interrupt',
  NOP:'No operation', HLT:'Halt', SHL:'Shift left', SHR:'Shift right', SAR:'Arithmetic shift right',
  ROL:'Rotate left', ROR:'Rotate right', CLC:'Clear carry', STC:'Set carry', CLI:'Clear interrupt flag', STI:'Set interrupt flag',
  LXI:'Load immediate pair', LDA:'Load A from address', STA:'Store A to address', MVI:'Move immediate',
  INR:'Increment register', DCR:'Decrement register', DAD:'Add pair to HL', RLC:'Rotate A left', RRC:'Rotate A right',
  RAL:'Rotate A left through carry', RAR:'Rotate A right through carry', DAA:'Decimal adjust A', EI:'Enable interrupts', DI:'Disable interrupts',
  MOVX:'External data move', MOVC:'Code memory move', ANL:'Logical AND', ORL:'Logical OR', XRL:'Logical XOR',
  CLR:'Clear bit/register', SETB:'Set bit', SJMP:'Short jump', AJMP:'Absolute jump', LJMP:'Long jump',
  JB:'Jump if bit set', JNB:'Jump if bit clear', JBC:'Jump if bit set and clear',
  CJNE:'Compare and jump if not equal', DJNZ:'Decrement and jump if not zero', ACALL:'Absolute call', LCALL:'Long call', RETI:'Return from interrupt',
};
let acEl = null, acMatches = [], acTok = null, acIdx = -1;
(function () {
  acEl = document.createElement('div');
  acEl.id = 'ac';
  acEl.style.display = 'none';
  document.body.appendChild(acEl);
  acEl.addEventListener('mousedown', (e) => {
    e.preventDefault();
    const item = e.target.closest('.aci');
    if (!item) return;
    acIdx = parseInt(item.dataset.i, 10);
    applyAc();
  });
})();
function acWordsFor() {
  return (ISA_MNEM[isa] || []).concat(REG_WORDS[isa] || [], ['ORG', 'DB', 'DW', 'EQU', 'END']);
}
function currentToken() {
  const v = editor.value, pos = editor.selectionStart;
  let s = pos; while (s > 0 && /[A-Za-z0-9_@.]/.test(v[s - 1])) s--;
  let e = pos; while (e < v.length && /[A-Za-z0-9_@.]/.test(v[e])) e++;
  return { start: s, end: e, text: v.slice(s, e) };
}
function updateAc() {
  const tok = currentToken();
  if (tok.text.length < 1) { hideAc(); return; }
  const prefix = tok.text.toUpperCase();
  const matches = acWordsFor().filter(w => w.startsWith(prefix) && w !== prefix);
  if (matches.length === 0) { hideAc(); return; }
  acMatches = matches; acTok = tok; acIdx = 0;
  acEl.innerHTML = matches.slice(0, 10).map((w, i) => {
    const d = DESC[w] || '';
    return `<div class="aci${i === 0 ? ' sel' : ''}" data-i="${i}"><span class="w">${w}</span>${d ? `<span class="d">${d}</span>` : ''}</div>`;
  }).join('');
  const c = getCaretCoordinates(editor);
  const r = editor.getBoundingClientRect();
  acEl.style.left = (r.left + window.scrollX + c.left) + 'px';
  acEl.style.top = (r.top + window.scrollY + c.top + c.height) + 'px';
  acEl.style.display = 'block';
}
function refreshAcSel() {
  [...acEl.querySelectorAll('.aci')].forEach((el, i) => el.classList.toggle('sel', i === acIdx));
  const sel = acEl.querySelector('.aci.sel');
  if (sel) sel.scrollIntoView({ block: 'nearest' });
}
function hideAc() { if (acEl) acEl.style.display = 'none'; acMatches = []; acIdx = -1; }
function applyAc() {
  if (acIdx < 0 || !acMatches[acIdx]) return;
  const w = acMatches[acIdx];
  editor.setRangeText(w, acTok.start, acTok.end, 'end');
  hideAc(); renderSource(); saveSource();
}
function getCaretCoordinates(el) {
  const div = document.createElement('div');
  const cs = getComputedStyle(el);
  const props = ['boxSizing','width','height','overflowX','overflowY','borderTopWidth','borderRightWidth','borderBottomWidth','borderLeftWidth','paddingTop','paddingRight','paddingBottom','paddingLeft','fontStyle','fontVariant','fontWeight','fontStretch','fontSize','fontFamily','lineHeight','textAlign','textTransform','textIndent','letterSpacing','wordSpacing','tabSize'];
  props.forEach(p => div.style[p] = cs[p]);
  div.style.position = 'absolute'; div.style.visibility = 'hidden';
  div.style.whiteSpace = 'pre-wrap'; div.style.wordWrap = 'break-word';
  div.textContent = el.value.substring(0, el.selectionStart);
  const span = document.createElement('span');
  span.textContent = el.value.substring(el.selectionStart) || '.';
  div.appendChild(span);
  document.body.appendChild(div);
  const top = span.offsetTop + parseInt(cs.borderTopWidth || '0', 10);
  const left = span.offsetLeft + parseInt(cs.borderLeftWidth || '0', 10);
  const height = parseInt(cs.lineHeight || '16', 10);
  document.body.removeChild(div);
  return { top, left, height };
}

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
    // Load as ROM: the program image is immutable, so the 8086/rv32 cores can
    // trust their decode caches (skip the per-step re-fetch) — much faster
    // interactive stepping/running. External memory edits clear the ROM mark.
    emu.load_rom(code, 0);
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
  history.push({ snap: emu.snapshot(), steps });
  if (history.length > MAX_HISTORY) history.shift();
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
  history.push({ snap: emu.snapshot(), steps });
  if (history.length > MAX_HISTORY) history.shift();
  const ret = callRetAddr();
  if (ret === null) {
    emu.step();
    steps++;
    refresh();
    maybePromptInput();
    return;
  }
  const active = bpUncondAddrs().filter(a => a !== emu.pc());
  const n = emu.run_bp(100000, active.concat([ret]));
  steps += n;
  refresh();
  maybePromptInput();
  if (!emu.halted() && !emu.waiting_input() && bpHit(emu.pc()) && emu.pc() !== ret) {
    toast('Breakpoint at ' + fmt(emu.pc()).padStart(4, '0') + 'h');
  }
}

function runToLine(addr) {
  if (runTimer || emu.halted()) return;
  history.push({ snap: emu.snapshot(), steps });
  if (history.length > MAX_HISTORY) history.shift();
  if (addr === emu.pc()) return;
  const active = bpUncondAddrs().filter(a => a !== emu.pc());
  const n = emu.run_bp(100000, active.concat([addr]));
  steps += n;
  refresh();
  maybePromptInput();
  if (!emu.halted() && !emu.waiting_input()) {
    if (bpHit(emu.pc()) && emu.pc() !== addr) toast('Breakpoint at ' + fmt(emu.pc()).padStart(4, '0') + 'h');
    else if (emu.pc() !== addr) toast('Never reached that line (jumped over it?)');
  }
}

 function startRun() {
   if (emu.halted() || runTimer) return;
   stopRequested = false;
   // Snapshot the pre-run state so Step-Back can undo the whole run (time-travel).
   history.push({ snap: emu.snapshot(), steps });
   if (history.length > MAX_HISTORY) history.shift();
   const condBps = bpAddrs().filter(a => bpCond(a));
   const active = bpUncondAddrs().filter(a => a !== emu.pc()); // continue past the bp we are stopped at
   $('stopBtn').disabled = false;
   refresh();
   // Drive the run from requestAnimationFrame so the UI repaints in lock-step
   // with the display (smoother than setInterval, and it naturally pauses when
   // the tab is backgrounded).
   const tick = () => {
     if (stopRequested || emu.halted()) { stopRun(); return; }
     if (condBps.length > 0) {
       // stepping mode so conditional breakpoints are tested every instruction
       let n = 0;
       while (n < 30000 && !emu.halted() && !stopRequested) {
         emu.step(); n++;
         if (emu.waiting_input() || bpHit(emu.pc())) break;
       }
       steps += n; refresh();
       if (emu.waiting_input()) { stopRun(); maybePromptInput(); return; }
       if (n < 30000 && !stopRequested && bpHit(emu.pc())) { stopRun(); toast('Breakpoint @' + fmt(emu.pc()).padStart(4, '0') + 'h'); return; }
       if (emu.halted() || stopRequested) { stopRun(); return; }
     } else {
       const n = emu.run_bp(30000, active);
       steps += n; refresh();
       if (emu.waiting_input()) { stopRun(); maybePromptInput(); return; }
       if (n < 30000 && !stopRequested && bpHit(emu.pc())) { stopRun(); toast('Breakpoint @' + fmt(emu.pc()).padStart(4, '0') + 'h'); return; }
       if (emu.halted() || stopRequested) { stopRun(); return; }
     }
     rafId = requestAnimationFrame(tick);
   };
   runTimer = 1;                 // running sentinel (button/state logic unchanged)
   rafId = requestAnimationFrame(tick);
 }

 function stopRun() {
   if (rafId) { cancelAnimationFrame(rafId); rafId = null; }
   if (runTimer) { clearInterval(runTimer); } // no-op when runTimer is the sentinel
   runTimer = null;
   stopRequested = false;
   refresh();
 }

$('asmBtn').onclick = assemble;
$('stepBtn').onclick = stepOnce;
$('overBtn').onclick = stepOver;
$('backBtn').onclick = () => {
  if (!history.length || runTimer) return;
  const h = history.pop();
  emu.restore(h.snap);
  steps = h.steps;
  refresh();
};
$('runBtn').onclick = startRun;
$('stopBtn').onclick = stopRun;
$('resetBtn').onclick = () => { stopRun(); newEmulator(); resetDevices(); refresh(); toast('CPU reset'); };
$('devResetBtn').onclick = () => { resetDevices(); renderDevices(emu, isa); toast('Devices cleared'); };
$('clearOutBtn').onclick = () => { accumOut = ''; outputBox.textContent = ''; };

// ---------- snapshot Save / Load ----------
$('saveBtn').onclick = () => {
  if (!emu) return;
  const bytes = emu.snapshot();
  const blob = new Blob([bytes], { type: 'application/octet-stream' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = `emu-${isa}-state.bin`;
  a.click();
  URL.revokeObjectURL(a.href);
  toast('State saved');
};
let loadInput = null;
$('loadBtn').onclick = () => {
  if (!emu) return;
  if (!loadInput) {
    loadInput = document.createElement('input');
    loadInput.type = 'file';
    loadInput.onchange = (e) => {
      const file = e.target.files[0];
      if (!file) return;
      file.arrayBuffer().then((buf) => {
        emu.restore(new Uint8Array(buf));
        history = [];
        refresh();
        toast('State loaded');
      });
    };
  }
  loadInput.click();
};

// ---------- load a ROM / firmware image (marked read-only) ----------
let romInput = null;
$('romBtn').onclick = () => {
  if (!emu) return;
  if (!romInput) {
    romInput = document.createElement('input');
    romInput.type = 'file';
    romInput.onchange = (e) => {
      const file = e.target.files[0];
      if (!file) return;
      file.arrayBuffer().then((buf) => {
        const addr = parseInt($('romaddr').value.trim(), 16) || 0;
        // 8051: a ROM image is external code, so force EA low.
        if (isa === '8051') emu.set_ea(false);
        emu.load_rom(new Uint8Array(buf), addr);
        renderMemMap(emu, isa);
        toast(`ROM loaded @ ${addr.toString(16)} (${buf.byteLength} bytes)`);
      });
    };
  }
  romInput.click();
};

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

 // share-by-URL: encode the current source in the location fragment
 let hashApplied = false;
 function applyHash() {
   if (hashApplied) return;
   const p = new URLSearchParams(location.hash.replace(/^#/, ''));
   if (!p.has('src')) return;
   hashApplied = true;
   const wantIsa = p.get('isa');
   if (wantIsa && ISA_LIST.includes(wantIsa) && wantIsa !== isa) {
     isa = wantIsa; $('isa').value = isa; newEmulator();
   }
   try { editor.value = decodeURIComponent(p.get('src')); }
   catch { editor.value = p.get('src'); }
   errLine = -1; codeMap = []; renderSource(); saveSource();
 }
 $('shareBtn').onclick = () => {
   const full = location.href.split('#')[0] + '#isa=' + isa + '&src=' + encodeURIComponent(editor.value);
   history.replaceState(null, '', '#isa=' + isa + '&src=' + encodeURIComponent(editor.value));
   const done = () => toast('Share link copied to clipboard');
   if (navigator.clipboard && navigator.clipboard.writeText) {
     navigator.clipboard.writeText(full).then(done, () => prompt('Copy this share link:', full));
   } else {
     prompt('Copy this share link:', full);
   }
 };

$('isa').onchange = () => {
  isa = $('isa').value;
  newEmulator();
  loadSource();
  $('memaddr').value = '0';
  memBase = 0;
  $('romaddr').value = isa === '8086' ? 'F0000' : '0';
  $('intrBar85').style.display = isa === '8085' ? '' : 'none';
  $('intrBar51').style.display = isa === '8051' ? '' : 'none';
  $('intrBar86').style.display = isa === '8086' ? '' : 'none';
  $('intrBarZ80').style.display = isa === 'Z80' ? '' : 'none';
  $('z80memPanel').style.display = isa === 'Z80' ? '' : 'none';
  if (isa === 'Z80') z80Dump();
  renderSource();
  updateTabsForIsa();
  // If the active tab is no longer valid for this ISA, fall back to Registers.
  if (currentTab === 'dev' && (isa !== '8086' && isa !== 'Z80')) showTab('regs');
  else refresh();
};

// Show/hide the Devices tab: only meaningful for 8086 (on-chip peripherals)
// and Z80 (memory editor). Other ISAs have no content there.
function updateTabsForIsa() {
  const devTab = $('devTab');
  if (devTab) devTab.classList.toggle('hidden', !(isa === '8086' || isa === 'Z80'));
}

// Tab navigation: only one right-column group is visible at a time.
document.querySelectorAll('.tabs .tab').forEach(t => {
  t.addEventListener('click', () => showTab(t.dataset.tab));
});

$('irqTrap').onclick = () => { emu.interrupt('TRAP', 0); refresh(); };
$('irq75').onclick = () => { emu.interrupt('RST75', 0); refresh(); };
$('irq65').onclick = () => { emu.interrupt('RST65', 0); refresh(); };
$('irq55').onclick = () => { emu.interrupt('RST55', 0); refresh(); };
$('irqIntr').onclick = () => { emu.interrupt('INTR', 0x08); refresh(); };
$('irqInt0').onclick = () => { emu.interrupt('INT0', 0); refresh(); };
$('irqInt1').onclick = () => { emu.interrupt('INT1', 0); refresh(); };
$('irqNmi').onclick = () => { emu.interrupt('NMI', 0); refresh(); };
$('irqIntr86').onclick = () => { emu.interrupt('INTR', 0x08); refresh(); };
$('irqNmiZ80').onclick = () => { emu.interrupt('NMI', 0); refresh(); };
$('irqIntZ80').onclick = () => { emu.interrupt('INT', 0); refresh(); };
$('z80im').onchange = () => { const m = parseInt($('z80im').value, 10) || 0; try { emu.set_interrupt_mode(m); } catch (e) {} refresh(); };

// ---------- 8085 SID/SOD ----------
let sidState = false;
$('sidBtn').onclick = () => {
  sidState = !sidState;
  try { emu.set_sid(sidState); } catch (e) {}
  $('sidBtn').classList.toggle('on', sidState);
  refresh();
};

// ---------- 8051 serial RX injector ----------
$('serSend').onclick = () => {
  const t = $('serIn').value || '';
  if (t.length === 0) return;
  try { emu.serial_rx(t.charCodeAt(0) & 0xFF); } catch (e) {}
  $('serIn').value = '';
  refresh();
};

// ---------- Z80 memory hex editor ----------
$('z80memPanel').style.display = 'none';
function z80Dump() {
  const base = (parseInt($('z80memBase').value, 16) || 0) & 0xFFFF;
  const bytes = new Uint8Array(emu.mem(base, 256));
  let out = '';
  for (let row = 0; row < 16; row++) {
    let line = fmt(base + row * 16, 4) + ': ';
    for (let i = 0; i < 16; i++) line += fmt(bytes[row * 16 + i], 2) + ' ';
    out += line.trimEnd() + '\n';
  }
  $('z80mem').value = out;
}
$('z80memDump').onclick = () => { z80Dump(); };
$('z80memApply').onclick = () => {
  const base = (parseInt($('z80memBase').value, 16) || 0) & 0xFFFF;
  const lines = $('z80mem').value.split('\n');
  let off = 0;
  for (const ln of lines) {
    const parts = ln.trim().split(/\s+/).filter(s => /^[0-9a-fA-F]{1,2}$/.test(s));
    for (const h of parts) {
      emu.mem_write((base + off) & 0xFFFF, [parseInt(h, 16)]);
      off++;
    }
  }
  toast('Wrote ' + off + ' bytes @ ' + fmt(base, 4));
  refresh();
};

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
    case 'F4': e.preventDefault(); stopRun(); newEmulator(); resetDevices(); refresh(); toast('CPU reset'); break;
    case 'Escape':
      if ($('aboutModal').style.display !== 'none') { closeAbout(); break; }
      stopRun();
      break;
  }
});

 newEmulator();
 loadSource();
 applyHash();
  $('intrBar85').style.display = 'none';
  $('intrBar51').style.display = 'none';
  $('intrBar86').style.display = '';   // default ISA is 8086
  $('intrBarZ80').style.display = 'none';
  $('romaddr').value = isa === '8086' ? 'F0000' : '0';
  if (window.applyI18n) window.applyI18n();
  updateTabsForIsa();
  showTab(currentTab);
  refresh();