// Paired CPU fixture observations only; no full-suite, native or performance acceptance.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', repo = root + 'fe2o3-r61-execution';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const captured = new Map();
function bytes(name) {
  const file = path.isAbsolute(name) ? name : root + name;
  assert(fs.lstatSync(file).isFile());
  if (!captured.has(file)) captured.set(file, fs.readFileSync(file));
  return captured.get(file);
}
const json = name => JSON.parse(bytes(name));
const runner = 'r125-development-run-v3.js';
assert.strictEqual(hash(bytes(runner)), 'f1c68f7542ed8b467b877a0c0d521a04af492f45822948946daaef5e2ad0359d');
const helper = 'r125-development-evidence-v3.js';
assert.strictEqual(hash(bytes(helper)), '515d128606f0aae8fa019e48eea6cf370c3982c1628a8706a66c28fee2a92e6b');
const E = require(root + helper);
bytes(__filename);
const head = 'c171d915047daa5153c1f77253e15260e55da8c5';
const boot = fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim();
assert.strictEqual(boot, '74856d82-79a2-4377-b6c4-aaf01159fd19');
const before = json('r125-development-timeout-fixture-gnu-01-source.json');
const after = json('r125-development-zero-scan-unit-gnu-01-source.json');
assert.strictEqual(hash(JSON.stringify(before)), '6486eaa2d52d9a4035db5d137fa216dcef9f853f8df6518c0aa47905534be72e');
assert.strictEqual(hash(JSON.stringify(after)), '29b127dd3e2f1eaa9cbc3e8bd7bfd7fb26bb8879e58bdf0ab801c291f188dd80');
assert.deepStrictEqual(Object.keys(before), Object.keys(after));
assert.strictEqual(Object.keys(after).length, 5709);
assert.deepStrictEqual(Object.keys(after).filter(name => before[name] !== after[name]), [
  'crates/fe2o3-kfd/src/queue_live/construction_primary/integration_platform.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_primary/integration_platform_tests.rs',
]);
const fixture = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::retained_control_cases::retained_control_retake_error_precedes_lower_error_but_never_lower_panic';
const unit = 'queue::live::construction_primary::integration_tests::platform::tests::context_zero_check_preserves_every_byte_including_unaligned_partial_pages';
const prefix = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '--all-features', '-p', 'fe2o3-kfd', '--lib'];
const specs = [
  ['timeout-fixture-gnu-01', 'musl-all-v3', 'gnu', fixture, before, 1242],
  ['timeout-fixture-musl-01', 'timeout-fixture-gnu-01', 'musl', fixture, before, 1242],
  ['zero-scan-unit-gnu-01', 'timeout-fixture-musl-01', 'gnu', unit, after, 1243],
  ['zero-scan-fixture-gnu-01', 'zero-scan-unit-gnu-01', 'gnu', fixture, after, 1243],
  ['zero-scan-unit-musl-01', 'zero-scan-fixture-gnu-01', 'musl', unit, after, 1243],
  ['zero-scan-fixture-musl-01', 'zero-scan-unit-musl-01', 'musl', fixture, after, 1243],
];
const runs = specs.map(([short, previous, kind, test, map, filtered]) => {
  const name = 'r125-development-' + short, predecessor = 'r125-development-' + previous + '.json';
  const command = [...prefix, ...(kind === 'musl' ? ['--target', 'x86_64-unknown-linux-musl'] : []), test, '--', '--exact'];
  const record = json(name + '.json');
  E.checkRecord(record, name, command, map, predecessor, 0, {read: json, bytes}, {
    head, cwd: repo, contract: 'r125-development-raw-utc-boot-monotonic-v1', runner, after: map,
  });
  assert.strictEqual(record.clock.start.boot_id, boot);
  assert(record.elapsed_seconds >= 0 && record.elapsed_seconds < record.deadline_ms / 1000);
  assert.deepStrictEqual(E.liveGroupMembers(record.process_group_cleanup.close.pgid), []);
  const log = bytes(name + '.log').toString();
  E.assertPassing(log);
  assert.deepStrictEqual(E.passing(log), [test]);
  assert.deepStrictEqual(E.ignored(log), []);
  assert.deepStrictEqual(E.totals(log), {harnesses: 1, passed: 1, failed: 0, ignored: 0});
  const summaries = [...log.matchAll(/^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; (\d+) filtered out; finished in ((?:0|[1-9]\d*)(?:\.\d+)?)s$/gm)];
  assert.strictEqual(summaries.length, 1);
  assert.strictEqual(Number(summaries[0][1]), filtered);
  const testSeconds = Number(summaries[0][2]);
  assert(Number.isFinite(testSeconds) && testSeconds >= 0);
  assert(testSeconds <= record.elapsed_seconds + 0.01, 'test time bounded by wrapper time, allowing display rounding');
  const targets = [...log.matchAll(/^     Running (.+)$/gm)];
  assert.strictEqual(targets.length, 1);
  assert(!/^test result:/m.test(log.slice(0, targets[0].index)));
  assert.strictEqual(targets[0][1].replace(/-[0-9a-f]{16}\)$/, ')'), 'unittests src/lib.rs (target/' +
    (kind === 'musl' ? 'x86_64-unknown-linux-musl/' : '') + 'debug/deps/fe2o3_kfd)');
  const body = log.slice(targets[0].index + targets[0][0].length).trim().split('\n').filter(line => line !== '');
  const progress = 'test ' + test + ' has been running for over 60 seconds';
  assert.deepStrictEqual(body, ['running 1 test', ...(body.length === 4 ? [progress] : []),
    'test ' + test + ' ... ok', summaries[0][0]], 'exact ordered single-executable body');
  assert.strictEqual((log.match(/^\s*Finished \x60test\x60 profile.*$/gm) || []).length, 1);
  return {name, kind, test, record_sha256: hash(bytes(name + '.json')),
    log_sha256: hash(bytes(name + '.log')), source_map_sha256: record.source_map_sha256,
    wrapper_seconds: record.elapsed_seconds, test_seconds: testSeconds,
    process_group: record.process_group_cleanup.close.pgid};
});
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), head);
const current = Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
  .filter(name => name && !name.startsWith('docs/')).sort().map(name => [name, hash(bytes(repo + '/' + name))]));
assert.deepStrictEqual(current, after);
for (const [file, value] of captured) assert.deepStrictEqual(fs.readFileSync(file), value);
console.log(JSON.stringify({development_only: true, source_map_sha256: hash(JSON.stringify(after)),
  source_identities: Object.keys(after).length, source_delta_files: 2, runs,
  interpretation: 'Single paired CPU fixture observations: test_seconds excludes compilation; wrapper_seconds includes Cargo startup and compilation. Not full regression, native qualification, formal correspondence or HIP/HSA performance.'}));
