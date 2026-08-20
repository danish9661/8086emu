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
    let src = "ORG 0\nSJMP main\nORG 03h\nRETI\nORG 30h\nmain:\nMOV IE, #81h\nstart:\nSJMP start\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.request_interrupt("INT0", 0).unwrap();
    emu.run(2); // dispatch: PC = 03h, SP pushed 2 bytes
    assert_eq!(reg(&emu.regs(), "SP"), 9, "dispatch must push two bytes");
    assert_eq!(emu.pc(), 0x03);
    emu.run(1); // RETI
    assert_eq!(emu.pc(), 0x33, "RETI must return to the address after MOV IE (PCL pushed first)");
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
    let src = "ORG 0\nSJMP main\nORG 03h\nMOV SBUF, #'0'\nRETI\nORG 0Bh\nPUSH ACC\nNOP\nNOP\nPOP ACC\nRETI\nORG 30h\nmain:\nMOV TMOD, #01h\nMOV TH0, #0FFh\nMOV TL0, #0FFh\nSETB TR0\nMOV IP, #02h\nMOV IE, #83h\nstart:\nSJMP start\nEND";
    let code = emu.assemble(src).unwrap();
    emu.mem_write(0, &code);
    emu.set_pc(0);
    emu.run(9); // inside the TF0 ISR (after NOP at 0x0E); no INT0 requested yet
    assert_eq!(emu.pc(), 0x0E, "TF0 (higher natural priority, no INT0 pending) must be in service");
    emu.request_interrupt("INT0", 0).unwrap();
    emu.run(2); // NOP, POP ACC -> INT0 must NOT preempt the high-priority TF0 ISR
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
