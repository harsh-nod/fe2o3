// Qualify a negative only by its declared behavioral assertion on exact scoped source.
const fs = require('fs'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const checker = root + 'r125-development-check-v3.js';
const checkerHash = '308fa81a3d7a475491f48490f8ab77761bcb0c45fb30ffbb77796f61c0e8aaaa';
assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(checker)).digest('hex'), checkerHash);
const C = require(checker), {E, map} = C;
C.pin(checker, checkerHash);
const p = JSON.parse(C.pin('r125-development-mutations-v3.json', '742c769f49e10907c0c971b6394ec9b503b54bc3c108fb028e860c3f10aca00e'));
C.pin(p.source_plan, p.source_plan_sha256);
C.pin(p.inventory, p.inventory_sha256);
const inventory = require(root + p.inventory);
assert.strictEqual(inventory.source_map_sha256, C.p.source_map_sha256);
assert.deepStrictEqual(p.mutations.map(mutation => mutation.id), inventory.rows.map(row => row[0]));
assert.deepStrictEqual(p.preserved_tests, inventory.preserved_tests);
assert.deepStrictEqual(p.allowed_tests, [...new Set([...C.p.new_tests.kfd, ...p.preserved_tests])].sort());
assert.strictEqual(new Set(p.mutations.map(mutation => mutation.patch.path)).size, 4);
assert.strictEqual(p.accepted, false);
assert.strictEqual(p.checker_sha256, checkerHash);
assert.strictEqual(p.source_parent, C.p.source_parent);
assert.strictEqual(p.source_map_sha256, C.p.source_map_sha256);
assert.strictEqual(p.source_count, C.p.source_count);
assert.strictEqual(p.test_count, C.p.counts.kfd);
assert.strictEqual(p.runner, C.p.runner);
assert.strictEqual(p.runner_sha256, C.p.runner_sha256);
assert.strictEqual(p.mutations.length, 18);
for (const [index, mutation] of p.mutations.entries())
  assert.strictEqual(mutation.predecessor, index === 0 ? 'r125-development-gate-check-all-v1.json'
    : 'r125-development-restored-' + p.mutations[index - 1].id + '-v1.json');
assert.strictEqual(new Set(p.mutations.map(mutation => mutation.expected_source_map_sha256)).size, 18);
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
  assert(p.allowed_tests.includes(mutation.test), 'new or explicitly preserved test');
  assert(!/panic in a destructor during cleanup|panic in a function that cannot unwind|thread caused non-unwinding panic|SIGABRT|signal: 6\b|^Aborted(?: \(core dumped\))?\s*$|fatal runtime error:|has overflowed its stack|memory allocation of \d+ bytes failed|could not compile|^error\[E\d+\]:/m.test(log));
  const blocks = log.split('---- ' + mutation.test + ' stdout ----');
  assert.strictEqual(blocks.length, 2, 'one selected failure body');
  const body = blocks[1].split('\nfailures:\n')[0];
  const headers = [...body.matchAll(/^thread '([^'\n]+)' \(([1-9][0-9]*)\) panicked at ([^\n]+):\n/gm)];
  assert(headers.length > 0, 'complete panic headers');
  const rawHeaders = [...log.matchAll(/^thread\b[^\n]*(?:\n|$)/gm)].map(header => header[0]);
  assert.deepStrictEqual(rawHeaders, headers.map(header => header[0]),
    'every panic header is complete and inside the selected failure body');
  const panics = headers.map((header, index) => ({
    test: header[1], location: header[3], text: body.slice(header.index, headers[index + 1]?.index ?? body.length).trim(),
  }));
  assert(panics.every(panic => panic.test === mutation.test), 'all panic records belong to the selected test');
  const decisive = panics.filter(panic => panic.location === location(mutation.oracle));
  assert.strictEqual(decisive.length, 1, 'one predeclared behavioral assertion');
  const primary = decisive[0];
  assert(mutation.oracle.fragments.every(fragment => primary.text.includes(fragment)), 'predeclared behavioral diagnostic');
  if (mutation.final_panic === 'caught_then_unwrap') {
    assert.strictEqual(panics.length, 2, 'only the declared invariant and outer catch failure');
    assert.strictEqual(panics.at(-2), primary, 'declared invariant immediately precedes outer catch failure');
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
  const rows = [...log.matchAll(/^test (?!result:).+$/gm)].map(match => match[0]);
  const failedRow = 'test ' + mutation.test + ' ... FAILED';
  const progressRow = 'test ' + mutation.test + ' has been running for over 60 seconds';
  assert.deepStrictEqual(rows, rows.length === 2 ? [progressRow, failedRow] : [failedRow],
    'only the selected failure and an optional preceding libtest progress row');
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
  const name = 'r125-development-negative-' + mutation.id + '-v1';
  const {record, log} = C.completed(name, [...p.test_command, mutation.test, '--', '--exact'],
    mutation.predecessor, changed, 101);
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
