//! Assembler entry points for all three ISAs.

pub mod asm8051;
pub mod asm8085;
pub mod asm8086;
pub mod asm6502;
pub mod asmz80;
pub mod asmrv32;
pub mod common;

pub use common::{AsmErr, LineInfo};

pub fn parse_8086(source: &str) -> (Vec<u8>, Vec<AsmErr>, Vec<LineInfo>) {
    asm8086::assemble(source)
}

pub fn parse_8085(source: &str) -> (Vec<u8>, Vec<AsmErr>, Vec<LineInfo>) {
    asm8085::assemble(source)
}

pub fn parse_8051(source: &str) -> (Vec<u8>, Vec<AsmErr>, Vec<LineInfo>) {
    asm8051::assemble(source)
}

pub fn parse_rv32(source: &str) -> (Vec<u8>, Vec<AsmErr>, Vec<LineInfo>) {
    asmrv32::assemble(source)
}

pub fn parse_6502(source: &str) -> (Vec<u8>, Vec<AsmErr>, Vec<LineInfo>) {
    asm6502::assemble(source)
}

pub fn parse_z80(source: &str) -> (Vec<u8>, Vec<AsmErr>, Vec<LineInfo>) {
    asmz80::assemble(source)
}