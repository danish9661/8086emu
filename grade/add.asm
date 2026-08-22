ORG 100h
    MOV AX, 5
    MOV BX, 7
    ADD AX, BX        ; AX = 0x000C
    MOV AH, 4Ch       ; AX becomes 0x4C0C
    INT 21h
