// Read-only development checks. This neither edits sources nor accepts a packet.
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const crypto = require('crypto');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const read = name => fs.readFileSync(path.join(root, name));
const json = name => JSON.parse(read(name));
const planBytes = read('r121-development-mutations-v2.json');
assert.strictEqual(hash(planBytes), 'bfbda6d05aac8905934b1a9b406d3e1a4f57fe2b8d3865c0b6a0808beb7ffe81');
const plan = JSON.parse(planBytes);
const map = json(plan.source_map);
for (const [name, digest] of Object.entries(plan.parser_inputs)) {
  assert.strictEqual(hash(read(name)), digest, 'pinned parser input ' + name);
}
const parser = require('./r119-integrated-qualification-evidence-v1.js');
assert.strictEqual(hash(JSON.stringify(map)), plan.source_map_sha256);
assert.strictEqual(hash(read(plan.runner)), plan.runner_sha256);
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), plan.source_parent);
function identities() {
  return Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(name => name && !name.startsWith('docs/')).sort()
    .map(name => [name, hash(fs.readFileSync(path.join(repo, name)))]));
}
function liveMembers(pgid) {
  const members = [];
  for (const pid of fs.readdirSync('/proc')) {
    if (!/^[1-9][0-9]*$/.test(pid)) continue;
    let stat;
    try {stat = fs.readFileSync('/proc/' + pid + '/stat', 'utf8');}
    catch (error) {if (['ENOENT', 'ESRCH'].includes(error.code)) continue; throw error;}
    const fields = stat.slice(stat.lastIndexOf(')') + 2).trim().split(/\s+/);
    if (Number(fields[2]) === pgid && !['Z', 'X'].includes(fields[0])) members.push(Number(pid));
  }
  return members;
}
function completed(name, command, expectedMap) {
  const record = json(name + '.json');
  assert.strictEqual(record.source_head, plan.source_parent);
  assert.deepStrictEqual(record.command, command);
  assert.strictEqual(record.cwd, repo);
  assert.strictEqual(record.child_closed, true);
  assert.strictEqual(record.spawn_error, null);
  assert.strictEqual(record.signal, null);
  assert.strictEqual(record.timed_out, false);
  assert.strictEqual(record.deadline_ms, 1800000);
  assert.strictEqual(record.child_returncode, record.returncode);
  assert.deepStrictEqual(record.runner, {name: plan.runner, sha256: plan.runner_sha256});
  assert.deepStrictEqual(record.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
  assert.strictEqual(record.clock.contract, 'r121-development-raw-utc-boot-monotonic-v1');
  assert.strictEqual(record.clock.error, null);
  const cleanup = record.process_group_cleanup.close;
  assert.strictEqual(record.process_group_cleanup.timeout, null);
  assert(['absent', 'signaled'].includes(cleanup.status));
  assert(Number.isSafeInteger(cleanup.pgid) && cleanup.pgid > 1);
  assert.deepStrictEqual(cleanup.live_members, []);
  assert.deepStrictEqual(liveMembers(cleanup.pgid), []);
  const samples = [record.clock.start, record.clock.finish, cleanup.observation, record.clock.source_verified];
  for (const sample of samples) {
    assert.strictEqual(sample.boot_id, samples[0].boot_id);
    assert(/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(sample.boot_id));
    assert.strictEqual(sample.clock_source, 'node-process-hrtime-linux-monotonic');
    assert(typeof sample.monotonic_ns === 'string' && /^\d+$/.test(sample.monotonic_ns));
    assert(Number.isSafeInteger(sample.utc_ms) && sample.utc_ms >= 0);
  }
  for (let i = 1; i < samples.length; i++) assert(BigInt(samples[i].monotonic_ns) >= BigInt(samples[i - 1].monotonic_ns));
  assert.strictEqual(record.started_at, new Date(samples[0].utc_ms).toISOString());
  assert.strictEqual(record.finished_at, new Date(samples[1].utc_ms).toISOString());
  assert.strictEqual(record.elapsed_seconds, Number(BigInt(samples[1].monotonic_ns) - BigInt(samples[0].monotonic_ns)) / 1e9);
  assert.strictEqual(record.verification_elapsed_seconds, Number(BigInt(samples[3].monotonic_ns) - BigInt(samples[1].monotonic_ns)) / 1e9);
  assert(record.elapsed_seconds >= 0 && record.elapsed_seconds < record.deadline_ms / 1000);
  assert.strictEqual(record.source_unchanged, true);
  assert.strictEqual(record.source_map_sha256, hash(JSON.stringify(expectedMap)));
  for (const [field, suffix] of [['source', '-source.json'], ['source_after', '-source-after.json']]) {
    assert.strictEqual(record[field], path.join(root, name + suffix));
    assert.deepStrictEqual(json(name + suffix), expectedMap);
  }
  assert.strictEqual(record.log, path.join(root, name + '.log'));
  return {record, log: read(name + '.log').toString()};
}
function predecessor(record, name, prior) {
  assert.strictEqual(prior.source_head, plan.source_parent);
  assert.strictEqual(prior.clock.contract, record.clock.contract);
  assert.strictEqual(prior.clock.error, null);
  assert.deepStrictEqual(record.clock.predecessor, {
    name, sha256: hash(read(name)), observation: prior.clock.source_verified,
  });
  assert.strictEqual(record.clock.start.boot_id, prior.clock.source_verified.boot_id);
  assert(BigInt(record.clock.start.monotonic_ns) >= BigInt(prior.clock.source_verified.monotonic_ns));
}
function baseline() {
  const result = completed(plan.baseline.name, plan.baseline.command, map);
  const prior = completed(
    plan.baseline.predecessor.replace(/\.json$/, ''),
    [...plan.test_command, 'shared_memory::tests::pristine_abort::cleanup_tests::data::typed_data_cleanup_preserves_all_five_inputs_and_refunds_once', '--', '--exact'],
    map,
  );
  assert.strictEqual(prior.record.returncode, 0);
  predecessor(result.record, plan.baseline.predecessor, prior.record);
  assert.strictEqual(result.record.returncode, 0);
  const accepted = plan.accepted_baseline;
  const oldLog = git(['show', accepted.commit + ':' + accepted.path]);
  assert.strictEqual(hash(oldLog), accepted.sha256);
  const expected = parser.executables(oldLog);
  const actual = parser.executables(result.log);
  assert.deepStrictEqual(parser.totals(oldLog), {harnesses: 48, passed: 2795, failed: 0, ignored: 5});
  assert.deepStrictEqual(parser.totals(result.log), {harnesses: 48, passed: 2818, failed: 0, ignored: 5});
  assert.strictEqual(actual.length, 49);
  assert.strictEqual(expected.length, 49);
  assert(!/^test .* \.\.\. FAILED$|^test result: FAILED\./m.test(result.log));
  assert.strictEqual(plan.new_tests.length, 23);
  assert.strictEqual(new Set(plan.new_tests).size, 23);
  const kfd = expected.filter(t => t.kind === 'libtest' && /fe2o3_kfd\)/.test(t.name));
  assert.strictEqual(kfd.length, 1);
  assert.deepStrictEqual(kfd[0].totals, {harnesses: 1, passed: 1149, failed: 0, ignored: 0});
  assert(plan.new_tests.every(name => !kfd[0].passing.includes(name)));
  kfd[0].passing = [...kfd[0].passing, ...plan.new_tests].sort();
  kfd[0].totals.passed += plan.new_tests.length;
  assert.deepStrictEqual(actual, expected, 'exact 49-target roster with only KFD additions');
  assert.deepStrictEqual(parser.passing(result.log), [...parser.passing(oldLog), ...plan.new_tests].sort());
  assert.deepStrictEqual(parser.ignored(result.log), parser.ignored(oldLog));
  const runtime = actual.filter(t => t.kind === 'libtest' && /fe2o3_runtime\)/.test(t.name));
  assert.strictEqual(runtime.length, 1);
  assert.strictEqual(runtime[0].totals.passed, 733);
  return {...result, kfdPassed: kfd[0].totals.passed};
}
const [mode, id] = process.argv.slice(2);
if (mode === 'restored') {
  assert.deepStrictEqual(identities(), map);
  console.log(JSON.stringify({development_only: true, restored: true, process_closure_checked: false, source_map_sha256: plan.source_map_sha256, identities: Object.keys(map).length}));
} else if (mode === 'baseline') {
  const {record} = baseline();
  assert.deepStrictEqual(identities(), map);
  console.log(JSON.stringify({development_only: true, baseline_passed: 2818, kfd_passed: 1172, elapsed_seconds: record.elapsed_seconds, source_map_sha256: plan.source_map_sha256, quiescent: true}));
} else {
  assert.strictEqual(mode, 'negative');
  const mutation = plan.mutations.find(m => m.id === id);
  assert(mutation, 'declared mutation');
  const changed = identities();
  assert.deepStrictEqual(Object.keys(changed), Object.keys(map));
  assert.deepStrictEqual(Object.keys(map).filter(name => map[name] !== changed[name]), [mutation.patch.path]);
  assert.strictEqual(changed[mutation.patch.path], mutation.expected_file_sha256, 'exact planned mutation');
  const name = 'r121-development-corrected-negative-' + id + '-v1';
  const {record, log} = completed(name, [...plan.test_command, mutation.test, '--', '--exact'], changed);
  const positive = baseline();
  assert(plan.new_tests.includes(mutation.test), 'mutation targets a declared new test');
  assert(!plan.source_guards.includes(mutation.test), 'source guards cannot qualify behavioral negatives');
  predecessor(record, plan.baseline.name + '.json', positive.record);
  const blocks = log.split('---- ' + mutation.test + ' stdout ----');
  const body = blocks.length === 2 ? blocks[1].split('\nfailures:\n')[0] : '';
  const panic = body.slice(body.lastIndexOf("thread '"));
  const oracleMatched = record.returncode === 101 && /Finished `test` profile/.test(log)
    && log.includes('running 1 test\n')
    && log.includes('test ' + mutation.test + ' ... FAILED\n')
    && log.includes('test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; ' + (positive.kfdPassed - 1) + ' filtered out;')
    && mutation.oracle.every(fragment => panic.includes(fragment));
  console.log(JSON.stringify({development_only: true, id, quiescent: true, expected_failure_observed: oracleMatched,
    source_map_sha256: record.source_map_sha256, log_sha256: hash(log), decisive_panic: panic.trim()}));
  process.exitCode = oracleMatched ? 0 : 1;
}
