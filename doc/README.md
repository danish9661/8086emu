# Documentation

This folder holds the project documentation. Each file is a **different kind**
of document so you can jump to what you need:

| File | Type | Audience |
|------|------|----------|
| [getting-started.md](getting-started.md) | **Tutorial / How-to** | New users: build, run, use the web IDE |
| [architecture.md](architecture.md) | **Design / Explanation** | Contributors: how the cores & wasm layer fit |
| [api-reference.md](api-reference.md) | **Reference** | Exact method signatures (Rust + WASM) |
| [isa-matrix.md](isa-matrix.md) | **Reference table** | Per-ISA instruction & feature coverage |
| [packaging.md](packaging.md) | **How-to / Packaging** | Publishing the wasm build as an npm package |
| [faq.md](faq.md) | **Q&A** | Common questions |

The web demo itself lives in `docs/` (note the **s** — that is the built
`wasm-pack` output + `index.html`); this `doc/` folder is human-readable docs.
