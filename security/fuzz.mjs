// Fuzz harness for the Seedling WebAssembly engine.
//
//   node security/fuzz.mjs [iterations]
//
// Loads the compiled .wasm (the same bytes embedded in seedling.html) and
// attacks every export with hostile inputs. Fails (exit 1) if the engine:
//   - traps (Rust panic, out-of-bounds index, integer underflow, etc.)
//   - emits a non-finite number (NaN / Infinity) into the draw buffer
//   - emits a malformed draw buffer (bad command, count past the end, no END)
//   - emits a link index that is not -1 or a whole number below the configured count
//   - hands the same link to two shapes in one frame
//   - reports a buffer length outside its own memory
//   - stays "poisoned" (can't grow normally) after hostile input
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const here = fileURLToPath(new URL('.', import.meta.url));
const bytes = readFileSync(here + '../target/wasm32-unknown-unknown/release/seedling_wasm.wasm');
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

// Payload sizes; leaf and flower end with a link index (-1 = none).
const ARITY = { 1: -1, 2: 7, 3: 8, 4: 4 };
let linksConfigured = 0; // what the harness last passed to configure()
function validate(ex, len, ctx) {
  const mem = ex.memory.buffer.byteLength;
  const ptr = ex.out_ptr();
  if (!Number.isInteger(len) || len < 1 || ptr + len * 4 > mem) return fail('bad length', ctx);
  const b = new Float32Array(ex.memory.buffer, ptr, len);
  let i = 0, sawEnd = false;
  const seenLinks = new Set();
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
      if (c === 2 || c === 3) {
        const link = b[i + a - 1];
        if (link !== -1 && !(Number.isInteger(link) && link >= 0 && link < Math.min(linksConfigured, 64)))
          return fail('bad link index', `${ctx} link=${link} configured=${linksConfigured}`);
        if (link >= 0) {
          if (seenLinks.has(link)) return fail('duplicate link in one frame', `${ctx} link=${link}`);
          seenLinks.add(link);
        }
      }
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
  call(ex, 'configure', [5, 1], 'pre-init');
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
  if (r < 0.004) { // reconfigure: link counts incl. hostile ones (u32 wraps from JS)
    const links = [0, 1, 3, 12, 64, 65, 255, 256, 4294967295, -1][(rnd() * 10) | 0];
    const persist = [0, 1, -1, 2 ** 31 - 1][(rnd() * 4) | 0];
    if (call(ex, 'configure', [links, persist], `iter ${it}`) !== undefined || true) {
      const u = links >>> 0; // how the u32 parameter receives it
      linksConfigured = u > 255 ? 64 : Math.min(u, 64);
    }
    continue;
  }
  // active: 0 = gone, 1 = at pointer, 2 = resting on a link, others = hostile
  const args = rnd() < 0.15 ? [pick(), pick(), pick(), ((rnd() * 6) | 0) - 2]
    : [0.016, 200 + rnd() * 600, 100 + rnd() * 500, rnd() < 0.9 ? 1 : (rnd() < 0.5 ? 2 : 0)];
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

// 5. Links are really handed out: with 5 configured, all of 0..4 appear.
{
  const e = await fresh();
  e.init(1000, 700, 11);
  e.configure(5, 1);
  linksConfigured = 5;
  const seen = new Set();
  for (let k = 0; k < 900; k++) {
    const t = k / 900;
    const len = e.tick(1 / 60, 500 + 250 * Math.sin(t * 6), 600 - 520 * t, 1);
    if (k % 50 === 0) validate(e, len, `links k=${k}`);
    const b = new Float32Array(e.memory.buffer, e.out_ptr(), len);
    for (let i = 0; i < len;) {
      const c = b[i++];
      if (c === 0) break;
      if (c === 1) { i++; const n = b[i++]; i += n * 4; continue; }
      if (c === 2 || c === 3) { const l = b[i + ARITY[c] - 1]; if (l >= 0) seen.add(l); }
      i += ARITY[c];
    }
  }
  if ([0, 1, 2, 3, 4].some(l => !seen.has(l))) fail('links not handed out', `saw ${[...seen].sort()}`);
  else console.log('links: all 5 configured links appeared on leaves/flowers');
}

console.log(`fuzzed ${ITER.toLocaleString()} frames`);
if (failures.size === 0) { console.log('PASS: no traps, no non-finite output, no malformed buffers, no poisoning'); process.exit(0); }
for (const [k, v] of failures) console.log(`FAIL ${k}: ${v.count}x  e.g. ${v.example}`);
process.exit(1);
