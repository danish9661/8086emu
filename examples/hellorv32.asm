; RV32 hello world via the tiny ECALL ABI
; (a7 = 64 write fd/a1/a2, a7 = 93 exit), then halt.
    ORG 0
    ADDI a1, x0, 0x100   ; pointer to message
    ADDI a2, x0, 3       ; length
    ADDI a7, x0, 64      ; syscall: write
    ECALL
    ADDI a7, x0, 93      ; syscall: exit
    ECALL
    ORG 0x100
    DB 'H','i',10
    END
