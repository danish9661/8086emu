; 6502 hello world: print chars by storing A to I/O port $01
; (the emulator maps STA $01 to the output console, like OUT 01h on 8085).
; BRK vectors through $FFFE (all zeros here, so it jumps to $0000 and spins);
; the printed output is what matters.
    ORG 0
    LDX #0
loop:
    LDA msg,X
    BEQ done
    STA $01
    INX
    JMP loop
done:
    BRK
msg: DB 'H','i',10,0
    END
