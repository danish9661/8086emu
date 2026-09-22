; 8086 PC speaker demo (port 61h + PIT channel 2).
; Channel 2 runs in mode 3 (square wave); port 61h bit 0 gates the timer
; and bit 1 enables the speaker (level = channel 2 OUT while enabled).
; Run headlessly: cargo run --example run -- examples/speaker86.asm
; or open the web IDE Devices tab to watch the speaker level.
    ORG 100h
    MOV AL, 0B6h
    OUT 43h, AL        ; PIT cmd: channel 2, LSB+MSB, mode 3, binary
    MOV AX, 0533h      ; count 1331 = ~896 Hz (1193182 / 1331)
    OUT 42h, AL        ; channel 2 count LSB
    MOV AL, AH
    OUT 42h, AL        ; channel 2 count MSB
    IN AL, 61h
    OR AL, 03h
    OUT 61h, AL        ; gate + enable: speaker follows channel 2 OUT
    MOV CX, 0FFFFh
beep:
    LOOP beep          ; hold the tone for a while
    IN AL, 61h
    AND AL, 0FCh
    OUT 61h, AL        ; gate off + disable: silent
    MOV AH, 4Ch
    INT 21h
    END
