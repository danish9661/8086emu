# Packaging as an npm package

The WebAssembly build in `docs/pkg/` is already a publishable npm package:
`wasm-pack` emits a `package.json`, `README.md`, and the `.js`/`.wasm` files.
You can publish it directly, or wrap it in your own package.

## Recommended package name

Keep it consistent with the Rust crate and the emitted module so imports stay
predictable:

- **`multi-cpu-emu`** ← recommended. It matches the crate name
  (`multi-cpu-emu`) and the wasm module (`multi_cpu_emu.js`), so users write
  `import init, { Emulator } from 'multi-cpu-emu'`.
- **`@danish9661/multi-cpu-emu`** ← scoped alternative (requires the
  `danish9661` npm scope). Scoped names avoid squatting collisions and let the
  package live under your account.
- Avoid reusing the repo name `8086emu` for the npm package — it only covers
  one of the three ISAs and would mislead 8051/8085 users. If you want a
  discoverable alias, also publish `retro-cpu-emu` or `emu-8086-8085-8051` as
  a thin wrapper/re-export.

> The crate is `multi-cpu-emu`, the GitHub repo is `8086emu`, and the wasm
> module file is `multi_cpu_emu.js` (snake_case is required by wasm-pack).
> The npm name should be the **kebab-case** form: `multi-cpu-emu`.

## Build for publish

```bash
wasm-pack build --target web --out-dir pkg --release --features wasm
```

`wasm-pack` writes `pkg/package.json` with `name: "multi_cpu_emu"` by default.
To set the published name/scope, either edit `pkg/package.json` before
publishing or pass `--scope danish9661` and rename:

```bash
wasm-pack build --target web --out-dir pkg --release --features wasm --scope danish9661
```

## A minimal published `package.json`

```json
{
  "name": "multi-cpu-emu",
  "version": "0.1.0",
  "description": "WebAssembly emulators for Intel 8086, 8085 and 8051 (MCS-51)",
  "type": "module",
  "main": "multi_cpu_emu.js",
  "types": "multi_cpu_emu.d.ts",
  "files": ["multi_cpu_emu.js", "multi_cpu_emu.d.ts", "multi_cpu_emu_bg.wasm", "README.md"],
  "sideEffects": ["multi_cpu_emu.js"],
  "repository": { "type": "git", "url": "https://github.com/danish9661/8086emu" },
  "keywords": ["emulator", "8086", "8085", "8051", "mcs-51", "wasm", "retro"]
}
```

## Publish

```bash
cd pkg
npm version patch        # bump as needed
npm publish --access public   # --access public required for unscoped
```

## Usage from the package

```js
import init, { Emulator } from 'multi-cpu-emu';

const wasm = await init();           // or init('./multi_cpu_emu_bg.wasm')
const emu = new Emulator('8086');
```

If you want the package to also work under Node (not just browsers), rebuild
with `--target nodejs` into a separate directory and publish that as
`multi-cpu-emu-node`, or use `--target bundler` for bundler-friendly output.
