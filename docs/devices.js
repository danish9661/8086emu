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
  if (isa !== '8086' && isa !== '8085') {
    box.style.display = 'none';
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

  panel.innerHTML =
    `<div class="dev"><h3>Traffic light <span class="p">10h</span></h3>${traffic}</div>` +
    `<div class="dev"><h3>7-segment <span class="p">11h/12h</span></h3>${seven}</div>` +
    `<div class="dev"><h3>Stepper <span class="p">13h</span></h3>${stepper}</div>` +
    `<div class="dev"><h3>Printer <span class="p">14h</span></h3>${printer}</div>` +
    `<div class="dev"><h3>Robot grid <span class="p">16h/17h</span></h3>${robot}</div>` +
    `<div class="dev"><h3>LED matrix <span class="p">20h-27h</span></h3>${led}</div>`;
}
