; 8051 peripheral demo: 8255 PPI + ADC0808 + LCD1602 + 8237 DMA.
; The 8051 has no OUT instruction; the same kit peripherals live in the top
; 256 bytes of XDATA, so MOVX @DPTR reaches port P as DPTR = 0FF00h + P
; (see examples/ports86.asm for the port map).
; Run headlessly: cargo run --example run -- --isa 8051 examples/ports51.asm
    ORG 0
; ---- 8255 PPI: all outputs, drive PA/PB, read PA back into R0 ----
    MOV DPTR, #0FFE3h
    MOV A, #080h
    MOVX @DPTR, A      ; PPI ctrl: all ports output
    MOV DPTR, #0FFE0h
    MOV A, #055h
    MOVX @DPTR, A      ; PA = 55h
    MOV DPTR, #0FFE1h
    MOV A, #0AAh
    MOVX @DPTR, A      ; PB = AAh
    MOV DPTR, #0FFE0h
    MOVX A, @DPTR
    MOV R0, A          ; R0 = PA read back (55h)
; ---- ADC0808: START channel 0, status into R1, data into R2 ----
    MOV DPTR, #0FF28h
    MOV A, #080h       ; D7=START, channel 0
    MOVX @DPTR, A
    NOP                ; one step lets EOC set
    MOVX A, @DPTR
    MOV R1, A          ; R1 = status (D7=EOC)
    MOV DPTR, #0FF29h
    MOVX A, @DPTR
    MOV R2, A          ; R2 = conversion result (default CH0 = 00h)
; ---- HD44780 LCD: clear, then write "Hi" ----
    MOV DPTR, #0FF38h
    MOV A, #001h
    MOVX @DPTR, A      ; clear display
    MOV DPTR, #0FF39h
    MOV A, #'H'
    MOVX @DPTR, A
    MOV A, #'i'
    MOVX @DPTR, A
; ---- 8237 DMA: CH0 addr=2000h count=0003h, unmask, status into R3 ----
    MOV DPTR, #0FFD0h
    MOV A, #000h
    MOVX @DPTR, A      ; CH0 addr LSB
    MOV DPTR, #0FFD1h
    MOV A, #020h
    MOVX @DPTR, A      ; CH0 addr MSB (base 2000h)
    MOV DPTR, #0FFD2h
    MOV A, #003h
    MOVX @DPTR, A      ; CH0 count LSB (4 transfers)
    MOV DPTR, #0FFD3h
    MOV A, #000h
    MOVX @DPTR, A      ; CH0 count MSB
    MOV DPTR, #0FFDEh
    MOV A, #000h
    MOVX @DPTR, A      ; single mask: ch0 unmask
    MOV DPTR, #0FFDFh
    MOVX @DPTR, A      ; mode: ch0 demand (A still 00h)
    MOV DPTR, #0FFD8h
    MOVX A, @DPTR
    MOV R3, A          ; R3 = DMA status
    SJMP $
    END
