const assert = require('assert');
const p = require('./r118-history-preflight-plan-v1.js');
const e = require('./r118-history-preflight-evidence-v1.js');
const suffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const handoffs = [
  'c1candidate-negative-handoff-r118.md', 'c2candidate-negative-handoff.md',
  'c3-lifecycle-handoff-r116.md', 'c3candidate-negative-handoff.md',
  'r118-isolated-history-handoff.md', 'r118-mutation-framework-handoff-v1.md',
];
const extraArtifacts = [
  ...handoffs, 'c1-run.js', 'c2candidate-run-v1.js', 'c3candidate-run-v1.js', 'r118-run-v1.js',
  'r118-c1-index-corroboration-v1.json', 'r118-mutation-core-inputs-v1.json',
  'r118-mutation-core-tests-v1.js', 'r118-mutation-core-v1.js', 'r118-mutation-plan-v1.js',
  'r118-mutations-c1-v1.js', 'r118-mutations-c2-v1.js', 'r118-mutations-c3-v1.js',
];
const prefixes = {
  c1: 'context::tests::submission_identity_tests::',
  c2: 'authorized_execution::tests::generated_identity::',
  c3: 'async_engine::tests::owned_tests::preparation_tests::completion_tests::',
};
function changedPaths(before, after) {
  assert.deepStrictEqual(Object.keys(after), Object.keys(before), 'unchanged historical inventory');
  return Object.keys(before).filter(name => JSON.stringify(before[name]) !== JSON.stringify(after[name])).sort();
}
function validateMap(map, specification) {
  assert(map && typeof map === 'object' && !Array.isArray(map));
  assert(['sha256-v1', 'sparse-v1'].includes(specification.schema), 'known source map schema');
  assert.strictEqual(Object.keys(map).length, specification.count);
  assert.deepStrictEqual(Object.keys(map), Object.keys(map).sort(), 'canonical map ordering');
  let materialized = 0;
  let sparse = 0;
  for (const [name, value] of Object.entries(map)) {
    assert(name && !name.startsWith('docs/') && !name.startsWith('/'), 'source inventory path');
    if (specification.schema === 'sha256-v1') {
      assert(typeof value === 'string' && /^[a-f0-9]{64}$/.test(value), 'flat source hash');
      continue;
    }
    assert.strictEqual(specification.schema, 'sparse-v1');
    assert(value && typeof value === 'object' && !Array.isArray(value), 'typed sparse-map entry');
    const keys = Object.keys(value);
    assert.strictEqual(keys.length, 1, 'one source identity representation');
    if (keys[0] === 'sha256') {
      assert(typeof value.sha256 === 'string' && /^[a-f0-9]{64}$/.test(value.sha256), 'typed source hash');
      materialized++;
    } else {
      assert.strictEqual(keys[0], 'unmaterialized_index_entry');
      assert(typeof value.unmaterialized_index_entry === 'string' &&
        /^100(?:644|755) [a-f0-9]{40} 0$/.test(value.unmaterialized_index_entry), 'stage-zero source descriptor');
      sparse++;
    }
  }
  if (specification.schema === 'sparse-v1') assert.deepStrictEqual({materialized, sparse}, {materialized: 5299, sparse: 381});
}
function tapNames(log, count) {
  const rows = [...log.matchAll(/^ok (\d+) - (.+)$/gm)];
  assert.deepStrictEqual(rows.map(row => Number(row[1])), Array.from({length: count}, (_, i) => i + 1));
  assert(!/^not ok /m.test(log));
  for (const marker of ['1..' + count, '# tests ' + count, '# pass ' + count, '# fail 0', '# cancelled 0', '# skipped 0', '# todo 0']) {
    assert(log.split('\n').includes(marker), 'exact TAP summary ' + marker);
  }
  const names = rows.map(row => row[2]).sort();
  assert.strictEqual(new Set(names).size, count);
  return names;
}
function checkHistory(finalMap, io = {bytes: e.bytes, read: e.read, git: e.git}, profile = p.history) {
  assert.deepStrictEqual(Object.keys(profile.chains), ['c1', 'c2', 'c3', 'integrated']);
  assert.deepStrictEqual(Object.values(profile.chains).map(names => names.length), [8, 5, 6, 11]);
  assert.deepStrictEqual(Object.keys(profile.runs), Object.values(profile.chains).flat());
  assert.strictEqual(Object.keys(profile.maps).length, 12);
  assert.strictEqual(profile.artifacts.length, 138);
  assert.strictEqual(new Set(profile.artifacts.map(item => item.name)).size, 138);
  assert.deepStrictEqual(profile.handoffs.slice().sort(), handoffs.slice().sort(), 'exact historical handoffs');
  const expectedArtifacts = [...Object.keys(profile.runs).flatMap(name => suffixes.map(suffix => name + suffix)), ...extraArtifacts];
  assert.deepStrictEqual(profile.artifacts.map(item => item.name).sort(), expectedArtifacts.sort(), 'exact historical artifact membership');
  assert.strictEqual(e.hash(JSON.stringify(finalMap)), p.sourceMapSha256);
  const artifactNames = new Set(profile.artifacts.map(item => item.name));
  for (const item of profile.artifacts) assert.strictEqual(e.hash(io.bytes(item.name)), item.sha256, 'pinned historical artifact: ' + item.name);
  for (const name of Object.keys(profile.runs)) {
    for (const suffix of suffixes) assert(artifactNames.has(name + suffix), 'complete historical run');
  }
  const maps = new Map();
  for (const [digest, specification] of Object.entries(profile.maps)) {
    const map = io.read(specification.file);
    validateMap(map, specification);
    assert.strictEqual(e.hash(JSON.stringify(map)), digest, 'historical map identity');
    maps.set(digest, map);
  }
  const baselineLog = io.git(['show', p.parent + ':' + p.previous + 'raw/r117-reviewed-gnu-all.log']);
  const runtime = e.executables(baselineLog).filter(target => target.kind === 'libtest' && /fe2o3_runtime\)/.test(target.name));
  assert.strictEqual(runtime.length, 1);
  assert.strictEqual(runtime[0].passing.length, 715);
  const allAdditions = p.newTests;
  const transitionKeys = new Set();
  const validateTransition = (before, after) => {
    if (before === after) return;
    const edge = profile.transitions.filter(edge => edge.before === before && edge.after === after);
    assert.strictEqual(edge.length, 1, 'declared source transition');
    assert.deepStrictEqual(changedPaths(maps.get(before), maps.get(after)), edge[0].paths);
    transitionKeys.add(before + ':' + after);
  };
  const full = {};
  let sourceChanging = 0;
  for (const [contextName, names] of Object.entries(profile.chains)) {
    const context = profile.contexts[contextName];
    assert(context && artifactNames.has(context.runner));
    assert.strictEqual(e.hash(io.bytes(context.runner)), context.runnerSha256);
    let previous = null;
    let previousMap = null;
    for (const name of names) {
      const specification = profile.runs[name];
      assert.strictEqual(specification.context, contextName);
      const before = maps.get(specification.before);
      const after = maps.get(specification.after);
      assert(before && after);
      assert.strictEqual(profile.maps[specification.before].schema, context.schema);
      assert.strictEqual(profile.maps[specification.after].schema, context.schema);
      if (previousMap) validateTransition(previousMap, specification.before);
      validateTransition(specification.before, specification.after);
      if (specification.before !== specification.after) sourceChanging++;
      const record = io.read(name + '.json');
      e.checkRecord(record, name, specification.command, before, previous, specification.exitCode, io, {...context, after});
      assert.strictEqual(record.clock.start.boot_id, context.boot);
      const log = io.bytes(name + '.log').toString();
      const additions = contextName === 'integrated' ? allAdditions : allAdditions.filter(test => test.startsWith(prefixes[contextName]));
      if (specification.kind === 'runtime' || specification.kind === 'focused') {
        const expected = specification.kind === 'runtime' ? [...runtime[0].passing, ...additions].sort() : additions;
        assert.deepStrictEqual(e.passing(log), expected, 'exact historical test roster: ' + name);
        assert.strictEqual(new Set(e.passing(log)).size, expected.length);
        assert.deepStrictEqual(e.totals(log), {harnesses: 1, passed: expected.length, failed: 0, ignored: 0});
        assert.strictEqual([...log.matchAll(/^     Running /gm)].length, 1);
        assert.deepStrictEqual(e.ignored(log), []);
      } else if (specification.kind.startsWith('full-')) {
        const target = specification.kind.slice(5);
        const old = io.git(['show', p.parent + ':' + p.previous + 'raw/r117-reviewed-' + target + '-all.log']);
        full[target] = e.checkFull(log, old);
      } else if (specification.kind === 'compile-failure') {
        assert.strictEqual(name, 'c1-initial-tests');
        assert.strictEqual(specification.exitCode, 101);
        assert.strictEqual([...log.matchAll(/^error\[E0308\]:/gm)].length, 1);
        assert(log.includes('submission_identity_tests.rs:377:17'));
        assert(!/^     Running |^test result:|^test .+ \.\.\. /m.test(log));
      } else if (specification.kind === 'core-contracts') {
        assert.strictEqual(e.hash(JSON.stringify(tapNames(log, 48))), 'f2840a9917492fd67dfff0947a1f544fe74a571b694e987b6fadc92049669a85');
      } else if (specification.kind === 'clippy') {
        assert(log.includes('Finished `dev` profile'));
        assert(!/^error(?:\[|:)/m.test(log));
      } else {
        assert.strictEqual(specification.kind, 'format-or-whitespace');
        assert.strictEqual(log, '');
      }
      assert.strictEqual(specification.exitCode, specification.kind === 'compile-failure' ? 101 : 0);
      previous = name + '.json';
      previousMap = specification.after;
    }
  }
  assert.strictEqual(sourceChanging, 4);
  assert.strictEqual(transitionKeys.size, 8);
  assert.strictEqual(profile.transitions.length, 8);
  assert.deepStrictEqual(profile.prerequisiteNames, ['r118-reviewed-format', 'r118-reviewed-runtime', 'r118-reviewed-clippy',
    'r118-reviewed-gnu-all', 'r118-reviewed-musl-all', 'r118-mutation-core-contracts-v1']);
  for (const name of profile.prerequisiteNames) {
    const specification = profile.runs[name];
    assert.strictEqual(specification.context, 'integrated');
    assert.strictEqual(specification.before, p.sourceMapSha256);
    assert.strictEqual(specification.after, p.sourceMapSha256);
    assert.strictEqual(specification.exitCode, 0);
  }
  const inputs = io.read('r118-mutation-core-inputs-v1.json');
  assert.strictEqual(e.hash(io.bytes('r118-mutation-core-inputs-v1.json')), '9afd082517e9ffd1669d78d61ab660fdf320554fb1b3b39377d53f0ab60af600');
  assert.strictEqual(inputs.parent, p.parent);
  assert.strictEqual(inputs.source_map_sha256, p.sourceMapSha256);
  for (const helper of inputs.helpers) assert.strictEqual(e.hash(io.bytes(helper.name)), helper.sha256);
  const c1 = maps.get(profile.runs['c1-corrected-tests'].after);
  const sparse = Object.fromEntries(Object.entries(c1).filter(([, value]) => Object.hasOwn(value, 'unmaterialized_index_entry')));
  assert.strictEqual(e.hash(JSON.stringify(sparse)), '0dc116b6afec39c4f3fbf9b9eb8455930bb97171ab3ecef8e52c7f03a82b761c');
  const descriptors = Object.fromEntries(Object.entries(sparse).map(([name, value]) => [name, value.unmaterialized_index_entry]));
  for (const [digest, specification] of Object.entries(profile.maps)) {
    if (specification.schema !== 'sparse-v1') continue;
    const actual = Object.fromEntries(Object.entries(maps.get(digest)).filter(([, value]) => Object.hasOwn(value, 'unmaterialized_index_entry')));
    assert.deepStrictEqual(actual, sparse, 'unchanged sparse descriptors');
  }
  const corroboration = io.read('r118-c1-index-corroboration-v1.json');
  assert.strictEqual(corroboration.scope, 'Current index and skip-worktree corroboration; not a historical raw snapshot');
  assert.strictEqual(corroboration.head, profile.contexts.c1.head);
  assert.strictEqual(corroboration.cwd, profile.contexts.c1.cwd);
  assert.deepStrictEqual(corroboration.descriptors, descriptors);
  assert.deepStrictEqual(corroboration.skipped, Object.keys(descriptors));
  e.ordered(corroboration.observation, corroboration.observation);
  assert.strictEqual(corroboration.observed_at, new Date(corroboration.observation.utc_ms).toISOString());
  const tree = new Map(io.git(['ls-tree', '-r', '-z', profile.contexts.c1.head]).split('\0').filter(Boolean).map(row => {
    const split = row.indexOf('\t');
    const [mode, type, object] = row.slice(0, split).split(' ');
    return [row.slice(split + 1), {type, descriptor: mode + ' ' + object + ' 0'}];
  }));
  for (const [name, descriptor] of Object.entries(descriptors)) {
    assert.deepStrictEqual(tree.get(name), {type: 'blob', descriptor}, 'original-HEAD sparse descriptor');
  }
  const oldMap = JSON.parse(io.git(['show', p.parent + ':' + p.previous + 'raw/r117-frozen-source.json']));
  const initial = maps.get(profile.runs['r118-initial-format'].before);
  const promoted = {...oldMap};
  for (const [contextName, paths] of Object.entries(profile.cohortSources)) {
    const lastName = profile.chains[contextName].at(-1);
    const source = maps.get(profile.runs[lastName].after);
    for (const name of paths) {
      const value = source[name];
      promoted[name] = contextName === 'c1' ? value.sha256 : value;
      assert(/^[a-f0-9]{64}$/.test(promoted[name]), 'materialized promoted source');
      if (!p.addedSource.includes(name)) assert.strictEqual(
        io.git(['show', profile.contexts[contextName].head + ':' + name]),
        io.git(['show', p.parent + ':' + name]), 'unchanged promotion base');
    }
  }
  assert.deepStrictEqual(Object.fromEntries(Object.entries(promoted).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)), initial);
  assert.deepStrictEqual(changedPaths(initial, finalMap), profile.cohortSources.c1.slice().sort());
  const delta = [...new Set([...Object.keys(oldMap), ...Object.keys(finalMap)])].filter(name => oldMap[name] !== finalMap[name]).sort();
  assert.deepStrictEqual(delta, p.sourceDelta);
  assert.deepStrictEqual(Object.keys(finalMap).filter(name => !Object.hasOwn(oldMap, name)).sort(), p.addedSource);
  assert.deepStrictEqual(Object.keys(oldMap).filter(name => !Object.hasOwn(finalMap, name)), []);
  return {runs: 30, isolated_runs: 19, integrated_runs: 11, raw_run_artifacts: 120,
    preserved_artifacts: 138, distinct_source_maps: 12, source_changing_runs: 4,
    source_transitions: 8, sparse_descriptors: 381, helper_contracts: 48, full};
}
module.exports = {changedPaths, validateMap, tapNames, checkHistory};
