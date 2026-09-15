const fs = require('fs');
const path = require('path');
const assert = require('assert');
const p = require('./r118-qualification-plan.js');
const e = require('./r118-qualification-evidence.js');
const h = require('./r118-history-v3.js');
const support = require('./r118-history-support-v1.js');
const suffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const baseline = () => e.read('r118-frozen-source.json');
function contractTranscript(log, kind, names) {
  assert.deepStrictEqual(log.split('\n').filter(line => line.startsWith('PASS ')), names.map(name => 'PASS ' + name));
  assert.strictEqual(new Set(names).size, names.length);
  assert(log.includes('PASS: ' + names.length + ' ' + kind + ' contract tests\n'));
  assert(!/^not ok |^FAIL /m.test(log));
}
function checkSupportContracts(map, env) {
  for (const [name, digest] of [
    ['r118-freeze-inputs-v1.json', '541eaffa4fb1d8cd2249ddfbc7c3287253f428f1225ec538dd162f665bdb6420'],
    ['r118-freeze-inputs-v2.json', '85a518cc76cab030adf5fe6c877776c346dbb9b25483ecb78008809ba2b2a317'],
    ['r118-qualification-lifecycle-inputs-v1.json', 'b2c7fdaa8eb11625ddfde7e9ef5372041b56fd5e7845f3862b5e8c0a84b8a996'],
  ]) {
    assert.strictEqual(e.hash(e.bytes(name)), digest, 'contract input manifest');
    const inputs = e.read(name);
    assert.strictEqual(inputs.parent, p.parent);
    assert.strictEqual(inputs.source_map_sha256, p.sourceMapSha256);
    for (const helper of inputs.helpers) assert.strictEqual(e.hash(e.bytes(helper.name)), helper.sha256, helper.name);
  }
  const failed = e.checkRun('r118-freeze-contract-tests', ['node', e.file('r118-freeze-tests.js')], map, 'r118-history-preflight-v1.json', 1);
  assert.deepStrictEqual(failed.log.split('\n').filter(line => line.startsWith('PASS ')), ['PASS unchanged']);
  assert(failed.log.includes('added-source unexpectedly accepted'));
  assert(!failed.log.includes('PASS: 50 freeze contract tests'));
  const corrected = e.checkRun('r118-freeze-contract-tests-v2', ['node', e.file('r118-freeze-tests-v2.js')], map, 'r118-freeze-contract-tests.json');
  const freezeNames = corrected.log.split('\n').filter(line => line.startsWith('PASS ')).map(line => line.slice(5));
  assert.strictEqual(freezeNames.length, p.freezeContractCount);
  assert.strictEqual(e.hash(JSON.stringify(freezeNames)), 'b89b9018782ba81f21d7a6ee658a897fabe71da6d5e5308e21783d53c083f796');
  contractTranscript(corrected.log, 'freeze', freezeNames);
  assert(corrected.log.endsWith('PASS: 50 freeze contract tests\n'));
  const lifecycle = e.checkRun('r118-qualification-lifecycle-contracts-v1',
    ['node', '--test', e.file('r118-qualification-lifecycle-tests-v1.js')], map, 'r118-freeze-contract-tests-v2.json');
  const lifecycleNames = h.tapNames(lifecycle.log, 22);
  assert.strictEqual(e.hash(JSON.stringify(lifecycleNames)), '5413c005fa131f2647779f80858c96a1d68fbd04938ed3716c9312ba6acc44ce');
  e.ordered(lifecycle.record.clock.source_verified, env.observation);
  return 'r118-qualification-lifecycle-contracts-v1.json';
}
function checkGateRows(map, name, priorName, count, prefix, sourcePrefix) {
  const rows = e.read(name);
  const oldRows = JSON.parse(e.committed('raw/' + priorName));
  assert.strictEqual(rows.length, count);
  assert.deepStrictEqual(rows.map(row => row.name), oldRows.map(row => row.name));
  assert.deepStrictEqual(p.docRelocations, []);
  for (const [index, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, oldRows[index].command.map(argument =>
      argument.replace('r117-production-metadata.log', 'r118-production-metadata.log')));
    assert.strictEqual(row.cwd, oldRows[index].cwd);
    assert.strictEqual(row.log, e.file(prefix + '-' + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
    assert(Number.isFinite(row.elapsed_seconds) && row.elapsed_seconds >= 0);
    const log = e.bytes(prefix + '-' + row.name + '.log').toString();
    const oldLog = e.committed('raw/' + prefix.replace('r118', 'r117') + '-' + row.name + '.log');
    if (row.command[0] === 'cargo' && row.command[2] === 'test') {
      e.assertPassing(log);
      assert.deepStrictEqual(e.passing(log), e.passing(oldLog), row.name);
      assert.deepStrictEqual(e.ignored(log), e.ignored(oldLog), row.name);
      assert.deepStrictEqual(e.totals(log), e.totals(oldLog), row.name);
    }
    if (['python', 'dependency-tests'].includes(row.name)) {
      assert.deepStrictEqual([...log.matchAll(/^Ran (\d+) tests? /gm)].map(match => match[1]),
        [...oldLog.matchAll(/^Ran (\d+) tests? /gm)].map(match => match[1]));
      assert(/^OK$/m.test(log));
      assert(!/^FAILED \(/m.test(log));
    }
    if (['fmt', 'whitespace'].includes(row.name)) assert.strictEqual(log, '');
    if (row.name === 'production-metadata') {
      assert.strictEqual(row.stderr_log, e.file('r118-production-metadata.stderr.log'));
      e.bytes('r118-production-metadata.stderr.log');
      const metadata = JSON.parse(log);
      assert.strictEqual(metadata.workspace_root, e.repo);
      assert(metadata.packages.some(item => item.name === 'fe2o3-runtime'));
      assert(metadata.resolve.nodes.length > 0);
    } else assert.strictEqual(row.stderr_log, undefined);
  }
  for (const suffix of ['source-inputs', 'source-after']) assert.deepStrictEqual(e.read(sourcePrefix + '-' + suffix + '.json'), map);
  assert.deepStrictEqual(e.read(sourcePrefix + '-complete.json'), {completed: true, gates: count, source_identities: p.sourceCount});
}
function checkCampaignTranscript(log, rows, auxiliary) {
  const lines = log.trimEnd().split('\n');
  assert.strictEqual(lines.length, rows.length + 1, 'exact campaign transcript rows');
  assert.strictEqual(lines.at(-1), 'PASS: ' + p.sourceCount + (auxiliary
    ? ' auxiliary source identities unchanged' : ' non-documentation source identities unchanged'));
  assert.deepStrictEqual(lines.slice(0, -1).map(line => JSON.parse(line)), rows, 'wrapper and result-file agreement');
}
function checkFocused(result, focused, allNames) {
  const [, filter, count, exact] = focused;
  const expected = allNames.filter(name => exact ? name === filter : name.startsWith(filter));
  assert.strictEqual(expected.length, count);
  e.assertPassing(result.log);
  assert.deepStrictEqual(e.passing(result.log), expected);
  assert.deepStrictEqual(e.ignored(result.log), []);
  assert.deepStrictEqual(e.totals(result.log), {harnesses: 1, passed: count, failed: 0, ignored: 0});
}
function checkBaseline() {
  const map = baseline();
  assert.strictEqual(Object.keys(map).length, p.sourceCount);
  assert.strictEqual(e.hash(JSON.stringify(map)), p.sourceMapSha256);
  assert.deepStrictEqual(e.identities(), map);
  const env = e.read('r118-environment.json');
  assert.strictEqual(env.publication_parent, p.parent);
  assert.strictEqual(env.accepted_runtime_checkpoint, p.accepted);
  assert.deepStrictEqual(env.source_delta, p.sourceDelta);
  assert.deepStrictEqual(env.binaries, JSON.parse(e.committed('raw/r117-environment.json')).binaries);
  for (const binary of env.binaries) assert.strictEqual(e.hash(fs.readFileSync(binary.path)), binary.sha256);
  assert.deepStrictEqual(env.runners.map(item => item.name), p.freezeHelpers);
  for (const item of [...env.runners, ...env.prior_artifacts, ...env.prerequisite_artifacts, ...env.prerequisite_validation_inputs]) {
    assert.strictEqual(e.hash(e.bytes(item.name)), item.sha256, item.name);
  }
  const inputNames = [...new Set([...p.history.artifacts.map(item => item.name), ...support.inputNames(),
    ...p.freezeHelpers, ...env.prior_artifacts.map(item => item.name)])].sort();
  assert.deepStrictEqual(env.prerequisite_validation_inputs.map(item => item.name), inputNames);
  assert.deepStrictEqual(env.environment_overrides, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'});
  assert.deepStrictEqual(env.environment_removed, ['XDG_RUNTIME_DIR']);
  assert.strictEqual(env.ambient_stack.RUST_MIN_STACK, null);
  assert.strictEqual(process.env.RUST_MIN_STACK ?? null, null);
  for (const field of ['test_deadlines_changed', 'hardware_qualification', 'solver_rerun']) assert.strictEqual(env[field], false);
  const prerequisiteNames = p.prerequisites.map(([name]) => name);
  assert.deepStrictEqual(env.prerequisite_runs, prerequisiteNames);
  assert.deepStrictEqual(env.prerequisite_artifacts.map(item => item.name), prerequisiteNames.flatMap(name => suffixes.map(suffix => name + suffix)).sort());
  const history = h.checkHistory(map);
  assert.deepStrictEqual(env.history, history);
  assert.deepStrictEqual(env.support_history, support.check(map, history));
  for (const name of p.history.prerequisiteNames) e.ordered(e.read(name + '.json').clock.source_verified, env.observation);
  const previousContract = checkSupportContracts(map, env);
  const source = e.checkRun('r118-source-campaign', ['python3', '-B', e.file('r118-source-gate.py'), 'r118-final'], map, previousContract);
  e.ordered(env.observation, source.record.clock.start);
  const auxiliary = e.checkRun('r118-auxiliary-campaign', ['python3', '-B', e.file('r118-auxiliary-gates.py')], map, 'r118-source-campaign.json');
  checkCampaignTranscript(source.log, e.read('r118-final-source-gate.json'), false);
  checkCampaignTranscript(auxiliary.log, e.read('r118-auxiliary-results.json'), true);
  checkGateRows(map, 'r118-final-source-gate.json', 'r117-final-source-gate.json', 15, 'r118-final', 'r118-final');
  checkGateRows(map, 'r118-auxiliary-results.json', 'r117-auxiliary-results.json', 10, 'r118', 'r118-auxiliary');
  const allNames = e.passing(e.bytes('r118-reviewed-gnu-all.log').toString());
  let previous = 'r118-auxiliary-campaign.json';
  for (const focused of p.focused) {
    const name = 'r118-frozen-' + focused[0];
    checkFocused(e.checkRun(name, e.focusedCommand(focused), map, previous), focused, allNames);
    previous = name + '.json';
  }
  const tests = e.checkRun('r118-qualification-contract-tests', ['node', e.file('r118-qualification-tests.js')], map, previous);
  const cases = require('./r118-qualification-tests.js').cases;
  assert(Array.isArray(cases) && cases.length > 0);
  contractTranscript(tests.log, 'qualification', cases);
  const pins = [...tests.log.matchAll(/^HELPER_PINS: (.+)$/gm)];
  assert.strictEqual(pins.length, 1);
  const helpers = p.helpers.map(name => ({name, sha256: e.hash(e.bytes(name))}));
  assert.deepStrictEqual(JSON.parse(pins[0][1]), helpers);
  const launcherPins = [...tests.log.matchAll(/^LAUNCHER_PIN: (.+)$/gm)];
  assert.strictEqual(launcherPins.length, 1);
  const launcher = {name: 'r118-qualification-launch-v1.js', sha256: e.hash(e.bytes('r118-qualification-launch-v1.js'))};
  assert.deepStrictEqual(JSON.parse(launcherPins[0][1]), launcher);
  return {map, env, history, allNames, helpers, launcher, cases, previous: 'r118-qualification-contract-tests.json'};
}
function validateManifest(manifest, baseline, io = e) {
  const {map, helpers, launcher, previous: prerequisite} = baseline;
  assert.strictEqual(e.hash(JSON.stringify(map)), p.sourceMapSha256, 'fixed qualified source map');
  assert.strictEqual(Object.keys(map).length, p.sourceCount);
  assert.strictEqual(manifest.source_head, p.parent);
  assert.strictEqual(manifest.source_map_sha256, e.hash(JSON.stringify(map)));
  assert.deepStrictEqual(manifest.helpers, helpers);
  assert.deepStrictEqual(manifest.launcher, launcher, 'qualified launcher identity');
  assert.strictEqual(launcher.name, 'r118-qualification-launch-v1.js');
  assert.strictEqual(e.hash(io.bytes(launcher.name)), launcher.sha256);
  for (const helper of helpers) assert.strictEqual(e.hash(io.bytes(helper.name)), helper.sha256, helper.name);
  assert.deepStrictEqual(manifest.mutations, p.mutations);
  e.originalSources(map, manifest.original_sources);
  assert.deepStrictEqual(manifest.entries, e.expectedEntries(map, manifest.original_sources));
  assert.deepStrictEqual(manifest.historical_artifacts.map(item => item.name), io.historicalNames());
  for (const item of manifest.historical_artifacts) assert.strictEqual(e.hash(io.bytes(item.name)), item.sha256, item.name);
  assert.strictEqual(manifest.clock.contract, e.contract);
  assert.strictEqual(manifest.clock.error, null);
  assert.strictEqual(manifest.clock.kind, 'manifest');
  assert.strictEqual(manifest.recorded_at, new Date(manifest.clock.source_verified.utc_ms).toISOString());
  const priorBytes = io.bytes(prerequisite);
  assert.deepStrictEqual(manifest.clock.predecessor, {name: prerequisite, sha256: e.hash(priorBytes), observation: JSON.parse(priorBytes).clock.source_verified});
  e.ordered(manifest.clock.predecessor.observation, manifest.clock.source_verified);
}
function collect(closed, getBaseline = checkBaseline) {
  const manifestName = 'r118-qualification-manifest.json';
  const manifestBytes = e.bytes(manifestName);
  const manifest = JSON.parse(manifestBytes);
  const manifestHash = e.hash(manifestBytes);
  const historicalNames = e.historicalNames();
  const mutationNames = p.mutations.flatMap(mutation => [
    ...suffixes.map(suffix => 'r118-qualified-mut-' + mutation.name + suffix),
    ...['.json', '-source.json'].map(suffix => 'r118-qualified-restoration-' + mutation.name + suffix),
  ]);
  const focusedNames = p.focused.flatMap(([kind]) => suffixes.map(suffix => 'r118-qualified-restored-' + kind + suffix));
  const collectorNames = closed ? suffixes.map(suffix => 'r118-qualified-collector-validation' + suffix) : [];
  const capturedNames = [...new Set([...historicalNames, manifestName, ...mutationNames, ...focusedNames, ...collectorNames])].sort();
  const captured = new Map(capturedNames.map(name => [name, Buffer.from(name === manifestName ? manifestBytes : e.bytes(name))]));
  const baseline = getBaseline();
  const {map, env, history, allNames, cases} = baseline;
  validateManifest(manifest, baseline);
  let previous = manifestName;
  const hashes = [];
  for (const mutation of p.mutations) {
    const files = e.mutationSources(map, manifest.original_sources, mutation);
    const expected = e.mutationMap(map, files);
    const name = 'r118-qualified-mut-' + mutation.name;
    const entry = manifest.entries[name + '.json'];
    assert.strictEqual(entry.predecessor, previous);
    const result = e.checkRun(name, entry.command, expected, previous, 101);
    e.checkMutation(result.log, mutation);
    hashes.push([mutation.name, e.hash(JSON.stringify(expected))]);
    previous = name + '.json';
    const restoredName = 'r118-qualified-restoration-' + mutation.name + '.json';
    e.checkRestoration(e.read(restoredName), files, map, manifestHash,
      {name: previous, sha256: e.hash(e.bytes(previous))}, result.record, e.read(restoredName.replace('.json', '-source.json')));
    previous = restoredName;
  }
  e.checkMutationGroups(hashes);
  for (const focused of p.focused) {
    const name = 'r118-qualified-restored-' + focused[0];
    checkFocused(e.checkRun(name, e.focusedCommand(focused), map, previous), focused, allNames);
    previous = name + '.json';
  }
  const collector = 'r118-qualified-collector-validation';
  const command = ['node', e.file('r118-qualification-collect.js'), '--check'];
  assert.deepStrictEqual(manifest.entries[collector + '.json'], {kind: 'run', predecessor: previous,
    source_map_sha256: e.hash(JSON.stringify(map)), command, returncode: 0});
  const summary = {source_head: p.parent, accepted_parent: p.accepted, source_map_sha256: p.sourceMapSha256,
    scope: 'Host submission identity, generated descriptor identity and scripted retained-owner lifecycle qualification',
    source_identities: p.sourceCount, changed_source: p.sourceDelta, full: history.full,
    original_history: history, source_gates: 17, subsequent_source_gates: 15, auxiliary_gates: 10,
    focused: Object.fromEntries(p.focused.map(([name, , count]) => [name, count])),
    compiled_negatives: p.mutations.length, distinct_mutations: p.uniqueMutationCount,
    mutation_categories: p.expected, mutation_source_groups: p.mutationSourceGroups,
    new_behavioral_tests: p.newTests.length, new_source_guards: 0,
    core_contract_tests: 48, runner_contract_tests: 9, schema_contract_tests: 13,
    freeze_contract_tests: p.freezeContractCount, lifecycle_contract_tests: 22, qualification_contract_tests: cases.length,
    formal_qualification: false, native_qualification: false, performance_qualification: false,
    production_semantics_changed: false, total_memory_qualification: false};
  if (closed) assert.deepStrictEqual(JSON.parse(e.checkRun(collector, command, map, previous).log), summary, 'closed collector transcript');
  assert.deepStrictEqual(e.identities(), map);
  assert.deepStrictEqual(e.historicalNames(), historicalNames, 'unchanged collection membership');
  for (const [name, bytes] of captured) assert.strictEqual(e.hash(e.bytes(name)), e.hash(bytes), 'unchanged collection input: ' + name);
  return {summary, manifest, env, captured, names: Object.keys(manifest.entries)};
}
function archive({summary, manifest, names, captured}, io = {
  mkdir: name => fs.mkdirSync(name),
  write: (name, bytes) => fs.writeFileSync(name, bytes, {flag: 'wx'}),
  read: name => fs.readFileSync(name),
}) {
  const destination = path.join(e.repo, 'docs/evidence/local-r118-admission-lifecycle-2026-09-14');
  const cohort = names.flatMap(name => name.includes('-restoration-') ? [name, name.replace('.json', '-source.json')]
    : suffixes.map(suffix => name.replace('.json', suffix)));
  const artifacts = [...new Set([...manifest.historical_artifacts.map(item => item.name), 'r118-qualification-manifest.json', ...cohort])].sort();
  for (const name of [...e.historicalNames(), ...p.helpers, 'r118-environment.json', 'r118-frozen-source.json']) {
    assert(artifacts.includes(name), 'archive coverage: ' + name);
  }
  assert.deepStrictEqual(artifacts, [...captured.keys()], 'complete validated archive snapshot');
  io.mkdir(destination); io.mkdir(path.join(destination, 'raw'));
  summary.artifacts = artifacts.map(name => {
    const bytes = captured.get(name);
    io.write(path.join(destination, 'raw', name), bytes);
    assert.strictEqual(e.hash(io.read(path.join(destination, 'raw', name))), e.hash(bytes), 'exact archived bytes');
    return {name, sha256: e.hash(bytes)};
  });
  io.write(path.join(destination, 'summary.json'), JSON.stringify(summary, null, 2) + '\n');
  io.write(path.join(destination, 'README.md'), '# R118 Admission And Lifecycle Evidence\n\n' +
    'Local CPU/test qualification only; no production API, semantics, feature or dependency change.\n' +
    'GNU and musl each pass 2,780 tests with five ignored. Eighteen runtime tests are added; every other exact target roster is retained.\n' +
    'All seventeen source gates, ten auxiliary checks and six frozen/restored suites pass. The 78 compiled negative executions cover 74 distinct source maps, with two helper calibrations explicitly distinguished from production mutations.\n' +
    'Every negative reaches its named behavioral oracle and restores all 5,692 source identities. Runner, freeze, lifecycle and qualification contracts pass, and the collector has a closed matching transcript.\n' +
    'Original candidate source maps, formatter changes, compilation failures and failed helper attempts are retained with their original contexts. Old-boot history remains independent. Endpoint equality does not establish continuous immutability.\n' +
    'This does not establish generated native ISSUE/COMPLETE, production journal integration, Worker/compiler authority, formal correspondence, aggregate memory bounds or HIP/HSA performance parity.\n');
  return {archive: destination, artifacts: artifacts.length, summary};
}
function main() {
  const mode = process.argv[2];
  assert(['--check', '--archive'].includes(mode));
  const result = collect(mode === '--archive');
  console.log(JSON.stringify(mode === '--archive' ? archive(result) : result.summary, null, 2));
}
if (require.main === module) main();
module.exports = {checkBaseline, checkSupportContracts, checkGateRows, checkCampaignTranscript, checkFocused, contractTranscript, validateManifest, collect, archive, main};
