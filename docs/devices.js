// Peripheral device models — separate from the CPU core.
//
// The chip WASM only handles the CPU + I/O ports. This module simulates the
// attached peripherals purely from the live port values, so no device logic
// lives in the wasm binary. Each device maps to fixed OUT ports:
//
//   0x10        traffic light      bits 0/1/2 = red / yellow / green
//   0x11,0x12   7-segment display  digit value 0..15 (low, high)
//   0x13        stepper motor      4-bit coil pattern -> rotor position
//   0x14        printer            write a byte (accumulated by diffing)
//   0x15        printer status     reads 0x80 (ready)
//   0x16,0x17   robot grid          X / Y position (0..15), trail by diffing
//   0x20..0x27  LED matrix          8 rows x 8 columns (bit7 = leftmost)
//
// Stateful devices (printer, robot) accumulate by detecting changes to the
// live port value between refreshes, which is enough for step/run teaching.

// 7-segment glyphs for digits 0..15 (bit0=a … bit6=g).
const SEG7 = [
  0b0111111, 0b0000110, 0b1011011, 0b1001111, 0b1100110, 0b1101101,
  0b1111101, 0b0000111, 0b1111111, 0b1101111, 0b1110111, 0b1111100,
  0b0111001, 0b1011110, 0b1111001, 0b1110001,
];

// 4-bit coil pattern -> rotor position (full + half steps).
const STEP_POS = { 1: 0, 3: 1, 2: 2, 6: 3, 4: 4, 12: 5, 8: 6, 9: 7 };

let printerBuf = '';
let lastPrinter = 0;
let robotTrail = [];
let lastRobot = null;
let lastStep = 0;

export function resetDevices() {
  printerBuf = '';
  lastPrinter = 0;
  robotTrail = [];
  lastRobot = null;
  lastStep = 0;
  for (const id in floaters) { floaters[id].win.remove(); delete floaters[id]; }
}

// ---- device pop-out floaters -------------------------------------------
// Any device can be detached into a draggable floating window (like the
// reference modern8086 IDE). The window position persists in localStorage;
// while popped out the device keeps updating live during Run.
const POP_KEY = 'mce_popped';
const POS_KEY = 'mce_floater_pos';
const floaters = {};
function getPopped() { try { return new Set(JSON.parse(localStorage.getItem(POP_KEY) || '[]')); } catch { return new Set(); } }
function setPopped(s) { try { localStorage.setItem(POP_KEY, JSON.stringify([...s])); } catch {} }
function getPos() { try { return JSON.parse(localStorage.getItem(POS_KEY) || '{}'); } catch { return {}; } }
function setPos(p) { try { localStorage.setItem(POS_KEY, JSON.stringify(p)); } catch {} }
function deviceTitle(id) {
  return ({ traffic:'Traffic light', seven:'7-segment', stepper:'Stepper', printer:'Printer', robot:'Robot grid', led:'LED matrix', timing:'Clock / timers', ppi:'8255 PPI', flash:'Flash/EEPROM', rtc:'RTC DS1307', adc:'ADC0808', lcd:'LCD 1602', dma:'8237 DMA' })[id] || id;
}
function ensureFloater(id) {
  if (floaters[id]) return floaters[id];
  const win = document.createElement('div');
  win.className = 'floater';
  const pos = getPos()[id] || { x: 140, y: 140 };
  win.style.left = pos.x + 'px';
  win.style.top = pos.y + 'px';
  win.innerHTML = `<div class="floater-head">${deviceTitle(id)}<button class="fl-close" title="Return to panel">✕</button></div><div class="floater-body"></div>`;
  document.body.appendChild(win);
  const head = win.querySelector('.floater-head');
  head.addEventListener('mousedown', (e) => {
    if (e.target.classList.contains('fl-close')) return;
    e.preventDefault();
    const sx = e.clientX, sy = e.clientY, ox = win.offsetLeft, oy = win.offsetTop;
    const mv = (ev) => { win.style.left = (ox + ev.clientX - sx) + 'px'; win.style.top = (oy + ev.clientY - sy) + 'px'; };
    const up = () => {
      document.removeEventListener('mousemove', mv);
      document.removeEventListener('mouseup', up);
      const p = getPos(); p[id] = { x: win.offsetLeft, y: win.offsetTop }; setPos(p);
    };
    document.addEventListener('mousemove', mv);
    document.addEventListener('mouseup', up);
  });
  win.querySelector('.fl-close').addEventListener('click', () => closeFloater(id));
  const f = { win, body: win.querySelector('.floater-body') };
  floaters[id] = f;
  return f;
}
function closeFloater(id) {
  if (floaters[id]) { floaters[id].win.remove(); delete floaters[id]; }
  const s = getPopped(); s.delete(id); setPopped(s);
}
function togglePop(id) {
  const s = getPopped();
  if (s.has(id)) { closeFloater(id); } else { s.add(id); setPopped(s); ensureFloater(id); }
}
if (typeof document !== 'undefined' && document.addEventListener) {
  document.addEventListener('click', (e) => {
    const b = e.target.closest && e.target.closest('.dev-pop');
    if (!b) return;
    togglePop(b.dataset.dev);
  });
}

function seg7svg(mask) {
  const on = (b) => (mask >> b) & 1;
  const seg = (x1, y1, x2, y2, b) =>
    `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" class="sg ${on(b) ? 'on' : 'off'}" />`;
  return (
    `<svg width="34" height="58" viewBox="0 0 34 58">` +
    seg(8, 4, 26, 4, 0) +    // a
    seg(28, 6, 28, 27, 1) +  // b
    seg(28, 31, 28, 52, 2) + // c
    seg(8, 54, 26, 54, 3) +  // d
    seg(6, 31, 6, 52, 4) +   // e
    seg(6, 6, 6, 27, 5) +    // f
    seg(8, 29, 26, 29, 6) +  // g
    `</svg>`
  );
}

function ledMatrixHtml(rd) {
  let cells = '';
  for (let r = 0; r < 8; r++) {
    const row = rd(0x20 + r);
    for (let b = 7; b >= 0; b--) {
      const lit = (row >> b) & 1;
      cells += `<span class="px ${lit ? 'on' : ''}"></span>`;
    }
  }
  return `<div class="matrix">${cells}</div>`;
}

function robotHtml(rx, ry) {
  let cells = '';
  for (let y = 0; y < 16; y++) {
    for (let x = 0; x < 16; x++) {
      const onTrail = robotTrail.some(([tx, ty]) => tx === x && ty === y);
      const isRobot = x === rx && y === ry;
      const cls = isRobot ? 'rob on' : onTrail ? 'rob tr' : 'rob';
      cells += `<span class="${cls}"></span>`;
    }
  }
  return `<div class="robot">${cells}</div>`;
}

export function renderDevices(emu, isa) {
  const panel = document.getElementById('devices');
  if (!panel) return;
  const box = document.getElementById('devicePanel');
  if (isa !== '8086' && isa !== '8085' && isa !== '8051') {
    box.style.display = '';
    panel.innerHTML =
      '<div class="hint">Peripheral devices (traffic light, 7-segment, stepper, ' +
      'printer, robot, LED matrix) are available for the 8086, 8085, and 8051. ' +
      'Choose one of those ISAs to drive them via OUT/IN ports.</div>';
    return;
  }
  box.style.display = '';

  const rd = (p) => emu.port_read(p);

  // traffic light
  const tr = rd(0x10) & 0x07;
  const light = (bit, color) =>
    `<span class="lamp ${color} ${tr & bit ? 'on' : ''}"></span>`;
  const traffic = `<div class="traffic">${light(1, 'red')}${light(2, 'yel')}${light(4, 'grn')}</div>`;

  // 7-segment
  const d0 = rd(0x11) & 0x0f, d1 = rd(0x12) & 0x0f;
  const seven = `<div class="segs">${seg7svg(SEG7[d0])}${seg7svg(SEG7[d1])}</div>`;

  // stepper
  const sp = rd(0x13) & 0x0f;
  if (sp in STEP_POS) lastStep = STEP_POS[sp];
  const deg = lastStep * 45;
  const stepper = `<div class="stepper"><svg width="56" height="56" viewBox="0 0 56 56">
    <circle cx="28" cy="28" r="24" class="stp-ring"/>
    <g transform="rotate(${deg} 28 28)"><polygon points="28,10 33,28 23,28" class="stp-rot"/></g>
    </svg><div class="lbl">pos ${lastStep} (pat ${sp.toString(16)})</div></div>`;

  // printer (accumulate by diffing the live port)
  const pp = rd(0x14);
  if (pp !== lastPrinter && pp !== 0) { printerBuf += String.fromCharCode(pp); lastPrinter = pp; }
  const printer = `<pre class="printer">${printerBuf.replace(/</g, '&lt;') || '…'}</pre>`;

  // robot (accumulate trail by diffing X/Y)
  const rx = rd(0x16) & 0x0f, ry = rd(0x17) & 0x0f;
  if (!lastRobot || lastRobot[0] !== rx || lastRobot[1] !== ry) {
    robotTrail.push([rx, ry]);
    lastRobot = [rx, ry];
  }
  const robot = robotHtml(rx, ry);

  const led = ledMatrixHtml(rd);

  // cycle-accurate clock / timer view
  const cycles = emu.cycles();
  let timing = `<div class="dev" id="dev-timing"><h3>Clock / timers <span class="p">real-time</span><button class="dev-pop" data-dev="timing" title="Pop out">↗</button></h3><div class="mono">cycles: ${cycles}</div>`;
  if (isa === '8086') {
    timing += `<div class="mono">PIT0: ${emu.pit_count(0)}  PIT1: ${emu.pit_count(1)}  PIT2: ${emu.pit_count(2)}</div>`;
    timing += `<div class="mono">PIT input 1.19318 MHz; CPU 4x -> 1 tick / 4 cycles</div>`;
  } else if (isa === '8085') {
    const tl = rd(0x84), th = rd(0x85);
    const tcount = ((th << 8) | tl) & 0x3FFF;
    const cmd = rd(0x80);
    timing += `<div class="mono">8155 timer: ${tcount}  (cmd ${cmd.toString(16)})</div>`;
    timing += `<div class="mono">8155 PA=${rd(0x81).toString(16)} PB=${rd(0x82).toString(16)} PC=${rd(0x83).toString(16)}</div>`;
    timing += `<div class="mono">8155 RAM @ 0x8000 (T-states clock it)</div>`;
  } else if (isa === '8051') {
    timing += `<div class="mono">machine cycles; timers count 1 / cycle</div>`;
  }
  timing += `</div>`;

  // 8255 PPI (0xE0..0xE3 Mode 0)
  const ppiA = rd(0xE0), ppiB = rd(0xE1), ppiC = rd(0xE2), ppiCtrl = rd(0xE3);
  const ppiHtml = `<div class="mono">PA=${ppiA.toString(16).padStart(2,'0')} PB=${ppiB.toString(16).padStart(2,'0')} PC=${ppiC.toString(16).padStart(2,'0')} ctrl=${ppiCtrl.toString(16).padStart(2,'0')}</div>`+
                  `<div class="mono">Mode 0: OUT PA/PB/PC per ctrl D4/D1/D3/D0</div>`;
  const ppiDev = `<div class="dev" id="dev-ppi"><h3>8255 PPI <span class="p">E0h-E3h</span><button class="dev-pop" data-dev="ppi" title="Pop out">↗</button></h3>${ppiHtml}</div>`;

  // External Flash/EEPROM (64 KiB @ E0000h / A000h)
  let flashHtml = '';
  try {
    const fr = emu.flash_region ? emu.flash_region() : null;
    if (fr) flashHtml = `<div class="mono">Flash @ ${fr[0].toString(16).toUpperCase()}h len ${fr[1].toString(16).toUpperCase()}h</div>`;
    else flashHtml = `<div class="mono">Flash 32K @ E0000h (8086) / 8K @ A000h (8085) — erased FFh</div>`;
  } catch { flashHtml = `<div class="mono">Flash status ${rd(0xE8).toString(16)} (E8h WIP/WEL, E9h cmd 06/04/20/60)</div>`; }
  flashHtml += `<div class="mono">Status E8h=${rd(0xE8).toString(16).padStart(2,'0')} (b0 WIP b1 WEL)</div>`;
  const flashDev = `<div class="dev" id="dev-flash"><h3>Flash/EEPROM <span class="p">E8h-E9h</span><button class="dev-pop" data-dev="flash" title="Pop out">↗</button></h3>${flashHtml}</div>`;

  // RTC/CMOS (0x70/0x71 BCD, I2C shim 30/31)
  const rtcSel = rd(0x70);
  const rtcRegs = [];
  try {
    for(let r=0;r<7;r++){ let v; try{ v=emu.rtc_reg?emu.rtc_reg(r): rd(0x71); }catch{ v=0;} rtcRegs.push(v.toString(16).padStart(2,'0')); }
  } catch { rtcRegs.push('..'); }
  const rtcHtml = `<div class="mono">CMOS sel 70h=${rtcSel.toString(16).padStart(2,'0')} data 71h=${rd(0x71).toString(16).padStart(2,'0')}</div>`+
                  `<div class="mono">RTC BCD 00-06h: ${rtcRegs.join(' ')} (sec min hr wday date mon year)</div>`+
                  `<div class="mono">I2C shim 30h SDA/SCL 31h data</div>`;
  const rtcDev = `<div class="dev" id="dev-rtc"><h3>RTC DS1307 <span class="p">70h/71h</span><button class="dev-pop" data-dev="rtc" title="Pop out">↗</button></h3>${rtcHtml}</div>`;

  // ADC0808 (0x28/0x29) 8 channels
  const adcStat = rd(0x28), adcData = rd(0x29);
  let adcCh = '';
  try{
    const vals=[]; for(let i=0;i<8;i++){ let v; try{ v=emu.adc_get?emu.adc_get(i):0; }catch{ v=0;} vals.push(`CH${i}:${v.toString(16).padStart(2,'0')}`); }
    adcCh = vals.join(' ');
  }catch{ adcCh='CH0..7'; }
  const adcHtml = `<div class="mono">Status 28h=${adcStat.toString(16).padStart(2,'0')} (b7 EOC b6 OE) Data 29h=${adcData.toString(16).padStart(2,'0')}</div>`+
                  `<div class="mono">${adcCh}</div>`+
                  `<div class="mono">OUT 28h ch|(0x80 START) selects CH0-7</div>`;
  const adcDev = `<div class="dev" id="dev-adc"><h3>ADC0808 <span class="p">28h/29h</span><button class="dev-pop" data-dev="adc" title="Pop out">↗</button></h3>${adcHtml}</div>`;

  // HD44780 LCD 16x2 (0x38/0x39)
  let lcdLines = ['',''];
  try{ const t=emu.lcd_text?emu.lcd_text():null; if(t){ lcdLines=t; } }catch{}
  const lcdStat = rd(0x38);
  const lcdHtml = `<div class="mono">Status 38h=${lcdStat.toString(16).padStart(2,'0')} (b7 busy + addr)</div>`+
                  `<pre class="printer" style="background:#002200;color:#8f8;">${(lcdLines[0]||'                ').replace(/</g,'&lt;')}\n${(lcdLines[1]||'                ').replace(/</g,'&lt;')}</pre>`+
                  `<div class="mono">OUT 38h cmd (01 clear 80 addr) / 39h data</div>`;
  const lcdDev = `<div class="dev" id="dev-lcd"><h3>LCD 1602 <span class="p">38h/39h</span><button class="dev-pop" data-dev="lcd" title="Pop out">↗</button></h3>${lcdHtml}</div>`;

  // 8237 DMA (D0h-DFh)
  const dmaStat = rd(0xD0+8);
  const dmaHtml = `<div class="mono">Status D8h=${dmaStat.toString(16).padStart(2,'0')} (TC3..0 + REQ)</div>`+
                  `<div class="mono">CH0  D0h-03h CH1 D4h-07h CH2 D8h-0Bh CH3 DCh-0Fh</div>`+
                  `<div class="mono">Req D0h 0Dh  Mask 0Eh Mode 0Fh</div>`;
  const dmaDev = `<div class="dev" id="dev-dma"><h3>8237 DMA <span class="p">D0h-DFh</span><button class="dev-pop" data-dev="dma" title="Pop out">↗</button></h3>${dmaHtml}</div>`;

  panel.innerHTML =
    `<div class="dev" id="dev-traffic"><h3>Traffic light <span class="p">10h</span><button class="dev-pop" data-dev="traffic" title="Pop out">↗</button></h3>${traffic}</div>` +
    `<div class="dev" id="dev-seven"><h3>7-segment <span class="p">11h/12h</span><button class="dev-pop" data-dev="seven" title="Pop out">↗</button></h3>${seven}</div>` +
    `<div class="dev" id="dev-stepper"><h3>Stepper <span class="p">13h</span><button class="dev-pop" data-dev="stepper" title="Pop out">↗</button></h3>${stepper}</div>` +
    `<div class="dev" id="dev-printer"><h3>Printer <span class="p">14h</span><button class="dev-pop" data-dev="printer" title="Pop out">↗</button></h3>${printer}</div>` +
    `<div class="dev" id="dev-robot"><h3>Robot grid <span class="p">16h/17h</span><button class="dev-pop" data-dev="robot" title="Pop out">↗</button></h3>${robot}</div>` +
    `<div class="dev" id="dev-led"><h3>LED matrix <span class="p">20h-27h</span><button class="dev-pop" data-dev="led" title="Pop out">↗</button></h3>${led}</div>` +
    ppiDev + flashDev + rtcDev + adcDev + lcdDev + dmaDev +
    timing;

  // Keep popped-out floaters live and hide the in-panel copy while popped.
  const popped = getPopped();
  for (const id of popped) {
    const src = panel.querySelector('#dev-' + id);
    if (!src) continue;
    src.style.display = 'none';
    ensureFloater(id).body.innerHTML = src.innerHTML;
  }
}

// Static memory-map overview per ISA for the "Memory map" panel.
export function renderMemMap(emu, isa) {
   const el = document.getElementById('memmap');
   if (!el) return;
   const fmt = (a) => a.toString(16).toUpperCase().padStart(4, '0');
   const rom = emu.rom_region();
   const sram = emu.sram_region();
   const ext = emu.ext_code_region();
   let rows;
    if (isa === '8086') {
      rows = [
        '00000–9FFFF  RAM (640 KiB)',
        'A0000–BFFFF  VGA / video',
        'C0000–EFFFF  ROM / expansion',
      ];
      rows.push(rom
        ? `${fmt(rom[0])}–${fmt(rom[0] + rom[1] - 1)}  ROM (BIOS) — LOADED`
        : 'F0000–FFFFF  ROM (BIOS) — load via “Load ROM”');
      try {
        const fr = emu.flash_region ? emu.flash_region() : null;
        if (fr) rows.push(`${fmt(fr[0])}–${fmt(fr[0]+fr[1]-1)}  Flash/EEPROM — LOADED`);
        else rows.push('E0000–EFFFF  Flash/EEPROM 64 KiB (E8h/E9h)');
      } catch { rows.push('E0000–EFFFF  Flash/EEPROM 64 KiB'); }
      rows.push('0000–FFFF    I/O ports: E0 PPI, 28 ADC, 38 LCD, 70 RTC, D0 DMA');
    } else if (isa === '8085') {
      rows = [
        '0000–7FFF   main RAM',
        '8000–80FF   8155 external RAM',
        '8000–80FF   8155 I/O (ports 80–85)',
      ];
      rows.push(sram
        ? `${fmt(sram[0])}–${fmt(sram[0] + sram[1] - 1)}  external SRAM — LOADED`
        : '9000–9FFF   external SRAM (8 KiB)');
      try {
        const fr = emu.flash_region ? emu.flash_region() : null;
        if (fr) rows.push(`${fmt(fr[0])}–${fmt(fr[0]+fr[1]-1)}  Flash 8-32 KiB`);
        else rows.push('A000–BFFF   Flash/EEPROM 8 KiB (A000h)');
      } catch {}
      rows.push('00–FF       I/O ports: E0 PPI, 28 ADC, 38 LCD, 70 RTC, D0 DMA');
   } else {
     rows = [];
     rows.push(emu.ea_active()
       ? '0000–FFFF   code ROM (internal, EA=1)'
       : '0000–FFFF   code ROM (internal) — EA LOW');
     rows.push(ext
       ? `${fmt(ext[0])}–${fmt(ext[0] + ext[1] - 1)}  XDATA = external ROM (EA=0) + RAM`
       : '0000–FFFF   XDATA = external ROM (EA=0) + RAM');
     rows.push('FF00–FFFF   XDATA top → I/O ports');
     rows.push('00–7F       internal RAM');
     rows.push('80–FF       SFRs');
   }
   el.innerHTML = rows.map((r) => `<div>${r}</div>`).join('');
 }

 // 8051 special-function-register readout (click a cell to edit it live).
 export function renderPeripherals(emu, isa) {
   const el = document.getElementById('peripherals');
   if (!el) return;
   if (isa !== '8051') {
     el.innerHTML = '<div class="hint">Peripheral (SFR) readouts are shown for the 8051. Use the Ports panel for 8086/8085 I/O.</div>';
     return;
   }
   const sfrs = [
     ['P0', 0x80], ['P1', 0x90], ['P2', 0xA0], ['P3', 0xB0],
     ['SP', 0x81], ['DPL', 0x82], ['DPH', 0x83], ['PCON', 0x87],
     ['TCON', 0x88], ['TMOD', 0x89], ['TL0', 0x8A], ['TH0', 0x8C],
     ['TL1', 0x8B], ['TH1', 0x8D], ['SCON', 0x98], ['SBUF', 0x99],
     ['IE', 0xA8], ['IP', 0xB8], ['PSW', 0xD0], ['ACC', 0xE0], ['B', 0xF0],
   ];
   el.innerHTML = sfrs
     .map(([n, a]) => `<span class="sfr" data-sfr="${a}" title="${n} (${a.toString(16).toUpperCase()}h) — click to edit">${n} ${emu.sfr(a).toString(16).toUpperCase().padStart(2, '0')}</span>`)
     .join('');
 }
