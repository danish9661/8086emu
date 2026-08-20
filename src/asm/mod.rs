//! Assembler entry points for all three ISAs.

pub mod asm8051;
pub mod asm8085;
pub mod asm8086;
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