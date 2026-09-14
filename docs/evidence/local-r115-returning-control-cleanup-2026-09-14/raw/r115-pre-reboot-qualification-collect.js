const fs = require('fs');
const path = require('path');
const assert = require('assert');
const p = require('./r115-qualification-plan.js');
const e = require('./r115-qualification-evidence.js');
const baseline = () => e.read('r115-frozen-source.json');
const artifactSuffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const prerequisiteNames = ['r115-preliminary-gnu-all', 'r115-preliminary-musl-all'];
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
  assert.deepStrictEqual(e.totals(text), {harnesses: 48, passed: 2727, failed: 0, ignored: 5});
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
  const pinned = new Set(env.prior_artifacts.map(x => x.name));
  let previous = null;
  for (const [short, command, code, mapHash, changed, outcome] of p.historicalAttempts) {
    const name = 'r115-' + short;
    for (const suffix of artifactSuffixes) assert(pinned.has(name + suffix), 'preserved preliminary artifact');
    const record = e.read(name + '.json');
    const before = e.read(name + '-source.json');
    const after = e.read(name + '-source-after.json');
    assert.strictEqual(record.source_head, p.parent);
    assert.deepStrictEqual(record.command, command);
    assert.strictEqual(record.cwd, e.repo);
    assert.strictEqual(record.log, e.file(name + '.log'));
    assert.strictEqual(record.source, e.file(name + '-source.json'));
    assert.strictEqual(record.source_after, e.file(name + '-source-after.json'));
    assert.strictEqual(record.deadline_ms, 1800000);
    assert.deepStrictEqual(record.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
    assert.strictEqual(record.source_map_sha256, mapHash);
    assert.strictEqual(e.hash(JSON.stringify(before)), mapHash);
    assert.strictEqual(record.returncode, code);
    assert.strictEqual(record.child_returncode, code);
    assert.strictEqual(record.child_closed, true);
    assert.strictEqual(record.timed_out, false);
    assert.strictEqual(record.signal, null);
    assert.strictEqual(record.spawn_error, null);
    assert(e.canRestore(record), 'closed historical process group');
    assert.strictEqual(record.process_group_cleanup.timeout, null);
    assert.strictEqual(record.source_unchanged, !changed);
    if (changed) assert.notDeepStrictEqual(before, after); else assert.deepStrictEqual(before, after);
    assert.deepStrictEqual(record.runner, {name: 'r115-run-v4.js', sha256: e.hash(e.bytes('r115-run-v4.js'))});
    assert.strictEqual(record.clock.contract, e.contract);
    assert.strictEqual(record.clock.error, null);
    if (previous) {
      const prior = e.read(previous);
      assert.deepStrictEqual(record.clock.predecessor, {name: previous, sha256: e.hash(e.bytes(previous)), observation: prior.clock.source_verified});
      e.ordered(prior.clock.source_verified, record.clock.start);
    } else assert.strictEqual(record.clock.predecessor, null);
    e.ordered(record.clock.start, record.clock.finish);
    e.ordered(record.clock.finish, record.process_group_cleanup.close.observation);
    e.ordered(record.process_group_cleanup.close.observation, record.clock.source_verified);
    assert.strictEqual(record.started_at, new Date(record.clock.start.utc_ms).toISOString());
    assert.strictEqual(record.finished_at, new Date(record.clock.finish.utc_ms).toISOString());
    assert.strictEqual(record.elapsed_seconds, Number(BigInt(record.clock.finish.monotonic_ns) - BigInt(record.clock.start.monotonic_ns)) / 1e9);
    const log = e.bytes(name + '.log').toString();
    if (typeof outcome === 'number') assert.deepStrictEqual(e.totals(log), {harnesses: 1, passed: outcome, failed: 0, ignored: 0});
    if (outcome === 'missing-module') assert(log.includes('queue_dispatch_binding/tests.rs') && log.includes('does not exist'));
    if (outcome === 'compile-errors') {
      assert.strictEqual([...log.matchAll(/^error\[E\d+\]:/gm)].length, 10);
      for (const code of ['E0277', 'E0369', 'E0507']) assert(log.includes('error[' + code + ']'));
      assert(!/^test result:/m.test(log));
    }
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
  const oldMap = JSON.parse(e.committed('raw/r114-frozen-source.json'));
  assert.deepStrictEqual([...new Set([...Object.keys(oldMap), ...Object.keys(map)])].filter(n => oldMap[n] !== map[n]).sort(), p.sourceDelta);
  assert.deepStrictEqual(Object.keys(map).filter(n => !Object.hasOwn(oldMap, n)).sort(), p.addedSource);
  assert.deepStrictEqual(Object.keys(oldMap).filter(n => !Object.hasOwn(map, n)), []);
  const env = e.read('r115-environment.json');
  assert.strictEqual(env.publication_parent, p.parent);
  assert.strictEqual(env.accepted_runtime_checkpoint, p.accepted);
  assert.deepStrictEqual(env.source_delta, p.sourceDelta);
  assert.deepStrictEqual(env.binaries, JSON.parse(e.committed('raw/r114-environment.json')).binaries);
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
  const priorGates = JSON.parse(e.committed('raw/r114-final-source-gate.json'));
  assert.strictEqual(priorGates.length, 15);
  const full = {};
  let previous = p.prerequisites[0][2];
  for (const [index, name] of prerequisiteNames.entries()) {
    const target = index === 0 ? 'gnu' : 'musl';
    const priorName = 'r114-preliminary-' + target + '-all';
    const prior = JSON.parse(e.committed('raw/' + priorName + '.json'));
    assert.deepStrictEqual(p.prerequisites[index], [name, prior.command, previous]);
    const result = e.checkRun(name, prior.command, map, previous);
    full[target + '-tests'] = checkFull(result.log, e.committed('raw/' + priorName + '.log'));
    e.ordered(result.record.clock.source_verified, env.observation);
    previous = name + '.json';
  }
  previous = 'r115-preliminary-musl-all.json';
  for (const [kind, count] of [['runner', 9], ['freeze', p.freezeContractCount]]) {
    const name = 'r115-' + kind + '-contract-tests';
    const result = e.checkRun(name, ['node', e.file('r115-' + kind + '-tests.js')], map, previous);
    contractTranscript(result.log, kind, count);
    previous = name + '.json';
  }
  const source = e.checkRun('r115-source-campaign', ['python3', '-B', e.file('r115-source-gate.py'), 'r115-final'], map, previous);
  e.ordered(env.observation, source.record.clock.start);
  e.checkRun('r115-auxiliary-campaign', ['python3', '-B', e.file('r115-auxiliary-gates.py')], map, 'r115-source-campaign.json');
  for (const [name, oldRows, count, prefix, sourcePrefix] of [
    ['r115-final-source-gate.json', priorGates, 15, 'r115-final', 'r115-final'],
    ['r115-auxiliary-results.json', JSON.parse(e.committed('raw/r114-auxiliary-results.json')), 10, 'r115', 'r115-auxiliary'],
  ]) {
    const rows = e.read(name);
    assert.strictEqual(rows.length, count);
    assert.deepStrictEqual(rows.map(r => r.name), oldRows.map(r => r.name));
    for (const [index, row] of rows.entries()) {
      assert.deepStrictEqual(row.command, oldRows[index].command.map(s => s.replace('r114-production-metadata.log', 'r115-production-metadata.log')));
      assert.strictEqual(row.cwd, oldRows[index].cwd);
      assert.strictEqual(row.log, e.file(prefix + '-' + row.name + '.log'));
      assert.strictEqual(row.returncode, 0, row.name);
      assert(Number.isFinite(row.elapsed_seconds) && row.elapsed_seconds >= 0);
      const text = e.bytes(prefix + '-' + row.name + '.log').toString();
      const oldText = e.committed('raw/' + prefix.replace('r115', 'r114') + '-' + row.name + '.log');
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
        assert.strictEqual(row.stderr_log, e.file('r115-production-metadata.stderr.log'));
        e.bytes('r115-production-metadata.stderr.log');
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
  previous = 'r115-auxiliary-campaign.json';
  for (const [kind, filter, count] of p.focused) {
    const name = 'r115-frozen-' + kind;
    const result = e.checkRun(name, [...p.test, filter], map, previous);
    const expected = allNames.filter(n => n.includes(filter));
    assert.strictEqual(expected.length, count);
    assert.deepStrictEqual(e.passing(result.log), expected);
    assert.deepStrictEqual(e.totals(result.log), {harnesses: 1, passed: count, failed: 0, ignored: 0});
    previous = name + '.json';
  }
  const name = 'r115-qualification-contract-tests';
  const tests = e.checkRun(name, ['node', e.file('r115-qualification-tests.js')], map, previous);
  contractTranscript(tests.log, 'qualification', p.qualificationContractCount);
  const pins = [...tests.log.matchAll(/^HELPER_PINS: (.+)$/gm)];
  assert.strictEqual(pins.length, 1);
  const helpers = p.helpers.map(name => ({name, sha256: e.hash(e.bytes(name))}));
  assert.deepStrictEqual(JSON.parse(pins[0][1]), helpers);
  return {map, env, full, allNames, helpers, previous: name + '.json'};
}
function collect(closed) {
  const {map, env, full, allNames, helpers, previous: prerequisite} = checkBaseline();
  const manifestName = 'r115-qualification-manifest.json';
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
  const names = p.mutations.flatMap(m => ['r115-qualified-mut-' + m.name + '.json', 'r115-qualified-restoration-' + m.name + '.json'])
    .concat(p.focused.map(([kind]) => 'r115-qualified-restored-' + kind + '.json'), ['r115-qualified-collector-validation.json']);
  assert.deepStrictEqual(Object.keys(manifest.entries), names);
  let previous = manifestName;
  const mutantHashes = new Set();
  for (const mutation of p.mutations) {
    const mutated = e.mutationSource(map, manifest.original_sources, mutation);
    const expected = {...map, [mutation.path]: e.hash(mutated)};
    const name = 'r115-qualified-mut-' + mutation.name;
    const entry = manifest.entries[name + '.json'];
    assert.strictEqual(entry.predecessor, previous);
    const result = e.checkRun(name, entry.command, expected, previous, 101);
    e.checkMutation(result.log, mutation);
    mutantHashes.add(e.hash(JSON.stringify(expected)));
    previous = name + '.json';
    const restoredName = 'r115-qualified-restoration-' + mutation.name + '.json';
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
    const name = 'r115-qualified-restored-' + kind;
    const result = e.checkRun(name, [...p.test, filter], map, previous);
    assert.deepStrictEqual(e.passing(result.log), allNames.filter(n => n.includes(filter)));
    assert.deepStrictEqual(e.totals(result.log), {harnesses: 1, passed: count, failed: 0, ignored: 0});
    previous = name + '.json';
  }
  const collectorName = 'r115-qualified-collector-validation';
  const command = ['node', e.file('r115-qualification-collect.js'), '--check'];
  assert.deepStrictEqual(manifest.entries[collectorName + '.json'], {kind: 'run', predecessor: previous,
    source_map_sha256: e.hash(JSON.stringify(map)), command, returncode: 0});
  const summary = {source_head: p.parent, accepted_parent: p.accepted, source_map_sha256: e.hash(JSON.stringify(map)),
    scope: 'returning-control cleanup custody and consuming lower bridge; local scripted tests',
    source_identities: p.sourceCount, changed_source: p.sourceDelta, full, source_gates: 17,
    source_gate_prerequisites: prerequisiteNames, subsequent_source_gates: 15, auxiliary_gates: 10,
    focused: Object.fromEntries(p.focused.map(([kind, , count]) => [kind, count])), compiled_negatives: p.mutations.length,
    new_behavioral_tests: p.newTests.length - 1, new_source_guards: 1, qualification_contract_tests: p.qualificationContractCount,
    runner_contract_tests: 9, freeze_contract_tests: p.freezeContractCount, doctest_relocations: p.docRelocations,
    harnessless_benchmark_targets_per_full_run: 1, formal_qualification: false, native_qualification: false,
    performance_qualification: false, lower_returning_control_cleanup_qualification: true,
    persistent_cleanup_qualification: false, ordinary_data_cleanup_qualification: false,
    live_returning_transport_qualification: false, queue_teardown_qualification: false};
  if (closed) assert.deepStrictEqual(JSON.parse(e.checkRun(collectorName, command, map, previous).log), summary, 'closed collector transcript');
  assert.deepStrictEqual(e.identities(), map);
  return {summary, manifest, env, names};
}
if (require.main === module) {
  const mode = process.argv[2];
  assert(['--check', '--archive'].includes(mode));
  const {summary, manifest, names} = collect(mode === '--archive');
  if (mode === '--archive') {
    const dest = path.join(e.repo, 'docs/evidence/local-r115-returning-control-cleanup-2026-09-13');
    const cohort = names.flatMap(name => name.includes('-restoration-') ? [name, name.replace('.json', '-source.json')]
      : artifactSuffixes.map(s => name.replace('.json', s)));
    const artifacts = [...new Set([...manifest.historical_artifacts.map(x => x.name), 'r115-qualification-manifest.json', ...cohort])].sort();
    const coverage = new Set(artifacts);
    for (const name of [...e.historicalNames(), ...p.helpers, 'r115-environment.json', 'r115-frozen-source.json',
      ...prerequisiteNames.map(n => n + '.log'), 'r115-auxiliary-results.json']) assert(coverage.has(name), 'archive coverage: ' + name);
    for (const name of artifacts) e.bytes(name);
    fs.mkdirSync(dest); fs.mkdirSync(path.join(dest, 'raw'));
    summary.artifacts = artifacts.map(name => {
      const bytes = e.bytes(name);
      fs.writeFileSync(path.join(dest, 'raw', name), bytes, {flag: 'wx'});
      return {name, sha256: e.hash(bytes)};
    });
    fs.writeFileSync(path.join(dest, 'summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
    fs.writeFileSync(path.join(dest, 'README.md'), '# R115 Returning Control Cleanup Evidence\n\n' +
      'Local scripted qualification only. GNU/musl each pass 2,727 tests with five ignored across 48 libtest harnesses and one unchanged harnessless CSV benchmark.\n' +
      'The two full runs are separately recorded pre-freeze prerequisites with matching source endpoints, not executions of the later fifteen-gate campaign.\n' +
      'All 17 source gates, ten auxiliary gates, exact frozen/restored focused rosters and ' + p.mutations.length + ' compiled behavioral negatives pass.\n' +
      'All 5,685 source identities are restored. Preliminary formatter and compile failures and intentional formatter changes are preserved without relabeling source HEADs.\n' +
      'Raw UTC is retained; causal order uses the same Linux boot and monotonic clock. Endpoint equality does not establish continuous source immutability or unrecorded ambient state.\n\n' +
      'This does not qualify live Linux/KFD execution or returning transport, formal proofs, performance, persistent/data cleanup, queue teardown or full HIP/HSA parity.\n', {flag: 'wx'});
    console.log(JSON.stringify({archive: dest, artifacts: artifacts.length, summary}, null, 2));
  } else console.log(JSON.stringify(summary, null, 2));
}
module.exports = {checkBaseline, collect, checkFull, docNames, contractTranscript};
