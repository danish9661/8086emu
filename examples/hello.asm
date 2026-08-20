; 8086 hello world via DOS INT 21h
ORG 100h
MOV DX, OFFSET msg
MOV AH, 09h
INT 21h
MOV AH, 4Ch
INT 21h
msg: DB 'Hello, 8086!$'
END