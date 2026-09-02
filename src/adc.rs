//! ADC0808 8-channel, 8-bit successive approximation ADC.
//!
//! Ports (both 8086 and 8085, free region):
//!   0x28  control / channel select (OUT: D2-D0 = channel 0..7, D7 = START pulse)
//!         status (IN: D7 = EOC, D6 = OE, low bits = 0)
//!   0x29  data (IN: last conversion result, 0..255)
//! Conversion is instantaneous in the model (EOC set one host step after START).
//! The host can preload channel voltages via `set_channel(ch, value)` or
//! `Emulator::adc_set` (WASM). Default: channel = index*32.

#[derive(Clone)]
pub struct Adc0808 {
    channels: [u8; 8],
    selected: u8,
    result: u8,
    eoc: bool,
    oe: bool,
    start_pending: bool,
}

impl Default for Adc0808 {
    fn default()->Self {
        let mut a = Adc0808 { channels:[0;8], selected:0, result:0, eoc:true, oe:false, start_pending:false };
        for i in 0..8 { a.channels[i]=(i as u8)*32; }
        a
    }
}

impl Adc0808 {
    pub fn new()->Self { Self::default() }
    pub fn set_channel(&mut self, ch: usize, v:u8){ if ch<8 { self.channels[ch]=v; } }
    pub fn get_channel(&self, ch: usize)->u8 { if ch<8 { self.channels[ch] } else { 0 } }

    pub fn write_ctrl(&mut self, v:u8){
        self.selected = v & 0x07;
        if v & 0x80 !=0 {
            // START pulse — begin conversion
            self.eoc = false;
            self.oe = false;
            self.start_pending = true;
            // In teaching model convert instantly but keep one-step EOC delay
            self.result = self.channels[self.selected as usize];
        }
        if v & 0x40 !=0 { self.oe = true; } // OE
    }
    pub fn read_status(&self)->u8 {
        let mut s = 0u8;
        if self.eoc { s |= 0x80; }
        if self.oe { s |= 0x40; }
        s |= self.selected & 0x07;
        s
    }
    pub fn read_data(&self)->u8 { self.result }

    /// Advance one host step: complete pending conversion
    pub fn tick(&mut self){
        if self.start_pending { self.eoc = true; self.start_pending=false; }
    }

    pub fn snapshot(&self)->Vec<u8>{
        let mut v=Vec::new();
        v.extend_from_slice(&self.channels);
        v.push(self.selected); v.push(self.result);
        v.push(self.eoc as u8); v.push(self.oe as u8); v.push(self.start_pending as u8);
        v
    }
    pub fn restore(&mut self, d:&[u8]){
        if d.len()<13 { return; }
        self.channels.copy_from_slice(&d[0..8]);
        self.selected=d[8]; self.result=d[9];
        self.eoc=d[10]!=0; self.oe=d[11]!=0; self.start_pending=d[12]!=0;
    }
}
