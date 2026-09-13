const fs = require('fs');
const path = require('path');
const assert = require('assert');
const p = require('./r114-qualification-plan.js');
const e = require('./r114-qualification-evidence.js');
const baseline = () => e.read('r114-frozen-source.json');
const artifactSuffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const prerequisiteNames = ['r114-preliminary-gnu-all', 'r114-preliminary-musl-all'];
function checkFull(text, oldText) {
  assert.deepStrictEqual(e.passing(text), [...e.passing(oldText), ...p.newTests].sort());
  assert.deepStrictEqual(e.ignored(text), e.ignored(oldText));
  const targets = e.executables(text);
  const oldTargets = e.executables(oldText);
  assert.strictEqual(targets.length, 49);
  assert.strictEqual(oldTargets.length, 49);
  assert.strictEqual(targets.filter(t => t.kind === 'libtest').length, 48);
  let changed = 0;
  for (const [index, actual] of targets.entries()) {
    const expected = structuredClone(oldTargets[index]);
    if (expected.kind === 'libtest' && /fe2o3_kfd(?:-|\))/.test(expected.name)) {
      expected.passing = [...expected.passing, ...p.newTests].sort();
      expected.totals.passed += p.newTests.length;
      changed++;
    }
    assert.deepStrictEqual(actual, expected, 'exact executable roster: ' + actual.name);
  }
  assert.strictEqual(changed, 1);
  assert.deepStrictEqual(e.totals(text), {harnesses: 48, passed: 2712, failed: 0, ignored: 5});
  return e.totals(text);
}
function docNames(names) {
  const rename = new Map();
  for (const [source, type, oldLine, newLine] of p.docRelocations) {
    const file = 'crates/fe2o3-kfd/src/' + source;
    const oldText = e.git(['show', p.accepted + ':' + file]);
    const newText = fs.readFileSync(path.join(e.repo, file), 'utf8');
    const fence = (text, line) => {
      const lines = text.split('\n');
      assert.strictEqual(lines[line - 1], '/// ```compile_fail');
      const end = lines.findIndex((text, i) => i >= line && text === '/// ```');
      assert(end >= line);
      return lines.slice(line - 1, end + 1);
    };
    assert.deepStrictEqual(fence(newText, newLine), fence(oldText, oldLine), 'unchanged relocated doctest');
    const name = line => file + ' - ' + type + ' (line ' + line + ') - compile fail';
    rename.set(name(oldLine), name(newLine));
  }
  return names.map(name => rename.get(name) || name).sort();
}
function checkHistoricalAttempts(env) {
  const attempts = [
    ['initial-pristine-tests', p.accepted, 0, '8749ce0353fcdd6cee4bf62acff7fdc82737219becbcb896ac094aa76f13a7b0', 20],
    ['cleanup-matrix-tests', p.accepted, 101, '3169a00ee2d7cd9d75ee651e44843038a7fd1998a250684520048dee44f7420e', 'E0560'],
    ['reviewed-pristine-tests', p.accepted, 101, 'bd996fd11291a112754c5a157979c4edc16c50e368946f968e95824d6c042751', 'E0583'],
    ['wired-pristine-tests', p.accepted, 0, '07e344b44fcffd94ba440272d61b2a49455ba2487fdf98ea5b88dd22fa936c8b', 32],
    ['planning-format', p.parent, 0, 'db70390d250d4a854d902c909381e8efa7f5c5f932874d80a20b1851085e45d8', null],
    ['complete-oracle-tests', p.parent, 0, '033381c581a1ade328f6894a272465fce53e72df29b2dc8e8c7afb48cf087474', 37],
    ['first-strict-clippy', p.parent, 0, '033381c581a1ade328f6894a272465fce53e72df29b2dc8e8c7afb48cf087474', null],
    ['device-oracle-format', p.parent, 0, '7e1972654c2e124b8a5adc28b318e92b90c2d885f926eea2d20629de832a5318', null],
  ];
  const pinned = new Set(env.prior_artifacts.map(x => x.name));
  let previous = null;
  for (const [short, head, code, mapHash, outcome] of attempts) {
    const name = 'r114-' + short;
    for (const suffix of artifactSuffixes) assert(pinned.has(name + suffix), 'preserved preliminary artifact');
    const record = e.read(name + '.json');
    const before = e.read(name + '-source.json');
    const after = e.read(name + '-source-after.json');
    assert.strictEqual(record.source_head, head);
    assert.strictEqual(record.source_map_sha256, mapHash);
    assert.strictEqual(e.hash(JSON.stringify(before)), mapHash);
    assert.strictEqual(record.returncode, code);
    assert.strictEqual(record.child_returncode, code);
    assert.strictEqual(record.child_closed, true);
    assert.strictEqual(record.timed_out, false);
    assert.strictEqual(record.signal, null);
    assert.strictEqual(record.spawn_error, null);
    assert(e.canRestore(record), 'closed historical process group');
    const changed = short.endsWith('-format');
    assert.strictEqual(record.source_unchanged, !changed);
    if (changed) assert.notDeepStrictEqual(before, after); else assert.deepStrictEqual(before, after);
    const runner = head === p.accepted ? 'r114-run-v3.js' : 'r114-run-v4.js';
    assert.deepStrictEqual(record.runner, {name: runner, sha256: e.hash(e.bytes(runner))});
    assert.strictEqual(record.clock.contract, e.contract);
    assert.strictEqual(record.clock.error, null);
    if (short === 'planning-format') previous = null;
    if (previous) {
      const prior = e.read(previous);
      assert.deepStrictEqual(record.clock.predecessor, {name: previous, sha256: e.hash(e.bytes(previous)), observation: prior.clock.source_verified});
      e.ordered(prior.clock.source_verified, record.clock.start);
    } else assert.strictEqual(record.clock.predecessor, null);
    e.ordered(record.clock.start, record.clock.finish);
    e.ordered(record.clock.finish, record.clock.source_verified);
    const log = e.bytes(name + '.log').toString();
    if (typeof outcome === 'number') assert.deepStrictEqual(e.totals(log), {harnesses: 1, passed: outcome, failed: 0, ignored: 0});
    if (typeof outcome === 'string') assert(log.includes('error[' + outcome + ']'));
    previous = name + '.json';
  }
}
function contractTranscript(log, kind, count) {
  const passes = log.split('\n').filter(line => line.startsWith('PASS '));
  assert.strictEqual(passes.length, count);
  assert.strictEqual(new Set(passes).size, count);
  assert(log.endsWith('PASS: ' + count + ' ' + kind + ' contract tests\n'));
}
function checkBaseline() {
  const map = baseline();
  assert.strictEqual(Object.keys(map).length, p.sourceCount);
  assert.deepStrictEqual(e.identities(), map);
  const oldMap = JSON.parse(e.committed('raw/r113-frozen-source.json'));
  assert.deepStrictEqual([...new Set([...Object.keys(oldMap), ...Object.keys(map)])].filter(n => oldMap[n] !== map[n]).sort(), p.sourceDelta);
  const env = e.read('r114-environment.json');
  assert.strictEqual(env.publication_parent, p.parent);
  assert.strictEqual(env.accepted_runtime_checkpoint, p.accepted);
  assert.deepStrictEqual(env.source_delta, p.sourceDelta);
  assert.deepStrictEqual(env.binaries, JSON.parse(e.committed('raw/r113-environment.json')).binaries);
  for (const binary of env.binaries) assert.strictEqual(e.hash(fs.readFileSync(binary.path)), binary.sha256);
  assert.deepStrictEqual(env.runners.map(x => x.name), p.freezeHelpers);
  for (const item of [...env.runners, ...env.prior_artifacts, ...env.prerequisite_artifacts, ...env.prerequisite_validation_inputs]) assert.strictEqual(e.hash(e.bytes(item.name)), item.sha256, item.name);
  assert.deepStrictEqual(env.environment_overrides, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'});
  assert.deepStrictEqual(env.environment_removed, ['XDG_RUNTIME_DIR']);
  assert.strictEqual(env.ambient_stack.RUST_MIN_STACK, null);
  assert.strictEqual(process.env.RUST_MIN_STACK ?? null, null);
  for (const field of ['test_deadlines_changed', 'hardware_qualification', 'solver_rerun']) assert.strictEqual(env[field], false);
  assert.deepStrictEqual(env.prerequisite_runs, prerequisiteNames);
  assert.deepStrictEqual(env.prerequisite_artifacts.map(x => x.name), prerequisiteNames.flatMap(n => artifactSuffixes.map(s => n + s)).sort());
  assert.deepStrictEqual(env.prerequisite_validation_inputs.map(x => x.name), [...new Set([
    ...env.prerequisite_artifacts.map(x => x.name), ...p.prerequisites.map(([, , previous]) => previous), ...p.freezeHelpers])].sort());
  checkHistoricalAttempts(env);
  const priorGates = JSON.parse(e.committed('raw/r113-final-source-gate.json'));
  assert.strictEqual(priorGates.length, 17);
  const full = {};
  let previous = 'r114-device-oracle-format.json';
  for (const [index, name] of prerequisiteNames.entries()) {
    const prior = priorGates[index];
    assert.deepStrictEqual(p.prerequisites[index], [name, prior.command, previous]);
    const result = e.checkRun(name, prior.command, map, previous);
    full[prior.name] = checkFull(result.log, e.committed('raw/r113-final-' + prior.name + '.log'));
    e.ordered(result.record.clock.source_verified, env.observation);
    previous = name + '.json';
  }
  previous = 'r114-preliminary-gnu-all.json';
  for (const [kind, count, suffix] of [['runner', 9, ''], ['freeze', p.freezeContractCount, '-restoration']]) {
    const name = 'r114-' + kind + '-contract-tests' + suffix;
    const result = e.checkRun(name, ['node', e.file('r114-' + kind + '-tests.js')], map, previous);
    contractTranscript(result.log, kind, count);
    previous = name + '.json';
  }
  const source = e.checkRun('r114-source-campaign', ['python3', '-B', e.file('r114-source-gate.py'), 'r114-final'], map, previous);
  e.ordered(env.observation, source.record.clock.start);
  e.checkRun('r114-auxiliary-campaign', ['python3', '-B', e.file('r114-auxiliary-gates.py')], map, 'r114-source-campaign.json');
  for (const [name, oldRows, count, prefix, sourcePrefix] of [
    ['r114-final-source-gate.json', priorGates.slice(2), 15, 'r114-final', 'r114-final'],
    ['r114-auxiliary-results.json', JSON.parse(e.committed('raw/r113-auxiliary-results.json')), 10, 'r114', 'r114-auxiliary'],
  ]) {
    const rows = e.read(name);
    assert.strictEqual(rows.length, count);
    assert.deepStrictEqual(rows.map(r => r.name), oldRows.map(r => r.name));
    for (const [index, row] of rows.entries()) {
      assert.deepStrictEqual(row.command, oldRows[index].command.map(s => s.replace('r113-production-metadata.log', 'r114-production-metadata.log')));
      assert.strictEqual(row.cwd, oldRows[index].cwd);
      assert.strictEqual(row.log, e.file(prefix + '-' + row.name + '.log'));
      assert.strictEqual(row.returncode, 0, row.name);
      assert(Number.isFinite(row.elapsed_seconds) && row.elapsed_seconds >= 0);
      const text = e.bytes(prefix + '-' + row.name + '.log').toString();
      const oldText = e.committed('raw/' + prefix.replace('r114', 'r113') + '-' + row.name + '.log');
      if (row.command[0] === 'cargo' && row.command[2] === 'test') {
        const expected = row.command.includes('--doc') ? docNames(e.passing(oldText)) : e.passing(oldText);
        assert.deepStrictEqual(e.passing(text), expected, row.name);
        assert.deepStrictEqual(e.ignored(text), e.ignored(oldText), row.name);
        assert.deepStrictEqual(e.totals(text), e.totals(oldText), row.name);
      }
      if (['python', 'dependency-tests'].includes(row.name)) {
        assert.deepStrictEqual([...text.matchAll(/^Ran (\d+) tests? /gm)].map(m => m[1]), [...oldText.matchAll(/^Ran (\d+) tests? /gm)].map(m => m[1]));
        assert(/^OK$/m.test(text));
      }
      if (row.name === 'production-metadata') {
        assert.strictEqual(row.stderr_log, e.file('r114-production-metadata.stderr.log'));
        e.bytes('r114-production-metadata.stderr.log');
        const metadata = JSON.parse(text);
        assert.strictEqual(metadata.workspace_root, e.repo);
        assert(metadata.packages.some(x => x.name === 'fe2o3-runtime'));
        assert(metadata.resolve.nodes.length > 0);
      } else assert.strictEqual(row.stderr_log, undefined);
    }
    for (const suffix of ['source-inputs', 'source-after']) assert.deepStrictEqual(e.read(sourcePrefix + '-' + suffix + '.json'), map);
    assert.deepStrictEqual(e.read(sourcePrefix + '-complete.json'), {completed: true, gates: count, source_identities: p.sourceCount});
  }
  const allNames = e.passing(e.bytes(prerequisiteNames[0] + '.log').toString());
  previous = 'r114-auxiliary-campaign.json';
  for (const [kind, filter, count] of p.focused) {
    const name = 'r114-frozen-' + kind;
    const result = e.checkRun(name, [...p.test, filter], map, previous);
    const expected = allNames.filter(n => n.includes(filter));
    assert.strictEqual(expected.length, count);
    assert.deepStrictEqual(e.passing(result.log), expected);
    assert.deepStrictEqual(e.totals(result.log), {harnesses: 1, passed: count, failed: 0, ignored: 0});
    previous = name + '.json';
  }
  const name = 'r114-qualification-contract-tests';
  const tests = e.checkRun(name, ['node', e.file('r114-qualification-tests.js')], map, previous);
  contractTranscript(tests.log, 'qualification', p.qualificationContractCount);
  const pins = [...tests.log.matchAll(/^HELPER_PINS: (.+)$/gm)];
  assert.strictEqual(pins.length, 1);
  const helpers = p.helpers.map(name => ({name, sha256: e.hash(e.bytes(name))}));
  assert.deepStrictEqual(JSON.parse(pins[0][1]), helpers);
  return {map, env, full, allNames, helpers, previous: name + '.json'};
}
function collect(closed) {
  const {map, env, full, allNames, helpers, previous: prerequisite} = checkBaseline();
  const manifestName = 'r114-qualification-manifest.json';
  const manifest = e.read(manifestName);
  const manifestHash = e.hash(e.bytes(manifestName));
  assert.strictEqual(manifest.source_head, p.parent);
  assert.strictEqual(manifest.source_map_sha256, e.hash(JSON.stringify(map)));
  assert.deepStrictEqual(manifest.helpers, helpers);
  assert.deepStrictEqual(manifest.mutations, p.mutations);
  e.originalSources(map, manifest.original_sources);
  assert.deepStrictEqual(manifest.entries, e.expectedEntries(map, manifest.original_sources));
  assert.deepStrictEqual(manifest.historical_artifacts.map(x => x.name), e.historicalNames());
  for (const item of manifest.historical_artifacts) assert.strictEqual(e.hash(e.bytes(item.name)), item.sha256, item.name);
  assert.strictEqual(manifest.clock.contract, e.contract);
  assert.strictEqual(manifest.clock.error, null);
  assert.deepStrictEqual(manifest.clock.predecessor, {name: prerequisite, sha256: e.hash(e.bytes(prerequisite)), observation: e.read(prerequisite).clock.source_verified});
  e.ordered(manifest.clock.predecessor.observation, manifest.clock.source_verified);
  const names = p.mutations.flatMap(m => ['r114-qualified-mut-' + m.name + '.json', 'r114-qualified-restoration-' + m.name + '.json'])
    .concat(p.focused.map(([kind]) => 'r114-qualified-restored-' + kind + '.json'), ['r114-qualified-collector-validation.json']);
  assert.deepStrictEqual(Object.keys(manifest.entries), names);
  let previous = manifestName;
  const mutantHashes = new Set();
  for (const mutation of p.mutations) {
    const mutated = e.mutationSource(map, manifest.original_sources, mutation);
    const expected = {...map, [mutation.path]: e.hash(mutated)};
    const name = 'r114-qualified-mut-' + mutation.name;
    const entry = manifest.entries[name + '.json'];
    assert.strictEqual(entry.predecessor, previous);
    const result = e.checkRun(name, entry.command, expected, previous, 101);
    e.checkMutation(result.log, mutation);
    mutantHashes.add(e.hash(JSON.stringify(expected)));
    previous = name + '.json';
    const restoredName = 'r114-qualified-restoration-' + mutation.name + '.json';
    const restoration = e.read(restoredName);
    assert.deepStrictEqual(manifest.entries[restoredName], {kind: 'restoration', predecessor: previous, source_map_sha256: e.hash(JSON.stringify(map))});
    assert.strictEqual(restoration.manifest_sha256, manifestHash);
    assert.strictEqual(restoration.source_head, p.parent);
    assert.strictEqual(restoration.source_map_sha256, e.hash(JSON.stringify(map)));
    assert.strictEqual(restoration.mutated_source_sha256, expected[mutation.path]);
    assert.strictEqual(restoration.source_identities, p.sourceCount);
    assert.strictEqual(restoration.source_unchanged, true);
    assert.deepStrictEqual(e.read(restoredName.replace('.json', '-source.json')), map);
    assert.strictEqual(restoration.clock.contract, e.contract);
    assert.strictEqual(restoration.clock.error, null);
    assert.deepStrictEqual(restoration.clock.predecessor, {name: previous, sha256: e.hash(e.bytes(previous)), observation: result.record.clock.source_verified});
    e.ordered(result.record.clock.source_verified, restoration.clock.start);
    e.ordered(restoration.clock.start, restoration.clock.source_verified);
    previous = restoredName;
  }
  assert.strictEqual(mutantHashes.size, p.mutations.length);
  for (const [kind, filter, count] of p.focused) {
    const name = 'r114-qualified-restored-' + kind;
    const result = e.checkRun(name, [...p.test, filter], map, previous);
    assert.deepStrictEqual(e.passing(result.log), allNames.filter(n => n.includes(filter)));
    assert.deepStrictEqual(e.totals(result.log), {harnesses: 1, passed: count, failed: 0, ignored: 0});
    previous = name + '.json';
  }
  const collectorName = 'r114-qualified-collector-validation';
  const command = ['node', e.file('r114-qualification-collect.js'), '--check'];
  assert.deepStrictEqual(manifest.entries[collectorName + '.json'], {kind: 'run', predecessor: previous,
    source_map_sha256: e.hash(JSON.stringify(map)), command, returncode: 0});
  const summary = {source_head: p.parent, accepted_parent: p.accepted, source_map_sha256: e.hash(JSON.stringify(map)),
    scope: 'pristine control cleanup custody and settled parent transport; local scripted tests',
    source_identities: p.sourceCount, changed_source: p.sourceDelta, full, source_gates: 17,
    source_gate_prerequisites: prerequisiteNames, subsequent_source_gates: 15, auxiliary_gates: 10,
    focused: Object.fromEntries(p.focused.map(([kind, , count]) => [kind, count])), compiled_negatives: 15,
    new_behavioral_tests: 16, new_source_guards: 1, qualification_contract_tests: p.qualificationContractCount,
    runner_contract_tests: 9, freeze_contract_tests: p.freezeContractCount, doctest_relocations: p.docRelocations,
    harnessless_benchmark_targets_per_full_run: 1, formal_qualification: false, native_qualification: false,
    performance_qualification: false, ordinary_returning_cleanup_qualification: false};
  if (closed) assert.deepStrictEqual(JSON.parse(e.checkRun(collectorName, command, map, previous).log), summary, 'closed collector transcript');
  assert.deepStrictEqual(e.identities(), map);
  return {summary, manifest, env, names};
}
if (require.main === module) {
  const mode = process.argv[2];
  assert(['--check', '--archive'].includes(mode));
  const {summary, manifest, names} = collect(mode === '--archive');
  if (mode === '--archive') {
    const dest = path.join(e.repo, 'docs/evidence/local-r114-pristine-control-cleanup-2026-09-13');
    const cohort = names.flatMap(name => name.includes('-restoration-') ? [name, name.replace('.json', '-source.json')]
      : artifactSuffixes.map(s => name.replace('.json', s)));
    const artifacts = [...new Set([...manifest.historical_artifacts.map(x => x.name), 'r114-qualification-manifest.json', ...cohort])].sort();
    const coverage = new Set(artifacts);
    for (const name of [...e.historicalNames(), ...p.helpers, 'r114-environment.json', 'r114-frozen-source.json',
      ...prerequisiteNames.map(n => n + '.log'), 'r114-auxiliary-results.json']) assert(coverage.has(name), 'archive coverage: ' + name);
    for (const name of artifacts) e.bytes(name);
    fs.mkdirSync(dest); fs.mkdirSync(path.join(dest, 'raw'));
    summary.artifacts = artifacts.map(name => {
      const bytes = e.bytes(name);
      fs.writeFileSync(path.join(dest, 'raw', name), bytes, {flag: 'wx'});
      return {name, sha256: e.hash(bytes)};
    });
    fs.writeFileSync(path.join(dest, 'summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
    fs.writeFileSync(path.join(dest, 'README.md'), '# R114 Pristine Control Cleanup Evidence\n\n' +
      'Local scripted qualification only. GNU/musl each pass 2,712 tests with five ignored across 48 libtest harnesses and one unchanged harnessless CSV benchmark.\n' +
      'The two full runs are separately recorded pre-freeze prerequisites with matching source endpoints, not executions of the later fifteen-gate campaign.\n' +
      'All 17 source gates, ten auxiliary gates, exact frozen/restored focused rosters and 15 compiled behavioral negatives pass.\n' +
      'All 5,683 source identities are restored. Preliminary compile failures and intentional formatter changes are preserved without relabeling source HEADs.\n' +
      'An earlier unwrapped formatter failure has no saved command record or before/after inventories; it is not reconstructed or accepted as evidence here.\n' +
      'Raw UTC is retained; causal order uses the same Linux boot and monotonic clock. Endpoint equality does not establish continuous source immutability or unrecorded ambient state.\n\n' +
      'This does not qualify live Linux/KFD execution, formal proofs, performance, ordinary/returning cleanup, data cleanup or full HIP/HSA parity.\n', {flag: 'wx'});
    console.log(JSON.stringify({archive: dest, artifacts: artifacts.length, summary}, null, 2));
  } else console.log(JSON.stringify(summary, null, 2));
}
module.exports = {checkBaseline, collect, checkFull, docNames, contractTranscript};
