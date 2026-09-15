// A negative qualifies only through its declared behavioral assertion on exact scoped source.
const fs = require('fs'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const checker = root + 'r123-development-check-v1.js';
const checkerHash = 'd83f5ab4a189ab67e52d1a938bfacf85d01081b8bdcd3646ca96023387892c4e';
assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(checker)).digest('hex'), checkerHash);
const C = require(checker), {E, map} = C;
C.pin(checker, checkerHash);
const p = JSON.parse(C.pin('r123-development-mutations-v1.json', '592a345f8a8876ea789cae1385f33b62224f04f24c2135c62d46c0f63e481c52'));
C.pin(p.source_plan, p.source_plan_sha256);
assert.strictEqual(p.accepted, false);
assert.strictEqual(p.checker_sha256, checkerHash);
assert.strictEqual(p.source_parent, C.p.source_parent);
assert.strictEqual(p.source_map_sha256, C.p.source_map_sha256);
assert.strictEqual(p.source_count, C.p.source_count);
assert.strictEqual(p.test_count, C.p.counts.kfd);
assert.strictEqual(p.runner, C.p.runner);
assert.strictEqual(p.runner_sha256, C.p.runner_sha256);
assert.strictEqual(p.mutations.length, 10);
assert.strictEqual(new Set(p.mutations.map(mutation => mutation.expected_source_map_sha256)).size, 10);
function assertMutationSource(mutation, changed) {
  assert.strictEqual(C.hash(changed), mutation.expected_file_sha256, 'declared mutated bytes');
  const reverse = {...mutation.patch, edits: mutation.patch.edits.toReversed().map(([from, to]) => [to, from])};
  const original = E.core.mutatedSource(changed, reverse);
  assert.strictEqual(C.hash(original), map[mutation.patch.path], 'authenticated recovered original');
  assert.strictEqual(E.core.mutatedSource(original, mutation.patch), changed, 'exact scoped source derivation');
}
function location(oracle) {
  const parsed = /^(.+):([1-9][0-9]*):([1-9][0-9]*)$/.exec(oracle.location);
  assert(parsed, 'declared assertion coordinates');
  return [p.diagnostic_path_aliases[parsed[1]] || parsed[1], parsed[2], parsed[3]].join(':');
}
function assertNegative(log, mutation) {
  assert(p.mutations.some(item => item.id === mutation.id && item.test === mutation.test));
  assert(C.p.new_tests.kfd.includes(mutation.test));
  assert(!/panic in a destructor during cleanup|panic in a function that cannot unwind|thread caused non-unwinding panic|SIGABRT|signal: 6\b|^Aborted(?: \(core dumped\))?\s*$|fatal runtime error:|has overflowed its stack|memory allocation of \d+ bytes failed|could not compile|^error\[E\d+\]:/m.test(log));
  const blocks = log.split('---- ' + mutation.test + ' stdout ----');
  assert.strictEqual(blocks.length, 2, 'one selected failure body');
  const body = blocks[1].split('\nfailures:\n')[0];
  const headers = [...body.matchAll(/^thread '([^'\n]+)' \(([1-9][0-9]*)\) panicked at ([^\n]+):\n/gm)];
  assert(headers.length > 0, 'complete panic headers');
  const panics = headers.map((header, index) => ({
    test: header[1], location: header[3], text: body.slice(header.index, headers[index + 1]?.index ?? body.length).trim(),
  }));
  assert(panics.every(panic => panic.test === mutation.test), 'all panic records belong to the selected test');
  const decisive = panics.filter(panic => panic.location === location(mutation.oracle));
  assert.strictEqual(decisive.length, 1, 'one predeclared behavioral assertion');
  const primary = decisive[0];
  assert(mutation.oracle.fragments.every(fragment => primary.text.includes(fragment)), 'predeclared behavioral diagnostic');
  if (mutation.final_panic === 'caught_then_unwrap') {
    assert.strictEqual(panics.at(-2), primary, 'transport assertion immediately precedes outer catch failure');
    const following = panics.at(-1);
    assert.strictEqual(following.location, location(mutation.following_panic), 'declared outer unwrap location');
    assert(mutation.following_panic.fragments.every(fragment => following.text.includes(fragment)));
  } else {
    assert.strictEqual(mutation.final_panic, 'primary');
    assert.strictEqual(panics.at(-1), primary, 'last panic is the declared behavioral assertion');
  }
  assert.deepStrictEqual(E.failed(log), [mutation.test]);
  assert.deepStrictEqual(E.passing(log), []);
  assert.deepStrictEqual(E.ignored(log), []);
  assert.deepStrictEqual([...log.matchAll(/^test (?!result:).+$/gm)].map(match => match[0]), ['test ' + mutation.test + ' ... FAILED']);
  assert.deepStrictEqual(E.totals(log), {harnesses: 1, passed: 0, failed: 1, ignored: 0});
  assert.deepStrictEqual(C.summaries(log), [['FAILED', 0, 1, 0, 0, p.test_count - 1]]);
  const markers = [...log.matchAll(/^     Running (.+)$/gm)];
  assert.deepStrictEqual(markers.map(marker => marker[1].replace(/-[0-9a-f]{16}\)$/, ')')),
    ['unittests src/lib.rs (target/debug/deps/fe2o3_kfd)'], 'exact sole negative executable');
  assert(!/^test result:/m.test(log.slice(0, markers[0].index)), 'no unaccounted summary');
  assert(!/^   Doc-tests /m.test(log), 'no extra doctest executable');
  assert.strictEqual((log.match(/^\s*Finished \x60test\x60 profile.*$/gm) || []).length, 1);
  assert.strictEqual((log.match(/^running 1 test$/gm) || []).length, 1);
  return primary.text;
}
function negative(mutation) {
  const changed = C.identities();
  assert.deepStrictEqual(Object.keys(changed), Object.keys(map));
  assert.deepStrictEqual(Object.keys(map).filter(name => map[name] !== changed[name]), [mutation.patch.path]);
  assert.strictEqual(C.hash(JSON.stringify(changed)), mutation.expected_source_map_sha256);
  assertMutationSource(mutation, C.bytes(C.repo + '/' + mutation.patch.path).toString());
  C.baseline();
  const name = 'r123-development-negative-' + mutation.id + '-v1';
  const {record, log} = C.completed(name, [...p.test_command, mutation.test, '--', '--exact'],
    'r123-development-gnu-all-v1.json', changed, 101);
  const panic = assertNegative(log, mutation);
  assert.deepStrictEqual(C.identities(), changed, 'unchanged closing mutant source');
  return {development_only: true, packet_accepted: false, id: mutation.id, quiescent: true,
    expected_failure_observed: true, source_map_sha256: record.source_map_sha256, log_sha256: C.hash(log), decisive_panic: panic};
}
module.exports = {C, p, assertMutationSource, location, assertNegative, negative};
if (require.main === module) {
  C.bytes(__filename);
  const [mode, id] = process.argv.slice(2);
  let report;
  if (mode === 'restored') {
    assert.deepStrictEqual(C.identities(), map);
    report = {development_only: true, restored: true, process_closure_checked: false, source_map_sha256: p.source_map_sha256};
  } else {
    assert.strictEqual(mode, 'negative');
    const mutation = p.mutations.find(mutation => mutation.id === id);
    assert(mutation, 'declared mutation');
    report = negative(mutation);
  }
  C.stable();
  console.log(JSON.stringify(report));
}
