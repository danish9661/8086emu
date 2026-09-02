//! I2C bus + 24C02-like 256B EEPROM (and DS1307 RTC shadow).
//!
//! The 8051 typically bit-bangs I2C via P1.0=SDA, P1.1=SCL. This model has both
//! a high-level port-mapped helper (XDATA 0xFF60/0xFF61 -> ports 0x60/0x61) and a
//! low-level bit-bang detector that watches P1 writes.
//!
//! Ports (8051 via MOVX @DPTR where DPTR=0xFF60/0xFF61, also 8086/8085 via OUT 0x60/0x61):
//!   0x60  I2C address / control (OUT: eeprom addr 0x00..0xFF, or DS1307 regs)
//!         For RTC shadow, addresses 0xD0..0xD7 map to RTC BCD regs.
//!   0x61  I2C data (OUT: write to that addr, IN: read from that addr)
//! Status is implicit (no busy). The 24C02 is byte-writable instantly.

#[derive(Clone)]
pub struct I2cEeprom {
    eeprom: [u8; 256],
    addr_latch: u8,
    /// Last SDA/SCL seen for edge detection (P1 bit-bang)
    sda: bool,
    scl: bool,
    /// Simple shift register for bit-bang (collects 8 bits then writes)
    shift: u8,
    bits: u8,
}

impl Default for I2cEeprom {
    fn default()->Self {
        I2cEeprom { eeprom: [0xFF;256], addr_latch:0, sda:true, scl:true, shift:0, bits:0 }
    }
}

impl I2cEeprom {
    pub fn new()->Self { Self::default() }

    /// High-level port-mapped access (what IDE and simple lab programs use)
    pub fn write_addr(&mut self, a:u8){ self.addr_latch = a; }
    pub fn write_data(&mut self, v:u8){
        self.eeprom[self.addr_latch as usize]=v;
        // auto-increment like 24C02 page write
        self.addr_latch = self.addr_latch.wrapping_add(1);
    }
    pub fn read_data(&mut self)->u8{
        let v = self.eeprom[self.addr_latch as usize];
        self.addr_latch = self.addr_latch.wrapping_add(1);
        v
    }
    pub fn read_addr(&self)->u8{ self.addr_latch }
    pub fn load(&mut self, data:&[u8], offset:u8){
        for (i,b) in data.iter().enumerate(){
            let idx = offset.wrapping_add(i as u8) as usize;
            if idx<256 { self.eeprom[idx]=*b; }
        }
    }
    pub fn dump(&self)->Vec<u8>{ self.eeprom.to_vec() }

    /// Called when 8051 writes to P1 (port 0x90). P1.0=SDA, P1.1=SCL.
    /// We detect START (SDA falling while SCL high) and collect bytes.
    pub fn p1_write(&mut self, old_p1:u8, new_p1:u8){
        let old_sda = old_p1 & 1 !=0;
        let old_scl = old_p1 & 2 !=0;
        let new_sda = new_p1 & 1 !=0;
        let new_scl = new_p1 & 2 !=0;
        // SCL rising edge => sample SDA
        if !old_scl && new_scl {
            self.shift = (self.shift<<1) | (new_sda as u8);
            self.bits +=1;
            if self.bits==8 {
                // full byte received; first byte after START is addr
                // For teaching we just treat it as data write to latched addr
                self.eeprom[self.addr_latch as usize]=self.shift;
                self.addr_latch = self.addr_latch.wrapping_add(1);
                self.shift=0; self.bits=0;
            }
        }
        // START: SDA falling while SCL high
        if old_sda && !new_sda && new_scl { self.bits=0; self.shift=0; }
        self.sda=new_sda; self.scl=new_scl;
    }

    pub fn snapshot(&self)->Vec<u8>{
        let mut v=Vec::new();
        v.extend_from_slice(&self.eeprom);
        v.push(self.addr_latch);
        v.push(self.sda as u8); v.push(self.scl as u8);
        v.push(self.shift); v.push(self.bits);
        v
    }
    pub fn restore(&mut self, d:&[u8]){
        if d.len()<261 { return; }
        self.eeprom.copy_from_slice(&d[0..256]);
        self.addr_latch=d[256];
        self.sda=d[257]!=0; self.scl=d[258]!=0;
        self.shift=d[259]; self.bits=d[260];
    }
}
