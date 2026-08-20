//! Integration tests for all three cores: assembly + execution.

use multi_cpu_emu::make_emulator;

fn run_asm(isa: &str, src: &str, max_steps: u32) -> (Vec<multi_cpu_emu::cpu::Reg>, String, bool) {
    let mut emu = make_emulator(isa).unwrap();
    let code = emu.assemble(src).expect("assembly should succeed");
    let entry = if isa == "8086" { 0x100 } else { 0 };
    emu.mem_write(0, &code);
    emu.set_pc(entry);
    let r = emu.run(max_steps);
    let regs = emu.regs();
    let out = emu.take_output();
    (regs, out, r.halted)
}

fn reg(regs: &[multi_cpu_emu::cpu::Reg], name: &str) -> u32 {
    regs.iter().find(|r| r.name == name).map(|r| r.value).unwrap_or(0)
}

#[test]
fn hello_8086() {
    let src = r#"
        ORG 100h
        MOV DX, OFFSET msg
        MOV AH, 09h
        INT 21h
        MOV AH, 4Ch
        INT 21h
    msg: DB 'Hello, 8086!$'
    END
    "#;
    let (_, out, _) = run_asm("8086", src, 1000);
    assert_eq!(out, "Hello, 8086!");
}

#[test]
fn hello_8085() {
    let src = r#"
        MVI C, 05h
        LXI H, msg
    loop:
        MOV A, M
        CPI '$'
        JZ done
        OUT 01h
        INX H
        JMP loop
    done:
        HLT
    msg: DB 'Hello, 8085!$'
    END
    "#;
    let (_, out, _) = run_asm("8085", src, 1000);
    assert_eq!(out, "Hello, 8085!");
}

#[test]
fn hello_8051() {
    let src = r#"
        MOV DPTR, #msg
        MOV R1, #00h
    loop:
        MOV A, R1
        MOVC A, @A+DPTR
        JZ done
        MOV SBUF, A
        INC R1
        SJMP loop
    done:
        SJMP done
    msg: DB 'Hello, 8051!', 0
    END
    "#;
    let (_, out, _) = run_asm("8051", src, 2000);
    assert_eq!(out, "Hello, 8051!");
}

#[test]
fn arithmetic_8086() {
    // 5*3 + 2 = 17
    let src = r#"
        ORG 100h
        MOV AX, 5
        MOV BX, 3
        MUL BX          ; AX = 15
        ADD AX, 2       ; AX = 17
        MOV CX, AX
        MOV AH, 4Ch
        INT 21h
    END
    "#;
    let (regs, _, _) = run_asm("8086", src, 1000);
    assert_eq!(reg(&regs, "CX"), 17); // result saved before exit
    assert_eq!(reg(&regs, "AX"), 0x4C11); // AH=4Ch exit code, AL=result low byte
}

#[test]
fn arithmetic_8085() {
    // A = 25 + 5 = 30
    let src = r#"
        MVI A, 25
        ADI 05h
        MOV B, A
        HLT
    END
    "#;
    let (regs, _, _) = run_asm("8085", src, 100);
    assert_eq!(reg(&regs, "A"), 30);
    assert_eq!(reg(&regs, "B"), 30);
}

#[test]
fn arithmetic_8051() {
    // A = 40 - 7 = 33
    let src = r#"
        MOV A, #40
        SUBB A, #07
        MOV R0, A
        END
    "#;
    let (regs, _, _) = run_asm("8051", src, 100);
    assert_eq!(reg(&regs, "A"), 33);
    assert_eq!(reg(&regs, "R0"), 33);
}

#[test]
fn snapshot_restore_roundtrip() {
    let mut emu = make_emulator("8086").unwrap();
    let src = "ORG 100h\nMOV AX, 1234h\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.step(); // MOV AX,1234
    let snap = emu.snapshot();
    let ax = emu.regs()[0].value;
    assert_eq!(ax, 0x1234);
    let mut emu2 = make_emulator("8086").unwrap();
    emu2.restore(&snap);
    assert_eq!(emu2.regs()[0].value, 0x1234);
    assert_eq!(emu2.pc(), emu.pc());
}

#[test]
fn memory_access() {
    let mut emu = make_emulator("8085").unwrap();
    let src = "LXI H, 2000h\nMVI M, 42h\nHLT";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(100);
    assert_eq!(emu.mem_read(0x2000, 1)[0], 0x42);
}
#[test]
fn bound_8086() {
    let src = "ORG 100h\nMOV AX, 2\nLEA BX, [bounds]\nBOUND AX, [BX]\nMOV CX, 1\nHLT\nbounds: DW 0, 3\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).expect("assembly should succeed");
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "CX"), 1, "in-bounds index must not trap");

    let src = "ORG 100h\nMOV AX, 99\nLEA BX, [bounds]\nBOUND AX, [BX]\nHLT\nbounds: DW 0, 3\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).expect("assembly should succeed");
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    let regs = emu.regs();
    assert_eq!(reg(&regs, "CS"), 0, "out-of-bounds index must trap via INT 5 (empty IVT -> CS:IP = 0:0)");
    assert_eq!(reg(&regs, "IP"), 0xC2, "trapped code at 0:0 runs 2-byte 00 00 (ADD [BX+SI],AL) per step");
}

#[test]
fn io_string_8086() {
    let src = "ORG 100h\nMOV DI, 2000h\nMOV CX, 3\nREP INSB\nMOV SI, 3000h\nMOV CX, 2\nREP OUTSW\nMOV CX, 1\nHLT\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).expect("assembly should succeed");
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    let regs = emu.regs();
    assert_eq!(reg(&regs, "DI"), 0x2003, "INSB x3 must advance DI by 3");
    assert_eq!(reg(&regs, "SI"), 0x3004, "OUTSW x2 must advance SI by 4");
    assert_eq!(reg(&regs, "CX"), 1);
    assert_eq!(emu.mem_read(0x2000, 3), vec![0, 0, 0], "INS writes port reads (0)");
}

#[test]
fn wait_and_bound_encoding() {
    let emu = make_emulator("8086").unwrap();
    let code = emu.assemble("ORG 100h\nWAIT\nBOUND AX, [BX]\nINSB\nINSW\nOUTSB\nOUTSW\nEND").unwrap();
    assert_eq!(code.len(), 0x107, "ORG 100h must pad the image to 0x100");
    assert_eq!(&code[..0x100], vec![0; 0x100]);
    assert_eq!(&code[0x100..], vec![0x9B, 0x62, 0x07, 0x6C, 0x6D, 0x6E, 0x6F]);
}

#[test]
fn jmp_a_dptr_8051() {
    let src = "MOV DPTR, #100h\nMOV A, #04h\nJMP @A+DPTR\nDB 0, 0, 0, 0, 0\nEND";
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).expect("assembly should succeed");
    assert_eq!(code[5], 0x73, "JMP @A+DPTR must encode to 0x73");
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(3);
    assert_eq!(emu.pc(), 0x0104, "PC must jump to A+DPTR");
}

#[test]
fn interrupts_8085() {
    // TRAP fires even with EI disabled and keeps the interrupt-enable flip-flop
    let mut emu = make_emulator("8085").unwrap();
    emu.mem_write(0, &[0x00, 0x76]); // NOP, then HLT at vector 0x24
    emu.mem_write(0x24, &[0x76]);
    emu.set_pc(0);
    emu.request_interrupt("TRAP", 0).unwrap();
    emu.step(); // NOP
    assert_eq!(emu.pc(), 0x24, "TRAP must vector to 0x24");
    assert_eq!(emu.mem_read(0xFFFB, 2), vec![0x01, 0x00], "return address pushed");
    assert_eq!(emu.mem_read(0xFFF9, 2), vec![0x00, 0x00], "PSW pushed");

    // DI blocks RST 7.5
    let mut emu = make_emulator("8085").unwrap();
    let code = emu.assemble("DI\nHLT").unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.request_interrupt("RST75", 0).unwrap();
    emu.step(); // DI
    assert_eq!(emu.pc(), 1, "maskable interrupt must wait for EI");
    emu.step(); // HLT
    assert_eq!(emu.pc(), 2, "HLT still halts; interrupt not taken while DI");

    // EI + RST 7.5 -> vector 0x3C, IE cleared, pending latch cleared
    let mut emu = make_emulator("8085").unwrap();
    let code = emu.assemble("EI\nHLT").unwrap();
    emu.mem_write(0, &code);
    emu.mem_write(0x3C, &[0x76]);
    emu.set_pc(0);
    emu.request_interrupt("RST75", 0).unwrap();
    emu.step(); // EI -> interrupt serviced at the end of EI
    assert_eq!(emu.pc(), 0x3C, "RST 7.5 must vector to 0x3C");
    assert_eq!(emu.mem_read(0xFFFB, 2), vec![0x01, 0x00], "return address = after EI");

    // SIM masks RST 5.5; RIM reports the mask
    let mut emu = make_emulator("8085").unwrap();
    let code = emu.assemble("MVI A, 09h\nSIM\nRIM\nHLT").unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "A"), 0x01, "RIM must report M5.5=1");

    // INTR with external vector 0x08 (RST 0)
    let mut emu = make_emulator("8085").unwrap();
    let code = emu.assemble("EI\nHLT").unwrap();
    emu.mem_write(0, &code);
    emu.mem_write(0x08, &[0x76]);
    emu.set_pc(0);
    emu.request_interrupt("INTR", 0x08).unwrap();
    emu.step(); // EI -> INTR serviced at the end of EI
    assert_eq!(emu.pc(), 0x08, "INTR must jump to the supplied vector");
    assert!(!emu.flags().interrupt, "INTR must clear the IE flip-flop");
}

#[test]
fn org_placement_8085() {
    let emu = make_emulator("8085").unwrap();
    let code = emu.assemble("NOP\nORG 24h\nMVI A, 'T'\nOUT 01h\nRET\nEND").unwrap();
    assert_eq!(code[0], 0x00, "NOP at 0");
    assert_eq!(code[0x24], 0x3E, "ORG 24h must place the handler at 0x24");
    assert_eq!(code[0x25], b'T');
    let err = emu.assemble("NOP\nORG 0\nEND").unwrap_err();
    assert!(err.contains("backwards"), "backward ORG must be rejected: {err}");
}

#[test]
fn interrupt_isr_8085() {
    // Full program with ISRs placed at the hardware vectors via ORG
    let mut emu = make_emulator("8085").unwrap();
    let src = "EI\nmain:\nJMP main\nORG 24h\nMVI A, 'T'\nOUT 01h\nRET\nORG 3Ch\nMVI A, '7'\nOUT 01h\nEI\nRET\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.request_interrupt("TRAP", 0).unwrap();
    emu.run(1000);
    assert_eq!(emu.take_output(), "T", "TRAP handler must print 'T'");
    emu.request_interrupt("RST75", 0).unwrap();
    emu.run(1000);
    assert_eq!(emu.take_output(), "7", "RST 7.5 handler must print '7'");
}
