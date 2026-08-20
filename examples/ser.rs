fn main() {
    let mut emu = multi_cpu_emu::make_emulator("8051").unwrap();
    let src = "ORG 0\nSJMP main\nORG 03h\nMOV SBUF, #'0'\nRETI\nORG 0Bh\nPUSH ACC\nNOP\nNOP\nPOP ACC\nRETI\nORG 30h\nmain:\nMOV TMOD, #01h\nMOV TH0, #0FFh\nMOV TL0, #0FFh\nSETB TR0\nMOV IP, #02h\nMOV IE, #83h\nstart:\nSJMP start\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.request_interrupt("INT0", 0).unwrap();
    for i in 0..14 {
        emu.step();
        println!("step {}: pc={:02X} tcon={:02X} ie={:02X} ip={:02X}", i+1, emu.pc(), emu.mem_read(0x88,1)[0], emu.mem_read(0xA8,1)[0], emu.mem_read(0xB8,1)[0]);
    }
    println!("out: {:?}", emu.take_output());
}
