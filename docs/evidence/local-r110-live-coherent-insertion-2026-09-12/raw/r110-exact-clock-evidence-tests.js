const assert = require('assert');
const fs = require('fs');
const crypto = require('crypto');
const helperNames = ['r110-exact-clock-check-mutation.js', 'r110-exact-clock-evidence-tests.js', 'r110-exact-clock-evidence.js',
  'r110-exact-clock-prepare.js', 'r110-exact-clock-restoration.js', 'r110-exact-clock-retain-local.js', 'r110-exact-clock-run.js',
  'r110-exact-qualification-plan.js'];
const helperPins = () => helperNames.map(name => ({name,
  sha256: crypto.createHash('sha256').update(fs.readFileSync('/home/harsh/.codex-tmp/' + name)).digest('hex')}));
const initialHelpers = helperPins();
console.log('HELPER_PINS: ' + JSON.stringify(initialHelpers));
const c = require('./r110-exact-clock-evidence.js');
let count = 0;
async function test(name, body) { await body(); count++; console.log('PASS: ' + name); }
function fake(values, step = 10000000n) {
  let i = 0;
  return {read: () => ({utc_ms: values[Math.min(i, values.length - 1)], monotonic_ns: (BigInt(i++) * step).toString()}), sleep: async () => {}};
}
(async () => {
  await test('already above floor preserves actual observation', async () => {
    const r = await c.gate(100, fake([101])); c.verifyGate(r, 100); assert.strictEqual(r.samples[0].utc_ms, 101);
  });
  await test('equality is admitted without sleep', async () => {
    const r = await c.gate(100, {read: () => ({utc_ms: 100, monotonic_ns: '1'}), sleep: async () => {throw Error('unexpected sleep');}});
    c.verifyGate(r, 100);
  });
  await test('repeated and regressed UTC waits to exact floor', async () => {
    const r = await c.gate(100, fake([99, 99, 80, 100])); c.verifyGate(r, 100); assert.strictEqual(r.samples.length, 4);
  });
  await test('qualifying sample is never reread', async () => {
    let calls = 0;
    const r = await c.gate(100, {read: () => ({utc_ms: calls++ ? 0 : 100, monotonic_ns: '1'})});
    assert.strictEqual(calls, 1); assert.strictEqual(r.samples[0].utc_ms, 100);
  });
  await test('never reaching floor fails at exact monotonic budget', async () => {
    await assert.rejects(c.gate(100, fake([99], 10000000000n)), /wait budget exhausted/);
  });
  await test('floor reached at exhausted budget still rejects', async () => {
    await assert.rejects(c.gate(100, fake([99, 100], 10000000000n)), /wait budget exhausted/);
  });
  await test('stalled monotonic clock hits hard sample bound', async () => {
    await assert.rejects(c.gate(100, fake([99], 0n)), /sample bound exhausted/);
  });
  await test('invalid readings fail closed with retained observations', async () => {
    for (const utc of [NaN, Infinity, -1, 0.5]) await assert.rejects(c.gate(100, fake([utc])), e => !!e.clock_failure && /UTC sample/.test(e.message));
    await assert.rejects(c.gate(100, {read: () => ({utc_ms: 100, monotonic_ns: '-1'})}), /monotonic sample/);
  });
  await test('monotonic regression rejects', async () => {
    let i = 0;
    await assert.rejects(c.gate(100, {read: () => ({utc_ms: 99, monotonic_ns: String(2 - i++)}), sleep: async () => {}}), /monotonic regression/);
  });
  await test('sleep errors fail closed and prevent the next action', async () => {
    let spawned = false;
    await assert.rejects((async () => {await c.gate(100, {read: () => ({utc_ms: 99, monotonic_ns: '0'}), sleep: async () => {throw Error('sleep failed');}}); spawned = true;})(), /sleep failed/);
    assert.strictEqual(spawned, false);
  });
  await test('raw child-close regressions remain rejected', async () => {
    assert.throws(() => c.verifyFinish({utc_ms: 100, monotonic_ns: '1'}, {utc_ms: 99, monotonic_ns: '2'}), /UTC regression/);
    assert.throws(() => c.verifyFinish({utc_ms: 100, monotonic_ns: '2'}, {utc_ms: 101, monotonic_ns: '1'}), /monotonic regression/);
  });
  await test('collector rejects altered observations', async () => {
    const r = await c.gate(100, fake([99, 100]));
    assert.throws(() => c.verifyGate({...r, floor_ms: 99}, 100));
    assert.throws(() => c.verifyGate({...r, samples: []}, 100));
    assert.throws(() => c.verifyGate({...r, samples: [...r.samples].reverse()}, 100));
  });
  await test('foreign cohort and missing predecessors reject', async () => {
    assert.throws(() => c.predecessor('r110-repeat-mut-slot-only.json', 'finished_at'), /clock cohort/);
    assert.throws(() => c.predecessor('r110-exact-clock-does-not-exist.json', 'finished_at'), /ENOENT/);
    assert.throws(() => c.predecessor('r110-exact-clock-does-not-exist.json', 'anything'), /predecessor field/);
  });
  const predecessorName = 'r110-exact-clock-test-parent.json';
  const expected = {name: predecessorName, field: 'finished_at', kind: 'run', source_head: 'head',
    source_map_sha256: 'source', manifest_sha256: 'manifest', returncode: 101};
  const parent = {source_head: 'head', source_map_sha256: 'source', manifest_sha256: 'manifest',
    source_unchanged: true, returncode: 101, signal: null, timed_out: false,
    finished_at: '2026-09-12T00:00:00.000Z', clock: {contract: c.contract, policy: c.policy, kind: 'run', error: null}};
  const io = value => ({lstatSync: () => ({isFile: () => true}), readFileSync: () => Buffer.from(JSON.stringify(value))});
  await test('exact predecessor bytes and source identity validate', async () => {
    const bytes = Buffer.from(JSON.stringify(parent));
    const p = c.predecessor(predecessorName, 'finished_at', {...expected, sha256: c.hash(bytes)}, io(parent));
    assert.strictEqual(p.sha256, c.hash(bytes)); assert.strictEqual(p.utc_ms, Date.parse(parent.finished_at));
  });
  await test('wrong event source cohort bytes or outcome rejects', async () => {
    for (const patch of [{name: 'r110-exact-clock-other.json'}, {field: 'verified_at'}, {kind: 'restoration'},
      {source_head: 'other'}, {source_map_sha256: 'other'}, {manifest_sha256: 'other'}, {sha256: 'other'}, {returncode: 0}]) {
      assert.throws(() => c.predecessor(predecessorName, 'finished_at', {...expected, ...patch}, io(parent)));
    }
    for (const patch of [{source_unchanged: false}, {signal: 'SIGKILL'}, {timed_out: true},
      {clock: {...parent.clock, error: 'bad'}}, {clock: {...parent.clock, contract: 'wrong'}}, {finished_at: 'bad'}]) {
      assert.throws(() => c.predecessor(predecessorName, 'finished_at', expected, io({...parent, ...patch})));
    }
  });
  await test('symlink and malformed predecessor reject before reading or spawning', async () => {
    assert.throws(() => c.predecessor(predecessorName, 'finished_at', expected,
      {lstatSync: () => ({isFile: () => false}), readFileSync: () => {throw Error('must not read');}}), /regular predecessor/);
    assert.throws(() => c.predecessor(predecessorName, 'finished_at', expected,
      {...io(parent), readFileSync: () => Buffer.from('{invalid')}), SyntaxError);
  });
  assert.deepStrictEqual(helperPins(), initialHelpers, 'unchanged external test inputs');
  console.log('PASS: all ' + count + ' evidence-clock contract tests');
})().catch(error => {console.error(error); process.exitCode = 1;});
