const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const old = require('./r112-exact-clock-evidence.js');
const p = require('./r112-exact-qualification-plan.js');
const root = old.root;
const repo = root + 'fe2o3-r61-execution';
const prefix = 'r112-exact-clock-continuation-';
const manifestName = prefix + 'manifest.json';
const contract = 'r112-raw-utc-boot-monotonic-continuation-v1';
const clockSource = 'node-process-hrtime-linux-monotonic';
const helpers = ['evidence.js', 'evidence-tests.js', 'prepare.js', 'run.js', 'retain-local.js'].map(n => prefix + n);
const hash = old.hash;
const file = name => path.join(root, name);
const read = name => JSON.parse(fs.readFileSync(file(name), 'utf8'));
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const boot = () => fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim();
const observation = bootId => ({utc_ms: Date.now(), monotonic_ns: process.hrtime.bigint().toString(), boot_id: bootId, clock_source: clockSource});
function valid(s, bootId) {
  assert(Number.isSafeInteger(s.utc_ms) && s.utc_ms >= 0, 'valid raw UTC');
  assert(typeof s.monotonic_ns === 'string' && /^\d+$/.test(s.monotonic_ns), 'valid monotonic sample');
  assert(/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(bootId), 'valid boot ID');
  assert.strictEqual(s.boot_id, bootId, 'same boot');
  assert.strictEqual(s.clock_source, clockSource, 'same clock source');
}
function ordered(a, b, bootId) {
  valid(a, bootId); valid(b, bootId);
  assert(BigInt(b.monotonic_ns) >= BigInt(a.monotonic_ns), 'monotonic order');
}
function identities() {
  const paths = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(n => n && !n.startsWith('docs/')).sort();
  return Object.fromEntries(paths.map(n => [n, hash(fs.readFileSync(path.join(repo, n)))]));
}
function pins(names) {
  return names.map(name => {
    assert(fs.lstatSync(file(name)).isFile());
    return {name, sha256: hash(fs.readFileSync(file(name)))};
  });
}
function verifyBytes(bytes, expected) {
  assert.strictEqual(hash(bytes), expected, 'exact predecessor bytes');
}
function checkPins(items) {
  for (const item of items) {
    assert(fs.lstatSync(file(item.name)).isFile());
    verifyBytes(fs.readFileSync(file(item.name)), item.sha256);
  }
}
function verifyRun(record, entry, previous, manifest, manifestHash) {
  assert.strictEqual(record.manifest_sha256, manifestHash);
  assert.strictEqual(record.source_head, manifest.source_head);
  assert.strictEqual(record.source_map_sha256, manifest.source_map_sha256, 'source map');
  assert.strictEqual(record.source_unchanged, true, 'unchanged source');
  assert.deepStrictEqual(record.command, entry.command);
  assert.strictEqual(record.returncode, 0, 'successful child');
  assert.strictEqual(record.signal, null);
  assert.strictEqual(record.timed_out, false);
  assert.strictEqual(record.clock.error, null);
  assert.strictEqual(record.clock.contract, contract);
  assert.strictEqual(record.clock.kind, 'run');
  assert.deepStrictEqual(record.clock.predecessor, previous);
  const {start, finish, source_verified: verified} = record.clock;
  ordered(previous.observation, start, manifest.boot_id);
  ordered(start, finish, manifest.boot_id);
  ordered(finish, verified, manifest.boot_id);
  assert.strictEqual(record.started_at, new Date(start.utc_ms).toISOString());
  assert.strictEqual(record.finished_at, new Date(finish.utc_ms).toISOString());
  assert.strictEqual(record.elapsed_seconds, Number(BigInt(finish.monotonic_ns) - BigInt(start.monotonic_ns)) / 1e9);
  assert.strictEqual(record.verification_elapsed_seconds, Number(BigInt(verified.monotonic_ns) - BigInt(finish.monotonic_ns)) / 1e9);
}
function loadManifest() {
  const bytes = fs.readFileSync(file(manifestName));
  const manifest = JSON.parse(bytes);
  assert.strictEqual(manifest.contract, contract);
  assert.strictEqual(manifest.source_head, p.parent);
  assert.strictEqual(manifest.source_map_sha256, hash(JSON.stringify(read('r112-frozen-source.json'))));
  assert.deepStrictEqual(manifest.helpers.map(h => h.name), helpers);
  checkPins([...manifest.helpers, ...manifest.prior_artifacts, ...manifest.contract_tests]);
  valid(manifest.observation, manifest.boot_id);
  return {manifest, manifestHash: hash(bytes)};
}
function plan(name) {
  const {manifest, manifestHash} = loadManifest();
  assert.strictEqual(boot(), manifest.boot_id, 'execution boot');
  const entry = manifest.entries[name + '.json'];
  assert(entry, 'planned continuation command');
  const before = entry.predecessor;
  const bytes = fs.readFileSync(file(before));
  assert(fs.lstatSync(file(before)).isFile());
  const record = JSON.parse(bytes);
  let observed;
  if (before === manifestName) {
    verifyBytes(bytes, manifestHash);
    observed = manifest.observation;
  } else {
    const previousPlan = plan(before.slice(0, -5));
    verifyRun(record, previousPlan.entry, previousPlan.previous, manifest, manifestHash);
    observed = record.clock.source_verified;
  }
  const previous = {name: before, sha256: hash(bytes), observation: observed};
  return {manifest, manifestHash, entry, previous};
}
module.exports = {fs, assert, root, repo, prefix, manifestName, contract, clockSource, helpers, hash, file, read, git,
  boot, observation, valid, ordered, identities, pins, verifyBytes, checkPins, verifyRun, loadManifest, plan};
