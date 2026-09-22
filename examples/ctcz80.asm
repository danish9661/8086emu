; Z80 CTC demo: channel 0 as a timer interrupt (ports 10h-13h).
; Channel 0 fires every 2 x 16 = 32 instructions; the ISR (IM 1 vector 38h)
; prints 'C' while the main loop prints '.'. Expect a mix like "..C.....".
; Run headlessly: cargo run --example run -- --isa z80 examples/ctcz80.asm
    ORG 0
    JP main
    ORG 38h
isr:
    LD A, 'C'
    OUT (1), A
    RETI
main:
    EI
    LD A, 0C5h
    OUT (10h), A       ; ch0 ctrl: IE + timer/16 + time-constant follows
    LD A, 2
    OUT (10h), A       ; time constant 2 -> interrupt every 32 steps
    LD B, 40
loop:
    LD A, '.'
    OUT (1), A
    DJNZ loop
    HALT
    END
