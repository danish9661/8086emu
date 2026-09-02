//! HD44780 16×2 character LCD (plus 20×4 capable).
//!
//! Ports (both 8086 and 8085):
//!   0x38  command (OUT: RS=0) / status (IN: D7=busy, address counter in D6-D0)
//!   0x39  data (OUT: RS=1 char, IN: read DDRAM at cursor)
//! Commands subset: 0x01 clear, 0x02 home, 0x06 entry mode, 0x0C display on,
//! 0x80+addr set DDRAM address, 0x40+addr set CGRAM (ignored).

#[derive(Clone)]
pub struct Lcd1602 {
    /// 80 bytes DDRAM (HD44780 has 80): 0x00..0x27 line0, 0x40..0x67 line1
    ddram: [u8; 80],
    addr: u8, // 7-bit DDRAM address
    display_on: bool,
    cursor_on: bool,
    blink_on: bool,
    entry_inc: bool,
    busy: u8, // countdown
}

impl Default for Lcd1602 {
    fn default()->Self {
        let mut l = Lcd1602 { ddram:[b' ';80], addr:0, display_on:true, cursor_on:false, blink_on:false, entry_inc:true, busy:0 };
        l.clear();
        l
    }
}

impl Lcd1602 {
    pub fn new()->Self{ Self::default() }
    fn clear(&mut self){ for b in &mut self.ddram{ *b=b' '; } self.addr=0; }
    fn addr_to_idx(addr:u8)->Option<usize>{
        if addr < 0x40 { if addr < 40 { Some(addr as usize) } else { None } }
        else { let a=(addr-0x40) as usize; if a<40 { Some(40 + a) } else { None } }
    }
    pub fn write_cmd(&mut self, v:u8){
        self.busy=1;
        match v {
            0x01 => self.clear(),
            0x02 => self.addr=0,
            v if v & 0xFC == 0x04 => { // entry mode
                self.entry_inc = v & 0x02 !=0;
            }
            v if v & 0xF8 == 0x08 => {
                self.display_on = v & 0x04 !=0;
                self.cursor_on = v & 0x02 !=0;
                self.blink_on = v & 0x01 !=0;
            }
            v if v & 0x80 !=0 => {
                self.addr = v & 0x7F;
            }
            _ => {}
        }
    }
    pub fn write_data(&mut self, v:u8){
        self.busy=1;
        if let Some(i)=Self::addr_to_idx(self.addr){
            self.ddram[i]=v;
        }
        if self.entry_inc { self.addr = self.addr.wrapping_add(1) & 0x7F; }
        else { self.addr = self.addr.wrapping_sub(1) & 0x7F; }
    }
    pub fn read_status(&self)->u8 {
        // D7 busy, D6-D0 addr
        if self.busy!=0 { 0x80 | (self.addr & 0x7F) } else { self.addr & 0x7F }
    }
    pub fn read_data(&mut self)->u8 {
        let v = if let Some(i)=Self::addr_to_idx(self.addr){ self.ddram[i] } else { 0x20 };
        if self.entry_inc { self.addr = self.addr.wrapping_add(1) & 0x7F; }
        else { self.addr = self.addr.wrapping_sub(1) & 0x7F; }
        v
    }
    pub fn tick(&mut self){ if self.busy>0 { self.busy-=1; } }

    /// Return two 16-char lines (or 20-char if content beyond 16)
    pub fn lines(&self)->[String;2]{
        let l0: String = self.ddram[0..16].iter().map(|&b| b as char).collect();
        let l1: String = self.ddram[40..56].iter().map(|&b| b as char).collect();
        [l0,l1]
    }
    /// Raw DDRAM for snap
    pub fn ddram(&self)->Vec<u8>{ self.ddram.to_vec() }

    pub fn snapshot(&self)->Vec<u8>{
        let mut v=Vec::new();
        v.extend_from_slice(&self.ddram);
        v.push(self.addr); v.push(self.display_on as u8); v.push(self.cursor_on as u8);
        v.push(self.blink_on as u8); v.push(self.entry_inc as u8); v.push(self.busy);
        v
    }
    pub fn restore(&mut self, d:&[u8]){
        if d.len()<86 { return; }
        self.ddram.copy_from_slice(&d[0..80]);
        self.addr=d[80]; self.display_on=d[81]!=0; self.cursor_on=d[82]!=0;
        self.blink_on=d[83]!=0; self.entry_inc=d[84]!=0; self.busy=d[85];
    }
}
