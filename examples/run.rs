//! Headless CLI: assemble + run (or grade) an assembly file.
//!
//! Run a program and print registers/flags/output:
//!     cargo run --example run -- examples/hello.asm
//!     cargo run --example run -- --isa 8085 examples/hello85.asm
//!
//! Grade a program against a spec file:
//!     cargo run --example run -- --grade spec.txt examples/prog.asm
//!
//! The spec file is line-oriented (blank lines and `;`/`#` comments ignored):
//!     out   expected.txt        ; program output must match this file
//!     reg   AX      0x1234       ; register compare (decimal or 0x ok)
//!     mem   0x200   0xAB         ; memory byte compare
//!     steps 1000                ; must finish within this many steps (optional)
//!
//! Exit code is 0 when every check passes, 1 otherwise.

use multi_cpu_emu::make_emulator;
use std::env;

struct Check {
    kind: String,
    target: String,
    want: u32,
}

fn parse_value(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(h, 16).ok()
    } else if s.ends_with('h') || s.ends_with('H') {
        u32::from_str_radix(&s[..s.len() - 1], 16).ok()
    } else if s.ends_with('b') || s.ends_with('B') {
        u32::from_str_radix(&s[..s.len() - 1], 2).ok()
    } else {
        s.parse::<u32>().ok().or_else(|| u32::from_str_radix(s, 16).ok())
    }
}

fn load_spec(path: &str) -> Vec<Check> {
    let txt = std::fs::read_to_string(path).unwrap_or_default();
    let mut checks = Vec::new();
    for line in txt.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let kind = it.next().unwrap_or("").to_lowercase();
        match kind.as_str() {
            "out" => {
                if let Some(f) = it.next() {
                    checks.push(Check { kind: "out".into(), target: f.into(), want: 0 });
                }
            }
            "reg" => {
                let name = it.next().unwrap_or("").to_string();
                let val = it.next().and_then(parse_value).unwrap_or(0);
                checks.push(Check { kind: "reg".into(), target: name.to_uppercase(), want: val });
            }
            "mem" => {
                let addr = it.next().and_then(parse_value).unwrap_or(0);
                let val = it.next().and_then(parse_value).unwrap_or(0);
                checks.push(Check { kind: "mem".into(), target: format!("{addr}"), want: val });
            }
            "steps" => {
                let val = it.next().and_then(parse_value).unwrap_or(0);
                checks.push(Check { kind: "steps".into(), target: String::new(), want: val });
            }
            _ => {}
        }
    }
    checks
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut isa = "8086".to_string();
    let mut path: Option<String> = None;
    let mut grade: Option<String> = None;
    let mut max_steps: u32 = 5_000_000;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--isa" => { i += 1; isa = args.get(i).cloned().unwrap_or("8086".into()); }
            "--max-steps" => { i += 1; max_steps = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(5_000_000); }
            "--grade" => { i += 1; grade = args.get(i).cloned(); }
            "--help" | "-h" => {
                println!("usage: run [--isa 8086|8085|8051] [--max-steps N] [--grade spec.txt] <file.asm>");
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
    let entry = if isa == "8086" { 0x100 } else { 0 };
    emu.mem_write(0, &code);
    emu.set_pc(entry);

    let res = emu.run(max_steps);

    if grade.is_none() {
        print_run(&mut emu, &res);
        return;
    }

    // ---- grading mode ----
    let spec_path = grade.unwrap();
    let checks = load_spec(&spec_path);
    let out = emu.take_output();
    let mut failed = 0;
    let mut tested = 0;

    for c in &checks {
        tested += 1;
        let ok = match c.kind.as_str() {
            "out" => {
                let expected = std::fs::read_to_string(&c.target).unwrap_or_default();
                out == expected
            }
            "reg" => emu.regs().iter().any(|r| r.name == c.target && (r.value & 0xFFFF) == c.want),
            "mem" => {
                let addr = c.target.parse::<u32>().unwrap_or(0);
                emu.mem_read(addr, 1).first().copied().unwrap_or(0) as u32 == c.want
            }
            "steps" => (res.steps as u32) <= c.want,
            _ => true,
        };
        if ok {
            println!("PASS  {} {}", c.kind, c.target);
        } else {
            failed += 1;
            let got = match c.kind.as_str() {
                "out" => format!("(len {})", out.len()),
                "reg" => match emu.regs().iter().find(|r| r.name == c.target) {
                    Some(r) => format!("{:04X}", r.value & 0xFFFF),
                    None => "missing".into(),
                },
                "mem" => {
                    let addr = c.target.parse::<u32>().unwrap_or(0);
                    format!("{:02X}", emu.mem_read(addr, 1).first().copied().unwrap_or(0))
                }
                "steps" => format!("{}", res.steps),
                _ => String::new(),
            };
            println!("FAIL  {} {} (want {}, got {})", c.kind, c.target, c.want, got);
        }
    }

    if tested == 0 {
        println!("no checks in spec file {}", spec_path);
        std::process::exit(1);
    }
    if failed > 0 {
        println!("--- {failed}/{tested} checks FAILED ---");
        std::process::exit(1);
    }
    println!("--- all {tested} checks PASSED ({} steps) ---", res.steps);
}

fn print_run(emu: &mut multi_cpu_emu::Emulator, res: &multi_cpu_emu::cpu::RunResult) {
    println!("--- ran {} steps (halted: {}) ---", res.steps, res.halted);
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
