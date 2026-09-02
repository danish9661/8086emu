//! External Flash/EEPROM model — 28C256-like 32 KiB and W25Q-like 64 KiB.
//!
//! The emulator exposes a flat external memory window (separate from main `Mem`)
//! that behaves like byte-writable EEPROM/Flash: reads always succeed, writes
//! update the latch immediately (no erase cycle needed for the teaching model),
//! but a status register tracks busy/writes. An erase command wipes to 0xFF.
//! For 8086 the window defaults to 32 KiB at 0xE0000 (just below BIOS), for
//! 8085 to 16 KiB at 0xA000. Both are configurable via `Emulator::set_flash`.

#[derive(Clone)]
pub struct ExternalFlash {
    data: Vec<u8>,
    base: u32,
    size: u32,
    /// Write-Enable Latch (WEL) — set by OUT to status port, cleared after write
    wel: bool,
    /// Status register: bit0 WIP (write in progress, one-step pulse), bit1 WEL
    wip: bool,
    /// Last command byte (for JEDEC-like sequences; simplified)
    cmd: u8,
}

impl Default for ExternalFlash {
    fn default() -> Self {
        ExternalFlash { data: Vec::new(), base: 0, size: 0, wel: false, wip: false, cmd: 0 }
    }
}

impl ExternalFlash {
    pub fn new() -> Self { Self::default() }

    /// Configure window [base, base+len). Data is initialized to 0xFF (erased).
    pub fn configure(&mut self, base: u32, len: u32) {
        self.base = base;
        self.size = len;
        self.data = vec![0xFF; len as usize];
        self.wel = false;
        self.wip = false;
    }

    pub fn region(&self) -> Option<(u32,u32)> {
        if self.size>0 { Some((self.base,self.size)) } else { None }
    }

    pub fn in_range(&self, addr: u32) -> bool {
        self.size>0 && addr >= self.base && addr < self.base + self.size
    }

    pub fn read(&self, addr: u32) -> u8 {
        if !self.in_range(addr) { return 0xFF; }
        self.data[(addr - self.base) as usize]
    }

    pub fn write(&mut self, addr: u32, v: u8) -> bool {
        if !self.in_range(addr) { return false; }
        // Simplified: require WEL set unless flash is in "always-writable" mode
        // For teaching, allow writes always but set WIP pulse
        self.data[(addr - self.base) as usize] = v;
        self.wip = true; // one step pulse
        self.wel = false;
        true
    }

    /// Port interface (8086/8085):
    ///   0xE8  status (bit0 WIP, bit1 WEL, bit2..7 0)
    ///   0xE9  command: 0x06 = WREN (set WEL), 0x04 = WRDI, 0x20 = sector erase 4K, 0x60 = chip erase
    pub fn status(&self) -> u8 {
        (self.wip as u8) | ((self.wel as u8)<<1)
    }
    pub fn command(&mut self, v: u8) {
        self.cmd = v;
        match v {
            0x06 => self.wel = true,
            0x04 => self.wel = false,
            0x20 => {
                // sector erase 4K from base
                let n = 4096.min(self.data.len());
                for b in &mut self.data[..n] { *b = 0xFF; }
                self.wip = true;
            }
            0x60 | 0xC7 => {
                for b in &mut self.data { *b = 0xFF; }
                self.wip = true;
            }
            _ => {}
        }
    }

    pub fn load(&mut self, data: &[u8], offset: u32) {
        if self.size==0 { self.configure(offset, data.len() as u32); }
        let start = if offset < self.base { 0 } else { (offset - self.base) as usize };
        for (i,b) in data.iter().enumerate() {
            if start+i < self.data.len() { self.data[start+i]=*b; }
        }
    }

    /// Consume WIP pulse (called once per step)
    pub fn tick(&mut self) { self.wip = false; }

    pub fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.base.to_le_bytes());
        v.extend_from_slice(&self.size.to_le_bytes());
        v.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        v.extend_from_slice(&self.data);
        v.push(self.wel as u8); v.push(self.wip as u8); v.push(self.cmd);
        v
    }
    pub fn restore(&mut self, d: &[u8]) {
        if d.len() < 12 { return; }
        self.base = u32::from_le_bytes([d[0],d[1],d[2],d[3]]);
        self.size = u32::from_le_bytes([d[4],d[5],d[6],d[7]]);
        let len = u32::from_le_bytes([d[8],d[9],d[10],d[11]]) as usize;
        if d.len() < 12+len+3 { return; }
        self.data = d[12..12+len].to_vec();
        self.wel = d[12+len]!=0;
        self.wip = d[12+len+1]!=0;
        self.cmd = d[12+len+2];
    }

    pub fn mem_read(&self, addr: u32, len: usize) -> Vec<u8> {
        (0..len).map(|i| self.read(addr + i as u32)).collect()
    }
}
