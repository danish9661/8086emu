//! Integration tests for all three cores: assembly + execution.

use multi_cpu_emu::make_emulator;

fn run_asm(isa: &str, src: &str, max_steps: u32) -> (Vec<multi_cpu_emu::cpu::Reg>, String, bool) {
    let mut emu = make_emulator(isa).unwrap();
    let code = emu.assemble(src).expect("assembly should succeed");
    let origin = if isa == "8086" { 0x100 } else { 0 };
    emu.mem_write(origin, &code);
    emu.set_pc(origin);
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
    emu.mem_write(0x100, &code);
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