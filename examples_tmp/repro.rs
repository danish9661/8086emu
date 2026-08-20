fn main() {
    use multi_cpu_emu::make_emulator;
    let src = "ORG 0\nDW isrNmi\nDW 0000h\nDW isrIntr\nDW 0000h\nORG 100h\nSTI\nMOV CX, 0000h\nspin:\nINC CX\nJMP spin\nisrNmi:\nMOV DX, 1111h\nIRET\nisrIntr:\nMOV DX, 2222h\nIRET\nEND";
    let mut e = make_emulator("8086").unwrap();
    let code = e.assemble(src).unwrap();
    e.mem_write(0, &code);
    let r = e.run(50);
    println!("mid: pc={:X} dx={:X} halted={}", e.pc(), e.regs().iter().find(|x| x.name=="DX").map(|x|x.value).unwrap(), r.halted);
    let res = e.request_interrupt("NMI", 0);
    println!("request: {:?}", res);
    let r2 = e.run(1000);
    println!("after: pc={:X} dx={:X} halted={}", e.pc(), e.regs().iter().find(|x| x.name=="DX").map(|x|x.value).unwrap(), r2.halted);
}
