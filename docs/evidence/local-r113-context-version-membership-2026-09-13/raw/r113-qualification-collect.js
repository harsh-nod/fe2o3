const fs = require('fs');
const path = require('path');
const assert = require('assert');
const p = require('./r113-qualification-plan.js');
const e = require('./r113-qualification-evidence.js');
const baseline = () => e.read('r113-frozen-source.json');
function checkHistory(env, map) {
  const isolated = ['initial-all-targets', 'initial-clippy', 'initial-format-corrected', 'initial-tests', 'production-clippy']
    .flatMap(n => ['.json', '.log', '-source.json', '-source-after.json'].map(s => 'v2-membership-' + n + s))
    .concat(['v2-membership-initial-format-preflight-failure.json', 'v2-membership-run-preflight-v0.js', 'v2-membership-run.js']).sort();
  assert.strictEqual(isolated.length, 23);
  assert.deepStrictEqual(env.isolated_artifacts.map(x => x.name), isolated);
  const main = ['integrated-format', 'integrated-journal', 'reviewed-format', 'reviewed-journal', 'reviewed-model',
    'overlap-format', 'final-journal', 'freeze-contract-tests', 'runner-contract-tests', 'runner-contract-tests-v2',
    'freeze-contract-tests-v2', 'final-model-clippy', 'final-production-clippy-corrected', 'final-format-check', 'final-whitespace-check'];
  const fixtures = ['clean', 'success', 'failure', 'inner-timeout', 'spawn'].flatMap(name =>
    ['runner-fixture-' + name, 'runner-v2-fixture-' + name]);
  const names = [...main, ...fixtures];
  assert.strictEqual(names.length, 25);
  const expectedArtifacts = names.flatMap(n => ['.json', '.log', '-source.json', '-source-after.json'].map(s => 'r113-' + n + s))
    .concat(['r113-final-production-clippy-gate-failure.json']).sort();
  assert.deepStrictEqual(env.prior_artifacts.map(x => x.name), expectedArtifacts);
  const oldMaps = {
    'integrated-format': 'c321cf6411f78a1fac1d4dfab705ef8cd866975efbdcec34cf2356ce9168cb47',
    'integrated-journal': 'c321cf6411f78a1fac1d4dfab705ef8cd866975efbdcec34cf2356ce9168cb47',
    'reviewed-format': 'd371cf54fe86cbe106f34e23b876825b261af353b74b5bb70a7e583b83e27e41',
    'reviewed-journal': 'd371cf54fe86cbe106f34e23b876825b261af353b74b5bb70a7e583b83e27e41',
    'reviewed-model': 'd371cf54fe86cbe106f34e23b876825b261af353b74b5bb70a7e583b83e27e41',
    'overlap-format': '55a13c68c9bf8cd5a81eeb51e957e117b0bd5fc880d3548243c56e81c65fa5e5',
  };
  const membershipPath = p.sourceDelta[1];
  for (const name of names) {
    const record = e.read('r113-' + name + '.json');
    const before = e.read('r113-' + name + '-source.json');
    const after = e.read('r113-' + name + '-source-after.json');
    assert.strictEqual(record.source_head, p.parent);
    assert.strictEqual(record.signal, null); assert.strictEqual(record.timed_out, false);
    assert.strictEqual(record.clock.contract, e.contract); assert.strictEqual(record.clock.error, null);
    const intentional = name.startsWith('runner-') && name.includes('-fixture-');
    const code = intentional && name.endsWith('-failure') ? 17 : intentional && name.endsWith('-inner-timeout') ? 1 : intentional && name.endsWith('-spawn') ? 127 : 0;
    assert.strictEqual(record.returncode, code);
    assert.strictEqual(record.source_map_sha256, oldMaps[name] || e.hash(JSON.stringify(map)));
    assert.strictEqual(record.source_map_sha256, e.hash(JSON.stringify(before)));
    assert.deepStrictEqual(Object.keys(before), Object.keys(map));
    assert.deepStrictEqual(Object.keys(after), Object.keys(map));
    for (const source of Object.keys(map)) if (source !== membershipPath) {
      assert.strictEqual(before[source], map[source]); assert.strictEqual(after[source], map[source]);
    }
    assert.strictEqual(record.source_unchanged, name !== 'overlap-format');
    if (name === 'overlap-format') {
      assert.notDeepStrictEqual(before, after); assert.deepStrictEqual(after, map);
      assert.deepStrictEqual(record.command, ['cargo', '+nightly-2026-04-03', 'fmt', '-p', 'fe2o3-runtime-model']);
    } else assert.deepStrictEqual(before, after);
    if (name.endsWith('-inner-timeout')) assert(e.bytes('r113-' + name + '.log').toString().includes('TimeoutExpired'));
    if (name.endsWith('-spawn')) assert(record.spawn_error.includes('ENOENT'));
  }
  const rejection = e.read('r113-final-production-clippy-gate-failure.json');
  assert.strictEqual(rejection.accepted, false);
  assert(rejection.error.includes('ENOENT') && rejection.error.includes('r113-final-model-clippy.json'));
  for (const suffix of ['.json', '.log', '-source.json', '-source-after.json']) assert(!fs.existsSync(e.file('r113-final-production-clippy' + suffix)));
  const contracts = [
    ['runner', 'r113-runner-contract-tests-v2', 'r113-runner-tests-v2.js', 'r113-runner-contract-tests.json',
      ['real-clean', 'real-success', 'real-failure', 'real-inner-timeout', 'real-spawn', 'cleanup-eperm-retains-success',
        'outer-timeout-retains-signal', 'cleanup-esrch-is-benign', 'timeout-eperm-unclosed-is-bounded']],
    ['freeze', 'r113-freeze-contract-tests-v2', 'r113-freeze-tests-v2.js', 'r113-runner-contract-tests-v2.json',
      ['unchanged', 'added-source', 'deleted-untracked-member-test', 'deleted-tracked-source', 'modified-lockfile',
        'same-count-replacement', 'late-head-change', 'wrong-opening-head', 'docs-only-change']],
  ];
  for (const [kind, name, script, previous, cases] of contracts) {
    const result = e.checkRun(name, ['node', e.file(script)], map, previous);
    assert.deepStrictEqual(result.log.trimEnd().split('\n'), [...cases.map(n => 'PASS ' + n), 'PASS: 9 ' + kind + ' contract tests']);
  }
}
function checkBaseline() {
  const map = baseline();
  assert.strictEqual(Object.keys(map).length, p.sourceCount);
  assert.deepStrictEqual(e.identities(), map);
  const oldMap = JSON.parse(e.committed('raw/r112-frozen-source.json'));
  assert.deepStrictEqual([...new Set([...Object.keys(oldMap), ...Object.keys(map)])].filter(n => oldMap[n] !== map[n]).sort(), p.sourceDelta);
  const env = e.read('r113-environment.json');
  assert.strictEqual(env.publication_parent, p.parent);
  assert.strictEqual(env.accepted_runtime_checkpoint, p.parent);
  assert.deepStrictEqual(env.binaries, JSON.parse(e.committed('raw/r112-environment.json')).binaries);
  assert.deepStrictEqual(env.runners.map(x => x.name), ['r113-run.js', 'r113-run-v2.js', 'r113-run-v3.js', 'r113-source-gate.py', 'r113-auxiliary-gates.py',
    'r113-freeze.js', 'r113-freeze-v2.js', 'r113-freeze-v3.js', 'r113-runner-tests.js', 'r113-runner-tests-v2.js', 'r113-freeze-tests.js', 'r113-freeze-tests-v2.js']);
  for (const binary of env.binaries) assert.strictEqual(e.hash(fs.readFileSync(binary.path)), binary.sha256);
  for (const item of [...env.runners, ...env.prior_artifacts, ...env.isolated_artifacts]) assert.strictEqual(e.hash(e.bytes(item.name)), item.sha256, item.name);
  assert.deepStrictEqual(env.environment_overrides, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'});
  assert.deepStrictEqual(env.environment_removed, ['XDG_RUNTIME_DIR']);
  assert.strictEqual(env.ambient_stack.RUST_MIN_STACK, null);
  assert.strictEqual(process.env.RUST_MIN_STACK ?? null, null);
  for (const field of ['test_deadlines_changed', 'hardware_qualification', 'solver_rerun']) assert.strictEqual(env[field], false);
  checkHistory(env, map);
  const source = e.checkRun('r113-source-campaign', ['python3', '-B', e.file('r113-source-gate.py'), 'r113-final'], map, 'r113-final-whitespace-check.json');
  e.ordered(env.observation, source.record.clock.start);
  e.checkRun('r113-auxiliary-campaign', ['python3', '-B', e.file('r113-auxiliary-gates.py')], map, 'r113-source-campaign.json');
  const full = {};
  for (const [current, prior, count, artifactPrefix, oldPrefix, sourcePrefix] of [
    ['r113-final-source-gate.json', 'r112-final-source-gate.json', 17, 'r113-final', 'r112-final', 'r113-final'],
    ['r113-auxiliary-results.json', 'r112-auxiliary-results.json', 10, 'r113', 'r112', 'r113-auxiliary'],
  ]) {
    const rows = e.read(current);
    const oldRows = JSON.parse(e.committed('raw/' + prior));
    assert.strictEqual(rows.length, count);
    assert.deepStrictEqual(rows.map(r => r.name), oldRows.map(r => r.name));
    for (const [index, row] of rows.entries()) {
      assert.deepStrictEqual(row.command, oldRows[index].command.map(s => s.replace('r112-production-metadata.log', 'r113-production-metadata.log')));
      assert.strictEqual(row.cwd, oldRows[index].cwd);
      assert.strictEqual(row.log, e.file(artifactPrefix + '-' + row.name + '.log'));
      assert.strictEqual(row.returncode, 0, row.name);
      assert(Number.isFinite(row.elapsed_seconds) && row.elapsed_seconds >= 0);
      const text = e.bytes(artifactPrefix + '-' + row.name + '.log').toString();
      const oldText = e.committed('raw/' + oldPrefix + '-' + row.name + '.log');
      if (['gnu-tests', 'musl-tests'].includes(row.name)) {
        assert.deepStrictEqual(e.passing(text), [...e.passing(oldText), ...p.newTests].sort());
        assert.deepStrictEqual(e.ignored(text), e.ignored(oldText));
        const currentTargets = e.executables(text);
        const oldTargets = e.executables(oldText);
        assert.strictEqual(currentTargets.length, 49);
        assert.strictEqual(oldTargets.length, 49);
        assert.strictEqual(currentTargets.filter(x => x.kind === 'libtest').length, 48);
        assert.strictEqual(oldTargets.filter(x => x.kind === 'libtest').length, 48);
        let changedHarnesses = 0;
        for (const [i, actual] of currentTargets.entries()) {
          const expected = structuredClone(oldTargets[i]);
          if (expected.kind === 'libtest' && (expected.name.includes('fe2o3_runtime_model-') || expected.name.includes('fe2o3_runtime_model)'))) {
            expected.passing = [...expected.passing, ...p.newTests].sort();
            expected.totals.passed += 12; changedHarnesses++;
          }
          assert.deepStrictEqual(actual, expected, row.name + ': ' + actual.name);
        }
        assert.strictEqual(changedHarnesses, 1);
        assert.deepStrictEqual(e.totals(text), {harnesses: 48, passed: 2695, failed: 0, ignored: 5});
        full[row.name] = e.totals(text);
      } else if (row.command[0] === 'cargo' && row.command[2] === 'test') {
        assert.deepStrictEqual(e.passing(text), e.passing(oldText), row.name);
        assert.deepStrictEqual(e.ignored(text), e.ignored(oldText), row.name);
        assert.deepStrictEqual(e.totals(text), e.totals(oldText), row.name);
      }
      if (row.name === 'python' || row.name === 'dependency-tests') {
        assert.deepStrictEqual([...text.matchAll(/^Ran (\d+) tests? /gm)].map(m => m[1]),
          [...oldText.matchAll(/^Ran (\d+) tests? /gm)].map(m => m[1]));
        assert(/^OK$/m.test(text));
      }
      if (row.name === 'production-metadata') {
        assert.strictEqual(row.stderr_log, e.file('r113-production-metadata.stderr.log'));
        e.bytes('r113-production-metadata.stderr.log');
        const metadata = JSON.parse(text);
        assert.strictEqual(metadata.workspace_root, e.repo);
        assert(metadata.packages.some(x => x.name === 'fe2o3-runtime'));
        assert(metadata.resolve.nodes.length > 0);
      } else assert.strictEqual(row.stderr_log, undefined);
    }
    for (const suffix of ['source-inputs', 'source-after']) assert.deepStrictEqual(e.read(sourcePrefix + '-' + suffix + '.json'), map);
    assert.deepStrictEqual(e.read(sourcePrefix + '-complete.json'), {completed: true, gates: count, source_identities: p.sourceCount});
  }
  const allNames = e.passing(e.bytes('r113-final-gnu-tests.log').toString());
  let previous = 'r113-auxiliary-campaign.json';
  for (const [kind, filter, count] of p.focused) {
    const name = 'r113-frozen-' + kind;
    const result = e.checkRun(name, [...p.test, filter], map, previous);
    const expected = allNames.filter(n => n.startsWith(filter));
    assert.strictEqual(expected.length, count);
    assert.deepStrictEqual(e.passing(result.log), expected);
    assert.deepStrictEqual(e.totals(result.log), {harnesses: 1, passed: count, failed: 0, ignored: 0});
    previous = name + '.json';
  }
  const originalTests = e.checkRun('r113-qualification-contract-tests', ['node', e.file('r113-qualification-tests.js')], map, previous);
  assert(originalTests.log.endsWith('PASS: 36 qualification contract tests\n'));
  assert.strictEqual(originalTests.log.split('\n').filter(line => line.startsWith('PASS ')).length, 36);
  const snapshot = e.read('r113-preparation-rejected-helper-snapshot.json');
  assert.deepStrictEqual(snapshot.helpers.map(x => x.name), p.helpers);
  for (const helper of snapshot.helpers) assert.strictEqual(e.hash(helper.text), helper.sha256);
  const originalPins = [...originalTests.log.matchAll(/^HELPER_PINS: (.+)$/gm)];
  assert.strictEqual(originalPins.length, 1);
  assert.deepStrictEqual(JSON.parse(originalPins[0][1]), snapshot.helpers.map(({name, sha256}) => ({name, sha256})));
  const rejected = e.checkRun('r113-qualification-prepare-rejected', ['node', e.file('r113-qualification-prepare.js')],
    map, 'r113-qualification-contract-tests.json', 1);
  assert(rejected.log.includes('AssertionError [ERR_ASSERTION]: nonempty test summaries'));
  const initial = e.read('r113-qualification-prepare-initial-failure.json');
  assert.strictEqual(initial.exit_code, 1);
  assert.strictEqual(initial.manifest_created, false);
  assert.strictEqual(initial.source_mutation_started, false);
  assert.strictEqual(rejected.log, initial.stderr);
  const correctedName = 'r113-qualification-contract-tests-corrected';
  const tests = e.checkRun(correctedName, ['node', e.file('r113-qualification-tests.js')], map, 'r113-qualification-prepare-rejected.json');
  assert(tests.log.endsWith('PASS: 52 qualification contract tests\n'));
  assert.strictEqual(tests.log.split('\n').filter(line => line.startsWith('PASS ')).length, 52);
  const pins = [...tests.log.matchAll(/^HELPER_PINS: (.+)$/gm)];
  assert.strictEqual(pins.length, 1);
  const helpers = p.helpers.map(name => ({name, sha256: e.hash(e.bytes(name))}));
  assert.deepStrictEqual(JSON.parse(pins[0][1]), helpers);
  return {map, env, full, allNames, helpers, previous: correctedName + '.json'};
}
function collect(closed) {
  const {map, env, full, allNames, helpers, previous: prerequisite} = checkBaseline();
  const manifestName = 'r113-qualification-manifest.json';
  const manifest = e.read(manifestName);
  const manifestHash = e.hash(e.bytes(manifestName));
  assert.strictEqual(manifest.source_head, p.parent);
  assert.strictEqual(manifest.source_map_sha256, e.hash(JSON.stringify(map)));
  assert.deepStrictEqual(manifest.helpers, helpers);
  assert.deepStrictEqual(manifest.mutations, p.mutations);
  assert.strictEqual(manifest.original_source.path, p.production);
  assert.strictEqual(e.hash(manifest.original_source.text), map[p.production]);
  assert.deepStrictEqual(manifest.entries, e.expectedEntries(map, manifest.original_source.text));
  assert.deepStrictEqual(manifest.historical_artifacts.map(x => x.name), e.historicalNames());
  for (const item of manifest.historical_artifacts) assert.strictEqual(e.hash(e.bytes(item.name)), item.sha256, item.name);
  assert.strictEqual(manifest.clock.contract, e.contract);
  assert.strictEqual(manifest.clock.error, null);
  assert.deepStrictEqual(manifest.clock.predecessor, {name: prerequisite, sha256: e.hash(e.bytes(prerequisite)), observation: e.read(prerequisite).clock.source_verified});
  e.ordered(manifest.clock.predecessor.observation, manifest.clock.source_verified);
  const names = p.mutations.flatMap(m => ['r113-qualified-mut-' + m.name + '.json', 'r113-qualified-restoration-' + m.name + '.json'])
    .concat(p.focused.map(([kind]) => 'r113-qualified-restored-' + kind + '.json'), ['r113-qualified-collector-validation.json']);
  assert.deepStrictEqual(Object.keys(manifest.entries), names);
  let previous = manifestName;
  const mutantHashes = new Set();
  for (const mutation of p.mutations) {
    const original = fs.readFileSync(path.join(e.repo, mutation.path), 'utf8');
    const mutated = e.mutatedSource(original, mutation);
    const expected = {...map, [mutation.path]: e.hash(mutated)};
    const name = 'r113-qualified-mut-' + mutation.name;
    const entry = manifest.entries[name + '.json'];
    assert.deepStrictEqual(entry, {kind: 'run', predecessor: previous,
      source_map_sha256: e.hash(JSON.stringify(expected)), command: [...p.test, mutation.test, '--', '--exact'], returncode: 101});
    const result = e.checkRun(name, entry.command, expected, previous, 101);
    e.checkMutation(result.log, mutation);
    mutantHashes.add(expected[mutation.path]);
    previous = name + '.json';
    const restoredName = 'r113-qualified-restoration-' + mutation.name + '.json';
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
  assert.strictEqual(mutantHashes.size, 17);
  for (const [kind, filter, count] of p.focused) {
    const name = 'r113-qualified-restored-' + kind;
    const entry = manifest.entries[name + '.json'];
    assert.deepStrictEqual(entry, {kind: 'run', predecessor: previous, source_map_sha256: e.hash(JSON.stringify(map)), command: [...p.test, filter], returncode: 0});
    const result = e.checkRun(name, entry.command, map, previous);
    assert.deepStrictEqual(e.passing(result.log), allNames.filter(n => n.startsWith(filter)));
    assert.deepStrictEqual(e.totals(result.log), {harnesses: 1, passed: count, failed: 0, ignored: 0});
    previous = name + '.json';
  }
  const collectorName = 'r113-qualified-collector-validation';
  const command = ['node', e.file('r113-qualification-collect.js'), '--check'];
  assert.deepStrictEqual(manifest.entries[collectorName + '.json'], {kind: 'run', predecessor: previous,
    source_map_sha256: e.hash(JSON.stringify(map)), command, returncode: 0});
  const summary = {source_head: p.parent, source_map_sha256: e.hash(JSON.stringify(map)), scope: 'executable-model allocation membership and whole-roster Begin',
    source_identities: p.sourceCount, changed_source: p.sourceDelta, full, source_gates: 17, auxiliary_gates: 10,
    focused: {journal: 23, membership: 12}, compiled_negatives: 17, new_behavioral_tests: 11, new_source_guards: 1,
    qualification_contract_tests: 52, runner_contract_tests: 9, freeze_contract_tests: 9, harnessless_benchmark_targets_per_full_run: 1,
    formal_qualification: false, native_qualification: false, performance_qualification: false, production_context_integration: false};
  if (closed) {
    const result = e.checkRun(collectorName, command, map, previous);
    assert.deepStrictEqual(JSON.parse(result.log), summary, 'closed collector transcript');
  }
  assert.deepStrictEqual(e.identities(), map);
  return {summary, manifest, env, names};
}
if (require.main === module) {
  const mode = process.argv[2];
  assert(['--check', '--archive'].includes(mode));
  const result = collect(mode === '--archive');
  if (mode === '--archive') {
    const {summary, manifest, env, names} = result;
    const dest = path.join(e.repo, 'docs/evidence/local-r113-context-version-membership-2026-09-13');
    const cohort = names.flatMap(name => name.includes('-restoration-') ? [name, name.replace('.json', '-source.json')]
      : [name, name.replace('.json', '.log'), name.replace('.json', '-source.json'), name.replace('.json', '-source-after.json')]);
    const artifacts = [...new Set([...manifest.historical_artifacts.map(x => x.name), ...env.isolated_artifacts.map(x => x.name),
      'r113-qualification-manifest.json', ...cohort])].sort();
    const coverage = new Set(artifacts);
    for (const name of [...e.historicalNames(), ...p.helpers, 'r113-environment.json', 'r113-frozen-source.json',
      'r113-final-gnu-tests.log', 'r113-final-musl-tests.log', 'r113-auxiliary-results.json']) assert(coverage.has(name), 'archive coverage: ' + name);
    for (const name of artifacts) e.bytes(name);
    fs.mkdirSync(dest);
    fs.mkdirSync(path.join(dest, 'raw'));
    summary.artifacts = artifacts.map(name => {
      const bytes = e.bytes(name);
      fs.writeFileSync(path.join(dest, 'raw', name), bytes, {flag: 'wx'});
      return {name, sha256: e.hash(bytes)};
    });
    fs.writeFileSync(path.join(dest, 'summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
    fs.writeFileSync(path.join(dest, 'README.md'), '# R113 Context Version Membership Evidence\n\n' +
      'Local executable-model qualification only. GNU/musl each pass 2,695 tests with five ignored across 48 harnesses.\n' +
      'All 17 source gates, 10 auxiliary gates, frozen/restored journal and membership rosters, and 17 compiled behavioral negatives pass.\n' +
      'All 5,680 source identities are restored; the closed collector transcript and raw artifact hashes are retained.\n\n' +
      'The record preserves isolated and main preliminary attempts, the formatter source change, runner versions,\n' +
      'intentional process-fixture failures, the rejected production-lint preflight and rejected preparation attempts.\n' +
      'The corrected collector accounts for all 49 targets: 48 libtest harnesses and one named harnessless CSV benchmark.\n' +
      'The initial parser did not account for that unchanged benchmark; no runtime source was mutated by preparation.\n' +
      'Nine runner and nine freeze tests qualify the revised helpers; 52 tests cover the corrected cohort evidence checks.\n' +
      'Raw UTC is retained without rewriting; causality uses the same Linux boot and monotonic clock.\n\n' +
      'This does not qualify settlement, production Context hooks, authenticated proofs, native GPU execution, aggregate memory or performance.\n', {flag: 'wx'});
    console.log(JSON.stringify({archive: dest, artifacts: artifacts.length, summary}, null, 2));
  } else console.log(JSON.stringify(result.summary, null, 2));
}
module.exports = {checkBaseline, collect};
