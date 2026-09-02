//! DS1307-like RTC + PC CMOS compatible interface.
//!
//! The RTC keeps BCD registers for seconds/minutes/hours/day/date/month/year
//! and is driven by the DOS clock in the host. Ports:
//!   0x70  register select (CMOS index, 0x00..0x3F)
//!   0x71  data (read/write selected register)
//! Additional I2C-style ports for 8051/8085 bit-banging compatibility:
//!   0x30  I2C SDA+SCL bits (bit0 SDA, bit1 SCL) — write starts a fake transaction
//!   0x31  I2C data register (mirrors selected RTC register for simple polling)
//!
//! Registers 0x00..0x06 hold time (BCD, DS1307 layout):
//!   0x00 seconds (CH bit in D7), 0x01 minutes, 0x02 hours, 0x03 weekday,
//!   0x04 date, 0x05 month, 0x06 year (00..99 => 2000..2099). 0x32 century.

#[derive(Clone)]
pub struct Rtc {
    reg_sel: u8,
    // raw registers 0x00..0x3F in BCD where applicable
    regs: [u8; 64],
}

impl Default for Rtc {
    fn default() -> Self {
        let mut r = Rtc { reg_sel: 0, regs: [0;64] };
        // initialize to 2026-01-01 00:00:00 (BCD)
        r.regs[0x00] = 0x00; // sec
        r.regs[0x01] = 0x00; // min
        r.regs[0x02] = 0x00; // hour
        r.regs[0x03] = 0x05; // weekday (Thu)
        r.regs[0x04] = 0x01; // date
        r.regs[0x05] = 0x01; // month
        r.regs[0x06] = 0x26; // year 26
        r.regs[0x32] = 0x20; // century 20
        r.regs[0x0A] = 0x26; // status A
        r.regs[0x0B] = 0x02; // status B
        r
    }
}

fn to_bcd(v:u8)->u8 { ((v/10)<<4)|(v%10) }
fn from_bcd(v:u8)->u8 { (v>>4)*10 + (v&0x0F) }

impl Rtc {
    pub fn new()->Self { Self::default() }

    pub fn set_time(&mut self, year:u16, mon:u8, day:u8, hour:u8, min:u8, sec:u8) {
        self.regs[0x00] = to_bcd(sec & 0x7F);
        self.regs[0x01] = to_bcd(min);
        self.regs[0x02] = to_bcd(hour);
        // weekday not recomputed; keep
        self.regs[0x04] = to_bcd(day);
        self.regs[0x05] = to_bcd(mon);
        self.regs[0x06] = to_bcd((year%100) as u8);
        self.regs[0x32] = to_bcd((year/100) as u8);
        // CMOS mirrors
        self.regs[0x00] = to_bcd(sec);
        self.regs[0x02] = to_bcd(hour);
        self.regs[0x04] = to_bcd(day);
    }

    pub fn tick_from_host(&mut self, year:u16, mon:u8, day:u8, hour:u8, min:u8, sec:u8) {
        self.set_time(year,mon,day,hour,min,sec);
    }

    pub fn write_sel(&mut self, v: u8) { self.reg_sel = v & 0x3F; }
    pub fn read_sel(&self)->u8 { self.reg_sel }
    pub fn write_data(&mut self, v: u8) {
        let idx = self.reg_sel as usize;
        if idx < 64 {
            // protect read-only status bits? Allow all for teaching
            self.regs[idx]=v;
        }
    }
    pub fn read_data(&self)->u8 {
        let idx = self.reg_sel as usize;
        if idx < 64 { self.regs[idx] } else { 0xFF }
    }

    // I2C shim ports
    pub fn i2c_write(&mut self, _v:u8) { /* bit-bang ignored; keep BCD in sync */ }
    pub fn i2c_read(&self)->u8 { self.read_data() }

    pub fn snapshot(&self)->Vec<u8> {
        let mut v = Vec::new();
        v.push(self.reg_sel);
        v.extend_from_slice(&self.regs);
        v
    }
    pub fn restore(&mut self, d: &[u8]) {
        if d.len() < 65 { return; }
        self.reg_sel=d[0];
        self.regs.copy_from_slice(&d[1..65]);
    }
}
