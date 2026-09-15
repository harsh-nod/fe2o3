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
const planBytes = read('r121-development-mutations-v1.json');
assert.strictEqual(hash(planBytes), 'a29ed2371ff07eaa205c61e70b88f5cdec6621aeac0882af9341ab0682c4dd74');
const plan = JSON.parse(planBytes);
const map = json(plan.source_map);
const mutantHashes = {
  '01-active-owner': 'c05fcd079b66f4cc8b932de104a30bea54af0d94e95ef8dce27da69da758fb53',
  '02-suffix-owner': 'abc86eea3307322e4aabe2da03d1a6f99c914781a138a65262cf3a791003c49f',
  '03-interrupted-receipt': '3ec4ea324bb104578fd77e6400f335111c299a7f6f8e6075c5618fa77d00c9c7',
  '04-free-currentness-order': '60878df46e14c4894b829f3d76de1ef4d40d139777b800070623b79d5f295da7',
  '05-inflight-admission': '52d602f67f90147b560dae60dd8c5947520991d289cc000bbca339bfadd97945',
  '06-ordinary-success': 'aeaa3a0f88203bfe377a1f59ea12b5492419c417389872cdff1dca6cf69896fa',
  '07-typed-success': '5ab17a0a00fdf4cbe56eebd86b76310b850ae4c485a290e40d252326f9a407fe',
  '08-receipt-identity': 'd09deb21e1f9f544f0c766b86de773875e794595e07e1fdd7976309268b7e1c8',
};
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
function baseline() {
  const result = completed('r121-development-kfd-all-v1', ['cargo', 'test', '-p', 'fe2o3-kfd', '--all-features', '--lib'], map);
  assert.strictEqual(result.record.clock.predecessor, null);
  assert.strictEqual(result.record.returncode, 0);
  assert(result.log.includes('test result: ok. 1172 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;'));
  assert.strictEqual((result.log.match(/^test .* \.\.\. ok$/gm) || []).length, 1172);
  return result;
}
const [mode, id] = process.argv.slice(2);
if (mode === 'restored') {
  assert.deepStrictEqual(identities(), map);
  console.log(JSON.stringify({development_only: true, restored: true, process_closure_checked: false, source_map_sha256: plan.source_map_sha256, identities: Object.keys(map).length}));
} else if (mode === 'baseline') {
  const {record} = baseline();
  assert.deepStrictEqual(identities(), map);
  console.log(JSON.stringify({development_only: true, baseline_passed: 1172, elapsed_seconds: record.elapsed_seconds, source_map_sha256: plan.source_map_sha256, quiescent: true}));
} else {
  assert.strictEqual(mode, 'negative');
  const mutation = plan.mutations.find(m => m.id === id);
  assert(mutation, 'declared mutation');
  const changed = identities();
  assert.deepStrictEqual(Object.keys(changed), Object.keys(map));
  assert.deepStrictEqual(Object.keys(map).filter(name => map[name] !== changed[name]), [mutation.patch.path]);
  assert.strictEqual(changed[mutation.patch.path], mutantHashes[id], 'exact planned mutation');
  const name = 'r121-development-negative-' + id + '-v1';
  const {record, log} = completed(name, ['cargo', 'test', '-p', 'fe2o3-kfd', '--all-features', '--lib', mutation.test, '--', '--exact'], changed);
  const prior = baseline().record.clock.source_verified;
  const predecessorName = 'r121-development-kfd-all-v1.json';
  assert.deepStrictEqual(record.clock.predecessor, {name: predecessorName, sha256: hash(read(predecessorName)), observation: prior});
  assert.strictEqual(record.clock.start.boot_id, prior.boot_id);
  assert(BigInt(record.clock.start.monotonic_ns) >= BigInt(prior.monotonic_ns));
  const blocks = log.split('---- ' + mutation.test + ' stdout ----');
  const body = blocks.length === 2 ? blocks[1].split('\nfailures:\n')[0] : '';
  const panic = body.slice(body.lastIndexOf("thread '"));
  const oracleMatched = record.returncode === 101 && /Finished `test` profile/.test(log)
    && log.includes('running 1 test\n')
    && log.includes('test ' + mutation.test + ' ... FAILED\n')
    && log.includes('test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1171 filtered out;')
    && mutation.oracle.every(fragment => panic.includes(fragment));
  console.log(JSON.stringify({development_only: true, id, quiescent: true, expected_failure_observed: oracleMatched,
    source_map_sha256: record.source_map_sha256, log_sha256: hash(log), decisive_panic: panic.trim()}));
  process.exitCode = oracleMatched ? 0 : 1;
}
