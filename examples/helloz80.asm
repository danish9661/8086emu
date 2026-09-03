; Z80 hello world: print chars via OUT to port 1
; (the emulator maps port 1 to the output console).
    ORG 0
    LD A, 'H'
    OUT (1), A
    LD A, 'i'
    OUT (1), A
    LD A, 10
    OUT (1), A
    HALT
    END
