const c = require('./r112-exact-clock-continuation-evidence.js');
const a = c.assert;
const bootId = '11111111-2222-3333-4444-555555555555';
const sample = (utc, ticks) => ({utc_ms: utc, monotonic_ns: String(ticks), boot_id: bootId, clock_source: c.clockSource});
const predecessor = {name: 'manifest.json', sha256: 'a', observation: sample(3000, 1)};
const entry = {command: ['test']};
const manifest = {boot_id: bootId, source_head: 'h', source_map_sha256: 's'};
const record = {manifest_sha256: 'm', source_head: 'h', source_map_sha256: 's', source_unchanged: true,
  command: ['test'], returncode: 0, signal: null, timed_out: false,
  started_at: new Date(2000).toISOString(), finished_at: new Date(1000).toISOString(),
  elapsed_seconds: 1e-9, verification_elapsed_seconds: 1e-9,
  clock: {contract: c.contract, kind: 'run', predecessor, start: sample(2000, 2), finish: sample(1000, 3),
    source_verified: sample(500, 4), error: null}};
const check = r => c.verifyRun(r, entry, predecessor, manifest, 'm');
const reject = change => { const r = structuredClone(record); change(r); a.throws(() => check(r)); };
const tests = [
  ['raw UTC rewind is preserved', () => { const bytes = JSON.stringify(record); check(record); a.strictEqual(JSON.stringify(record), bytes); }],
  ['start before predecessor rejects', () => reject(r => {r.clock.start.monotonic_ns = '0';})],
  ['child-close regression rejects', () => reject(r => {r.clock.finish.monotonic_ns = '1';})],
  ['source-verification regression rejects', () => reject(r => {r.clock.source_verified.monotonic_ns = '2';})],
  ['boot substitution rejects', () => reject(r => {r.clock.finish.boot_id = '99999999-2222-3333-4444-555555555555';})],
  ['clock-source substitution rejects', () => reject(r => {r.clock.finish.clock_source = 'wall';})],
  ['predecessor substitution rejects', () => reject(r => {r.clock.predecessor.sha256 = 'b';})],
  ['predecessor-byte substitution rejects', () => a.throws(() => c.verifyBytes(Buffer.from('changed'), c.hash('original')))],
  ['source substitution rejects', () => reject(r => {r.source_map_sha256 = 'other';})],
  ['source change rejects', () => reject(r => {r.source_unchanged = false;})],
  ['failed child rejects', () => reject(r => {r.returncode = 101;})],
  ['signalled child rejects', () => reject(r => {r.signal = 'SIGKILL';})],
  ['timeout rejects', () => reject(r => {r.timed_out = true;})],
  ['duration substitution rejects', () => reject(r => {r.elapsed_seconds = 1;})],
  ['raw UTC substitution rejects', () => reject(r => {r.finished_at = r.started_at;})],
  ['malformed raw UTC rejects', () => reject(r => {r.clock.finish.utc_ms = NaN;})],
];
console.log('HELPER_PINS: ' + JSON.stringify(c.pins(c.helpers)));
for (const [name, run] of tests) {run(); console.log('PASS: ' + name);}
console.log('PASS: all ' + tests.length + ' continuation contract tests');
