// Fuzz harness for the Seedling WebAssembly engine.
//
//   node security/fuzz.mjs [iterations]
//
// Loads the compiled .wasm (the same bytes embedded in seedling.html) and
// attacks every export with hostile inputs. Fails (exit 1) if the engine:
//   - traps (Rust panic, out-of-bounds index, integer underflow, etc.)
//   - emits a non-finite number (NaN / Infinity) into the draw buffer
//   - emits a malformed draw buffer (bad command, count past the end, no END)
//   - reports a buffer length outside its own memory
//   - stays "poisoned" (can't grow normally) after hostile input
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const here = fileURLToPath(new URL('.', import.meta.url));
const bytes = readFileSync(here + '../target/wasm32-unknown-unknown/release/seedling.wasm');
const ITER = Number(process.argv[2] || 200000);

const HOSTILE = [NaN, Infinity, -Infinity, -0, 0, 1e-45, -1, -1e9, 1e9, 3.4e38, -3.4e38,
  2 ** 31, -(2 ** 31), 2 ** 32, 1e30, 0.5, 4, 9, 36, 5000];

let rng = 0x9e3779b9;
const rnd = () => { rng ^= rng << 13; rng ^= rng >>> 17; rng ^= rng << 5; return (rng >>> 0) / 4294967296; };
const pick = () => (rnd() < 0.5 ? HOSTILE[(rnd() * HOSTILE.length) | 0] : (rnd() - 0.25) * 4000);

async function fresh() {
  return (await WebAssembly.instantiate(bytes, {})).instance.exports;
}

const failures = new Map();
function fail(kind, detail) {
  if (!failures.has(kind)) failures.set(kind, { count: 0, example: detail });
  failures.get(kind).count++;
}

const ARITY = { 1: -1, 2: 6, 3: 7, 4: 4 };
function validate(ex, len, ctx) {
  const mem = ex.memory.buffer.byteLength;
  const ptr = ex.out_ptr();
  if (!Number.isInteger(len) || len < 1 || ptr + len * 4 > mem) return fail('bad length', ctx);
  const b = new Float32Array(ex.memory.buffer, ptr, len);
  let i = 0, sawEnd = false;
  for (let k = 0; k < len; k++) if (!Number.isFinite(b[k])) return fail('non-finite output', `${ctx} @${k}=${b[k]}`);
  while (i < len) {
    const c = b[i++];
    if (c === 0) { sawEnd = true; break; }
    const a = ARITY[c];
    if (a === undefined) return fail('unknown command', `${ctx} cmd=${c}`);
    if (a === -1) {
      const kind = b[i++], n = b[i++];
      if ((kind !== 0 && kind !== 1) || !Number.isInteger(n) || n < 1 || i + n * 4 > len) return fail('bad stroke', ctx);
      i += n * 4;
    } else {
      if (i + a > len) return fail('truncated command', ctx);
      i += a;
    }
  }
  if (!sawEnd || i !== len) fail('missing END / trailing data', ctx);
}

function call(ex, name, args, ctx) {
  try { return ex[name](...args); }
  catch (e) { fail(`trap in ${name}`, `${ctx} args=${JSON.stringify(args.map(String))} ${e.message}`); return undefined; }
}

// 1. Calls before init (the page always inits first, but exports are public).
{
  const ex = await fresh();
  const len = call(ex, 'tick', [0.016, 100, 100, 1], 'pre-init');
  if (len !== undefined) validate(ex, len, 'pre-init');
  call(ex, 'ground_y', [], 'pre-init');
}

// 2. Random hostile sessions.
let ex = await fresh();
call(ex, 'init', [1000, 700, 1], 'setup');
for (let it = 0; it < ITER; it++) {
  const r = rnd();
  if (r < 0.002) { // re-init with hostile sizes; a trap here would kill the instance
    const args = [pick(), pick(), (rnd() * 2 ** 32) >>> 0];
    if (call(ex, 'init', args, `iter ${it}`) === undefined && failures.has('trap in init')) { ex = await fresh(); call(ex, 'init', [1000, 700, 1], 'recover'); }
    continue;
  }
  const args = rnd() < 0.15 ? [pick(), pick(), pick(), ((rnd() * 4) | 0) - 1]
    : [0.016, 200 + rnd() * 600, 100 + rnd() * 500, rnd() < 0.97 ? 1 : 0];
  const len = call(ex, 'tick', args, `iter ${it}`);
  if (len === undefined) { ex = await fresh(); call(ex, 'init', [1000, 700, 1], 'recover'); continue; }
  validate(ex, len, `iter ${it} args=${args.map(String)}`);
}

// 3. Poisoning check: after one NaN frame, does the plant still grow normally?
{
  const e = await fresh();
  e.init(1000, 700, 7);
  for (let k = 0; k < 30; k++) e.tick(0.016, 500, 300, 1);
  e.tick(NaN, NaN, NaN, 1);
  e.tick(0.016, Infinity, -Infinity, 1);
  let len = 0;
  for (let k = 0; k < 120; k++) len = e.tick(0.016, 650, 200, 1);
  const b = new Float32Array(e.memory.buffer, e.out_ptr(), len);
  if (b.some(v => !Number.isFinite(v)) || len < 60) fail('state poisoned by NaN/Inf input', `len=${len}`);
}

// 4. Capacity check: grow to the sample limit; buffer must never truncate.
{
  const e = await fresh();
  e.init(2000, 2000, 3);
  let len = 0, maxLen = 0;
  for (let k = 0; k < 30000; k++) {
    const a = k * 0.01;
    len = e.tick(0.05, 1000 + 900 * Math.cos(a), 900 + 800 * Math.sin(a * 1.3), 1);
    maxLen = Math.max(maxLen, len);
    if (k % 500 === 0) validate(e, len, `capacity k=${k}`);
  }
  validate(e, len, 'capacity end');
  console.log(`capacity: peak draw buffer ${maxLen.toLocaleString()} of 120,000 floats`);
}

console.log(`fuzzed ${ITER.toLocaleString()} frames`);
if (failures.size === 0) { console.log('PASS: no traps, no non-finite output, no malformed buffers, no poisoning'); process.exit(0); }
for (const [k, v] of failures) console.log(`FAIL ${k}: ${v.count}x  e.g. ${v.example}`);
process.exit(1);
