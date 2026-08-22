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

#[test]
fn interrupts_8051_external() {
    // INT0 -> vector 03h; PCL pushed first; IE0 cleared by hardware; EA gates
    let mut emu = make_emulator("8051").unwrap();
    let src = "ORG 0\nSJMP main\nORG 03h\nMOV SBUF, #'0'\nRETI\nORG 30h\nmain:\nSETB IT0\nMOV IE, #81h\nstart:\nSJMP start\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.request_interrupt("INT0", 0).unwrap();
    emu.run(100);
    assert_eq!(emu.take_output(), "0", "INT0 handler must print '0'");
    assert_eq!(emu.sfr(0x88) & 0x02, 0, "IE0 latch must be cleared on service");
    assert_eq!(emu.mem_read(0x09, 1)[0], 0, "SP must be back to 0x07");

    // EA=0: pending INT0 must not vector
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble("ORG 0\nSJMP main\nORG 30h\nmain:\nSETB IT0\nMOV IE, #01h\nstart:\nSJMP start\nEND").unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.request_interrupt("INT0", 0).unwrap();
    emu.run(20);
    assert_eq!(emu.pc(), 0x35, "with EA=0 the CPU must stay in the loop");
}

#[test]
fn interrupts_8051_stack_layout() {
    // PCL must sit at SP+1 (real 8051 pushes low byte first)
    let mut emu = make_emulator("8051").unwrap();
    let src = "ORG 0\nSJMP main\nORG 03h\nRETI\nORG 30h\nmain:\nSETB IT0\nMOV IE, #81h\nstart:\nSJMP start\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.request_interrupt("INT0", 0).unwrap();
    emu.run(3); // SETB IT0; MOV IE -> dispatch: PC = 03h, SP pushed 2 bytes
    assert_eq!(reg(&emu.regs(), "SP"), 9, "dispatch must push two bytes");
    assert_eq!(emu.pc(), 0x03);
    emu.run(1); // RETI
    assert_eq!(emu.pc(), 0x35, "RETI must return to the address after MOV IE (PCL pushed first)");
    assert_eq!(reg(&emu.regs(), "SP"), 7, "stack must be balanced after RETI");
}

#[test]
fn interrupts_8051_timer() {
    let mut emu = make_emulator("8051").unwrap();
    let src = "ORG 0\nSJMP main\nORG 0Bh\nMOV SBUF, #'T'\nRETI\nORG 30h\nmain:\nMOV TMOD, #01h\nMOV TH0, #0FFh\nMOV TL0, #0FFh\nSETB TR0\nMOV IE, #82h\nstart:\nSJMP start\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(100);
    assert_eq!(emu.take_output(), "T", "TF0 overflow must vector to 0Bh");
    assert_eq!(emu.sfr(0x88) & 0x20, 0, "TF0 must be cleared by hardware");
}

#[test]
fn interrupts_8051_priority() {
    // TF0 (PT0=1) blocks low-priority INT0 while its ISR runs; after RETI
    // the still-pending INT0 fires.
    let mut emu = make_emulator("8051").unwrap();
    let src = "ORG 0\nSJMP main\nORG 03h\nMOV SBUF, #'0'\nRETI\nORG 0Bh\nPUSH ACC\nNOP\nNOP\nPOP ACC\nRETI\nORG 30h\nmain:\nSETB IT0\nMOV TMOD, #01h\nMOV TH0, #0FFh\nMOV TL0, #0FFh\nSETB TR0\nMOV IP, #02h\nMOV IE, #83h\nstart:\nSJMP start\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(10); // inside the TF0 ISR (after NOP at 0x0E); no INT0 requested yet
    assert_eq!(emu.pc(), 0x0E, "TF0 (higher natural priority, no INT0 pending) must be in service");
    emu.request_interrupt("INT0", 0).unwrap();
    emu.run(2); // POP ACC, RETI -> INT0 must NOT preempt the high-priority TF0 ISR
    assert_eq!(emu.pc(), 0x11, "INT0 must stay blocked while the TF0 ISR is in service, pc={:X}", emu.pc());
    assert_eq!(emu.sfr(0x88) & 0x02, 0x02, "IE0 must remain pending (not serviced)");
    emu.run(10); // RETI -> INT0 fires
    assert_eq!(emu.take_output(), "0", "INT0 must fire after the TF0 ISR returns");
}

#[test]
fn interrupts_8051_serial() {
    // SBUF write sets TI; the serial ISR must clear TI itself or it re-fires
    let mut emu = make_emulator("8051").unwrap();
    let src = "ORG 0\nSJMP main\nORG 23h\nCLR TI\nMOV SBUF, #'B'\nRETI\nORG 30h\nmain:\nMOV IE, #90h\nMOV SBUF, #'A'\nstart:\nSJMP start\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(20);
    let out = emu.take_output();
    assert_eq!(out, "ABBBBBB", "TI re-fires each cycle until RETI, out={out}");
    assert_eq!(emu.pc(), 0x28, "the 3-step ISR cycle is CLR TI / MOV SBUF / RETI");
}

#[test]
fn keyboard_8086() {
    // AH=01 echo read: key popped, echoed
    let src = "ORG 100h\nMOV AH, 01h\nINT 21h\nMOV CL, AL\nMOV AH, 02h\nMOV DL, AL\nINT 21h\nMOV AH, 4Ch\nINT 21h\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.push_key(b'A');
    emu.run(100);
    assert_eq!(emu.take_output(), "AA", "AH=01 must echo the key, AH=02 prints it again");
    assert_eq!(reg(&emu.regs(), "CX"), b'A' as u32, "the key must land in AL (via CL)");

    // AH=07 no echo
    let src = "ORG 100h\nMOV AH, 07h\nINT 21h\nMOV AH, 4Ch\nINT 21h\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.push_key(b'X');
    emu.run(100);
    assert_eq!(emu.take_output(), "", "AH=07 must not echo");
    assert_eq!(reg(&emu.regs(), "AX") & 0xFF, b'X' as u32);
}

#[test]
fn keyboard_pending_8086() {
    // Empty buffer: run() must stop, waiting_input() true, IP re-pointed at INT 21h
    let src = "ORG 100h\nMOV AH, 01h\nINT 21h\nMOV AH, 4Ch\nINT 21h\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert!(emu.waiting_input(), "run must stop blocked on input");
    assert_eq!(emu.pc(), 0x102, "IP must point back at the INT 21h to re-execute");

    // Push a key -> the blocked INT re-executes and consumes it
    emu.push_key(b'K');
    assert!(!emu.waiting_input());
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "AX") & 0xFF, b'K' as u32);
    assert!(emu.is_halted(), "program must finish after the key");

    // step() must not advance while blocked
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble("ORG 100h\nMOV AH, 01h\nINT 21h\nHLT\nEND").unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.step();
    emu.step(); // INT 21h with empty buffer
    let pc = emu.pc();
    emu.step();
    assert_eq!(emu.pc(), pc, "step must not execute while input is pending");
}

#[test]
fn keyboard_flush_8086() {
    // AH=0C AL=00 flushes queued keys; a later AH=01 blocks until a new key
    let src = "ORG 100h\nMOV AH, 0Ch\nMOV AL, 00h\nINT 21h\nMOV AH, 01h\nINT 21h\nMOV AH, 4Ch\nINT 21h\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.push_key(b'a');
    emu.push_key(b'b');
    emu.run(100);
    assert!(emu.waiting_input(), "flushed keys must not satisfy the AH=01 read");
    emu.push_key(b'z');
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "AX") & 0xFF, b'z' as u32, "only the post-flush key must be read");
}

#[test]
fn assemble_info_lines() {
    // Per-line info: address + bytes for each emitting line, all three ISAs.
    type Case = (&'static str, &'static str, Vec<(u32, u32, &'static [u8])>);
    let cases: [Case; 3] = [
        (
            "8086",
            "ORG 100h\nMOV AX, 5\nMOV BX, 3\nMUL BX\nADD AX, 2\nMOV AH, 4Ch\nINT 21h\nEND",
            vec![
                (2, 0x100, &[0xB8, 0x05, 0x00]),
                (3, 0x103, &[0xBB, 0x03, 0x00]),
                (4, 0x106, &[0xF7, 0xE3]),
                (5, 0x108, &[0x05, 0x02, 0x00]),
                (6, 0x10B, &[0xB4, 0x4C]),
                (7, 0x10D, &[0xCD, 0x21]),
            ],
        ),
        (
            "8085",
            "ORG 8000h\nMVI A, 05h\nADI 02h\nJMP skip\nNOP\nskip: HLT\nEND",
            vec![
                (2, 0x8000, &[0x3E, 0x05]),
                (3, 0x8002, &[0xC6, 0x02]),
                (4, 0x8004, &[0xC3, 0x08, 0x80]),
                (5, 0x8007, &[0x00]),
                (6, 0x8008, &[0x76]),
            ],
        ),
        (
            "8051",
            "ORG 0\nSJMP main\nORG 30h\nmain: MOV A, #05h\nADD A, #02h\nMOV P1, A\nEND",
            vec![
                (2, 0x0000, &[0x80, 0x2E]),
                (4, 0x0030, &[0x74, 0x05]),
                (5, 0x0032, &[0x24, 0x02]),
                (6, 0x0034, &[0xF5, 0x90]),
            ],
        ),
    ];
    for (isa, src, expected) in cases {
        let emu = make_emulator(isa).unwrap();
        let (code, info) = emu.assemble_info(src).unwrap();
        let end = info.iter().map(|i| i.addr + i.bytes.len() as u32).max().unwrap_or(0) as usize;
        assert_eq!(code.len(), end, "{isa}: padded image must end at the last emitted byte");
        let got: Vec<(u32, u32, Vec<u8>)> = info.iter().map(|i| (i.line, i.addr, i.bytes.clone())).collect();
        assert_eq!(got.len(), expected.len(), "{isa}: line count");
        for (g, (el, ea, eb)) in got.iter().zip(expected.iter()) {
            assert_eq!(g.0, *el, "{isa}: line number");
            assert_eq!(g.1, *ea, "{isa}: address on line {}", g.0);
            assert_eq!(&g.2, eb, "{isa}: bytes on line {}", g.0);
        }
    }
}

// ---------- flags ----------

#[test]
fn flags_8086_arith() {
    // ADC chains carry, SBB borrows, INC/DEC preserve CF, CMP leaves operands
    let src = r#"
        ORG 100h
        CLC
        MOV AX, 7FFFh
        INC AX              ; OF=1, SF=1, no CF change
        MOV BX, 0001h
        ADD AX, BX          ; AX=8000h CF=0 OF=1 (7FFF+1), SF=1
        ADC AX, BX          ; AX=8002h CF=0
        ADC AX, 0FFFFh      ; 8002+FFFF+0 = 8001 CF=1
        SBB AX, 0FFFFh      ; 8001-FFFF-1 = 8001 CF=0 (8001 >= FFFF+1? no -> CF=1)
        SBB AX, 1           ; CF=0
        CMP AX, 0FFFFh      ; no write, CF=1 ZF=0
        MOV CX, 0
        INC CX
        CMP CX, 1           ; ZF=1
        MOV AH, 4Ch
        INT 21h
        END
    "#;
    let (_, _, _) = run_asm("8086", src, 200);
}

#[test]
fn flags_8086_logic_shifts() {
    // AND/OR/XOR/TEST clear CF/OF; SHL/SAR/RCL set CF; LAHF/SAHF round-trip
    let src = r#"
        ORG 100h
        STC
        MOV AX, 0FF00h
        AND AX, 0FFh        ; AX=0, CF=0 OF=0 ZF=1
        MOV AX, 8000h
        OR  AX, 0001h       ; AX=8001 CF=0
        MOV AX, 0F0F0h
        XOR AX, 0FFFFh      ; AX=0F0F
        TEST AX, 0F0Fh      ; AX unchanged, ZF=0
        MOV AX, 8000h
        SHL AX, 1           ; AX=0 CF=1 ZF=1
        MOV AX, 8000h
        SAR AX, 1           ; AX=C000 CF=0 SF=1
        CLC
        MOV AX, 0001h
        RCL AX, 1           ; AX=2 CF=0
        STC
        RCL AX, 1           ; AX=5 CF=0
        LAHF
        SAHF
        CMP AX, AX
        MOV AH, 4Ch
        INT 21h
        END
    "#;
    run_asm("8086", src, 200);
}

#[test]
fn flags_8085_arith_daa() {
    // DAA BCD-adjusts, ADI sets AC, INR/DCR keep CY, DAD sets only CY, CMP flags
    let src = r#"
        ORG 0
        STC
        MVI A, 09h
        ADI 01h             ; A=0Ah AC=1 CY=0
        DAA                 ; A=10h (BCD)
        MVI A, 99h
        ADI 01h             ; CY=1
        DAA                 ; A=00h CY=1
        MVI A, 0Fh
        INR A               ; A=10h, CY untouched (still 1), AC=1
        DCR A               ; A=0Fh, CY untouched
        LXI H, 0FFFFh
        LXI D, 0002h
        DAD D               ; HL=0001h CY=1
        MVI A, 05h
        CPI 06h             ; CY=1 (A < 6), ZF=0
        CPI 05h             ; ZF=1, CY=0
        HLT
        END
    "#;
    run_asm("8085", src, 200);
}

#[test]
fn flags_8085_rotates() {
    // RLC/RRC set CY; RAL/RAR rotate through carry
    let src = r#"
        ORG 0
        STC
        MVI A, 80h
        RLC                 ; A=01h CY=1
        RRC                 ; A=80h CY=1
        CMC                 ; CY=0
        RAL                 ; A=00h CY=1
        CMC
        RAR                 ; A=80h CY=0
        HLT
        END
    "#;
    run_asm("8085", src, 200);
}

#[test]
fn flags_8051_arith() {
    // AC on ADD, OV on 7F+01, SUBB borrow, DA, MUL/DIV flags
    let src = r#"
        ORG 30h
        MOV A, #0Fh
        ADD A, #01h         ; AC=1 CY=0
        MOV A, #7Fh
        ADD A, #01h         ; OV=1 CY=0
        MOV A, #00h
        SUBB A, #01h        ; CY=1 (borrow)
        MOV A, #99h
        ADD A, #01h
        DA A                ; A=00h CY=1
        MOV A, #0Ah
        MOV B, #0Ah
        MUL AB              ; A=64h B=00h CY=0 OV=0
        MOV A, #0FFh
        MOV B, #02h
        MUL AB              ; OV=1
        MOV A, #0Ah
        MOV B, #00h
        DIV AB              ; OV=1 (divide by zero), A=0
        RET
        END
    "#;
    run_asm("8051", src, 200);
}

#[test]
fn flags_8051_bit_ops() {
    // SETB/CLR/CPL on bits, ANL C/ORL C/MOV C
    let src = r#"
        ORG 30h
        SETB C
        SETB 00h            ; bit addressable RAM 0x20.0
        MOV C, 00h          ; C = 1
        ANL C, 01h          ; C = 1 AND 0 = 0
        ORL C, 00h          ; C = 1
        CPL C
        CLR 00h
        MOV 00h, C          ; bit = 0
        JNB 00h, skip
        MOV A, #0AAh        ; not taken
        SJMP $
    skip:
        MOV A, #55h
        SJMP $
        END
    "#;
    run_asm("8051", src, 200);
}

// ---------- 8086 string ops ----------

#[test]
fn string_movs_8086() {
    // REP MOVSB copies bytes; CLD forward / STD backward
    let src = r#"
        ORG 100h
        CLD
        MOV SI, OFFSET src
        MOV DI, OFFSET dst
        MOV CX, 5
        REP MOVSB
        STD
        MOV SI, OFFSET src + 4
        MOV DI, OFFSET dst + 10
        MOV CX, 5
        REP MOVSB           ; dst[10..14] = src[4..0] reversed
        CLD
        MOV AH, 4Ch
        INT 21h
    src: DB 'A','B','C','D','E'
    dst: DB 0,0,0,0,0, 0,0,0,0,0, 0,0,0,0,0
        END
    "#;
    let (regs, _, _) = run_asm("8086", src, 200);
    let _ = regs;
}

#[test]
fn string_cmps_scas_8086() {
    // REPE CMPSB stops on mismatch, REPNE SCASB finds target, CX counts
    let src = r#"
        ORG 100h
        CLD
        MOV SI, OFFSET a
        MOV DI, OFFSET b
        MOV CX, 4
        REPE CMPSB          ; a="ABXX", b="ABYZ" -> stops at 3rd byte, CX=1
        MOV DX, CX          ; DX = 1
        MOV DI, OFFSET b
        MOV AL, 'Y'
        MOV CX, 4
        REPNE SCASB         ; found Y at index 2 -> CX=1
        MOV BX, CX          ; BX = 1
        MOV AH, 4Ch
        INT 21h
    a: DB 'A','B','X','X'
    b: DB 'A','B','Y','Z'
        END
    "#;
    let (regs, _, _) = run_asm("8086", src, 200);
    assert_eq!(reg(&regs, "DX"), 1, "REPE CMPSB must stop after the mismatch");
    assert_eq!(reg(&regs, "BX"), 1, "REPNE SCASB must leave the remaining count");
}

#[test]
fn string_lods_stos_8086() {
    // LODSB + STOSB transfer via AL; LODSW moves a word
    let src = r#"
        ORG 100h
        CLD
        MOV SI, OFFSET tbl
        MOV DI, OFFSET out
        LODSW               ; AX = 0x1122
        STOSB               ; out[0] = 0x22
        LODSB               ; AL = 0x33
        STOSB               ; out[1] = 0x33
        MOV AH, 4Ch
        INT 21h
    tbl: DB 22h, 11h, 33h, 44h   ; LODSW -> 0x1122, LODSB -> 0x33
    out: DB 0, 0
        END
    "#;
    let (regs, _, _) = run_asm("8086", src, 200);
    assert_eq!(reg(&regs, "AX"), 0x4C33, "LODSW then LODSB leaves AX = 0x4C33");
}

// ---------- 8086 stack ----------

#[test]
fn stack_8086() {
    // PUSH/POP reg + imm + segment, SP discipline, CALL/RET nesting
    let src = r#"
        ORG 100h
        MOV AX, 1234h
        MOV BX, 5678h
        PUSH AX
        PUSH BX
        POP AX              ; AX = 5678
        POP BX              ; BX = 1234
        PUSH 0CAFEh
        POP CX              ; CX = CAFE
        PUSH CS
        POP DX              ; DX = CS
        CALL sub
        MOV DI, 1
        CALL nested
        MOV AH, 4Ch
        INT 21h
    sub:
        MOV SI, 2
        RET
    nested:
        CALL inner
        MOV DI, 3
        RET
    inner:
        MOV BP, 4
        RET
        END
    "#;
    let (regs, _, _) = run_asm("8086", src, 200);
    assert_eq!(reg(&regs, "AX"), 0x4C78, "AX = 5678 with AH=4Ch exit code");
    assert_eq!(reg(&regs, "BX"), 0x1234);
    assert_eq!(reg(&regs, "CX"), 0xCAFE);
    assert_eq!(reg(&regs, "DX"), reg(&regs, "CS"), "PUSH CS / POP DX");
    assert_eq!(reg(&regs, "SI"), 2);
    assert_eq!(reg(&regs, "DI"), 3);
    assert_eq!(reg(&regs, "BP"), 4);
}

// ---------- 8085 stack ----------

#[test]
fn stack_8085() {
    // PUSH/POP BC/DE/HL/PSW, XTHL, SPHL, PCHL, nested CALL/RET
    let src = r#"
        ORG 0
        LXI SP, 9000h
        LXI B, 1234h
        LXI D, 5678h
        LXI H, 9ABCh
        PUSH B
        PUSH D
        PUSH H
        POP D               ; DE = 9ABC
        POP H               ; HL = 5678
        POP B               ; BC = 1234
        MVI A, 42h
        PUSH PSW            ; A + flags
        POP B               ; B = flags, C = A
        LXI H, 0DEADh
        LXI SP, 9100h
        PUSH H
        LXI H, 0BEEFh
        XTHL                ; (9100)=BEEF, HL=DEAD
        SPHL                ; SP = DEAD
        LXI H, target
        PCHL
        HLT
    target:
        CALL nest
        HLT
    nest:
        LXI H, 8000h
        CALL nest2
        RET
    nest2:
        INR L
        RET
        END
    "#;
    let (regs, _, _) = run_asm("8085", src, 200);
    assert_eq!(reg(&regs, "B"), 0x42, "PUSH PSW / POP B puts A in B");
    assert_eq!(reg(&regs, "H"), 0x80, "nested calls must return to nest2");
    assert_eq!(reg(&regs, "L"), 0x01);
}

// ---------- 8051 stack / timers ----------

#[test]
fn stack_8051() {
    // PUSH/POP, ACALL/LCALL/RET layout (PCL first), SP after RET
    let src = r#"
        ORG 30h
        MOV SP, #40h
        MOV A, #11h
        MOV B, #22h
        PUSH ACC
        PUSH B
        POP ACC             ; ACC = 22h
        POP B               ; B = 11h
        MOV R7, A
        ACALL sub           ; 2-byte call
        LCALL far           ; 3-byte call
        MOV A, #00h
        SJMP $
    sub:
        MOV R0, #1
        RET
    far:
        MOV R1, #2
        RET
        END
    "#;
    let (regs, _, _) = run_asm("8051", src, 200);
    assert_eq!(reg(&regs, "R7"), 0x22, "POP ACC then POP B leaves A = 0x22");
    assert_eq!(reg(&regs, "B"), 0x11);
}

#[test]
fn timers_8051() {
    // TMOD mode 1 (16-bit): TL0/TH0 count on steps, TF0 on overflow;
    // mode 2 (8-bit auto-reload); TR0 gates; timer 1 independent
    let src = r#"
        ORG 30h
        MOV TMOD, #01h      ; T0 mode 1
        MOV TL0, #0FEh
        MOV TH0, #00h
        SETB TR0            ; start
        MOV A, #00h         ; 3 steps
        MOV B, #00h
        MOV R0, #00h        ; TL0 = 01h, TH0 = 00h
        SETB TR1            ; start timer 1 too
        MOV R1, #00h        ; TL0 = 02h
        MOV TMOD, #22h      ; both mode 2 (auto-reload)
        MOV TL0, #0FEh
        MOV TH0, #0FEh
        MOV TL1, #0FFh
        MOV TH1, #0FFh
        MOV A, #00h         ; TL0 wraps: FE->FF->00, TF0 set, TL0=FE (reloaded)
        MOV B, #00h         ; TL1 wraps: FF->00, TF1 set, TL1=FF
        JNB TF0, no0
        MOV R2, #1
        SJMP t1
    no0:
        MOV R2, #0
    t1:
        JNB TF1, no1
        MOV R3, #1
        SJMP done
    no1:
        MOV R3, #0
    done:
        SJMP $
        END
    "#;
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(200);
    let sfr = |a: u8| emu.sfr(a);
    assert!(sfr(0x8A) == 0xFE || sfr(0x8A) == 0xFF, "TL0 must stay near FE (auto-reload)");
    assert_eq!(sfr(0x8C), 0xFE, "TH0 must keep its reload value");
    assert_eq!(sfr(0x8D), 0xFF, "TH1 must keep its reload value");
    let regs = emu.regs();
    assert_eq!(reg(&regs, "R2"), 1, "TF0 must be set after TL0 overflow");
    assert_eq!(reg(&regs, "R3"), 1, "TF1 must be set after TL1 overflow");
}

#[test]
fn timers_8051_stopped() {
    // TR0 = 0: timer must not count; TR0 = 1 resumes
    let src = r#"
        ORG 30h
        MOV TMOD, #01h
        MOV TL0, #10h
        MOV TH0, #00h
        MOV A, #00h
        MOV B, #00h         ; TL0 still 10h (TR0 clear)
        SETB TR0
        MOV R0, #00h        ; TL0 = 11h (tick before exec)
        CLR TR0
        MOV R1, #00h        ; this step still ticks with TR0=1 -> 12h
        MOV R2, #00h        ; TR0 clear now -> TL0 stays 12h
        SJMP $
        END
    "#;
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(200);
    assert_eq!(emu.sfr(0x8A), 0x12, "TL0 must count only while TR0 is set");
}

#[test]
fn run_to_target() {
    // run_to stops with the target as the next instruction (not executed)
    let src = "ORG 100h\nMOV AX, 5\nMOV BX, 3\nMUL BX\nMOV AH, 4Ch\nINT 21h\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    let r = emu.run_to(1000, 0x106); // target = MUL BX
    assert_eq!(r.steps, 2, "runs exactly up to (not including) the target");
    assert_eq!(emu.pc(), 0x106);
    let r2 = emu.run_to(1000, 0xFFFF); // unreachable target
    assert!(r2.halted, "unreachable target runs to halt");
}

#[test]
fn run_to_step_over_call() {
    // step-over semantics: run_to(return address) executes a CALL body
    let src = "ORG 100h\nCALL sub\nMOV AX, 1234h\nMOV AH, 4Ch\nINT 21h\nsub:\nMOV BX, 5678h\nRET\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    let r = emu.run_to(1000, 0x103); // return address after CALL
    assert_eq!(r.steps, 3, "CALL + callee body + RET");
    assert_eq!(emu.pc(), 0x103, "stopped at the instruction after CALL");
    assert_eq!(reg(&emu.regs(), "BX"), 0x5678, "callee ran");
    assert_eq!(reg(&emu.regs(), "AX"), 0, "caller code after the call must not run");
}

#[test]
fn ports_8086_in_out() {
    // fixed-port and DX forms of IN/OUT round-trip; OUT 01h prints AL
    let src = r#"
        ORG 100h
        MOV AL, 42h
        OUT 03h, AL          ; port 3 = 0x42
        MOV AL, 00h
        IN AL, 03h           ; AL = 0x42
        MOV DX, 0004h
        MOV AX, 1234h
        OUT DX, AX           ; ports 4-5 = 34 12
        MOV AX, 0000h
        IN AX, DX            ; AX = 0x1234
        MOV BX, AX           ; save it
        MOV AL, 55h
        OUT DX, AL           ; port 4 = 0x55 (port 5 unchanged)
        MOV AL, 'Q'
        OUT 01h, AL          ; prints 'Q'
        MOV AH, 4Ch
        INT 21h
        END
    "#;
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(1000);
    assert_eq!(reg(&emu.regs(), "BX"), 0x1234, "IN AX,DX after OUT DX,AX");
    assert_eq!(emu.take_output(), "Q", "OUT 01h prints");
    assert_eq!(emu.port_read(3), 0x42);
    assert_eq!(emu.port_read(4), 0x55);
    assert_eq!(emu.port_read(5), 0x12);
}

#[test]
fn ports_8085_in_out() {
    let src = r#"
        ORG 0
        MVI A, 07h
        OUT 05h
        MVI A, 00h
        IN 05h
        MOV B, A
        MVI A, 'K'
        OUT 01h              ; prints 'K'
        HLT
        END
    "#;
    let (regs, out, _) = run_asm("8085", src, 100);
    assert_eq!(reg(&regs, "B"), 0x07, "IN reads back the OUT value");
    assert_eq!(out, "K");
}

#[test]
fn ports_8051_pin_input() {
    // injected pins are visible on port reads (latch | pin)
    let src = r#"
        MOV P1, #00h
        MOV A, P1            ; A = latch(0) | pin(0x55)
        MOV R0, A
        MOV B, P2            ; no injection: 0
        SJMP $
        END
    "#;
    let mut emu = make_emulator("8051").unwrap();
    emu.port_write(1, 0x55);
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "A"), 0x55, "pin state observed on P1 read");
    assert_eq!(reg(&emu.regs(), "R0"), 0x55);
    assert_eq!(reg(&emu.regs(), "B"), 0x00, "P2 has no injection");
}

#[test]
fn ports_8051_latch_without_pins() {
    // MOV Pn,#v writes the latch; reads return it when no pins are driven
    let src = r#"
        MOV P1, #0Fh
        MOV A, P1
        MOV P2, #0AAh
        MOV R1, P2
        SJMP $
        END
    "#;
    let (regs, _, _) = run_asm("8051", src, 100);
    assert_eq!(reg(&regs, "A"), 0x0F);
    assert_eq!(reg(&regs, "R1"), 0xAA);
}

#[test]
fn run_to_bp_breaks_before_target() {
    let src = "ORG 100h\nMOV AX, 1\nMOV BX, 2\nMOV CX, 3\nMOV AH, 4Ch\nINT 21h\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    let r = emu.run_to_bp(1000, &[0x106]); // MOV CX,3
    assert_eq!(r.steps, 2);
    assert_eq!(emu.pc(), 0x106, "stopped before executing the breakpoint");
    assert_eq!(reg(&emu.regs(), "CX"), 0);
    let r2 = emu.run_to_bp(1000, &[]); // resume (bp at PC skipped like a debugger): runs to halt
    assert!(r2.halted);
}

#[test]
fn run_to_bp_empty_set_runs_to_halt() {
    let src = "ORG 0\nMVI A, 01h\nHLT\nEND";
    let mut emu = make_emulator("8085").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    let r = emu.run_to_bp(1000, &[]);
    assert!(r.halted);
    assert_eq!(reg(&emu.regs(), "A"), 1);
}

#[test]
fn hw_intr_8086_nmi() {
    // NMI works even with IF=0 (CLI), vectors through the IVT, IRET returns
    let src = r#"
        ORG 100h
        MOV AX, 1234h
        CLI
        HLT
        ORG 300h
        MOV CX, 5678h
        IRET
        END
    "#;
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.mem_write(0x08, &[0x00, 0x03, 0x00, 0x00]); // vector 02h -> 0000:0300
    emu.set_pc(0x100);
    emu.request_interrupt("NMI", 0).unwrap();
    let r = emu.run(100);
    assert!(r.halted, "ISR ran and IRET returned to HLT");
    assert_eq!(reg(&emu.regs(), "AX"), 0x1234, "instruction before the NMI executed");
    assert_eq!(reg(&emu.regs(), "CX"), 0x5678, "NMI ISR ran despite CLI");
}

#[test]
fn hw_intr_8086_intr_masked() {
    // INTR is ignored while IF=0
    let src = "ORG 100h\nMOV AX, 1234h\nCLI\nHLT\nORG 300h\nMOV CX, 5678h\nIRET\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.mem_write(0x20, &[0x00, 0x03, 0x00, 0x00]); // vector 08h -> 0000:0300
    emu.set_pc(0x100);
    emu.request_interrupt("INTR", 8).unwrap();
    let r = emu.run(100);
    assert!(r.halted);
    assert_eq!(reg(&emu.regs(), "CX"), 0, "INTR must not fire with IF=0");
}

#[test]
fn hw_intr_8086_intr_enabled() {
    // STI enables INTR; ISR pushes FLAGS with IF cleared; IRET restores
    let src = r#"
        ORG 100h
        MOV AX, 1234h
        STI
        HLT
        ORG 300h
        MOV CX, 5678h
        PUSHF
        POP BX              ; BX = FLAGS saved by the CPU (IF cleared)
        IRET
        END
    "#;
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.mem_write(0x20, &[0x00, 0x03, 0x00, 0x00]); // vector 08h -> 0000:0300
    emu.set_pc(0x100);
    emu.request_interrupt("INTR", 8).unwrap();
    let r = emu.run(100);
    assert!(r.halted);
    assert_eq!(reg(&emu.regs(), "CX"), 0x5678, "INTR serviced after STI");
    assert_eq!(reg(&emu.regs(), "BX") & 0x200, 0, "IF was cleared in the pushed FLAGS");
}

#[test]
fn hw_intr_8086_nmi_over_intr_and_snapshot() {
    // NMI wins over a pending INTR; pending state survives snapshot/restore
    let src = "ORG 100h\nMOV AX, 1234h\nCLI\nHLT\nORG 300h\nMOV CX, 5678h\nIRET\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.mem_write(0x08, &[0x00, 0x03, 0x00, 0x00]);
    emu.mem_write(0x20, &[0x00, 0x03, 0x00, 0x00]);
    emu.set_pc(0x100);
    emu.request_interrupt("INTR", 8).unwrap();
    emu.request_interrupt("NMI", 0).unwrap();
    let snap = emu.snapshot();
    emu.restore(&snap);
    emu.step(); // MOV AX executes, then NMI (not INTR) is serviced
    assert_eq!(emu.pc(), 0x300, "NMI takes priority over INTR");
    emu.run(100);
    assert!(emu.is_halted());
    assert_eq!(reg(&emu.regs(), "CX"), 0x5678, "NMI ISR ran after restore");
}

#[test]
fn bcd_adjust_8086() {
    // DAA: 99h + 01h (BCD 99+1 = 100 -> AL=00, CF=1)
    let src = "ORG 100h\nMOV AL, 99h\nADD AL, 01h\nDAA\nHLT\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert!(emu.is_halted());
    assert_eq!(reg(&emu.regs(), "AX"), 0x0000, "DAA: 99+1 -> AL=00");
    assert!(emu.flags().carry, "DAA: CF set for 100");

    // DAA: 35h + 47h (BCD 35+47 = 82 -> no carry)
    let src = "ORG 100h\nMOV AL, 35h\nMOV BL, 47h\nADD AL, BL\nDAA\nHLT\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "AX"), 0x0082, "DAA: 35+47 -> AL=82");
    assert!(!emu.flags().carry);

    // DAS: 82h - 47h (BCD 82-47 = 35, no borrow)
    let src = "ORG 100h\nMOV AL, 82h\nMOV BL, 47h\nSUB AL, BL\nDAS\nHLT\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "AX"), 0x0035, "DAS: 82-47 -> AL=35");
    assert!(!emu.flags().carry);

    // AAA: '4'+'9' = 0x7D -> AL=03, AH=01, CF=1
    let src = "ORG 100h\nMOV AH, 0\nMOV AL, 7Dh\nAAA\nHLT\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "AX"), 0x0103, "AAA: 7Dh -> AH=01 AL=03");
    assert!(emu.flags().carry);

    // AAS: '0'-'9' (0x30-0x39) -> AL=07, AH=FF (borrow)
    let src = "ORG 100h\nMOV AH, 0\nMOV AL, 30h\nSUB AL, 39h\nAAS\nHLT\nEND";
    // 0x30-0x39 = 0xF7 with AF borrow; AAS: AL=0xF7-6=0xF1 & 0x0F = 01, AH=FF
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "AX"), 0xFF01, "AAS: 30h-39h -> AH=FF AL=01");
    assert!(emu.flags().carry);

    // AAM: 53h / 0Ah = 8 rem 3 -> AH=08 AL=03
    let src = "ORG 100h\nMOV AL, 53h\nAAM\nHLT\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "AX"), 0x0803, "AAM: 53h -> AH=08 AL=03");

    // AAD: AH=07 AL=08 -> 7*10+8 = 4Eh, AH=0
    let src = "ORG 100h\nMOV AX, 0708h\nAAD\nHLT\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "AX"), 0x004E, "AAD: 0708h -> AL=4E");
}

#[test]
fn trap_flag_8086() {
    // TF=1 traps every instruction into vector 1 (INT 1); IRET restores TF.
    // SI counts the traps. 5 instructions run after POPF sets TF.
    let src = "ORG 4\nDW isrTrap\nDW 0000h\nORG 100h\nMOV AX, 1111h\nMOV AX, 0100h\nPUSH AX\nPOPF\nMOV AX, 2222h\nMOV BX, 3333h\nMOV CX, 4444h\nMOV DX, 5555h\nMOV AH, 4Ch\nINT 21h\nisrTrap:\nINC SI\nIRET\nEND";
    let mut emu = make_emulator("8086").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.set_pc(0x100);
    emu.run(100);
    assert!(emu.is_halted());
    assert_eq!(reg(&emu.regs(), "SI"), 5, "one INT 1 per instruction with TF set");
}

#[test]
fn serial_rx_8051() {
    // Injecting a byte sets RI -> serial ISR (vector 23h) fires, reads SBUF.
    let src = "ORG 0\nSJMP main\nORG 23h\nLJMP isr\nORG 30h\nmain:\nMOV IE, #90h\nstart:\nSJMP start\nisr:\nMOV R7, SBUF\nCLR RI\nRETI\nEND";
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.serial_rx(b'X').unwrap();
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "R7"), b'X' as u32, "serial ISR captured SBUF");
    assert!(!emu.is_halted(), "program spins in main loop after RETI");
}

#[test]
fn xdata_banking_8051() {
    // XDATA bank (SFR 0xF8) extends MOVX @DPTR beyond 64 KiB: a write to
    // bank 1 does not disturb bank 0 at the same DPTR offset.
    let src = "ORG 0\n\
        MOV DPTR, #1234h\n\
        MOV A, #0AAh\n\
        MOV 0F8h, #1\n\
        MOVX @DPTR, A\n\
        MOV 0F8h, #0\n\
        MOVX A, @DPTR\n\
        MOV R0, A\n\
        MOV 0F8h, #1\n\
        MOVX A, @DPTR\n\
        MOV R1, A\n\
        SJMP $\n\
        END";
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "R0"), 0, "bank 0 is untouched by the bank-1 write");
    assert_eq!(reg(&emu.regs(), "R1"), 0xAA, "bank 1 received the MOVX write");
    assert_eq!(emu.sfr(0xF8), 1, "XPAGE SFR still selects bank 1");
}

#[test]
fn pcon_powerdown_freezes_8051() {
    // PD (PCON.1): oscillator stopped, so timers do NOT tick even with TRx set.
    let src = "ORG 0\n\
        MOV TMOD, #01h\n\
        MOV TH0, #0FEh\n\
        MOV TL0, #0FFh\n\
        SETB TR0\n\
        ORL 87h, #2\n\
        SJMP $\n\
        END";
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.run(200);
    assert_eq!(emu.sfr(0x88) & 0x20, 0, "TF0 must not set while in power-down");
}

#[test]
fn pcon_idle_ticks_timers_8051() {
    // IDL (PCON.0): core gated but peripherals run, so Timer 0 still overflows.
    let src = "ORG 0\n\
        MOV TMOD, #01h\n\
        MOV TH0, #0FFh\n\
        MOV TL0, #0FFh\n\
        SETB TR0\n\
        ORL 87h, #1\n\
        SJMP $\n\
        END";
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.run(100);
    assert_ne!(emu.sfr(0x88) & 0x20, 0, "TF0 sets during idle (peripherals keep running)");
}

#[test]
fn pcon_idle_wakes_on_intr_8051() {
    // An enabled interrupt wakes the idle CPU (IDL cleared on entry) and runs.
    let src = "ORG 0\nSJMP main\nORG 0Bh\nINC R7\nRETI\nORG 30h\nmain:\n\
        MOV R7, #0\nMOV TMOD, #01h\nMOV TH0, #0FFh\nMOV TL0, #0FFh\n\
        SETB TR0\nMOV IE, #82h\nORL 87h, #1\nSJMP $\nEND";
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.run(200);
    assert!(reg(&emu.regs(), "R7") > 0, "timer interrupt fires and wakes idle mode");
}

#[test]
fn serial_baud_delay_8051() {
    // With Timer 1 running as the baud generator, TI (and the emitted char) is
    // deferred by the frame time rather than set instantly.
    let src = "ORG 0\n\
        MOV TMOD, #22h\n\
        MOV TH1, #0FDh\n\
        SETB TR1\n\
        MOV SBUF, #'H'\n\
        MOV R0, SCON\n\
        SJMP $\n\
        END";
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.run(5); // fewer steps than the full frame, so TX not yet complete
    assert_eq!(reg(&emu.regs(), "R0") & 0x02, 0, "TI still pending mid-frame");
    assert_eq!(emu.take_output(), "", "char not emitted before the frame finishes");
    emu.run(2000); // let the frame finish
    assert_eq!(emu.take_output(), "H", "char emitted after TX frame completes");
}

#[test]
fn pusha_popa_8086() {
    // All GP registers pushed, scrambled, then restored by POPA (SP excluded).
    let src = r#"
        ORG 100h
        MOV AX, 1111h
        MOV BX, 2222h
        MOV CX, 3333h
        MOV DX, 4444h
        MOV BP, 5555h
        MOV SI, 6666h
        MOV DI, 7777h
        MOV SP, 8000h
        PUSHA
        MOV AX, 0AAAAh
        MOV BX, 0BBBBh
        MOV CX, 0CCCCh
        MOV DX, 0DDDDh
        MOV BP, 0EEEEh
        MOV SI, 0FFFFh
        MOV DI, 09999h
        POPA
        HLT
    END
    "#;
    let (regs, _, _) = run_asm("8086", src, 1000);
    assert_eq!(reg(&regs, "AX"), 0x1111);
    assert_eq!(reg(&regs, "BX"), 0x2222);
    assert_eq!(reg(&regs, "CX"), 0x3333);
    assert_eq!(reg(&regs, "DX"), 0x4444);
    assert_eq!(reg(&regs, "BP"), 0x5555);
    assert_eq!(reg(&regs, "SI"), 0x6666);
    assert_eq!(reg(&regs, "DI"), 0x7777);
    assert_eq!(reg(&regs, "SP"), 0x8000, "POPA restores SP to its pre-PUSHA value");
}

#[test]
fn fs_gs_arpl_8086() {
    // FS/GS segment overrides populate the right physical address.
    let src = r#"
        ORG 100h
        MOV AX, 1000h
        MOV FS, AX
        MOV AX, 2000h
        MOV GS, AX
        MOV AX, 0ABCDh
        MOV FS:[0200h], AX
        MOV BX, GS:[0200h]   ; GS:0200 = 20200, different page from FS:0200
        MOV CX, FS:[0200h]
        HLT
    END
    "#;
    let (regs, _, _) = run_asm("8086", src, 1000);
    assert_eq!(reg(&regs, "CX"), 0xABCD, "FS: override wrote the right physical byte");
    assert_ne!(reg(&regs, "BX"), 0xABCD, "GS: uses a different segment");
    assert_eq!(reg(&regs, "FS"), 0x1000);
    assert_eq!(reg(&regs, "GS"), 0x2000);

    // ARPL raises the dest RPL to the source RPL and sets ZF.
    let src2 = r#"
        ORG 100h
        MOV AX, 0003h     ; source RPL = 3
        MOV BX, 0001h     ; dest RPL = 1
        ARPL BX, AX       ; dest RPL raised to 3
        HLT
    END
    "#;
    let (regs, _, _) = run_asm("8086", src2, 1000);
    assert_eq!(reg(&regs, "BX"), 0x0003, "ARPL raised dest RPL to source RPL");
}#[test]
fn int0_edge_8051_fires_once() {
    // Edge-triggered (IT0=1 set before IE enables): one request -> one
    // service, IE0 latch cleared. If IT0 were still 0 (level) the held line
    // would re-trigger, so we set IT0 first.
    let src = r#"
        ORG 0
        SJMP main
        ORG 3
        isr: INC R0
        RETI
        ORG 30h
        main:
        SETB IT0
        MOV IE, #81h
        loop: SJMP loop
        END
    "#;
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.request_interrupt("INT0", 0).unwrap();
    emu.run(200);
    assert_eq!(reg(&emu.regs(), "R0"), 1, "edge mode fires INT0 exactly once");
}

#[test]
fn int0_level_8051_reasserts() {
    // Level-triggered (IT0=0): the held line re-asserts after RETI.
    let src = r#"
        ORG 0
        SJMP main
        ORG 3
        isr: INC R0
        RETI
        ORG 30h
        main:
        MOV IE, #81h
        CLR IT0
        loop: SJMP loop
        END
    "#;
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.request_interrupt("INT0", 0).unwrap();
    emu.run(200);
    assert!(reg(&emu.regs(), "R0") > 1, "level mode re-asserts INT0 while the line is held");
}

#[test]
fn sid_sod_8085() {
    // set_sid feeds the RIM SID bit; SIM with A=80h drives the SOD pin.
    let mut emu = make_emulator("8085").unwrap();
    emu.set_sid(true);
    let src = r#"
        RIM
        MVI A, 80h
        SIM
        HLT
    END
    "#;
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(100);
    assert_eq!(reg(&emu.regs(), "A"), 0x80, "RIM reads the SID pin into bit 7");
    assert_eq!(emu.sod(), 1, "SIM with SOD bit set drives the SOD output pin");
}

#[test]
fn fpu_basic_8086() {
    // x87: FLD1, FADD (ST0,ST1), FSTP to memory; FCOM equality via FSTSW AX.
    let mut emu = make_emulator("8086").unwrap();
    let src = r#"
        ORG 100h
        FINIT
        FLD1                 ; ST0 = 1.0
        FLD1                 ; ST0 = 1.0, ST1 = 1.0
        FADD                 ; ST0 = ST0 + ST1 = 2.0
        FSTP QWORD PTR [0200h] ; store 2.0 (double) to memory
        FLD1                 ; ST0 = 1.0
        FLDZ                 ; ST0 = 0.0, ST1 = 1.0
        FCOM                 ; compare ST0 (0.0) with ST1 (1.0)
        FSTSW AX             ; AX = status word (C0 set => ST0 < ST1)
        MOV AX, 4C00h
        INT 21h
        END
    "#;
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(200);
    let bytes = emu.mem_read(0x200, 8);
    let d = f64::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert!((d - 2.0).abs() < 1e-9, "FADD+FLD1 produced 2.0 (got {d})");
}

#[test]
fn fpu_math_8086() {
    // FLD mem, FMUL, FSUBR, FDIV, FCOM + pop -> verify ordering of condition codes.
    let mut emu = make_emulator("8086").unwrap();
    let src = r#"
        ORG 100h
        FINIT
        FLD QWORD PTR [0300h]  ; x = 6.0
        FLD QWORD PTR [0308h]  ; y = 2.0  -> ST0=2, ST1=6
        FDIVR                  ; ST0 = ST1 / ST0 = 6 / 2 = 3.0
        FSTP QWORD PTR [0310h] ; store 3.0
        ; compare 5.0 (ST0 after FLD) with 3.0
        FLD QWORD PTR [0318h]  ; 5.0
        FLD QWORD PTR [0310h]  ; 3.0  -> ST0=3, ST1=5
        FCOM
        FSTSW AX
        MOV AX, 4C00h
        INT 21h
        ORG 0300h
        DQ 6.0
        DQ 2.0
        DQ 5.0
        END
    "#;
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(200);
    let bytes = emu.mem_read(0x310, 8);
    let d = f64::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert!((d - 3.0).abs() < 1e-9, "FDIVR produced 3.0 (got {d})");
}

#[test]
fn dos_file_write_read_8086() {
    // INT 21h 3Ch/40h/3Eh write a file; 3Dh/3Fh read it back.
    let mut emu = make_emulator("8086").unwrap();
    emu.fs_put("OUT.TXT", b"").unwrap();
    let src = "ORG 100h\nMOV AH, 3Ch\nMOV CX, 0\nMOV DX, 0300h\nINT 21h\nMOV BX, AX\nMOV AH, 40h\nMOV CX, 2\nMOV DX, 0308h\nINT 21h\nMOV AH, 3Eh\nINT 21h\nMOV AX, 4C00h\nINT 21h\nORG 300h\nfname: DB 'OUT.TXT', 0\nbuf: DB 'HI', 0\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(emu.fs_get("OUT.TXT").unwrap().unwrap(), b"HI", "file written via INT 21h 3Ch/40h");

    // read a preloaded file (fresh state)
    let mut emu = make_emulator("8086").unwrap();
    emu.fs_put("IN.TXT", b"RD").unwrap();
    let src2 = "ORG 100h\nMOV AH, 3Dh\nMOV AL, 0\nMOV DX, 0300h\nINT 21h\nMOV BX, AX\nMOV AH, 3Fh\nMOV CX, 2\nMOV DX, 0308h\nINT 21h\nMOV AX, 4C00h\nINT 21h\nORG 300h\nfname: DB 'IN.TXT', 0\ndst: DB 0, 0\nEND";
    let code2 = emu.assemble(src2).unwrap();
    emu.mem_write(0, &code2);
    emu.set_pc(0x100);
    emu.run(100);
    let bytes = emu.mem_read(0x308, 2);
    assert_eq!(bytes, vec![b'R', b'D'], "file read via INT 21h 3Dh/3Fh");
}

#[test]
fn dos_clock_and_rtc_8086() {
    let mut emu = make_emulator("8086").unwrap();
    emu.set_clock(2025, 6, 15, 13, 30, 45).unwrap();

    // INT 21h 2Ah get date
    let code = emu.assemble("ORG 100h\nMOV AH, 2Ah\nINT 21h\nMOV AX, 4C00h\nINT 21h\nEND").unwrap();
    emu.mem_write(0, &code); emu.set_pc(0x100); emu.run(100);
    assert_eq!(reg(&emu.regs(), "CX"), 2025, "INT 21h 2Ah year");
    let dx = reg(&emu.regs(), "DX");
    assert_eq!((dx >> 8) as u8, 6, "month");
    assert_eq!((dx & 0xFF) as u8, 15, "day");

    // INT 21h 2Ch get time
    emu.reset(); emu.set_clock(2025, 6, 15, 13, 30, 45).unwrap();
    let code = emu.assemble("ORG 100h\nMOV AH, 2Ch\nINT 21h\nMOV AX, 4C00h\nINT 21h\nEND").unwrap();
    emu.mem_write(0, &code); emu.set_pc(0x100); emu.run(100);
    assert_eq!(reg(&emu.regs(), "CX"), (13u32 << 8) | 30, "INT 21h 2Ch hour:min");
    assert_eq!(reg(&emu.regs(), "DX") >> 8, 45, "INT 21h 2Ch sec");

    // INT 1Ah 00 read RTC time (BCD)
    emu.reset(); emu.set_clock(2025, 6, 15, 13, 30, 45).unwrap();
    let code = emu.assemble("ORG 100h\nMOV AH, 00h\nINT 1Ah\nMOV AX, 4C00h\nINT 21h\nEND").unwrap();
    emu.mem_write(0, &code); emu.set_pc(0x100); emu.run(100);
    assert_eq!((reg(&emu.regs(), "CX") >> 8) as u8, 0x13, "RTC hour BCD");
    assert_eq!((reg(&emu.regs(), "CX") & 0xFF) as u8, 0x30, "RTC min BCD");
    assert_eq!((reg(&emu.regs(), "DX") >> 8) as u8, 0x45, "RTC sec BCD");

    // INT 1Ah 04 read RTC date (BCD)
    emu.reset(); emu.set_clock(2025, 6, 15, 13, 30, 45).unwrap();
    let code = emu.assemble("ORG 100h\nMOV AH, 04h\nINT 1Ah\nMOV AX, 4C00h\nINT 21h\nEND").unwrap();
    emu.mem_write(0, &code); emu.set_pc(0x100); emu.run(100);
    assert_eq!((reg(&emu.regs(), "CX") >> 8) as u8, 0x20, "RTC century BCD");
    assert_eq!((reg(&emu.regs(), "CX") & 0xFF) as u8, 0x25, "RTC year BCD");
    assert_eq!((reg(&emu.regs(), "DX") >> 8) as u8, 0x06, "RTC month BCD");
    assert_eq!((reg(&emu.regs(), "DX") & 0xFF) as u8, 0x15, "RTC day BCD");
}

#[test]
fn dos_stdio_8086() {
    // INT 21h 40h to handle 1 (stdout) prints to Output; handle 2 (stderr) too.
    let mut emu = make_emulator("8086").unwrap();
    let src = "ORG 100h\nMOV AH, 40h\nMOV BX, 1\nMOV CX, 2\nMOV DX, 0200h\nINT 21h\nMOV AX, 4C00h\nINT 21h\nORG 200h\nmsg: DB 'HI'\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(emu.take_output(), "HI", "INT 21h 40h handle 1 writes to stdout");

    // handle 2 (stderr)
    let mut emu = make_emulator("8086").unwrap();
    let src = "ORG 100h\nMOV AH, 40h\nMOV BX, 2\nMOV CX, 3\nMOV DX, 0200h\nINT 21h\nMOV AX, 4C00h\nINT 21h\nORG 200h\nmsg: DB 'ERR'\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert_eq!(emu.take_output(), "ERR", "INT 21h 40h handle 2 writes to stderr");

    // INT 21h 3Fh from handle 0 (stdin) reads the keyboard queue, blocking when empty.
    let mut emu = make_emulator("8086").unwrap();
    let src = "ORG 100h\nMOV AH, 3Fh\nMOV BX, 0\nMOV CX, 3\nMOV DX, 0300h\nINT 21h\nMOV AX, 4C00h\nINT 21h\nORG 300h\nbuf: DB 0, 0, 0\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x100);
    emu.run(100);
    assert!(emu.waiting_input(), "3Fh handle 0 with empty buffer blocks on input");
    emu.push_key(b'A');
    emu.push_key(b'B');
    emu.push_key(b'C');
    emu.run(100);
    assert!(!emu.waiting_input(), "input satisfied, no longer blocked");
    assert_eq!(emu.mem_read(0x300, 3), vec![b'A', b'B', b'C'], "stdin bytes read into buffer");
}

#[test]
fn timer_mode0_8051() {
    // TMOD mode 0: 13-bit timer (TL0 low 5 bits + TH0 high 8 bits).
    let src = r#"
        ORG 30h
        MOV TMOD, #00h
        MOV TL0, #1Fh       ; low 5 bits = 0x1F
        MOV TH0, #0FFh      ; high 8 bits = 0xFF -> count = 0x1FFF (about to wrap)
        SETB TR0
        MOV R0, #00h        ; one tick wraps 0x1FFF -> 0x0000 and sets TF0
        JNB TF0, no
        MOV R1, #1
        SJMP done
    no:
        MOV R1, #0
    done:
        SJMP $
        END
    "#;
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0x30);
    emu.run(5); // exactly enough to arm TR0 and tick once into overflow
    // After the 13-bit counter rolls 0x1FFF -> 0x0000: TF0 must be set and the
    // latched TH0/TL0 must read back as 0 (low 5 bits of TL0 are the counter).
    let sfr = |a: u8| emu.sfr(a);
    assert!(sfr(0x88) & 0x20 != 0, "TF0 set after 13-bit wrap");
    assert_eq!(sfr(0x8A) & 0x1F, 0, "TL0 low 5 bits wrap to 0 (mode 0)");
    assert_eq!(sfr(0x8C), 0, "TH0 wraps to 0 (mode 0)");
}

#[test]
fn timer_mode3_8051() {
    // Timer 0 mode 3: TL0 and TH0 are two independent 8-bit timers (TH0 gated by
    // TR1); timer 1 is halted in mode 3.
    let src = r#"
        ORG 30h
        MOV TMOD, #33h      ; T0 mode 3, T1 mode 3
        MOV TL0, #0FEh
        MOV TH0, #0FEh
        MOV TL1, #00h       ; T1 must NOT count in mode 3
        SETB TR0
        SETB TR1
        MOV A, #00h         ; a few ticks: TL0 FE->FF->00 (TF0), TH0 FE->FF->00 (TF1)
        MOV B, #00h
        MOV R2, #00h
        MOV R3, #00h
        JNB TF0, n0
        MOV R0, #1
    n0:
        JNB TF1, n1
        MOV R1, #1
    n1:
        SJMP $
        END
    "#;
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(200);
    assert_eq!(reg(&emu.regs(), "R0"), 1, "mode 3 TL0 overflow sets TF0");
    assert_eq!(reg(&emu.regs(), "R1"), 1, "mode 3 TH0 overflow sets TF1");
    let sfr = |a: u8| emu.sfr(a);
    assert_eq!(sfr(0x8B), 0, "timer 1 frozen in mode 3 (TL1 unchanged)");
}

#[test]
fn movc_8051() {
    // MOVC A,@A+DPTR and MOVC A,@A+PC read from code (ROM) space.
    let src = r#"
        ORG 30h
        MOV DPTR, #0100h
        MOV A, #05h
        MOVC A, @A+DPTR     ; A = code[0x105] = 0x66
        MOV R0, A
        MOV A, #03h
        MOVC A, @A+PC       ; A = byte 3 past the MOVC = 0xAB (the DB)
        MOV R1, A
        SJMP $
        DB 0ABh
        ORG 0100h
        DB 11h, 22h, 33h, 44h, 55h, 66h, 77h
        END
    "#;
    let mut emu = make_emulator("8051").unwrap();
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(200);
    assert_eq!(reg(&emu.regs(), "R0"), 0x66, "MOVC A,@A+DPTR reads code space");
    assert_eq!(reg(&emu.regs(), "R1"), 0xAB, "MOVC A,@A+PC reads following byte");
}


