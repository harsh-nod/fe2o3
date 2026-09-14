const assert = require('assert');
const e = require('./r116-qualification-evidence.js');
const p = require('./r116-qualification-plan.js');
const [name, command, previous] = p.prerequisites[0];
const original = e.read(name + '.json');
const source = e.read('r116-frozen-source.json');
let count = 0;
function check(label, alter, rejects = true) {
  const record = structuredClone(original);
  alter(record);
  const verify = () => e.checkRecord(record, name, command, source, previous, 0);
  if (rejects) assert.throws(verify, assert.AssertionError, label);
  else verify();
  count++; console.log('PASS ' + label);
}
check('unchanged-record', () => {}, false);
check('unclosed-child', r => {r.child_closed = false;});
check('raw-child-mismatch', r => {r.child_returncode = 101;});
check('runner-substitution', r => {r.runner.sha256 = '0'.repeat(64);});
check('changed-source', r => {r.source_unchanged = false;});
check('wrong-source-hash', r => {r.source_map_sha256 = '0'.repeat(64);});
check('timeout', r => {r.timed_out = true;});
check('signal', r => {r.signal = 'SIGKILL';});
check('spawn-error', r => {r.spawn_error = 'ENOENT';});
check('changed-command', r => {r.command.push('--quiet');});
check('changed-deadline', r => {r.deadline_ms++;});
check('changed-environment', r => {r.environment.RUST_TEST_THREADS = '1';});
check('different-boot', r => {r.clock.finish.boot_id = '00000000-0000-0000-0000-000000000001';});
check('different-clock-source', r => {r.clock.finish.clock_source = 'other';});
check('monotonic-regression', r => {r.clock.finish.monotonic_ns = '0';});
check('invalid-raw-utc', r => {r.clock.finish.utc_ms = -1;});
check('timestamp-rewrite', r => {r.finished_at = r.started_at;});
check('predecessor-substitution', r => {r.clock.predecessor.sha256 = '0'.repeat(64);});
check('cleanup-error', r => {r.process_group_cleanup.close.status = 'error';});
check('live-descendant', r => {r.process_group_cleanup.close.live_members = [42];});
check('cleanup-after-verification', r => {r.process_group_cleanup.close.observation.monotonic_ns = String(BigInt(r.clock.source_verified.monotonic_ns) + 1n);});
check('elapsed-rewrite', r => {r.elapsed_seconds++;});
check('raw-utc-regression-is-preserved', r => {
  r.clock.finish.utc_ms = r.clock.start.utc_ms - 10;
  r.finished_at = new Date(r.clock.finish.utc_ms).toISOString();
}, false);
for (const suffix of ['-source.json', '-source-after.json']) {
  const io = {bytes: e.bytes, read: artifact => artifact === name + suffix ? {...source, 'Cargo.lock': '0'.repeat(64)} : e.read(artifact)};
  assert.throws(() => e.checkRecord(original, name, command, source, previous, 0, io), assert.AssertionError);
  count++; console.log('PASS substituted' + suffix);
}
const mutation = p.mutations[0];
const [anchorName, anchorCommand, anchorPrevious] = p.initialRuns[0];
const anchor = e.read(anchorName + '.json');
e.checkRecord(anchor, anchorName, anchorCommand, source, anchorPrevious, 0);
count++; console.log('PASS independent-initial-anchor');
for (const [label, modify] of [
  ['anchor-unexpected-predecessor', r => {r.clock.predecessor = original.clock.predecessor;}],
  ['anchor-unclosed-child', r => {r.child_closed = false;}],
  ['anchor-wrong-command', r => {r.command = ['false'];}],
]) {
  const r = structuredClone(anchor); modify(r);
  assert.throws(() => e.checkRecord(r, anchorName, anchorCommand, source, anchorPrevious, 0), assert.AssertionError);
  count++; console.log('PASS ' + label);
}
for (const suffix of ['-source.json', '-source-after.json']) {
  const io = {bytes: e.bytes, read: artifact => artifact === anchorName + suffix ? {...source, 'Cargo.lock': '0'.repeat(64)} : e.read(artifact)};
  assert.throws(() => e.checkRecord(anchor, anchorName, anchorCommand, source, anchorPrevious, 0, io), assert.AssertionError);
  count++; console.log('PASS anchor-substituted' + suffix);
}
const otherBoot = {...anchor.clock.start, boot_id: anchor.clock.start.boot_id === '00000000-0000-0000-0000-000000000001'
  ? '00000000-0000-0000-0000-000000000002' : '00000000-0000-0000-0000-000000000001'};
assert.throws(() => e.ordered(anchor.clock.start, otherBoot), assert.AssertionError);
count++; console.log('PASS cross-boot-causal-edge-rejected');
const diagnostic = ['Finished `test` profile', 'Running unittests', 'running 1 test',
  'test ' + mutation.test + ' ... FAILED', "thread '" + mutation.test + "' (123) panicked at " + mutation.oracle_path + ':' + mutation.oracle_line + ':1:',
  ...mutation.expected, 'test result: FAILED. 0 passed; 1 failed; 0 ignored;'].join('\n');
e.checkMutation(diagnostic, mutation); count++; console.log('PASS valid-synthetic-diagnostic');
for (const [label, text] of [
  ['wrong-oracle', diagnostic.replace(mutation.oracle_path + ':' + mutation.oracle_line, 'other.rs:1')],
  ['wrong-test', diagnostic.replace('test ' + mutation.test + ' ... FAILED', 'test other ... FAILED')],
  ['compile-error', diagnostic + '\nerror[E0308]: wrong type'],
  ['missing-diagnostic', diagnostic.replace(mutation.expected.at(-1), '')],
  ['unrelated-location-is-not-oracle', diagnostic.replace(mutation.oracle_path + ':' + mutation.oracle_line, 'other.rs:1') + '\n' + mutation.oracle_path + ':' + mutation.oracle_line + ':1:'],
]) {
  assert.throws(() => e.checkMutation(text, mutation), assert.AssertionError, label);
  count++; console.log('PASS ' + label);
}
assert(e.canRestore(original)); count++; console.log('PASS closed-owner-can-restore');
for (const [label, modify] of [
  ['unclosed-owner-cannot-restore', r => {r.child_closed = false;}],
  ['cleanup-error-cannot-restore', r => {r.process_group_cleanup.close.status = 'error';}],
  ['live-owner-cannot-restore', r => {r.process_group_cleanup.close.live_members = [42];}],
]) {
  const record = structuredClone(original); modify(record); assert.strictEqual(e.canRestore(record), false);
  count++; console.log('PASS ' + label);
}
const originals = p.productionPaths.map(path => ({path, text: require('fs').readFileSync(require('path').join(e.repo, path), 'utf8')}));
const entries = e.expectedEntries(source, originals);
assert.strictEqual(Object.keys(entries).length, 2 * p.mutations.length + p.focused.length + 1);
assert.deepStrictEqual(entries['r116-qualified-mut-' + mutation.name + '.json'].command, [...p.test, mutation.test, '--', '--exact', '--test-threads=1']);
count++; console.log('PASS exact-planned-command');
assert.deepStrictEqual(e.originalSources(source, originals), Object.fromEntries(originals.map(s => [s.path, s.text])));
count++; console.log('PASS exact-original-source-collection');
for (const [label, modify] of [
  ['missing-original', a => {a.pop();}],
  ['duplicate-original', a => {a[1] = a[0];}],
  ['reordered-original', a => {a.reverse();}],
  ['extra-original', a => {a.push({path: 'other.rs', text: 'other'});}],
  ['missing-original-text', a => {delete a[0].text;}],
  ['extra-original-field', a => {a[0].extra = true;}],
  ['nonstring-original', a => {a[0].text = 123;}],
  ['substituted-original', a => {a[0].text += 'substituted';}],
]) {
  const values = structuredClone(originals); modify(values);
  assert.throws(() => e.expectedEntries(source, values), assert.AssertionError, label);
  count++; console.log('PASS ' + label);
}
for (const [label, altered] of [
  ['undeclared-mutation-source', {...mutation, path: 'Cargo.lock'}],
  ['no-op-mutation', {...mutation, edits: [[mutation.edits[0][0], mutation.edits[0][0]]]}],
  ['missing-mutation-anchor', {...mutation, edits: [['absent exact source anchor', 'changed']]}],
]) {
  assert.throws(() => e.mutationSource(source, originals, altered), assert.AssertionError, label);
  count++; console.log('PASS ' + label);
}
for (const path of p.productionPaths) {
  const selected = {...mutation, path};
  assert.deepStrictEqual(e.recoverySource('manifest', source, selected), {manifest: 'manifest', path, sha256: source[path]});
}
count++; console.log('PASS recovery-selects-each-original-file');
assert.throws(() => e.recoverySource('manifest', source, {...mutation, path: 'Cargo.lock'}), assert.AssertionError);
count++; console.log('PASS undeclared-recovery-source');
const mutatedMap = {...source, [mutation.path]: e.hash(e.mutationSource(source, originals, mutation))};
const twoPaths = {...mutatedMap, 'Cargo.lock': '0'.repeat(64)};
const alteredRecord = structuredClone(original);
alteredRecord.source_map_sha256 = e.hash(JSON.stringify(mutatedMap));
const extraMapIO = {bytes: e.bytes, read: artifact => artifact === name + '-source.json' || artifact === name + '-source-after.json' ? twoPaths : e.read(artifact)};
assert.throws(() => e.checkRecord(alteredRecord, name, command, mutatedMap, previous, 0, extraMapIO), assert.AssertionError);
count++; console.log('PASS unexpected-second-mutated-source');
assert.strictEqual(new Set(p.mutations.map(m => e.hash(JSON.stringify({...source,
  [m.path]: e.hash(e.mutationSource(source, originals, m))})))).size, p.mutations.length);
count++; console.log('PASS exact-distinct-full-mutant-maps');
for (const mode of ['closed', 'edit-during-scan', 'prior-edit', 'unclosed', 'cleanup-error', 'live-group', 'not-spawned']) {
  const record = structuredClone(original);
  let bytes = mode === 'prior-edit' ? 'external' : 'mutated';
  let scans = 0;
  let writes = 0;
  let reads = 0;
  if (mode === 'unclosed') record.child_closed = false;
  if (mode === 'cleanup-error') record.process_group_cleanup.close.status = 'error';
  if (mode === 'not-spawned') {
    record.spawn_error = 'ENOENT'; record.child_returncode = -2;
    record.process_group_cleanup.close = {status: 'not_spawned'};
  }
  const target = '/fixture/' + mutation.path;
  const io = {
    live: pgid => {assert.strictEqual(pgid, record.process_group_cleanup.close.pgid); scans++;
      if (mode === 'edit-during-scan') bytes = 'external'; return mode === 'live-group' ? [42] : [];},
    read: file => {assert.strictEqual(file, target); reads++; return bytes;},
    write: (file, value) => {assert.strictEqual(file, target); assert.strictEqual(bytes, 'mutated'); writes++; bytes = value;},
  };
  const result = e.restoreOwnedMutation(target, e.hash('mutated'), 'original', record, io);
  const success = ['closed', 'not-spawned'].includes(mode);
  const external = ['edit-during-scan', 'prior-edit'].includes(mode);
  const quiescent = success || external;
  assert.strictEqual(result.restored, success);
  assert.strictEqual(result.quiescent, quiescent);
  assert.strictEqual(result.external_edit, external);
  assert.strictEqual(writes, Number(success));
  assert.strictEqual(reads, Number(quiescent));
  assert.strictEqual(scans, Number(!['unclosed', 'cleanup-error', 'not-spawned'].includes(mode)));
  assert.strictEqual(bytes, success ? 'original' : external ? 'external' : 'mutated');
  count++; console.log('PASS restoration-' + mode);
}
for (const [label, text] of [
  ['r116-gnu-target-roster', e.bytes('r116-reviewed-gnu-all.log').toString()],
  ['r116-musl-target-roster', e.bytes('r116-reviewed-musl-all.log').toString()],
  ['r115-gnu-target-roster', e.committed('raw/r115-refreshed-gnu-all.log')],
  ['r115-musl-target-roster', e.committed('raw/r115-refreshed-musl-all.log')],
]) {
  const targets = e.executables(text);
  assert.strictEqual(targets.length, 49);
  assert.strictEqual(targets.filter(t => t.kind === 'libtest').length, 48);
  assert.strictEqual(targets.filter(t => t.kind === 'harnessless-benchmark').length, 1);
  count++; console.log('PASS ' + label);
}
const summary = 'test result: ok. 1 passed; 0 failed; 0 ignored;';
const ordinary = '     Running unittests src/lib.rs (target/debug/deps/example-0000000000000000)\n' +
  'running 1 test\ntest example ... ok\n' + summary + '\n';
const benchmark = '     Running benches/completion_scaling.rs (target/debug/deps/completion_scaling-0000000000000000)\n' +
  'nodes,construction_ns,transitions_ns,transition_ns_per_node\n' +
  '1024,1,2,0.01\n4096,3,4,0.02\n16384,5,6,0.03\n65536,7,8,0.04\n';
const mixed = ordinary + benchmark;
assert.deepStrictEqual(e.executables(mixed.replace('1024,1,2,0.01', '1024,9,8,1.23')), e.executables(mixed));
count++; console.log('PASS benchmark-timings-are-not-compared');
for (const [label, text] of [
  ['unknown-summary-free-target', mixed.replace('benches/completion_scaling.rs', 'benches/unknown.rs')],
  ['duplicate-benchmark-target', mixed + benchmark],
  ['missing-libtest-summary', mixed.replace(summary, '')],
  ['duplicate-libtest-summary', mixed.replace(summary, summary + '\n' + summary)],
  ['unaccounted-summary', summary + '\n' + mixed],
  ['missing-csv-row', mixed.replace('65536,7,8,0.04\n', '')],
  ['duplicate-csv-row', mixed + '65536,7,8,0.04\n'],
  ['reordered-csv-rows', mixed.replace('1024,1,2,0.01\n4096,3,4,0.02', '4096,3,4,0.02\n1024,1,2,0.01')],
  ['wrong-csv-header', mixed.replace('nodes,construction_ns', 'items,construction_ns')],
  ['malformed-csv-number', mixed.replace('1024,1,2,0.01', '1024,1,NaN,0.01')],
  ['unexpected-benchmark-output', mixed + 'unexpected output\n'],
]) {
  assert.throws(() => e.executables(text), assert.AssertionError, label);
  count++; console.log('PASS ' + label);
}
const {checkFull, docNames} = require('./r116-qualification-collect.js');
for (const target of ['gnu', 'musl']) {
  const log = e.bytes('r116-reviewed-' + target + '-all.log').toString();
  const old = e.committed('raw/r115-refreshed-' + target + '-all.log');
  checkFull(log, old); count++; console.log('PASS exact-' + target + '-full-roster');
  const from = 'test ' + p.newTests[0] + ' ... ok';
  assert(log.includes(from));
  assert.throws(() => checkFull(log.replace(from, 'test substituted::test ... ok'), old), assert.AssertionError);
  count++; console.log('PASS substituted-' + target + '-passing-test');
}
for (const target of ['gnu', 'musl']) {
  const old = e.passing(e.committed('raw/r115-final-' + target + '-docs.log'));
  const renamed = docNames(old);
  assert.strictEqual(old.filter(n => !renamed.includes(n)).length, p.docRelocations.length);
  assert.deepStrictEqual(renamed, e.passing(e.bytes('r116-final-' + target + '-docs.log').toString()));
}
count++; console.log('PASS exact-unchanged-doctest-relocations');
const originalHistory = e.checkOriginalIntegratedHistory();
const originalSource = originalHistory.map;
assert.strictEqual(originalHistory.accepted_as_final_source_prerequisite, false);
assert.strictEqual(originalHistory.runs, 4);
assert.strictEqual(originalHistory.artifacts, 16);
count++; console.log('PASS exact-original-integrated-history');
for (const [label, name, modify] of [
  ['map', 'r116-initial-format-source.json', r => {r['Cargo.lock'] = '0'.repeat(64);}],
  ['last-unclosed', 'r116-preliminary-musl-all.json', r => {r.child_closed = false;}],
  ['last-predecessor', 'r116-preliminary-musl-all.json', r => {r.clock.predecessor.sha256 = '0'.repeat(64);}],
  ['after-map', 'r116-preliminary-gnu-all-source-after.json', r => {r['Cargo.lock'] = '0'.repeat(64);}],
]) {
  const changed = structuredClone(e.read(name)); modify(changed);
  assert.throws(() => e.checkOriginalIntegratedHistory({bytes: e.bytes,
    read: artifact => artifact === name ? changed : e.read(artifact)}), assert.AssertionError);
  count++; console.log('PASS original-integrated-reject-' + label);
}
for (const name of ['r116-initial-journal.log', 'r116-preliminary-gnu-all.log']) {
  const log = e.bytes(name).toString();
  const marker = 'test ' + p.originalNewTests[0] + ' ... ok';
  assert(log.includes(marker));
  assert.throws(() => e.checkOriginalIntegratedHistory({read: e.read,
    bytes: artifact => artifact === name ? Buffer.from(log.replace(marker, 'test substituted ... ok')) : e.bytes(artifact)}), assert.AssertionError);
  count++; console.log('PASS original-integrated-reject-roster-' + name);
}
assert.throws(() => e.checkOriginalIntegratedHistory({read: e.read,
  bytes: name => name === 'r116-run-v1.js' ? Buffer.concat([e.bytes(name), Buffer.from('changed')]) : e.bytes(name)}), assert.AssertionError);
count++; console.log('PASS original-integrated-reject-runner');
e.checkTestExtension(originalSource, source);
count++; console.log('PASS exact-append-only-test-extension');
assert.throws(() => e.checkTestExtension({...originalSource, 'Cargo.lock': '0'.repeat(64)}, source), assert.AssertionError);
count++; console.log('PASS extension-reject-wrong-original-map');
const missing = {...source}; delete missing['Cargo.lock'];
assert.throws(() => e.checkTestExtension(originalSource, missing), assert.AssertionError);
count++; console.log('PASS extension-reject-missing-key');
assert.throws(() => e.checkTestExtension(originalSource, {...source, 'Cargo.lock': '0'.repeat(64)}), assert.AssertionError);
count++; console.log('PASS extension-reject-unrelated-change');
const extensionBytes = require('fs').readFileSync(require('path').join(e.repo, p.testExtension.path));
assert.throws(() => e.checkTestExtension(originalSource, source, () => extensionBytes.subarray(0, p.testExtension.originalBytes)), assert.AssertionError);
count++; console.log('PASS extension-reject-empty-addition');
const prefixChanged = Buffer.from(extensionBytes); prefixChanged[0] ^= 1;
assert.throws(() => e.checkTestExtension(originalSource, {...source, [p.testExtension.path]: e.hash(prefixChanged)}, () => prefixChanged), assert.AssertionError);
count++; console.log('PASS extension-reject-rehashed-prefix-edit');
assert.throws(() => e.checkTestExtension(originalSource, source, () => Buffer.concat([extensionBytes, Buffer.from('changed')])), assert.AssertionError);
count++; console.log('PASS extension-reject-whole-file-substitution');
assert.strictEqual(e.checkIsolatedHistory(originalSource).accepted_as_integrated_prerequisite, false);
count++; console.log('PASS exact-original-context-history');
const firstIsolated = 'v3candidate-' + p.isolatedRuns[0][0];
for (const [label, change] of [
  ['head', r => {r.source_head = p.parent;}],
  ['cwd', r => {r.cwd = e.repo;}],
  ['clock-contract', r => {r.clock.contract = e.contract;}],
  ['runner-name', r => {r.runner.name = 'r116-run-v1.js';}],
  ['runner-hash', r => {r.runner.sha256 = '0'.repeat(64);}],
  ['unclosed', r => {r.child_closed = false;}],
  ['child-result', r => {r.child_returncode = 101;}],
  ['formatter-source-change', r => {r.source_unchanged = true;}],
  ['command', r => {r.command = ['true'];}],
  ['deadline', r => {r.deadline_ms++;}],
  ['boot', r => {r.clock.finish.boot_id = r.clock.start.boot_id === '00000000-0000-0000-0000-000000000001'
    ? '00000000-0000-0000-0000-000000000002' : '00000000-0000-0000-0000-000000000001';}],
  ['source-hash', r => {r.source_map_sha256 = '0'.repeat(64);}],
  ['predecessor', r => {r.clock.predecessor = {}; }],
]) {
  const record = structuredClone(e.read(firstIsolated + '.json'));
  change(record);
  const io = {bytes: e.bytes, read: name => name === firstIsolated + '.json' ? record : e.read(name)};
  assert.throws(() => e.checkIsolatedHistory(originalSource, io), assert.AssertionError, label);
  count++; console.log('PASS isolated-reject-' + label);
}
for (const suffix of ['-source.json', '-source-after.json']) {
  const name = firstIsolated + suffix;
  const changed = {...e.read(name), 'Cargo.lock': '0'.repeat(64)};
  const io = {bytes: e.bytes, read: artifact => artifact === name ? changed : e.read(artifact)};
  assert.throws(() => e.checkIsolatedHistory(originalSource, io), assert.AssertionError);
  count++; console.log('PASS isolated-reject-' + suffix);
}
for (const name of ['v3candidate-model-all.log', 'v3candidate-lineage-model-all.log']) {
  const text = e.bytes(name).toString();
  const marker = 'test ' + p.originalNewTests[0] + ' ... ok';
  assert(text.includes(marker));
  const io = {read: e.read, bytes: artifact => artifact === name ? Buffer.from(text.replace(marker, 'test substituted ... ok')) : e.bytes(artifact)};
  assert.throws(() => e.checkIsolatedHistory(originalSource, io), assert.AssertionError);
  count++; console.log('PASS isolated-reject-roster-' + name);
}
const changedRunner = {read: e.read, bytes: name => name === p.isolatedContext.runner
  ? Buffer.concat([e.bytes(name), Buffer.from('changed')]) : e.bytes(name)};
assert.throws(() => e.checkIsolatedHistory(originalSource, changedRunner), assert.AssertionError);
count++; console.log('PASS isolated-reject-runner-bytes');
assert.throws(() => e.checkIsolatedHistory({...originalSource, [p.sourceDelta[0]]: '0'.repeat(64)}), assert.AssertionError);
count++; console.log('PASS isolated-reject-different-integrated-candidate');
assert.strictEqual(count, p.qualificationContractCount);
console.log('HELPER_PINS: ' + JSON.stringify(p.helpers.map(name => ({name, sha256: e.hash(e.bytes(name))}))));
console.log('PASS: ' + count + ' qualification contract tests');
