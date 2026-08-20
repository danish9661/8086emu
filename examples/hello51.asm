; 8051 hello world: write chars to SBUF (serial output)
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