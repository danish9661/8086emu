; 8085 peripheral demo: 8255 PPI + ADC0808 + LCD1602 + 8237 DMA.
; Same port map as the 8086 kit (see examples/ports86.asm):
;   PPI  0E0h-0E3h   ADC 028h/029h   LCD 038h/039h   DMA 0D0h-0DFh
; Run headlessly: cargo run --example run -- --isa 8085 examples/ports85.asm
    ORG 0
; ---- 8255 PPI: all outputs, drive PA/PB, read PA back into B ----
    MVI A, 80h
    OUT 0E3h           ; PPI ctrl: all ports output
    MVI A, 55h
    OUT 0E0h           ; PA = 55h
    MVI A, 0AAh
    OUT 0E1h           ; PB = AAh
    IN 0E0h
    MOV B, A           ; B = PA read back (55h)
; ---- ADC0808: START channel 0, read status into C, data into D ----
    MVI A, 80h         ; D7=START, channel 0
    OUT 028h
    NOP                ; one step lets EOC set
    IN 028h
    MOV C, A           ; C = status (D7=EOC)
    IN 029h
    MOV D, A           ; D = conversion result (default CH0 = 00h)
; ---- HD44780 LCD: clear, then write "Hi" ----
    MVI A, 01h
    OUT 038h           ; clear display
    MVI A, 'H'
    OUT 039h
    MVI A, 'i'
    OUT 039h
; ---- 8237 DMA: CH0 addr=2000h count=0003h, unmask, status into E ----
    MVI A, 00h
    OUT 0D0h           ; CH0 addr LSB
    MVI A, 20h
    OUT 0D1h           ; CH0 addr MSB (base 2000h)
    MVI A, 03h
    OUT 0D2h           ; CH0 count LSB (4 transfers)
    MVI A, 00h
    OUT 0D3h           ; CH0 count MSB
    OUT 0DEh           ; single mask: A=00h still, ch0 unmask
    OUT 0DFh           ; mode: ch0 demand
    IN 0D8h
    MOV E, A           ; E = DMA status
    HLT
    END
