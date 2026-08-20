//! Headless CLI: assemble + run an assembly file, print registers/output.
//!
//!     cargo run --example run -- examples/hello.asm           # 8086
//!     cargo run --example run -- --isa 8085 examples/hello85.asm
//!     cargo run --example run -- --isa 8051 examples/hello51.asm

use multi_cpu_emu::make_emulator;
use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut isa = "8086".to_string();
    let mut path: Option<String> = None;
    let mut max_steps: u32 = 1_000_000;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--isa" => { i += 1; isa = args.get(i).cloned().unwrap_or("8086".into()); }
            "--max-steps" => { i += 1; max_steps = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(1_000_000); }
            "--help" | "-h" => {
                println!("usage: run [--isa 8086|8085|8051] [--max-steps N] <file.asm>");
                return;
            }
            p => path = Some(p.to_string()),
        }
        i += 1;
    }
    let path = path.expect("missing input file");
    let source = std::fs::read_to_string(&path).expect("cannot read file");

    let mut emu = make_emulator(&isa).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1); });

    let code = match emu.assemble(&source) {
        Ok(c) => c,
        Err(e) => { eprintln!("assembly error: {e}"); std::process::exit(1); }
    };

    let origin = if isa == "8086" { 0x100 } else { 0 };
    emu.mem_write(origin, &code);
    emu.set_pc(origin);

    let r = emu.run(max_steps);
    println!("--- ran {} steps (halted: {}) ---", r.steps, r.halted);
    for reg in emu.regs() {
        println!("  {} = {:04X}", reg.name, reg.value & 0xFFFF);
    }
    let f = emu.flags();
    let fstr = [
        ("CY", f.carry), ("PF", f.parity), ("AF", f.aux), ("ZF", f.zero),
        ("SF", f.sign), ("OF", f.overflow), ("DF", f.direction), ("IF", f.interrupt),
    ];
    println!("  flags: {}", fstr.iter().filter(|(_, s)| *s).map(|(n, _)| *n).collect::<Vec<_>>().join(" "));
    let out = emu.take_output();
    if !out.is_empty() {
        println!("--- program output ---");
        print!("{out}");
        if !out.ends_with('\n') { println!(); }
    }
}