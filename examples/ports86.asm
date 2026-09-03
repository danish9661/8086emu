; 8086 peripheral demo: 8255 PPI + ADC0808 + LCD1602 + 8237 DMA.
; Ports (see src/cpu.rs PORT_* and docs/devices.js):
;   PPI  0E0h-0E3h (PA/PB/PC/ctrl)   ADC 028h ctrl / 029h data
;   LCD  038h cmd / 039h data        DMA 0D0h-0DFh (CH0 D0-D3, mask DE, mode DF, status D8)
; Run headlessly: cargo run --example run -- examples/ports86.asm
; or open the web IDE Devices tab to watch the panels.
    ORG 100h
; ---- 8255 PPI: mode-set 80h = all ports output, then drive PA/PB ----
    MOV AL, 80h
    OUT 0E3h, AL       ; PPI ctrl: PA+PB+PClow+PChigh all output
    MOV AL, 55h
    OUT 0E0h, AL       ; PA = 55h
    MOV AL, 0AAh
    OUT 0E1h, AL       ; PB = AAh
    IN AL, 0E0h
    MOV BL, AL         ; BL = PA read back (55h)
; ---- ADC0808: START convert on channel 0, read status + data ----
    MOV AL, 80h        ; D7=START, D2-D0=channel 0
    OUT 028h, AL
    NOP                ; one step lets EOC set (model converts instantly)
    IN AL, 028h
    MOV BH, AL         ; BH = status (D7=EOC, D6=OE, D2-D0=channel)
    IN AL, 029h
    MOV CL, AL         ; CL = conversion result (default CH0 = 00h)
; ---- HD44780 LCD: clear, then write "Hi" ----
    MOV AL, 01h
    OUT 038h, AL       ; clear display
    MOV AL, 'H'
    OUT 039h, AL
    MOV AL, 'i'
    OUT 039h, AL
; ---- 8237 DMA: program CH0 addr=2000h count=0003h, unmask, read status ----
    MOV AL, 00h
    OUT 0D0h, AL       ; CH0 addr LSB = 00h
    MOV AL, 20h
    OUT 0D1h, AL       ; CH0 addr MSB = 20h  (base 2000h)
    MOV AL, 03h
    OUT 0D2h, AL       ; CH0 count LSB = 03h (4 transfers)
    MOV AL, 00h
    OUT 0D3h, AL       ; CH0 count MSB = 00h
    MOV AL, 00h
    OUT 0DEh, AL       ; single mask: ch0 unmask (D2=0, D1-D0=00)
    MOV AL, 00h
    OUT 0DFh, AL       ; mode: ch0 demand mode
    IN AL, 0D8h
    MOV CH, AL         ; CH = DMA status (TC3..0 + REQ)
    MOV AH, 4Ch
    INT 21h
    END
