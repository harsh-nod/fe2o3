// Focused development audit; this does not replace the complete qualification parser.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert/strict');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const read = name => fs.readFileSync(path.join(root, name));
const json = name => JSON.parse(read(name));
const inputBytes = read('r122-development-forwarding-negatives-inputs-v1.json');
assert.equal(hash(inputBytes), '9e2ac75f0d45903e96eb2cc1752ee78220616ad4b18717a53e7d43d9573b65a4');
const input = JSON.parse(inputBytes);
assert.equal(input.qualification, false);
assert.equal(cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo}).toString().trim(), input.source_head);
assert.equal(hash(read(input.runner.name)), input.runner.sha256);
const baseline = json(input.baseline_run + '-source.json');
assert.equal(hash(JSON.stringify(baseline)), input.source_map_sha256);
assert.equal(Object.keys(baseline).length, input.count);
for (const [file, sha] of Object.entries(input.files)) assert.equal(baseline[file], sha);

function checkRun(name, code, source, command, passed, failed, filtered) {
  assert(/^r122-development-[a-z0-9-]+$/.test(name));
  const run = json(name + '.json');
  assert.equal(run.source_head, input.source_head);
  assert.equal(run.cwd, repo);
  assert.equal(run.log, path.join(root, name + '.log'));
  assert.equal(run.source, path.join(root, name + '-source.json'));
  assert.equal(run.source_after, path.join(root, name + '-source-after.json'));
  assert.deepEqual(run.command, command);
  assert.equal(run.returncode, code);
  assert.equal(run.child_returncode, code);
  assert.equal(run.child_closed, true);
  assert.equal(run.timed_out, false);
  assert.equal(run.spawn_error, null);
  assert.equal(run.signal, null);
  assert.equal(run.source_unchanged, true);
  assert.deepEqual(run.runner, input.runner);
  assert.equal(run.clock.error, null);
  assert.equal(run.clock.contract, 'r122-development-raw-utc-boot-monotonic-v1');
  assert.equal(run.process_group_cleanup.timeout, null);
  assert.equal(run.process_group_cleanup.close.status, 'absent');
  assert.deepEqual(run.process_group_cleanup.close.live_members, []);
  assert.deepEqual(run.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
  assert.deepEqual(json(name + '-source.json'), source);
  assert.deepEqual(json(name + '-source-after.json'), source);
  assert.equal(run.source_map_sha256, hash(JSON.stringify(source)));
  const text = read(name + '.log').toString();
  assert(!/^error\[E\d+\]/m.test(text), 'compiler diagnostics are not behavioral evidence');
  assert(!/process didn't exit successfully|stack overflow|panicked in a destructor|fatal runtime error/.test(text));
  assert.equal((text.match(/^     Running unittests /gm) || []).length, 1);
  const results = [...text.matchAll(/^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in [\d.]+s$/gm)];
  assert.equal(results.length, 1);
  assert.deepEqual(results[0].slice(1), [failed ? 'FAILED' : 'ok', String(passed), String(failed), '0', '0', String(filtered)]);
  return {run, text, record_sha256: hash(read(name + '.json')), log_sha256: hash(read(name + '.log'))};
}

const command = ['cargo', 'test', '-p', 'fe2o3-kfd', '--lib'];
const positiveCommand = [...command, 'detached_release_'];
const positive = checkRun(input.baseline_run, 0, baseline, positiveCommand, 4, 0, 1186);
let previous = input.baseline_run;
const checks = [];
for (const mutation of input.mutations) {
  const original = fs.readFileSync(path.join(repo, mutation.file), 'utf8');
  assert.equal(hash(original), baseline[mutation.file], 'production file must be restored before audit');
  assert.equal(original.split(mutation.original).length, 2, 'unique mutation site');
  const source = {...baseline, [mutation.file]: hash(original.replace(mutation.original, mutation.replacement))};
  const checked = checkRun(mutation.run, 101, source, [...command, mutation.test, '--', '--exact'], 0, 1, 1189);
  assert.equal(checked.run.clock.predecessor.name, previous + '.json');
  assert.equal(checked.run.clock.predecessor.sha256, hash(read(previous + '.json')));
  assert.equal((checked.text.match(/^test .+ \.\.\. FAILED$/gm) || []).length, 1);
  assert(checked.text.includes('test ' + mutation.test + ' ... FAILED\n'));
  const panicLines = checked.text.split('\n').filter(line => line.startsWith("thread '") && line.includes(' panicked at '));
  assert.equal(panicLines.length, 1);
  assert(panicLines[0].includes(' panicked at ' + mutation.failure_location));
  assert(checked.text.includes('\n' + mutation.failure_message + '\n'));
  checks.push({name: mutation.run, test: mutation.test, source_map_sha256: checked.run.source_map_sha256, record_sha256: checked.record_sha256, log_sha256: checked.log_sha256});
  previous = mutation.run;
}
const restored = checkRun(input.restored_run, 0, baseline, positiveCommand, 4, 0, 1186);
assert.equal(restored.run.clock.predecessor.name, previous + '.json');
assert.equal(restored.run.clock.predecessor.sha256, hash(read(previous + '.json')));
const names = [...new Set(cp.execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString().split('\0'))]
  .filter(name => name && !name.startsWith('docs/')).sort();
const current = Object.fromEntries(names.map(name => [name, hash(fs.readFileSync(path.join(repo, name)))]));
assert.deepEqual(current, baseline, 'all non-doc source identities restored');
console.log(JSON.stringify({development_checks_passed: true, qualification: false, source_head: input.source_head, source_map_sha256: input.source_map_sha256, restored_source_count: names.length, baseline_record_sha256: positive.record_sha256, mutations: checks, restored_record_sha256: restored.record_sha256}));
