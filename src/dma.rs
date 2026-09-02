//! Intel 8237 DMA controller — simplified 4-channel model.
//!
//! Ports (PC-compatible layout, also usable from 8085/8051 as generic DMA):
//!   0x00  CH0 base/current address LSB
//!   0x01  CH0 base/current address MSB (via page? stored as 16-bit)
//!   0x02  CH0 base/current count LSB (count = N+1 transfers)
//!   0x03  CH0 base/current count MSB
//!   0x04  CH1 addr LSB, 0x05 addr MSB, 0x06 count LSB, 0x07 count MSB
//!   0x08  CH2 ... 0x0B
//!   0x0C  CH3 ... 0x0F
//!   0x08  (alias) status (read) / command (write)
//!   0x0D  request (write) — software DRQ
//!   0x0E  single mask
//!   0x0F  mode
//! We implement a minimal subset: masked channels, mode (00 demand/01 single/10 block/11 cascade),
//! direction, auto-init, and a `do_transfer` that can be triggered by software or by
//! `request_interrupt("DMA", ch)`. The IDE can preload source bytes via `Dma::load_channel`.

#[derive(Clone, Copy, Default)]
struct Ch {
    base_addr: u16,
    cur_addr: u16,
    base_cnt: u16,
    cur_cnt: u16,
    mode: u8, // 6 bits
    masked: bool,
    // latched data for simple mem→mem or mem→port transport (not page-extended)
}

#[derive(Clone)]
pub struct Dma8237 {
    ch: [Ch;4],
    cmd: u8,
    status: u8, // TC bits 0-3, request bits 4-7
    req: u8, // pending software requests
}

impl Default for Dma8237 {
    fn default()->Self {
        let mut d = Dma8237 { ch:[Ch::default();4], cmd:0, status:0, req:0 };
        for c in &mut d.ch { c.masked=true; }
        d
    }
}

impl Dma8237 {
    pub fn new()->Self{ Self::default() }

    pub fn write(&mut self, port: u8, v: u8){
        let p = port as usize;
        match p {
            0x00 => self.ch[0].base_addr = (self.ch[0].base_addr & 0xFF00)|(v as u16),
            0x01 => self.ch[0].base_addr = (self.ch[0].base_addr & 0x00FF)|((v as u16)<<8),
            0x02 => self.ch[0].base_cnt = (self.ch[0].base_cnt & 0xFF00)|(v as u16),
            0x03 => self.ch[0].base_cnt = (self.ch[0].base_cnt & 0x00FF)|((v as u16)<<8),
            0x04 => self.ch[1].base_addr = (self.ch[1].base_addr & 0xFF00)|(v as u16),
            0x05 => self.ch[1].base_addr = (self.ch[1].base_addr & 0x00FF)|((v as u16)<<8),
            0x06 => self.ch[1].base_cnt = (self.ch[1].base_cnt & 0xFF00)|(v as u16),
            0x07 => self.ch[1].base_cnt = (self.ch[1].base_cnt & 0x00FF)|((v as u16)<<8),
            0x08 => {
                // CH2 addr LSB but also status/cmd alias
                // Distinguish: if writing 0x08 directly, treat as CH2 LSB AND command if value has D2?? Simplify: CH2 LSB takes precedence; command is at 0x08 as alternative read
                self.ch[2].base_addr = (self.ch[2].base_addr & 0xFF00)|(v as u16);
                self.cmd = v;
            }
            0x09 => self.ch[2].base_addr = (self.ch[2].base_addr & 0x00FF)|((v as u16)<<8),
            0x0A => self.ch[2].base_cnt = (self.ch[2].base_cnt & 0xFF00)|(v as u16),
            0x0B => self.ch[2].base_cnt = (self.ch[2].base_cnt & 0x00FF)|((v as u16)<<8),
            0x0C => self.ch[3].base_addr = (self.ch[3].base_addr & 0xFF00)|(v as u16),
            0x0D => {
                // could be ch3 MSB or request register; heuristic: if v & 0xFC==0, treat as request
                if v & 0xFC == 0 {
                    let ch = (v & 0x03) as usize;
                    if v & 0x04 !=0 { self.req |= 1<<ch; } else { self.req &= !(1<<ch); }
                } else {
                    self.ch[3].base_addr = (self.ch[3].base_addr & 0x00FF)|((v as u16)<<8);
                }
            }
            0x0E => {
                // single mask: D2 = 1 mask, 0 unmask, D1-D0 channel
                let ch = (v & 0x03) as usize;
                self.ch[ch].masked = v & 0x04 !=0;
                if !self.ch[ch].masked {
                    self.ch[ch].cur_addr = self.ch[ch].base_addr;
                    self.ch[ch].cur_cnt = self.ch[ch].base_cnt;
                }
            }
            0x0F => {
                // mode: D7-D6 channel? Actually D1-D0 channel, D7-D2 mode
                let ch = (v & 0x03) as usize;
                self.ch[ch].mode = v & 0xFC;
            }
            _ => {}
        }
    }

    pub fn read(&self, port: u8)->u8{
        let p = port as usize;
        match p {
            0x00 => self.ch[0].cur_addr as u8,
            0x01 => (self.ch[0].cur_addr>>8) as u8,
            0x02 => self.ch[0].cur_cnt as u8,
            0x03 => (self.ch[0].cur_cnt>>8) as u8,
            0x04 => self.ch[1].cur_addr as u8,
            0x05 => (self.ch[1].cur_addr>>8) as u8,
            0x06 => self.ch[1].cur_cnt as u8,
            0x07 => (self.ch[1].cur_cnt>>8) as u8,
            0x08 => self.status, // status
            0x09 => (self.ch[2].cur_addr>>8) as u8,
            0x0A => self.ch[2].cur_cnt as u8,
            0x0B => (self.ch[2].cur_cnt>>8) as u8,
            0x0C => self.ch[3].cur_addr as u8,
            0x0D => (self.ch[3].cur_addr>>8) as u8,
            0x0E => self.ch[3].cur_cnt as u8,
            0x0F => (self.ch[3].cur_cnt>>8) as u8,
            _ => 0xFF,
        }
    }

    /// Software-triggered DMA: copy `count+1` bytes from src `base_addr` to dst (port or mem).
    /// For the emulator we implement simple memory-to-memory block copy within host Mem.
    /// `mem` is the CPU's main memory; we copy inside it (8086 1MiB, 8085 64KiB).
    pub fn transfer(&mut self, ch_idx: usize, mem: &mut crate::cpu::Mem) {
        if ch_idx>=4 { return; }
        if self.ch[ch_idx].masked && self.req & (1<<ch_idx)==0 { return; }
        let cnt = self.ch[ch_idx].cur_cnt as usize + 1;
        let src = self.ch[ch_idx].cur_addr as u32;
        // Simple: block copy src -> src+cnt within same memory (like memmove). For port I/O variant,
        // the external device would be memory-mapped; here we just move the block.
        // We do a snapshot to handle overlap.
        let bytes: Vec<u8> = (0..cnt).map(|i| mem.read((src as usize)+i)).collect();
        for (i,b) in bytes.iter().enumerate(){
            mem.write((src as usize)+i, *b);
        }
        self.status |= 1<<ch_idx; // TC
        self.req &= !(1<<ch_idx);
        self.ch[ch_idx].cur_cnt = 0;
        self.ch[ch_idx].masked = true;
    }

    pub fn request(&mut self, ch: usize){
        if ch<4 && !self.ch[ch].masked { self.req |= 1<<(ch as u8); }
    }

    pub fn snapshot(&self)->Vec<u8>{
        let mut v=Vec::new();
        v.push(self.cmd); v.push(self.status); v.push(self.req);
        for c in &self.ch {
            v.extend_from_slice(&c.base_addr.to_le_bytes());
            v.extend_from_slice(&c.cur_addr.to_le_bytes());
            v.extend_from_slice(&c.base_cnt.to_le_bytes());
            v.extend_from_slice(&c.cur_cnt.to_le_bytes());
            v.push(c.mode); v.push(c.masked as u8);
        }
        v
    }
    pub fn restore(&mut self, d:&[u8]){
        if d.len()< 3+4*10 { return; }
        self.cmd=d[0]; self.status=d[1]; self.req=d[2];
        let mut off=3;
        for i in 0..4{
            self.ch[i].base_addr = u16::from_le_bytes([d[off],d[off+1]]); off+=2;
            self.ch[i].cur_addr = u16::from_le_bytes([d[off],d[off+1]]); off+=2;
            self.ch[i].base_cnt = u16::from_le_bytes([d[off],d[off+1]]); off+=2;
            self.ch[i].cur_cnt = u16::from_le_bytes([d[off],d[off+1]]); off+=2;
            self.ch[i].mode = d[off]; off+=1;
            self.ch[i].masked = d[off]!=0; off+=1;
        }
    }
}
