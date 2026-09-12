const fs = require('fs');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const contract = 'r108-observed-utc-floor-v1';
const policy = {budget_ms: 10000, poll_ms: 10, max_samples: 1001};
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const sample = () => ({utc_ms: Date.now(), monotonic_ns: process.hrtime.bigint().toString()});
function valid(s) {
  assert(Number.isSafeInteger(s.utc_ms) && s.utc_ms >= 0, 'valid UTC sample');
  assert(typeof s.monotonic_ns === 'string' && /^\d+$/.test(s.monotonic_ns), 'valid monotonic sample');
}
async function gate(floor, options = {}) {
  assert(Number.isSafeInteger(floor) && floor >= 0, 'valid predecessor UTC');
  const read = options.read || sample;
  const sleep = options.sleep || (ms => new Promise(resolve => setTimeout(resolve, ms)));
  const samples = [];
  let rejected_sample = null;
  try {
    for (let i = 0; i < policy.max_samples; i++) {
      const s = read();
      rejected_sample = s;
      valid(s);
      if (samples.length) assert(BigInt(s.monotonic_ns) >= BigInt(samples.at(-1).monotonic_ns), 'monotonic regression');
      samples.push(s);
      rejected_sample = null;
      const elapsed = BigInt(s.monotonic_ns) - BigInt(samples[0].monotonic_ns);
      assert(elapsed < BigInt(policy.budget_ms) * 1000000n, 'UTC floor wait budget exhausted');
      if (s.utc_ms >= floor) return {floor_ms: floor, samples};
      if (i + 1 < policy.max_samples) await sleep(policy.poll_ms);
    }
    throw new Error('UTC floor sample bound exhausted');
  } catch (error) {
    error.clock_failure = {contract, policy, floor_ms: floor, samples, rejected_sample, error: String(error)};
    throw error;
  }
}
function predecessor(name, field, expected, io = fs) {
  assert(/^r108-clock-[a-z0-9-]+\.json$/.test(name), 'clock cohort predecessor');
  assert(['recorded_at', 'finished_at', 'verified_at'].includes(field), 'predecessor field');
  assert(io.lstatSync(root + name).isFile(), 'regular predecessor file');
  assert.strictEqual(name, expected.name, 'exact predecessor name');
  assert.strictEqual(field, expected.field, 'exact predecessor event');
  const bytes = io.readFileSync(root + name);
  const record = JSON.parse(bytes);
  if (expected.sha256) assert.strictEqual(hash(bytes), expected.sha256, 'predecessor bytes');
  assert.strictEqual(record.clock.contract, contract);
  assert.deepStrictEqual(record.clock.policy, policy);
  assert.strictEqual(record.clock.kind, expected.kind, 'predecessor kind');
  assert.strictEqual(record.source_head, expected.source_head, 'predecessor source head');
  assert.strictEqual(record.source_map_sha256, expected.source_map_sha256, 'predecessor source map');
  if (expected.kind !== 'manifest') assert.strictEqual(record.manifest_sha256, expected.manifest_sha256, 'predecessor manifest');
  if (expected.kind === 'run') {
    assert.strictEqual(record.clock.error, null, 'successful predecessor clock');
    assert.strictEqual(record.source_unchanged, true);
    assert.strictEqual(record.returncode, expected.returncode, 'expected predecessor outcome');
    assert.strictEqual(record.signal, null);
    assert.strictEqual(record.timed_out, false);
  } else if (expected.kind === 'restoration') {
    assert.strictEqual(record.source_unchanged, true);
    assert.strictEqual(record.source_identities, 5669);
  }
  const utc = Date.parse(record[field]);
  assert(Number.isSafeInteger(utc) && utc >= 0);
  assert.strictEqual(new Date(utc).toISOString(), record[field]);
  return {name, sha256: hash(bytes), field, utc_ms: utc};
}
function plan(name) {
  const manifestName = 'r108-clock-manifest.json';
  assert(fs.lstatSync(root + manifestName).isFile());
  const bytes = fs.readFileSync(root + manifestName);
  const manifest = JSON.parse(bytes);
  assert.strictEqual(manifest.clock.contract, contract);
  for (const helper of manifest.helpers) assert.strictEqual(hash(fs.readFileSync(root + helper.name)), helper.sha256, helper.name);
  const entry = manifest.entries[name];
  assert(entry, 'planned artifact');
  const prior = entry.predecessor;
  const previousEntry = prior.name === manifestName ? manifest : manifest.entries[prior.name];
  assert(previousEntry, 'planned predecessor');
  const expected = {...prior, kind: previousEntry.kind || 'manifest', source_head: manifest.source_head,
    source_map_sha256: previousEntry.source_map_sha256, manifest_sha256: hash(bytes), returncode: previousEntry.returncode};
  if (prior.name === manifestName) expected.sha256 = hash(bytes);
  return {manifest, manifest_sha256: hash(bytes), entry, expected};
}
function verifyGate(observation, floor) {
  assert.strictEqual(observation.floor_ms, floor);
  assert(observation.samples.length > 0 && observation.samples.length <= policy.max_samples);
  for (const [i, s] of observation.samples.entries()) {
    valid(s);
    assert(BigInt(s.monotonic_ns) - BigInt(observation.samples[0].monotonic_ns) < BigInt(policy.budget_ms) * 1000000n);
    if (i) assert(BigInt(s.monotonic_ns) >= BigInt(observation.samples[i - 1].monotonic_ns));
    if (i + 1 < observation.samples.length) assert(s.utc_ms < floor);
    else assert(s.utc_ms >= floor);
  }
}
function verifyFinish(start, finish) {
  valid(start); valid(finish);
  assert(finish.utc_ms >= start.utc_ms, 'raw child-close UTC regression');
  assert(BigInt(finish.monotonic_ns) >= BigInt(start.monotonic_ns), 'raw child-close monotonic regression');
}
module.exports = {root, contract, policy, hash, sample, gate, predecessor, plan, verifyGate, verifyFinish};
