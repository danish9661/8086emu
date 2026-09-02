//! Intel 8251 USART — simplified model for 8086/8085 kits.
//!
//! Ports:
//!   0x50  data (OUT: Tx, IN: Rx)
//!   0x51  status/cmd (OUT: cmd, IN: status b0 TxRDY=1, b1 RxRDY, b2 TxEMPTY)
//! Tx goes to Output buffer (like 8086 INT21), Rx comes from host queue (push via `serial_rx`).

use std::collections::VecDeque;

#[derive(Clone)]
pub struct Usart8251 {
    tx: VecDeque<u8>,
    rx: VecDeque<u8>,
    status: u8,
    cmd: u8,
    mode: u8,
}

impl Default for Usart8251 {
    fn default()->Self { Usart8251 { tx: VecDeque::new(), rx: VecDeque::new(), status: 0x01, cmd:0, mode:0 } }
}

impl Usart8251 {
    pub fn new()->Self{ Self::default() }
    pub fn write_data(&mut self, v:u8){
        self.tx.push_back(v);
        self.status |= 0x04; // TxEMPTY
    }
    pub fn read_data(&mut self)->u8{
        self.rx.pop_front().unwrap_or(0)
    }
    pub fn write_ctrl(&mut self, v:u8){
        // first write after reset is mode, subsequent are cmd
        if self.mode==0 && v & 0xC0 !=0 {
            self.mode=v;
        } else {
            self.cmd=v;
            if v & 0x40 !=0 { self.rx.clear(); } // internal reset
        }
    }
    pub fn read_status(&self)->u8{
        let mut s=0x01; // TxRDY always
        if !self.rx.is_empty() { s|=0x02; }
        if self.tx.is_empty() { s|=0x04; }
        s
    }
    pub fn push_rx(&mut self, v:u8){ self.rx.push_back(v); }
    pub fn take_tx(&mut self)->Option<u8>{ self.tx.pop_front() }
    pub fn snapshot(&self)->Vec<u8>{
        let mut v=Vec::new();
        v.push(self.status); v.push(self.cmd); v.push(self.mode);
        v.push(self.tx.len() as u8); v.extend(self.tx.iter());
        v.push(self.rx.len() as u8); v.extend(self.rx.iter());
        v
    }
    pub fn restore(&mut self, d:&[u8]){
        if d.len()<3 { return; }
        self.status=d[0]; self.cmd=d[1]; self.mode=d[2];
        let mut off=3;
        if off < d.len() { let n=d[off] as usize; off+=1; self.tx.clear(); for &b in &d[off..off.min(off+n)]{self.tx.push_back(b);} off+=n; }
        if off < d.len() { let n=d[off] as usize; off+=1; self.rx.clear(); for &b in &d[off..off.min(off+n)]{self.rx.push_back(b);} }
    }
}
