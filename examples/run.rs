//! Headless CLI: assemble + run (or grade) an assembly file.
//!
//! Run a program and print registers/flags/output:
//!     cargo run --example run -- examples/hello.asm
//!     cargo run --example run -- --isa 8085 examples/hello85.asm
//!
//! Grade a program against a spec file:
//!     cargo run --example run -- --grade spec.txt examples/prog.asm
//!
//! Trace each instruction and peripheral I/O writes:
//!     cargo run --example run -- --verbose examples/hello.asm
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
    } else if let Some(h) = s.strip_prefix("$") {
        u32::from_str_radix(h, 16).ok()
    } else if s.len() > 1 && (s.ends_with('h') || s.ends_with('H')) {
        u32::from_str_radix(&s[..s.len() - 1], 16).ok()
    } else if s.len() > 1 && (s.ends_with('b') || s.ends_with('B')) {
        u32::from_str_radix(&s[..s.len() - 1], 2).ok()
    } else {
        // Decimal only: no implicit-hex fallback (so "10" is always ten).
        s.parse::<u32>().ok()
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
    let mut verbose = false;
    let mut bench_steps: Option<u32> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--isa" => {
                i += 1;
                isa = args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("error: --isa requires a value (8086|8085|8051|6502|Z80|rv32)");
                    std::process::exit(2);
                });
            }
            "--max-steps" => {
                i += 1;
                max_steps = args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                    eprintln!("error: --max-steps requires a positive integer");
                    std::process::exit(2);
                });
            }
            "--grade" => { i += 1; grade = args.get(i).cloned(); }
            "--verbose" | "-v" => { verbose = true; }
            "--bench" => {
                i += 1;
                let n = args.get(i).and_then(|s| s.parse::<u32>().ok());
                bench_steps = Some(n.unwrap_or(10_000_000));
            }
            "--help" | "-h" => { print_usage(); return; }
            p if p.starts_with('-') => { eprintln!("error: unknown option '{p}'"); print_usage(); std::process::exit(2); }
            p => path = Some(p.to_string()),
        }
        i += 1;
    }

    // Create the emulator early so --bench can run without a source file.
    let mut emu = match make_emulator(&isa) {
        Ok(e) => e,
        Err(_) => {
            eprintln!("error: unsupported ISA '{}'. Valid choices: 8086, 8085, 8051, 6502, Z80, rv32", isa);
            std::process::exit(1);
        }
    };

    if let Some(bench) = bench_steps {
        run_bench(&mut emu, &isa, bench);
        return;
    }

    let Some(path) = path else {
        print_usage();
        std::process::exit(0);
    };
    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => { eprintln!("error: cannot read '{}': {}", path, e); std::process::exit(1); }
    };

    let code = match emu.assemble(&source) {
        Ok(c) => c,
        Err(e) => { eprintln!("assembly failed for '{}':\n{}", path, e); std::process::exit(1); }
    };
    let entry = if isa == "8086" { 0x100 } else { 0 };
    emu.mem_write(0, &code);
    emu.set_pc(entry);

    if verbose && grade.is_none() {
        run_verbose(&mut emu, max_steps);
        return;
    }

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

fn print_usage() {
    println!(
        "usage: run [options] <file.asm>\n\
         \n\
         Options:\n\
           --isa <isa>        one of: 8086, 8085, 8051, 6502, Z80, rv32  (default 8086)\n\
           --max-steps <N>   stop after N instructions (default 5,000,000)\n\
           --grade <spec>    grade the program against a spec file (see below)\n\
           --verbose, -v     trace each instruction and peripheral I/O writes\n\
           --bench [N]       measure emulation throughput over N steps (default 10M)\n\
           --help, -h        show this help and exit\n\
         \n\
         Spec file (--grade) is line-oriented; blank lines and ;/# comments ignored:\n\
           out   expected.txt   ; program output must match this file\n\
           reg   AX      0x1234  ; register compare (decimal or 0x ok)\n\
           mem   0x200   0xAB     ; memory byte compare\n\
           steps 1000            ; must finish within this many steps (optional)\n\
         \n\
         Exit code is 0 on success, 1 on failure, 2 on usage error."
    );
}

/// Step the emulator instruction-by-instruction and print the decoded
/// mnemonic plus any peripheral register (port) writes that occur, so the
/// user can see exactly what the program does to hardware.
fn run_verbose(emu: &mut multi_cpu_emu::Emulator, max_steps: u32) {
    use std::collections::HashMap;
    let mut prev: HashMap<u16, u8> = (0..=255).map(|p: u16| (p, emu.port_read(p as u8))).collect();
    let mut steps = 0u32;
    while steps < max_steps && !emu.is_halted() {
        let pc = emu.pc();
        let dis = emu
            .disassemble(pc, 1)
            .first()
            .map(|d| d.text.clone())
            .unwrap_or_else(|| {
                let b = emu.mem_read(pc, 1).first().copied().unwrap_or(0);
                format!("<?? {:02X}>", b)
            });
        emu.step();
        steps += 1;
        let mut changed = Vec::new();
        for p in 0u16..=255u16 {
            let n = emu.port_read(p as u8);
            if n != prev[&p] {
                changed.push((p, n));
                prev.insert(p, n);
            }
        }
        if changed.is_empty() {
            println!("{:06X}  {}", pc, dis);
        } else {
            let s = changed
                .iter()
                .map(|(p, v)| format!("P{:02X}={:02X}", p, v))
                .collect::<Vec<_>>()
                .join(" ");
            println!("{:06X}  {:<28} IO: {}", pc, dis, s);
        }
    }
    println!(
        "--- verbose trace: {} steps, halted={} ---",
        steps,
        emu.is_halted()
    );
}

/// Built-in tight loop per ISA, used to measure steady-state emulation
/// throughput without the program halting.
fn bench_loop(isa: &str) -> String {
    match isa {
        "8085" => "ORG 0\nagain:\nJMP again\nEND\n".to_string(),
        "8051" => "ORG 0\nagain:\nSJMP again\nEND\n".to_string(),
        "6502" => "ORG 0\nagain:\nJMP again\nEND\n".to_string(),
        "z80" => "ORG 0\nagain:\nJR again\nEND\n".to_string(),
        "rv32" => "ORG 0\nagain:\nBEQ x0, x0, again\nEND\n".to_string(),
        _ => "ORG 100h\nagain:\njmp again\nEND\n".to_string(), // 8086
    }
}

/// Assemble a busy loop, run it for `steps` instructions, and report throughput.
fn run_bench(emu: &mut multi_cpu_emu::Emulator, isa: &str, steps: u32) {
    let src = bench_loop(isa);
    let code = match emu.assemble(&src) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: benchmark loop failed to assemble for '{}':\n{}", isa, e);
            std::process::exit(1);
        }
    };
    let entry = if isa == "8086" { 0x100 } else { 0 };
    if isa == "rv32" || isa == "8086" {
        // Real code is read-only; loading as ROM exercises the decode-cache
        // trust fast path (skip the per-step instruction re-fetch/verify).
        emu.load_rom(&code, entry);
    } else {
        emu.mem_write(0, &code);
    }
    emu.set_pc(entry);
    // warm up (JIT/cache settle) before timing
    emu.run(1000);
    let start = std::time::Instant::now();
    let res = emu.run(steps);
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    let rate = if secs > 0.0 { res.steps as f64 / secs } else { 0.0 };
    println!(
        "bench [{}]: {} steps in {:.3} s  =>  {:.0} steps/sec  (halted={})",
        isa,
        res.steps,
        secs,
        rate,
        emu.is_halted()
    );
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
