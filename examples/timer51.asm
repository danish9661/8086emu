; 8051 timer demo (machine-cycle accurate).
; Timer 0 in mode 1 (16-bit), reloaded each overflow, toggling P1.0.
; Each machine cycle increments the timer; emu.cycles() shows the
; machine-cycle count and the timer fires roughly every 65536 cycles.
        ORG 0
        MOV TMOD, #01h    ; T0 mode 1, 16-bit
        MOV TL0, #0F0h
        MOV TH0, #0FFh
        SETB TR0
wait:   JNB TF0, wait
        CLR TF0
        CPL P1.0
        MOV TL0, #0F0h
        MOV TH0, #0FFh
        SJMP wait
        END
