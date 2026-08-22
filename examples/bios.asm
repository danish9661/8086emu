; 8086 BIOS boot demo. Load this as a ROM image reaching the top of memory
; (set the load address to F000:0000 / 0xF0000) and the CPU will start here
; at the reset vector FFFF:FFF0.
;   emu.set_rom_region(0xF0000, 0x10000)
;   emu.mem_write(0xFFFF0, &[0xEA,0x00,0xF0,0x00,0xF0])  ; JMP FAR F000:F000
;   emu.load_rom(code, 0xF0000)
;   emu.reset()        ; now PC = FFFF0
;   emu.run(...)
    ORG 0F000h
    MOV AL, 'B'
    OUT 01h, AL
    MOV AL, 'I'
    OUT 01h, AL
    MOV AL, 'O'
    OUT 01h, AL
    HLT
    END
