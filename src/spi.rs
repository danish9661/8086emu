//! SPI bus + W25Q-like flash (64 KiB for teaching, simplified).
//!
//! SPI via 8051 P1.5=MOSI, P1.6=MISO, P1.7=SCK, P1.4=SS (active low).
//! Also port-mapped helper at 0x62/0x63 (XDATA 0xFF62/0xFF63 -> ports 0x62/0x63):
//!   0x62  SPI cmd/address latch (OUT: addr 0x00..0xFF for demo window)
//!   0x63  SPI data (OUT: write, IN: read). Commands 0x03 read, 0x02 write, 0x20 erase 4K.

#[derive(Clone)]
pub struct SpiFlash {
    data: Vec<u8>,
    addr_latch: u8,
    cmd: u8,
    /// Shift register for bit-bang
    shift: u8,
    bits: u8,
    sck: bool,
    mosi: bool,
    ss: bool,
}

impl Default for SpiFlash {
    fn default()->Self {
        SpiFlash { data: vec![0xFF; 65536], addr_latch:0, cmd:0x03, shift:0, bits:0, sck:false, mosi:false, ss:true }
    }
}

impl SpiFlash {
    pub fn new()->Self { Self::default() }
    pub fn write_addr(&mut self, a:u8){ self.addr_latch=a; }
    pub fn write_data(&mut self, v:u8){
        match self.cmd {
            0x02 => { // page program
                let idx = self.addr_latch as usize;
                if idx < self.data.len() { self.data[idx]=v; }
                self.addr_latch = self.addr_latch.wrapping_add(1);
            }
            0x20 => { // sector erase 4K
                let base = (self.addr_latch as usize) & !0xFFF;
                for i in 0..4096 { if base+i < self.data.len() { self.data[base+i]=0xFF; } }
            }
            _ => {}
        }
    }
    pub fn read_data(&mut self)->u8{
        let v = self.data[self.addr_latch as usize];
        self.addr_latch = self.addr_latch.wrapping_add(1);
        v
    }
    pub fn write_cmd(&mut self, c:u8){ self.cmd=c; }
    pub fn read_cmd(&self)->u8{ self.cmd }
    pub fn load(&mut self, d:&[u8], off:u8){
        for (i,b) in d.iter().enumerate(){
            let idx = (off as usize + i) % self.data.len();
            self.data[idx]=*b;
        }
    }
    pub fn dump(&self)->Vec<u8>{ self.data[0..256].to_vec() }

    /// Bit-bang via P1: called on P1 writes, samples MOSI on SCK rising edge when SS low.
    pub fn p1_write(&mut self, old_p1:u8, new_p1:u8){
        let old_sck = old_p1 & 0x80 !=0;
        let new_sck = new_p1 & 0x80 !=0;
        let old_ss = old_p1 & 0x10 !=0;
        let new_ss = new_p1 & 0x10 !=0;
        let new_mosi = new_p1 & 0x20 !=0;
        // SS falling -> reset
        if old_ss && !new_ss { self.bits=0; self.shift=0; }
        if !new_ss && !old_sck && new_sck {
            self.shift = (self.shift<<1) | (new_mosi as u8);
            self.bits+=1;
            if self.bits==8 {
                // treat as data
                self.data[self.addr_latch as usize]=self.shift;
                self.addr_latch=self.addr_latch.wrapping_add(1);
                self.bits=0; self.shift=0;
            }
        }
        self.sck=new_sck; self.mosi=new_mosi; self.ss=new_ss;
    }

    pub fn snapshot(&self)->Vec<u8>{
        let mut v=Vec::new();
        v.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        v.extend_from_slice(&self.data);
        v.push(self.addr_latch); v.push(self.cmd);
        v.push(self.shift); v.push(self.bits);
        v.push(self.sck as u8); v.push(self.mosi as u8); v.push(self.ss as u8);
        v
    }
    pub fn restore(&mut self, d:&[u8]){
        if d.len()<4 { return; }
        let len = u32::from_le_bytes([d[0],d[1],d[2],d[3]]) as usize;
        if d.len() < 4+len+7 { return; }
        self.data = d[4..4+len].to_vec();
        self.addr_latch=d[4+len];
        self.cmd=d[4+len+1];
        self.shift=d[4+len+2]; self.bits=d[4+len+3];
        self.sck=d[4+len+4]!=0; self.mosi=d[4+len+5]!=0; self.ss=d[4+len+6]!=0;
    }
}
