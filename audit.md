# Audit

Security, memory, overhead, performance, and other parameters of the
`multi-cpu-emu` crate (8086 / 8085 / 8051 / 6502 / Z80 / rv32i).

> Scope: the Rust cores in `src/` and their WASM surface (`src/wasm.rs`,
> compiled behind the `wasm` feature). The native CLI (`examples/run.rs`)
> is covered only where it touches core behavior (e.g. benchmarking).

---

## 1. Security

| Property | Finding |
|---|---|
| `unsafe` code | **None.** `grep -rn unsafe src/` returns nothing. The only place `wasm-bindgen` may introduce `unsafe` is inside the optional dependency, gated behind the `wasm` feature; the core logic is 100% safe Rust. |
| External dependencies (cores) | **Zero.** `Cargo.toml` lists only `wasm-bindgen` (optional, feature `wasm`). No emulation/parsing/IO crates. |
| Host capabilities | The core performs **no** filesystem, network, or process access. The CLI reads a user-supplied `.asm` file, but that code lives in `examples/`, not the core/wasm. |
| Untrusted input | Assembly source is parsed by the assembler, which returns `Result<_, AsmError>` on bad input (line-numbered errors) — it never panics on malformed source. Executing arbitrary machine code (e.g. from an untrusted snapshot) is bounded: all memory accesses are masked to the address space, there are no out-of-bounds reads, and invalid opcodes are handled (treated as NOP/undefined) rather than causing UB. |
| Side channels | Execution is **deterministic** and data-independent in timing (no secret-dependent branches, no heap addresses leaked). There are no secrets in the model; state is fully caller-controlled via `snapshot`/`restore`. |
| WASM sandbox | The compiled module is pure compute; it only touches linear memory and calls exported functions the JS host wires up. No host imports beyond those the host chooses to provide. |

**Residual risk:** low. The main hardening recommendation is to fuzz the
assembler/decoder boundary (random byte streams -> `step()`) to confirm no
panics surface under adversarial input.

---

## 2. Memory model & leakage

### Fixed address spaces (allocated once at construction, never grow)

| ISA | RAM size | Constant |
|---|---|---|
| 8086 | 1 MiB | `MEM_SIZE = 1 << 20` (`src/i8086.rs:86`) |
| 8085 / 8051 / 6502 / Z80 / rv32 | 64 KiB | `MEM_SIZE = 64 * 1024` (e.g. `src/i8085.rs:9`) |

`Mem` is a `Vec<u8>` sized to a power of two; addresses are masked with
`len() - 1`, so reads/writes cannot index out of bounds and cannot grow the
buffer. **No heap reallocation occurs during steady-state execution.**

### Dynamic (bounded-growth) structures

| Structure | Type | Growth behavior |
|---|---|---|
| Program output | `Output` (`src/cpu.rs:142`) | Grows by one char per printed character. **Unbounded** if a program prints forever; in a long-lived WASM session this is the dominant memory-growth vector. `take_output()` drains it. |
| Keyboard type-ahead | `keybuf: VecDeque<u8>` (`src/i8086.rs:120`) | Grows on `push_key`; drained by reads. `INT 21h AH=0C` clears it. Bounded in practice by consumption. |
| Line-input buffer | `line_buf: Vec<u8>` (`src/i8086.rs:122`) | Reused per `INT 21h AH=0A` call; small. |
| Decode cache | `dec_cache: Option<(u32, Vec<u8>, …)>` (`src/i8086.rs:108`) | Holds only the prefix + opcode bytes (<= 8 bytes typically). Allocated once per decode *miss*, replaced (not accumulated) on the next miss. **Bounded.** |
| Port space | `[u8; 256]` | Fixed. |
| Snapshot | `Vec<u8>` | O(RAM) — ~1 MiB for 8086, ~64 KiB otherwise, plus a small header. Round-trips via `restore`. |

**Leakage assessment:** no classic leaks (no manual allocation, RAII-owned).
The only unbounded growth is program output and unconsumed keystrokes, both of
which are expected for an emulator and are reclaimable via `take_output()` /
`keybuf.clear()`. A host that never drains output from a print-loop program
will see linear memory growth — acceptable but worth documenting for WASM
deployments.

---

## 3. Overhead (per `step()`)

Each `step()` performs, in order:

1. **Halted / input-pending short-circuits** — O(1) early returns.
2. **Decode** — see section 5 (decode cache). On a hit: a byte-by-byte re-read
   verification of the cached prefix+op bytes (no heap allocation, no clone of
   the cache entry — it is compared by reference). On a miss: a prefix scan +
   opcode fetch, allocating a small `Vec` for the cached bytes.
3. **Segment-base sync** — six `u16 << 4` shifts written into the cached
   `cs_base..gs_base`; used by `pc()`/`fetch8()`/operand reads so the hot path
   avoids recomputing `seg << 4`.
4. **PIT gate** — `Pit8253::any_counting()` is a cheap `O(channels)` check; the
   timer is only advanced when a channel is actually counting.
5. **Interrupt gate** — `Pic8259::has_irq()` is a single `(irr & !imr) != 0`
   test; `service_interrupts()` (which scans the PIC) is skipped unless an
   interrupt can genuinely fire. This avoids a PIC scan on every step of a
   non-interrupt-driven program.
6. **Execution + operand fetch/store** — dominates cost; proportional to the
   decoded instruction's memory/ALU work.

**Snapshot/restore overhead:** O(RAM) copy. Fine for 64 KiB parts; ~1 MiB for
8086, so avoid calling it inside tight loops.

---

## 4. Performance

### Methodology

`cargo run --example run -- --isa <X> --bench` assembles a tight
self-loop (`JMP again` / `JR again` / `SJMP again` / `BEQ x0,x0,again`),
warms up 1000 steps, then runs **10,000,000 steps** and reports throughput.
These numbers are a **debug, native** build (unoptimized + overflow checks) and
represent a decode-bound worst case (no memory operands, no I/O, no interrupts
firing). Release builds and WASM are faster/slower respectively (see caveats).

### Throughput (10M-step busy loop, debug native)

| ISA | steps/sec | Notes |
|---|---|---|
| 8086 | **~16.3 M** | ~2x faster after the optimizations in `e6dea4b` (was ~8.5 M). |
| 8085 | **~20.5 M** | ~1.2x faster after removing the redundant `rd(pc)` opcode read + gating idle interrupt servicing (`4c71d3b`+cross-core pass). |
| 8051 | **~39.1 M** | ~1.6x faster after removing the redundant `code_byte(pc)` peek + gating idle timer/interrupt work. Timers tick per step otherwise. |
| 6502 | ~35–37 M | already lean (single fetch, no per-step timer, cheap interrupt check). |
| Z80 | **~50.0 M** | already lean; bench was previously broken (see section 7). |
| rv32 | **~52 M** | ~+57% after a re-read-verified decode cache: the 4-byte fetch is skipped when the PC is in ROM (ROM writes are ignored, so bytes are immutable); writable code is re-read/verified so self-modifying code stays correct. |

### Caveats

- **Debug build.** A release (`--release`) build is typically 2–5x faster;
  the WASM module is usually 2–10x slower than native depending on the
  engine (JIT vs interpreter).
- These loops are **decode-bound**. Programs doing real memory/ALU/IO work
  will show lower effective instruction rates because each step does more.
- 8086 is the most expensive core per step (variable-length decode, segmented
  addressing, and — when active — per-step PIT/PIC servicing). The section 5
  decode cache and the section 3 interrupt/PIT gates are what brought it from
  ~8.5 M to ~16 M steps/s.
- When timers/interrupts **are** active, `step()` performs the full PIT
  advance + PIC scan each iteration, which adds a fixed per-step cost on top
  of decode. The gates only help when the subsystem is idle.

---

## 5. Decode cache (8086)

`Cpu8086` memoizes the prefix scan + opcode of the current instruction in
`dec_cache`. The cache stores `(phys_ip, prefix+op bytes, seg_ov, rep,
ip_after)`.

- **Correctness for self-modifying code:** on every hit the cached bytes are
  *re-read* from memory and compared. If they differ (code was patched), the
  cache is discarded and the instruction is re-decoded. No write-hooking or
  code-gen is needed, so there is zero risk of executing stale translations.
- **No borrow/allocation churn:** the hit path compares by reference and only
  allocates on a miss.
- **Cache invalidation:** also cleared on `set_pc` (jumps/calls/ret),
  `reset`, and `restore`, so control-flow changes never reuse a stale decode.

This is conservative by design: it trades a small re-read on each hit for
complete correctness, including self-modifying code and the 8086's writable
code segment.

---

## 6. Determinism & reproducibility

- Given identical initial state and inputs, execution is fully deterministic
  (no RNG, no wall-clock, no threads in the core).
- `snapshot()` / `restore()` round-trip the entire core state (registers,
  flags, RAM, pending interrupts, PIT/PIC, FPU, key buffer, ports) and are
  versioned (a `ver` byte in the header; mismatched/short blobs are rejected).
  This supports the IDE's time-travel debugger.

---

## 7. Defects found during this audit

| # | Severity | Component | Description | Status |
|---|---|---|---|---|
| 1 | Low (benchmark only) | `examples/run.rs` | `--bench` for **Z80** matched the case-sensitive arm `"Z80"` while `--isa` supplies lowercase `"z80"`, so it fell through to the 8086 `jmp again` loop, which the Z80 assembler rejects. Z80 benchmarking was silently broken. | **Fixed** (`bench_loop` arm -> `"z80"`). Corrected throughput ~50 M steps/s. |
| 2 | **High (correctness)** | `src/z80.rs` `exec_ed` | `IN r,(C)` (mask `0x40`) discarded the port read into a throwaway, so the register was never updated; `OUT (C),r` (mask `0x41`) *read* the port instead of writing the register. Swapped behavior — I/O was broken. | **Fixed** (`In r,(C)` reads port (BC) into r/(HL); `Out (C),r` writes r/(HL) to port (BC)). |
| 3 | **High (correctness)** | `src/z80.rs` `exec_main` | `ADD HL,BC/DE/HL/SP` (opcodes `0x09/0x19/0x29/0x39`) had no exec arm in the main decoder (only the IX/IY `exec_xy` variant existed), so they were silently treated as NOP. | **Fixed** (added `add_hl` helper + four exec arms; H/N/V/C flags set, S/Z untouched). |
| 4 | **High (correctness)** | `src/z80.rs` `exec_main` + `src/asm/asmz80.rs` | `RST n` (restarts `0x00`–`0x38`) was not implemented in the core or assembler — any `RST` decoded to a NOP and assembled to an error. | **Fixed** (exec arm pushes PC then jumps to `n`; assembler encodes `0xC7 \| (n & 0x38)`, accepts `0,8,…,56`). |
| 5 | **High (correctness)** | `src/rv32.rs` `step` `0x73` | CSR instructions (funct3 ≠ 0) were disassembled but silently no-op'd (fell through the `ECALL`/`EBREAK` checks without executing). | **Fixed** (added `csr: [u32; 4096]` storage; CSRRW/CSRRS/CSRRC + immediate forms read-old/write-new with correct rs1=0 no-write semantics; covered by snapshot/restore). Assembler gained `CSRRW/CSRRS/CSRRC/CSRRWI/CSRRSI/CSRRCI` (+ shorthand `CSRWI/CSRSI/CSRCI`) and pseudos `CSRR`/`CSRW`. |
| 6 | Medium | `src/asm/asmz80.rs` | `ORG` only adjusted the first-pass address but never emitted a `Stmt::Org`, so forward `ORG` (needed for Z80 interrupt-vector placement) produced no padding — code was always contiguous from 0. | **Fixed** (first pass now pushes `Stmt::Org(v)`, so the emit pass `resize`s/pads to the new address). |

No **unfixed** core correctness defects remain from this audit. The section 3/5
optimizations were previously committed and tested; the defects above were
found and fixed by a later coverage pass (regression tests added in
`tests/emulation.rs`: `z80_rst_vectors`, `z80_add_hl_sp`, `z80_in_out_c`,
`rv32_csr_instructions`, `rv32_csr_survives_snapshot`, `x86_poke_code_invalidates_cache`).

---

## 8. Known limitations

- **8086:** subset of the ISA — no protected mode, paging, or full x87 beyond
  the implemented subset; DOS/BIOS services are partial (INT 21h/10h subset).
- **Timers** (PIT/8051/Z80/6502) tick on CPU steps, not wall-clock time, so
  "real-time" intervals depend on host step rate.
- **WASM:** `docs/pkg` must be rebuilt (`wasm-pack build … --features wasm`)
  after any Rust change; the prebuilt package is committed for Pages.
- **Coverage** per ISA mirrors the decoder/executor; see `AGENTS.md` for the
  instruction matrix. Unimplemented opcodes are treated as NOP/undefined
  rather than faulting.
