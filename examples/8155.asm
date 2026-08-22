; 8085 + Intel 8155 demo (timing-accurate).
; 8155 is mapped at I/O 0x80..0x85 and RAM at 0x8000..0x80FF.
; Here we load timer count 4 into the 8155 timer, start it in
; square-wave mode, and continuously drive port A. The 8155 timer is
; clocked by the CPU T-states, so emu.cycles() tracks real time.
;
;   cmd = 10110001b = 0B1h
;     D7=1 run timer, D6,D5=01 square-wave, D4=1 load count,
;     D3,D2=00 PC input, D1=0 PB input, D0=1 PA output
        ORG 0
        MVI A, 04h
        OUT 84h          ; timer count low
        MVI A, 00h
        OUT 85h          ; timer count high
        MVI A, 0B1h
        OUT 80h          ; command: load + start square-wave timer, PA out
loop:   MVI A, 55h
        OUT 81h          ; drive port A (RAM at 0x8000 is also accessible)
        JMP loop
