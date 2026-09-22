; 6502 VIA demo: timer T1 (one-shot) raises IRQ through the 6522 at $6000.
; The ISR clears the T1 flag, counts the interrupt in $41, and returns;
; the main program then stores the IFR snapshot in $40. Expect $41 = 1.
; ($6000-$600F is reserved for VIA registers; keep code and data clear of it.)
; Run headlessly: cargo run --example run -- --isa 6502 examples/via6522.asm
    ORG 0
    SEI
    LDA #$05
    STA $6006          ; T1 latch low = 5
    LDA #$00
    STA $6007          ; T1 latch high
    LDA #$C0
    STA $600E          ; IER set: enable T1 interrupt
    LDA #$05
    STA $6004          ; T1C-L (latch low again, harmless)
    LDA #$00
    STA $6005          ; T1C-H: counter = 5, start, clear flag
    CLI
    LDX #30
wait:
    DEX
    BNE wait           ; burn steps while T1 fires once
    SEI
    LDA $600D
    STA $40            ; IFR snapshot (T1 flag cleared by the ISR -> $00)
done:
    JMP done           ; spin here (6502 has no HLT; BRK would re-vector)
isr:
    LDA #$40
    STA $600D          ; IFR clear: acknowledge T1 (bit 7 = 0)
    INC $41            ; count this interrupt
    RTI
    ORG $FFFE
    DW isr             ; IRQ vector
    END
