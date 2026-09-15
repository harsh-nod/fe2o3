// Stopped R118 is historical evidence only, never a prerequisite for corrected source.
const fs = require('fs');
const assert = require('assert');
const P = require('./r118-qualification-plan.js');
const E = require('./r118-qualification-evidence.js');
const H = require('./r118-history-v3.js');
const S = require('./r118-history-support-v1.js');
const C = require('./r118-qualification-collect.js');
const originName = 'r118b-origin-v2.json';
const originHash = '197028ede4e23ac3dfad31c65f2e2f2c621c80e12d65c6aec51953a4a93da094';
const manifestName = 'r118-qualification-manifest.json';
const manifestHash = '62e084d3ca50068e1a350bb414edd882072dad37c6b48da4080668bd4ff4898a';
const oldMapHash = 'df2e27085ee920fdd07d2a6955503b1ecdcadc2d7c10c6fd4cec6768cecdadbb';
const newMapHash = '3aa1ab32e9e6e6ffd0474ec9055d3c29b02bf53e45129406617e50f0b3c15126';
const rejected = 'c3-continue-retirement-after-failure';
const diagnosticName = 'r118-qualification-failure-' + rejected + '.json';
const suffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const defaults = {bytes: E.bytes, git: E.git, names: () => fs.readdirSync(E.root)};
const sortedUnique = names => [...new Set(names)].sort();
const inputNames = origin => sortedUnique([originName,
  ...origin.historical_artifacts.map(item => item.name), ...origin.explicit_inputs.map(item => item.name)]);

function oldRun(io, name, command, map, previous, code = 0) {
  const record = io.read(name + '.json');
  E.checkRecord(record, name, command, map, previous, code, io, {
    head: P.parent, cwd: E.repo, contract: E.contract, runner: 'r118-run-v1.js', after: map,
  });
  return {record, log: io.bytes(name + '.log').toString()};
}

function expectedNames(manifest) {
  assert.strictEqual(P.mutations.length, 78);
  assert.strictEqual(P.mutations[72].name, rejected);
  return sortedUnique([...manifest.historical_artifacts.map(item => item.name), manifestName,
    ...P.mutations.slice(0, 72).flatMap(mutation => [
      ...suffixes.map(suffix => 'r118-qualified-mut-' + mutation.name + suffix),
      ...['.json', '-source.json'].map(suffix => 'r118-qualified-restoration-' + mutation.name + suffix),
    ]), ...suffixes.map(suffix => 'r118-qualified-mut-' + rejected + suffix), diagnosticName]);
}

function pinsFrom(log, marker) {
  const lines = log.split('\n').filter(line => line.startsWith(marker));
  assert.strictEqual(lines.length, 1, 'one ' + marker);
  return JSON.parse(lines[0].slice(marker.length));
}

function checkSupplement(result, helpers, launcher, manifest) {
  const name = 'final-captured-byte-check-rejects-semantically-identical-source-map';
  assert.deepStrictEqual(result.log.split('\n').filter(line => line.startsWith('PASS ')), ['PASS ' + name]);
  assert(result.log.endsWith('PASS: 1 snapshot contract test\n'));
  assert(!/^not ok |^FAIL /m.test(result.log));
  const pins = pinsFrom(result.log, 'HELPER_PINS: ');
  assert.deepStrictEqual(pins.slice(0, -1), [...helpers, launcher]);
  assert.strictEqual(pins.length, 20);
  assert.strictEqual(pins.at(-1).name, 'r118-qualification-snapshot-tests-v1.js');
  E.ordered(result.record.clock.source_verified, manifest.clock.source_verified);
  return pins;
}

function checkRejected(result, mutation, files, diagnostic) {
  assert.strictEqual(mutation.name, rejected);
  assert.throws(() => E.checkMutation(result.log, mutation), /one named failure/);
  assert.deepStrictEqual(E.failed(result.log), []);
  assert(!/^test result:/m.test(result.log));
  for (const marker of [mutation.oracle_path + ':317:17:', 'adoption_tests.rs:18:43:',
    'PoisonError', 'panic in a destructor during cleanup', 'SIGABRT: process abort signal']) {
    assert(result.log.includes(marker), 'retained rejected-run diagnostic: ' + marker);
  }
  assert.strictEqual(diagnostic.accepted, false);
  assert.strictEqual(diagnostic.mutation, rejected);
  const manifest = {name: manifestName, sha256: manifestHash};
  assert.deepStrictEqual(diagnostic.manifest, manifest);
  assert.strictEqual(diagnostic.expected_mutation_map_sha256, result.record.source_map_sha256);
  assert.strictEqual(diagnostic.runner_attempted, true);
  assert.strictEqual(diagnostic.authenticated_terminal, true);
  assert.strictEqual(diagnostic.oracle_passed, false);
  assert.deepStrictEqual(diagnostic.application_attempts, files.map(file => ({path: file.path, verified: true})));
  assert.deepStrictEqual(diagnostic.restoration, {restored: true, quiescent: true,
    all_mutant_before_restore: true, live_members: [], external_edit: false, error: null,
    files: files.map(file => ({path: file.path, before_sha256: file.changed_sha256,
      after_sha256: file.original_sha256, write_attempted: true}))});
  assert.deepStrictEqual(diagnostic.recovery_record, {manifest,
    sources: files.map(file => ({path: file.path, original_sha256: file.original_sha256, mutated_sha256: file.changed_sha256}))});
  assert.strictEqual(diagnostic.errors.length, 1);
  assert.strictEqual(diagnostic.errors[0].phase, 'execution-or-oracle');
  assert(diagnostic.errors[0].error.startsWith('AssertionError [ERR_ASSERTION]: one named failure\n'));
  assert(diagnostic.errors[0].error.includes(mutation.test));
}

function checkRecords(io, origin) {
  const map = io.read('r118-frozen-source.json');
  assert.strictEqual(E.hash(JSON.stringify(map)), oldMapHash, 'original source cohort only');
  assert.strictEqual(Object.keys(map).length, 5692);
  const manifest = io.read(manifestName);
  assert.strictEqual(E.hash(io.bytes(manifestName)), manifestHash);
  assert.deepStrictEqual(origin.historical_artifacts.map(item => item.name), expectedNames(manifest));
  assert.strictEqual(manifest.historical_artifacts.length, 297);
  assert.strictEqual(E.hash(JSON.stringify(manifest.historical_artifacts.map(item => item.name))),
    'a9658d2496c4e0cc0ff8293e38277f9296513d357600d2b1cbc645ea3ea5424f');
  const env = io.read('r118-environment.json');
  assert.strictEqual(env.publication_parent, P.parent);
  assert.strictEqual(env.accepted_runtime_checkpoint, P.accepted);
  assert.deepStrictEqual(env.source_delta, P.sourceDelta);
  const prior = JSON.parse(io.git(['show', P.parent + ':' + P.previous + 'raw/r117-environment.json']));
  assert.deepStrictEqual(env.binaries, prior.binaries, 'retained binary pins, not current binary observations');
  assert.deepStrictEqual(env.runners.map(item => item.name), P.freezeHelpers);
  for (const pin of [...env.runners, ...env.prior_artifacts, ...env.prerequisite_artifacts, ...env.prerequisite_validation_inputs]) {
    assert.strictEqual(E.hash(io.bytes(pin.name)), pin.sha256, pin.name);
  }
  assert.deepStrictEqual(env.environment_overrides, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'});
  assert.deepStrictEqual(env.environment_removed, ['XDG_RUNTIME_DIR']);
  assert.strictEqual(env.ambient_stack.RUST_MIN_STACK, null);
  for (const key of ['test_deadlines_changed', 'hardware_qualification', 'solver_rerun']) assert.strictEqual(env[key], false);
  assert.deepStrictEqual(env.prerequisite_runs, P.prerequisites.map(([name]) => name));
  assert.deepStrictEqual(env.prerequisite_artifacts.map(item => item.name),
    P.prerequisites.flatMap(([name]) => suffixes.map(suffix => name + suffix)).sort());
  const history = H.checkHistory(map, io, P.history);
  assert.deepStrictEqual(env.history, history);
  assert.deepStrictEqual(env.support_history, S.check(map, history, io));
  for (const name of P.history.prerequisiteNames) E.ordered(io.read(name + '.json').clock.source_verified, env.observation);

  // These retained collector functions read old files directly. Outer pin checks
  // establish endpoint consistency, not snapshot-only or continuous immutability.
  const previousContract = C.checkSupportContracts(map, env);
  const source = oldRun(io, 'r118-source-campaign', ['python3', '-B', E.file('r118-source-gate.py'), 'r118-final'], map, previousContract);
  E.ordered(env.observation, source.record.clock.start);
  const auxiliary = oldRun(io, 'r118-auxiliary-campaign', ['python3', '-B', E.file('r118-auxiliary-gates.py')], map, 'r118-source-campaign.json');
  C.checkCampaignTranscript(source.log, io.read('r118-final-source-gate.json'), false);
  C.checkCampaignTranscript(auxiliary.log, io.read('r118-auxiliary-results.json'), true);
  C.checkGateRows(map, 'r118-final-source-gate.json', 'r117-final-source-gate.json', 15, 'r118-final', 'r118-final');
  C.checkGateRows(map, 'r118-auxiliary-results.json', 'r117-auxiliary-results.json', 10, 'r118', 'r118-auxiliary');
  const allNames = E.passing(io.bytes('r118-reviewed-gnu-all.log').toString());
  let previous = 'r118-auxiliary-campaign.json';
  for (const focused of P.focused) {
    const name = 'r118-frozen-' + focused[0];
    C.checkFocused(oldRun(io, name, E.focusedCommand(focused), map, previous), focused, allNames);
    previous = name + '.json';
  }
  const contracts = oldRun(io, 'r118-qualification-contract-tests', ['node', E.file('r118-qualification-tests.js')], map, previous);
  const cases = contracts.log.split('\n').filter(line => line.startsWith('PASS ')).map(line => line.slice(5));
  assert.strictEqual(cases.length, 166);
  assert.strictEqual(E.hash(JSON.stringify(cases)), 'b3979d69a37060f7be7e6254e1c33b5db2b6f67e58d1c448186fdcbd35f3b834');
  C.contractTranscript(contracts.log, 'qualification', cases);
  const helpers = P.helpers.map(name => ({name, sha256: E.hash(io.bytes(name))}));
  const launcher = {name: 'r118-qualification-launch-v1.js', sha256: E.hash(io.bytes('r118-qualification-launch-v1.js'))};
  assert.deepStrictEqual(pinsFrom(contracts.log, 'HELPER_PINS: '), helpers);
  assert.deepStrictEqual(pinsFrom(contracts.log, 'LAUNCHER_PIN: '), launcher);
  previous = 'r118-qualification-contract-tests.json';
  C.validateManifest(manifest, {map, helpers, launcher, previous}, {
    bytes: io.bytes, historicalNames: () => manifest.historical_artifacts.map(item => item.name),
  });
  const supplement = oldRun(io, 'r118-qualification-snapshot-contract-tests-v1',
    ['node', E.file('r118-qualification-snapshot-tests-v1.js')], map, previous);
  for (const pin of checkSupplement(supplement, helpers, launcher, manifest)) assert.strictEqual(E.hash(io.bytes(pin.name)), pin.sha256);
  previous = manifestName;
  for (const mutation of P.mutations.slice(0, 72)) {
    const files = E.mutationSources(map, manifest.original_sources, mutation);
    const mutatedMap = E.mutationMap(map, files);
    const name = 'r118-qualified-mut-' + mutation.name;
    const entry = manifest.entries[name + '.json'];
    assert.strictEqual(entry.predecessor, previous);
    const result = oldRun(io, name, entry.command, mutatedMap, previous, 101);
    E.checkMutation(result.log, mutation);
    previous = name + '.json';
    const restored = 'r118-qualified-restoration-' + mutation.name + '.json';
    E.checkRestoration(io.read(restored), files, map, manifestHash,
      {name: previous, sha256: E.hash(io.bytes(previous))}, result.record, io.read(restored.replace('.json', '-source.json')));
    previous = restored;
  }
  const mutation = P.mutations[72];
  const files = E.mutationSources(map, manifest.original_sources, mutation);
  const name = 'r118-qualified-mut-' + rejected;
  assert.strictEqual(manifest.entries[name + '.json'].predecessor, previous);
  const result = oldRun(io, name, manifest.entries[name + '.json'].command, E.mutationMap(map, files), previous, 101);
  const diagnostic = io.read(diagnosticName);
  assert.deepStrictEqual(diagnostic.terminal_record, {name: name + '.json', sha256: E.hash(io.bytes(name + '.json'))});
  checkRejected(result, mutation, files, diagnostic);
  return {accepted: false, source_map_sha256: oldMapHash, source_identities: 5692,
    preserved_artifacts: 735, original_history_artifacts: 297, planned_entries: Object.keys(manifest.entries).length,
    planned_executions: 78, qualified_negatives: 72, rejected_execution: 73,
    unexecuted: [74, 75, 76, 77, 78], rejected_execution_physical_recovery: true,
    rejected_execution_qualified_restoration: false, full: history.full};
}

function checkPriorHistory(io = defaults) {
  const originBytes = Buffer.from(io.bytes(originName));
  assert.strictEqual(E.hash(originBytes), originHash, 'pinned correction origin');
  const origin = JSON.parse(originBytes);
  assert.strictEqual(origin.accepted_parent, P.parent);
  assert.strictEqual(origin.prior_source_map_sha256, oldMapHash);
  assert.strictEqual(origin.initial_source_map_sha256, newMapHash);
  assert.strictEqual(origin.source_count, 5692);
  assert.deepStrictEqual(origin.prior_campaign, {accepted: false, qualified_negatives: 72,
    rejected_execution: 73, unexecuted: [74, 75, 76, 77, 78]});
  const pins = [...origin.historical_artifacts, ...origin.explicit_inputs];
  assert.strictEqual(origin.historical_artifacts.length, 735);
  assert.strictEqual(origin.explicit_inputs.length, 10);
  assert.strictEqual(sortedUnique(pins.map(item => item.name)).length, pins.length);
  const captured = new Map([[originName, originBytes]]);
  for (const pin of pins) {
    const bytes = Buffer.from(io.bytes(pin.name));
    assert.strictEqual(E.hash(bytes), pin.sha256, 'historical input pin: ' + pin.name);
    captured.set(pin.name, bytes);
  }
  const checkMembership = () => assert.deepStrictEqual(sortedUnique([
    ...io.names().filter(name => /^r118-.*\.(json|log|js|py|md)$/.test(name)),
    ...origin.historical_artifacts.filter(item => !item.name.startsWith('r118-')).map(item => item.name),
  ]), origin.historical_artifacts.map(item => item.name), 'stopped historical membership');
  checkMembership();
  const snapshot = {bytes: name => {assert(captured.has(name), 'captured history input: ' + name); return Buffer.from(captured.get(name));}, git: io.git};
  snapshot.read = name => JSON.parse(snapshot.bytes(name));
  const oldMap = snapshot.read('r118-frozen-source.json');
  const newMap = snapshot.read('r118b-initial-source.json');
  assert.strictEqual(E.hash(JSON.stringify(newMap)), newMapHash);
  assert.deepStrictEqual(H.changedPaths(oldMap, newMap), origin.changed_source);
  assert.deepStrictEqual(origin.changed_source, ['crates/fe2o3-runtime/src/async_engine/tests/owned_tests/preparation_tests/completion_tests.rs']);
  const changed = origin.changed_source[0];
  assert.strictEqual(E.hash(snapshot.bytes('r118b-prior-completion-tests.rs')), oldMap[changed]);
  assert.strictEqual(E.hash(snapshot.bytes('r118b-initial-completion-tests.rs')), newMap[changed]);
  const format = snapshot.read('r118b-format.json');
  E.checkRecord(format, 'r118b-format', ['cargo', '+nightly-2026-04-03', 'fmt', '--all'], newMap, null, 0, snapshot,
    {head: P.parent, cwd: E.repo, contract: 'r118b-raw-utc-boot-monotonic-v1', runner: 'r118b-run-v1.js', after: newMap});
  assert.strictEqual(snapshot.bytes('r118b-format.log').toString(), '');
  E.ordered(format.clock.source_verified, origin.observation);
  const result = checkRecords(snapshot, origin);
  checkMembership();
  for (const [name, bytes] of captured) assert.strictEqual(E.hash(io.bytes(name)), E.hash(bytes), 'unchanged history input: ' + name);
  return result;
}

if (require.main === module) console.log(JSON.stringify(checkPriorHistory(), null, 2));
module.exports = {originName, originHash, inputNames, expectedNames, oldRun,
  checkSupplement, checkRejected, checkRecords, checkPriorHistory};
