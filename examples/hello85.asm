; 8085 hello world: print chars via OUT 01h
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
msg: DB 'Hello, 8085!', '$'
END