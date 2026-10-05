// Build worldgen-wasm: cargo (wasm profile) → wasm-bindgen (web + nodejs) → wasm-opt.
// Web output goes to app/src/gen/pkg; the nodejs build (for det-wasm.mjs) to target/wasm-node.
import { execFileSync } from 'node:child_process';
import { existsSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const run = (cmd, args) => execFileSync(cmd, args, { cwd: root, stdio: 'inherit', shell: process.platform === 'win32' });

run('cargo', ['build', '-p', 'worldgen-wasm', '--profile', 'wasm', '--target', 'wasm32-unknown-unknown']);
// Features rustc's wasm32 target emits (plus simd128 from .cargo/config.toml). Not --all-features:
// that lets wasm-opt use newer encodings some engines reject.
const WASM_FEATURES = ['--enable-simd', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--enable-reference-types', '--enable-multivalue'];
const wasm = 'target/wasm32-unknown-unknown/wasm/worldgen_wasm.wasm';

const wasmOpt = join(root, 'app/node_modules/.bin', process.platform === 'win32' ? 'wasm-opt.cmd' : 'wasm-opt');
const outputs = [
  ['app/src/gen/pkg', 'web'],
  ['target/wasm-node', 'nodejs'],
];
for (const [outDir, target] of outputs) {
  run('wasm-bindgen', [wasm, '--out-dir', outDir, '--target', target]);
  const bg = join(outDir, 'worldgen_wasm_bg.wasm');
  if (existsSync(wasmOpt)) {
    // Semantics-preserving only (no --fast-math): determinism must survive optimization.
    run(wasmOpt, ['-O3', ...WASM_FEATURES, bg, '-o', bg]);
  }
}
// The nodejs glue is CommonJS; the repo root is "type": "module".
writeFileSync(join(root, 'target/wasm-node/package.json'), '{"type": "commonjs"}\n');
console.log(existsSync(wasmOpt) ? 'wasm built and optimized' : 'wasm built (wasm-opt not installed; run npm install in app/)');
