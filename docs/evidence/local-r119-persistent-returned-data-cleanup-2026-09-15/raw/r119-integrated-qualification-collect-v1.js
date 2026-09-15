const fs = require('fs');
const path = require('path');
const assert = require('assert');
const p = require('./r119-integrated-campaign-plan-v1.js');
const e = require('./r119-integrated-qualification-evidence-v1.js');
const h = require('./r119-integrated-history-v2.js');
const support = require('./r119-integrated-freeze-v2.js');
const validation = require('./r119-integrated-validation-v2.js');
const campaign = require('./r119-integrated-campaign-validation-v1.js');
const suffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const baseline = () => e.read('r119-integrated-frozen-source.json');
const interleavedLinuxHelpersHash = 'b78cb48f28f17aaad884d876e8fce4b4cae038155be715c5b678d77113956815';
function contractTranscript(log, kind, names) {
  assert.deepStrictEqual(log.split('\n').filter(line => line.startsWith('PASS ')), names.map(name => 'PASS ' + name));
  assert.strictEqual(new Set(names).size, names.length);
  assert.deepStrictEqual(log.match(/^PASS: .*$/gm), ['PASS: ' + names.length + ' ' + kind + ' contract tests']);
  assert(!/^not ok |^FAIL /m.test(log));
}
function validateFrozenEnvironment(env, map, io = e) {
  assert.strictEqual(env.accepted, false);
  assert.strictEqual(env.publication_parent, p.parent);
  assert.strictEqual(env.accepted_runtime_checkpoint, p.accepted);
  assert.strictEqual(env.source_map_sha256, p.sourceMapSha256);
  assert.strictEqual(env.source_identities, p.sourceCount);
  assert.deepStrictEqual(env.helpers.map(pin => pin.name), support.helpers);
  const names = env.prior_artifacts.map(pin => pin.name);
  assert.strictEqual(names.length, 123);
  assert.deepStrictEqual(names, [...new Set(names)].sort(), 'exact canonical frozen history');
  for (const pin of env.helpers) assert(names.includes(pin.name), 'frozen helper captured');
  const captured = new Map(names.map(name => [name, Buffer.from(io.bytes(name))]));
  const snapshot = {bytes: name => {
    assert(captured.has(name), 'captured frozen input: ' + name);
    return Buffer.from(captured.get(name));
  }, git: io.git};
  snapshot.read = name => JSON.parse(snapshot.bytes(name));
  for (const pin of [...env.helpers, ...env.prior_artifacts])
    assert.strictEqual(e.hash(snapshot.bytes(pin.name)), pin.sha256, pin.name);
  const history = h.checkRecords(map, snapshot);
  assert.deepStrictEqual(env.history, history);
  assert.deepStrictEqual(env.support, support.checkSupport(map, history, snapshot));
  e.ordered(snapshot.read(env.support.previous).clock.source_verified, env.observation);
  assert.strictEqual(env.recorded_at, new Date(env.observation.utc_ms).toISOString());
  assert.deepStrictEqual(env.environment_overrides, {
    CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'});
  assert.deepStrictEqual(env.environment_removed, ['XDG_RUNTIME_DIR']);
  assert.strictEqual(env.ambient_stack.RUST_MIN_STACK, null);
  for (const field of ['test_deadlines_changed', 'hardware_qualification', 'solver_rerun'])
    assert.strictEqual(env[field], false);
  assert.deepStrictEqual(env.campaign_envelope, {deadline_ms: 7200000, individual_gate_deadline_ms: 1800000});
  for (const [name, bytes] of captured)
    assert.strictEqual(e.hash(io.bytes(name)), e.hash(bytes), 'unchanged frozen input: ' + name);
  return history;
}
function docNames(names, map, relocated, io = {
  old: file => e.git(['show', p.parent + ':' + file]),
  current: file => fs.readFileSync(path.join(e.repo, file), 'utf8'),
}) {
  const rename = new Map();
  const fence = (text, line) => {
    const lines = text.split('\n');
    assert.strictEqual(lines[line - 1], '/// ```compile_fail');
    const end = lines.findIndex((text, index) => index >= line && text === '/// ```');
    assert(end >= line, 'closed compile-fail fence');
    return lines.slice(line - 1, end + 1);
  };
  for (const {path: file, from, to} of p.docRelocations) {
    const old = io.old(file);
    const current = io.current(file);
    assert.strictEqual(e.hash(current), map[file], 'frozen doctest source');
    assert.deepStrictEqual(fence(current, to), fence(old, from), 'unchanged relocated doctest');
    const type = from === 3093 ? 'Gfx942CompletedDispatchBatchV1' : 'Gfx942DispatchBatchV1';
    const name = line => file + ' - queue::dispatch_binding::' + type + ' (line ' + line + ') - compile fail';
    assert.strictEqual(names.filter(value => value === name(from)).length, relocated ? 1 : 0,
      'exact old relocated doctest: ' + name(from));
    rename.set(name(from), name(to));
  }
  const result = names.map(name => rename.get(name) || name).sort();
  assert.strictEqual(new Set(result).size, result.length, 'unique doctest roster');
  return result;
}
function checkSupportContracts(map, previous) {
  const pins = campaign.inputs();
  for (const [kind, file, names] of [
    ['mutation-core-contracts-v1', 'mutation-core-tests-v1', p.coreNames],
    ['qualification-lifecycle-contracts-v1', 'qualification-lifecycle-tests-v1', p.lifecycleNames],
  ]) {
    const name = 'r119-integrated-' + kind;
    const result = e.checkRun(name, ['node', '--test', e.file('r119-integrated-' + file + '.js')], map, previous);
    validation.checkTap(campaign.transcript(result.log, pins.sha256, true), names);
    previous = name + '.json';
  }
  return previous;
}
function checkGateRows(map, name, priorName, count, prefix, sourcePrefix, io = e) {
  const rows = io.read(name);
  const oldRows = JSON.parse(io.committed('raw/' + priorName));
  assert.strictEqual(rows.length, count);
  assert.deepStrictEqual(rows.map(row => row.name), oldRows.map(row => row.name));
  for (const [index, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, oldRows[index].command.map(argument =>
      argument.replace('r118b-production-metadata.log', 'r119-integrated-production-metadata.log')));
    assert.strictEqual(row.cwd, oldRows[index].cwd);
    assert.strictEqual(row.log, e.file(prefix + '-' + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
    assert(Number.isFinite(row.elapsed_seconds) && row.elapsed_seconds >= 0);
    let log = io.bytes(prefix + '-' + row.name + '.log').toString();
    const oldLog = io.committed('raw/' + path.basename(oldRows[index].log));
    if (row.name === 'linux-helpers') {
      const test = 'queue_linux::tests::unpublished_payload_is_unmapped_when_final_admission_fails';
      const split = 'test ' + test + ' ... \nrunning 1 test\nok\n';
      if (log.includes(split)) {
        assert.strictEqual(e.hash(log), interleavedLinuxHelpersHash, 'exact retained child-output interleaving');
        assert.strictEqual(log.split(split).length, 2, 'one named interleaved result');
        log = log.replace(split, 'test ' + test + ' ... ok\nrunning 1 test\n');
      }
    }
    if (['proof-inventory', 'production-audit', 'dependency-policy', 'ci-test-gate', 'standalone-lockfiles'].includes(row.name))
      assert.strictEqual(log, oldLog, 'exact deterministic gate transcript: ' + row.name);
    if (['clippy-all', 'clippy-production'].includes(row.name)) {
      assert.strictEqual((log.match(/^\s+Finished `dev` profile \[unoptimized \+ debuginfo\] target\(s\) in .+$/gm) || []).length, 1,
        'one Clippy completion');
      assert(/^\s+Finished `dev` profile \[unoptimized \+ debuginfo\] target\(s\) in .+$/.test(log.trimEnd().split('\n').at(-1)),
        'terminal Clippy completion');
      assert(!/^(?:error(?:\[E\d+\])?|warning):/m.test(log), 'no Clippy diagnostics');
    }
    if (row.command[0] === 'cargo' && row.command[2] === 'test') {
      e.assertPassing(log);
      const expected = row.command.includes('--doc')
        ? docNames(e.passing(oldLog), map, ['gnu-docs', 'musl-docs'].includes(row.name), io.docs)
        : e.passing(oldLog);
      assert.deepStrictEqual(e.passing(log), expected, row.name);
      assert.deepStrictEqual(e.ignored(log), e.ignored(oldLog), row.name);
      assert.deepStrictEqual(e.totals(log), e.totals(oldLog), row.name);
      if (row.name === 'macro-fixtures') {
        assert.strictEqual((log.match(/^\s+Finished `test` profile /gm) || []).length, 1, 'macro-fixture compilation completion');
        assert.deepStrictEqual(log.match(/^     Running .+$/gm),
          [log.match(/^     Running tests\/typed_kernel_fixtures\.rs \(target\/debug\/deps\/typed_kernel_fixtures-[0-9a-f]{16}\)$/m)?.[0]],
          'exact macro-fixture executable');
      }
    }
    if (['python', 'dependency-tests'].includes(row.name)) {
      assert.deepStrictEqual([...log.matchAll(/^Ran (\d+) tests? /gm)].map(match => match[1]),
        [...oldLog.matchAll(/^Ran (\d+) tests? /gm)].map(match => match[1]));
      assert(/^OK$/m.test(log));
      assert(!/^FAILED \(/m.test(log));
    }
    if (['fmt', 'whitespace'].includes(row.name)) assert.strictEqual(log, '');
    if (row.name === 'production-metadata') {
      assert.strictEqual(row.stderr_log, e.file('r119-integrated-production-metadata.stderr.log'));
      assert.strictEqual(io.bytes('r119-integrated-production-metadata.stderr.log').toString(), '');
      const metadata = JSON.parse(log);
      assert.deepStrictEqual(metadata, JSON.parse(oldLog), 'exact production dependency graph');
      assert.strictEqual(metadata.workspace_root, e.repo);
      assert(metadata.packages.some(item => item.name === 'fe2o3-runtime'));
      assert(metadata.resolve.nodes.length > 0);
    } else assert.strictEqual(row.stderr_log, undefined);
  }
  for (const suffix of ['source-inputs', 'source-after']) assert.deepStrictEqual(io.read(sourcePrefix + '-' + suffix + '.json'), map);
  assert.deepStrictEqual(io.read(sourcePrefix + '-complete.json'), {completed: true, gates: count, source_identities: p.sourceCount});
}
function checkCampaignTranscript(log, rows, auxiliary) {
  const lines = log.trimEnd().split('\n');
  assert.strictEqual(lines.length, rows.length + 1, 'exact campaign transcript rows');
  assert.strictEqual(lines.at(-1), 'PASS: ' + p.sourceCount + (auxiliary
    ? ' auxiliary source identities unchanged' : ' non-documentation source identities unchanged'));
  assert.deepStrictEqual(lines.slice(0, -1).map(line => JSON.parse(line)), rows, 'wrapper and result-file agreement');
}
function checkFocused(result, focused, allNames) {
  const count = focused[2];
  const expected = e.focusedNames(focused, allNames);
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
  assert.strictEqual(e.hash(e.bytes('r119-integrated-environment.json')),
    '4a74a5a41e1ef8eaeb42116bbc022f8a906046a2ac7e922dd7cbffd00af28643', 'immutable frozen environment');
  const env = e.read('r119-integrated-environment.json');
  const history = validateFrozenEnvironment(env, map);
  assert.deepStrictEqual(env.binaries, JSON.parse(e.committed('raw/r118b-environment.json')).binaries);
  for (const binary of env.binaries) assert.strictEqual(e.hash(fs.readFileSync(binary.path)), binary.sha256);
  assert.strictEqual(process.env.RUST_MIN_STACK ?? null, null);
  const source = e.checkRun('r119-integrated-source-campaign',
    ['python3', '-B', e.file('r119-integrated-source-gate-v1.py'), 'r119-integrated-reviewed'],
    map, env.support.previous);
  e.ordered(env.observation, source.record.clock.start);
  const auxiliary = e.checkRun('r119-integrated-auxiliary-campaign',
    ['python3', '-B', e.file('r119-integrated-auxiliary-gates-v1.py')], map, 'r119-integrated-source-campaign.json');
  checkCampaignTranscript(source.log, e.read('r119-integrated-reviewed-source-gate.json'), false);
  checkCampaignTranscript(auxiliary.log, e.read('r119-integrated-auxiliary-results.json'), true);
  checkGateRows(map, 'r119-integrated-reviewed-source-gate.json', 'r118b-final-source-gate.json',
    15, 'r119-integrated-reviewed', 'r119-integrated-reviewed');
  checkGateRows(map, 'r119-integrated-auxiliary-results.json', 'r118b-auxiliary-results.json',
    10, 'r119-integrated', 'r119-integrated-auxiliary');
  const allNames = e.passing(e.bytes('r119-integrated-gnu-all.log').toString());
  let previous = checkSupportContracts(map, 'r119-integrated-auxiliary-campaign.json');
  for (const focused of p.focused) {
    const name = 'r119-integrated-frozen-' + focused[0];
    checkFocused(e.checkRun(name, e.focusedCommand(focused), map, previous), focused, allNames);
    previous = name + '.json';
  }
  const tests = e.checkRun('r119-integrated-qualification-contract-tests',
    ['node', e.file('r119-integrated-qualification-tests-v1.js')], map, previous);
  const cases = require('./r119-integrated-qualification-tests-v1.js').cases;
  assert(Array.isArray(cases) && cases.length > 0);
  const log = campaign.transcript(tests.log, campaign.inputs().sha256);
  contractTranscript(log, 'qualification', cases);
  const pins = [...log.matchAll(/^HELPER_PINS: (.+)$/gm)];
  assert.strictEqual(pins.length, 1);
  const helpers = p.helpers.map(name => ({name, sha256: e.hash(e.bytes(name))}));
  assert.deepStrictEqual(JSON.parse(pins[0][1]), helpers);
  const launcherPins = [...log.matchAll(/^LAUNCHER_PIN: (.+)$/gm)];
  assert.strictEqual(launcherPins.length, 1);
  const launcher = {name: p.launcherName, sha256: e.hash(e.bytes(p.launcherName))};
  assert.deepStrictEqual(JSON.parse(launcherPins[0][1]), launcher);
  return {map, env, history, allNames, helpers, launcher, cases, previous: 'r119-integrated-qualification-contract-tests.json'};
}
function validateManifest(manifest, baseline, io = e) {
  const {map, helpers, launcher, previous: prerequisite} = baseline;
  assert.strictEqual(e.hash(JSON.stringify(map)), p.sourceMapSha256, 'fixed qualified source map');
  assert.strictEqual(Object.keys(map).length, p.sourceCount);
  assert.strictEqual(manifest.source_head, p.parent);
  assert.strictEqual(manifest.source_map_sha256, e.hash(JSON.stringify(map)));
  assert.deepStrictEqual(manifest.helpers, helpers);
  assert.deepStrictEqual(manifest.launcher, launcher, 'qualified launcher identity');
  assert.strictEqual(launcher.name, 'r119-integrated-qualification-launch-v1.js');
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
  const manifestName = 'r119-integrated-qualification-manifest.json';
  const manifestBytes = e.bytes(manifestName);
  const manifest = JSON.parse(manifestBytes);
  const manifestHash = e.hash(manifestBytes);
  const historicalNames = e.historicalNames();
  const mutationNames = p.mutations.flatMap(mutation => [
    ...suffixes.map(suffix => 'r119-integrated-qualified-mut-' + mutation.name + suffix),
    ...['.json', '-source.json'].map(suffix => 'r119-integrated-qualified-restoration-' + mutation.name + suffix),
  ]);
  const focusedNames = p.focused.flatMap(([kind]) => suffixes.map(suffix => 'r119-integrated-qualified-restored-' + kind + suffix));
  const collectorNames = closed ? suffixes.map(suffix => 'r119-integrated-qualified-collector-validation' + suffix) : [];
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
    const name = 'r119-integrated-qualified-mut-' + mutation.name;
    const entry = manifest.entries[name + '.json'];
    assert.strictEqual(entry.predecessor, previous);
    const result = e.checkRun(name, entry.command, expected, previous, 101);
    e.checkMutation(result.log, mutation);
    hashes.push([mutation.name, e.hash(JSON.stringify(expected))]);
    previous = name + '.json';
    const restoredName = 'r119-integrated-qualified-restoration-' + mutation.name + '.json';
    e.checkRestoration(e.read(restoredName), files, map, manifestHash,
      {name: previous, sha256: e.hash(e.bytes(previous))}, result.record, e.read(restoredName.replace('.json', '-source.json')));
    previous = restoredName;
  }
  e.checkMutationGroups(hashes);
  for (const focused of p.focused) {
    const name = 'r119-integrated-qualified-restored-' + focused[0];
    checkFocused(e.checkRun(name, e.focusedCommand(focused), map, previous), focused, allNames);
    previous = name + '.json';
  }
  const collector = 'r119-integrated-qualified-collector-validation';
  const command = ['node', e.file('r119-integrated-qualification-collect-v1.js'), '--check'];
  assert.deepStrictEqual(manifest.entries[collector + '.json'], {kind: 'run', predecessor: previous,
    source_map_sha256: e.hash(JSON.stringify(map)), command, returncode: 0});
  const summary = {source_head: p.parent, accepted_parent: p.accepted, source_map_sha256: p.sourceMapSha256,
    scope: 'Persistent returned-data cleanup and exact retained-control custody qualification',
    source_identities: p.sourceCount, changed_source: p.sourceDelta, full: history.full,
    history, rejected_timeout: history.rejected_timeout, source_gates: 17, subsequent_source_gates: 15, auxiliary_gates: 10,
    focused: Object.fromEntries(p.focused.map(([name, , count]) => [name, count])),
    compiled_negatives: p.mutations.length, distinct_mutations: p.uniqueMutationCount,
    mutation_categories: p.expected, mutation_source_groups: p.mutationSourceGroups,
    new_behavioral_tests: p.newTests.length - 1, new_source_guards: 1,
    linux_helpers_interleaved_log_sha256: interleavedLinuxHelpersHash,
    core_contract_tests: p.coreNames.length, runner_contract_tests: 9, history_contract_tests: 46,
    freeze_contract_tests: 26, lifecycle_contract_tests: p.lifecycleNames.length, qualification_contract_tests: cases.length,
    formal_qualification: false, native_qualification: false, performance_qualification: false,
    production_semantics_changed: true, total_memory_qualification: false};
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
  const destination = path.join(e.repo, 'docs/evidence/local-r119-persistent-returned-data-cleanup-2026-09-15');
  const cohort = names.flatMap(name => name.includes('-restoration-') ? [name, name.replace('.json', '-source.json')]
    : suffixes.map(suffix => name.replace('.json', suffix)));
  const artifacts = [...new Set([...manifest.historical_artifacts.map(item => item.name), 'r119-integrated-qualification-manifest.json', ...cohort])].sort();
  for (const name of [...e.historicalNames(), ...p.helpers, 'r119-integrated-environment.json', 'r119-integrated-frozen-source.json']) {
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
  io.write(path.join(destination, 'README.md'), '# R119 Persistent Returned-Data Cleanup Evidence\n\n' +
    'Local CPU/test qualification of complete-owner cleanup, one-shot returned data, and retained incomplete controls.\n' +
    'GNU and musl each pass 2,795 tests with five ignored. Fourteen behavioral tests and one source guard are added to KFD; all other exact target rosters are retained. Four unchanged compile-fail fences are relocated with explicit byte checks.\n' +
    'All seventeen source gates, ten auxiliary checks and six frozen/restored suites pass. The 37 compiled negative executions cover 30 distinct production source maps with exact repeated-source groups and no helper calibrations.\n' +
    'Every negative reaches its named behavioral oracle and restores all 5,693 source identities. History, runner, freeze, core, lifecycle and qualification contracts pass; the collector has a closed matching transcript.\n' +
    'The original musl timeout remains rejected history; only the separately completed retry supplies musl acceptance. Original isolated cohorts and their zero-test compile-only attempt retain their own source maps, parents and commands. Endpoint equality does not establish continuous immutability.\n' +
    'This does not establish live/queue teardown, data disposal or N5 adoption, production journal integration, native execution, formal correspondence, aggregate memory bounds or HIP/HSA performance parity.\n');
  return {archive: destination, artifacts: artifacts.length, summary};
}
function main() {
  const mode = process.argv[2];
  assert(['--check', '--archive'].includes(mode));
  const result = collect(mode === '--archive');
  console.log(JSON.stringify(mode === '--archive' ? archive(result) : result.summary, null, 2));
}
module.exports = {checkBaseline, validateFrozenEnvironment, docNames, checkSupportContracts, checkGateRows, checkCampaignTranscript, checkFocused, contractTranscript, validateManifest, collect, archive, main};
if (require.main === module) main();
