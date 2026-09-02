//! Intel 8279 keyboard/display controller — 8-digit 7-seg + 8x8 keys.
//!
//! Ports:
//!   0x68  8279 cmd (OUT: 0x00 clear, 0x80.. display write)
//!   0x69  8279 data (OUT: segment byte to display RAM, IN: key FIFO + sensor)
//! Display RAM holds 8 bytes (one per digit), shown as hex on 7-seg.
//! Key FIFO holds last 8 keycodes injected via `push_key`.

#[derive(Clone)]
pub struct KbDisplay {
    pub disp: [u8; 8],
    pub keys: Vec<u8>,
    pub sensor: u8,
    ctrl: u8,
    addr: u8,
}

impl Default for KbDisplay {
    fn default()->Self{ KbDisplay { disp:[0;8], keys:Vec::new(), sensor:0, ctrl:0, addr:0 } }
}

impl KbDisplay {
    pub fn new()->Self{ Self::default() }
    pub fn write_cmd(&mut self, v:u8){
        self.ctrl=v;
        if v==0 { self.disp=[0;8]; }
    }
    pub fn write_data(&mut self, v:u8){
        self.disp[self.addr as usize %8]=v;
        self.addr = (self.addr+1)&7;
    }
    pub fn read_data(&mut self)->u8{
        if !self.keys.is_empty() { self.keys.remove(0) } else { 0xFF }
    }
    pub fn read_status(&self)->u8{
        // D7 = IRQ due to key, D3..0 = FIFO count
        let mut s = (self.keys.len() as u8) & 0x07;
        if !self.keys.is_empty() { s|=0x80; }
        s
    }
    pub fn push_key(&mut self, k:u8){ if self.keys.len()<8 { self.keys.push(k); } }
    pub fn snapshot(&self)->Vec<u8>{
        let mut v=Vec::new();
        v.extend_from_slice(&self.disp);
        v.push(self.keys.len() as u8); v.extend(&self.keys);
        v.push(self.sensor); v.push(self.ctrl); v.push(self.addr);
        v
    }
    pub fn restore(&mut self, d:&[u8]){
        if d.len()<8 { return; }
        self.disp.copy_from_slice(&d[0..8]);
        let mut off=8;
        if off < d.len() { let n=d[off] as usize; off+=1; self.keys.clear(); for &b in &d[off..off.min(off+n)]{self.keys.push(b);} off+=n; }
        if off+2 < d.len() { self.sensor=d[off]; self.ctrl=d[off+1]; self.addr=d[off+2]; }
    }
}
