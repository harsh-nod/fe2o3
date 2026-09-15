// Exact R124 CPU/source evidence. Hardware and formal correspondence remain separate.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp', repo = path.join(root, 'fe2o3-r61-execution');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const captured = new Map();
function bytes(name) {
  const file = path.isAbsolute(name) ? name : path.join(root, name);
  assert(fs.lstatSync(file).isFile(), 'regular input: ' + file);
  if (!captured.has(file)) captured.set(file, fs.readFileSync(file));
  return captured.get(file);
}
const json = name => JSON.parse(bytes(name));
function pin(name, digest) {
  const value = bytes(name);
  assert.strictEqual(hash(value), digest, name);
  return value;
}
const p = JSON.parse(pin('r124-development-full-plan-v2.json', 'e0d5f50276944d64e626c8f7dac4a06477fd1a290c6a396ef2a011dbb568a63d'));
assert.strictEqual(p.accepted, false);
for (const [name, digest] of Object.entries(p.parser_inputs)) pin(name, digest);
for (const [name, digest] of Object.entries(p.runners)) pin(name, digest);
pin('r124-development-evidence-v2.js', 'ba17604e65e687e6f05079c91f8f2e6d4c60eadc3376cd2f5a676a71b229243a');
const E = require('./r124-development-evidence-v2.js');
const resumption = JSON.parse(pin(p.resumption.plan, p.resumption.sha256));
assert.strictEqual(resumption.accepted, false);
assert.strictEqual(resumption.prior_boot, p.resumption.prior_boot);
assert.strictEqual(resumption.current_boot, p.resumption.current_boot);
assert.strictEqual(resumption.source_parent, p.source_parent);
assert.strictEqual(resumption.source_map_sha256, p.source_map_sha256);
assert.strictEqual(resumption.source_count, p.source_count);
const boot = () => fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim();
const map = json(p.source_map);
assert.strictEqual(hash(JSON.stringify(map)), p.source_map_sha256);
assert.strictEqual(Object.keys(map).length, p.source_count);
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), p.source_parent);
assert.strictEqual(p.accepted_commit, p.source_parent);
for (const [name, digest] of Object.entries(p.accepted_inputs)) {
  assert.strictEqual(hash(git(['show', p.accepted_commit + ':' + name])), digest);
}
function identities() {
  assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), p.source_parent);
  return Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(name => name && !name.startsWith('docs/')).sort()
    .map(name => [name, hash(fs.readFileSync(path.join(repo, name)))]));
}
function completed(name, command, previous, expected = map, code = 0, runner = p.runner) {
  assert(/^r124-development-[a-z0-9-]+$/.test(name));
  assert(Object.hasOwn(p.runners, runner), 'declared runner');
  const record = json(name + '.json');
  E.checkRecord(record, name, command, expected, previous, code, {read: json, bytes}, {
    head: p.source_parent, cwd: repo, contract: 'r124-development-raw-utc-boot-monotonic-v1', runner, after: expected,
  });
  assert(record.elapsed_seconds >= 0 && record.elapsed_seconds < record.deadline_ms / 1000);
  observeClosure(name, record);
  return {record, log: bytes(name + '.log').toString()};
}
const summaries = log => [...log.matchAll(/^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/gm)]
  .map(match => [match[1], ...match.slice(2).map(Number)]);
function accepted(spec) {
  const log = git(['show', p.accepted_commit + ':' + spec.accepted_log]);
  assert.strictEqual(hash(log), spec.accepted_log_sha256);
  return log;
}
function assertFull(log, oldLog) {
  E.assertPassing(oldLog); E.assertPassing(log);
  assert.deepStrictEqual(E.totals(oldLog), {harnesses: p.counts.harnesses, passed: p.counts.baseline_full, failed: 0, ignored: p.counts.ignored});
  assert.deepStrictEqual(E.totals(log), {harnesses: p.counts.harnesses, passed: p.counts.full, failed: 0, ignored: p.counts.ignored});
  const expected = E.executables(oldLog), actual = E.executables(log);
  assert.strictEqual(expected.length, 49); assert.strictEqual(actual.length, 49);
  const expectedSummaries = summaries(oldLog);
  assert.strictEqual(expectedSummaries.length, p.counts.harnesses);
  const libraries = expected.filter(target => target.kind === 'libtest');
  for (const [kind, count] of [['kfd', 17], ['runtime', 0]]) {
    const matches = expected.filter(target => target.kind === 'libtest' && target.name.endsWith('/fe2o3_' + kind + ')'));
    assert.strictEqual(matches.length, 1, kind + ' target identity');
    const target = matches[0], additions = p.new_tests[kind];
    assert.strictEqual(additions.length, count);
    assert.strictEqual(new Set(additions).size, count);
    assert.strictEqual(target.totals.passed, p.counts['baseline_' + kind]);
    assert(additions.every(name => !target.passing.includes(name)));
    target.passing = [...target.passing, ...additions].sort();
    target.totals.passed += count;
    expectedSummaries[libraries.indexOf(target)][1] += count;
    assert.strictEqual(target.totals.passed, p.counts[kind]);
  }
  assert.deepStrictEqual(actual, expected, 'exact full executable and test roster');
  assert.deepStrictEqual(summaries(log), expectedSummaries, 'all summary fields, including measured and filtered');
  for (const target of actual.filter(target => target.kind === 'libtest')) {
    assert.strictEqual(target.passing.length, target.totals.passed);
    assert.strictEqual(target.ignored.length, target.totals.ignored);
    assert.strictEqual(new Set([...target.passing, ...target.ignored]).size, target.passing.length + target.ignored.length);
  }
  assert.deepStrictEqual(E.passing(log), [...E.passing(oldLog), ...p.new_tests.kfd].sort());
  assert.deepStrictEqual(E.ignored(log), E.ignored(oldLog));
}
function anchorRoster(spec, log) {
  const baseline = E.executables(accepted(p.full_runs.find(run => run.kind === 'gnu')))
    .find(target => target.kind === 'libtest' && target.name.endsWith('/fe2o3_kfd)'));
  assert(baseline, 'accepted KFD target');
  const separator = spec.command.indexOf('--');
  assert(separator >= 0, 'declared focused filter separator');
  const filters = spec.command.slice(separator + 1);
  assert(filters.length > 0 && filters.every(filter => !filter.startsWith('-')), 'substring filters only');
  const expected = [...baseline.passing, ...p.new_tests.kfd]
    .filter(name => filters.some(filter => name.includes(filter))).sort();
  assert.strictEqual(expected.length, spec.passed);
  const markers = [...log.matchAll(/^     Running (.+)$/gm)];
  assert.deepStrictEqual(markers.map(marker => marker[1].replace(/-[0-9a-f]{16}\)$/, ')')),
    ['unittests src/lib.rs (target/debug/deps/fe2o3_kfd)'], 'exact focused executable');
  assert(!/^test result:/m.test(log.slice(0, markers[0].index)), 'no unaccounted focused summary');
  assert(!/^   Doc-tests /m.test(log), 'no extra focused doctest');
  E.assertPassing(log);
  assert.deepStrictEqual(summaries(log), [['ok', spec.passed, 0, 0, 0, p.counts.kfd - spec.passed]]);
  assert.deepStrictEqual(E.passing(log), expected, 'exact filtered KFD test roster');
  assert.deepStrictEqual(E.ignored(log), [], 'no ignored focused tests');
  return expected;
}
function anchors() {
  const names = [];
  for (const spec of p.anchors) {
    pin(spec.name + '.json', spec.record_sha256);
    pin(spec.name + '.log', spec.log_sha256);
    const {log} = completed(spec.name, spec.command, spec.predecessor, map, 0, spec.runner);
    if (spec.passed === null) {
      assert.strictEqual((log.match(/^\s*Finished \x60dev\x60 profile.*$/gm) || []).length, 1);
      assert(/^\s*Finished \x60dev\x60 profile.*$/.test(log.trimEnd().split('\n').at(-1)));
      assert(!/^(?:warning|error)(?:\[|:)/m.test(log));
    } else {
      names.push(...anchorRoster(spec, log));
    }
  }
  assert.strictEqual(names.length, p.anchor_test_count);
  assert.strictEqual(new Set(names).size, p.anchor_test_count);
}
function baseline(kind = 'gnu') {
  assert(['gnu', 'musl'].includes(kind));
  anchors();
  const spec = p.full_runs.find(spec => spec.kind === kind);
  const result = completed(spec.name, spec.command, spec.predecessor, map, 0, spec.runner || p.runner);
  assert.strictEqual(result.record.deadline_ms, spec.deadline_ms);
  assertFull(result.log, accepted(spec));
  return result;
}

function observeClosure(name, record, scan = E.liveGroupMembers) {
  assert.deepStrictEqual(record, json(name + '.json'), 'closure object matches recorded bytes');
  if (Object.hasOwn(resumption.historical_records, name + '.json'))
    assert.strictEqual(record.clock.start.boot_id, resumption.prior_boot, 'historical run boot');
  return E.observeClosure(record, {current_boot: boot(), admitted_boot: resumption.current_boot,
    historical_boot: resumption.prior_boot, historical_sha256: resumption.historical_records[name + '.json'],
    record_sha256: hash(bytes(name + '.json'))}, scan);
}
function resumptionInputs(io = {bytes, exists: name => fs.existsSync(path.join(root, name))},
    currentMap = identities(), currentBoot = boot()) {
  assert.strictEqual(currentBoot, resumption.current_boot, 'admitted new boot');
  assert.notStrictEqual(resumption.prior_boot, currentBoot);
  assert.deepStrictEqual(currentMap, map, 'unchanged admitted source');
  for (const [name, digest] of Object.entries(resumption.inputs))
    assert.strictEqual(hash(io.bytes(name)), digest, 'pinned prerequisite: ' + name);
  for (const name of resumption.missing_completion_artifacts)
    assert.strictEqual(io.exists(name), false, 'interrupted run cannot acquire completion evidence');
}
function priorReportArtifacts(report) {
  assert(Array.isArray(report.artifacts));
  assert.strictEqual(new Set(report.artifacts.map(item => item.file)).size, report.artifacts.length);
  for (const item of report.artifacts) {
    assert.strictEqual(path.dirname(item.file), root, 'prior checker input path');
    pin(item.file, item.sha256);
  }
}
function readmission() {
  resumptionInputs();
  baseline();
  let calibrationCount = 0, anchorCheck;
  for (const spec of resumption.runs) {
    const result = completed(spec.name, spec.command, spec.predecessor, map, 0, spec.runner);
    if (spec.name === 'r124-development-format-02') assert.strictEqual(result.log, '');
    else if (p.anchors.some(anchor => anchor.name === spec.name) || spec.kind === 'gnu') continue;
    else {
      const report = JSON.parse(result.log);
      if (spec.cases) {
        assert.deepStrictEqual(report, {development_only: true, helper_calibration_only: true,
          runtime_mutation_count: 0, helper_sha256: spec.helper_sha256, passed: spec.cases.length, cases: spec.cases});
        assert.strictEqual(new Set(spec.cases.map(item => item.name)).size, spec.cases.length);
        calibrationCount += spec.cases.length;
      } else if (spec.name === 'r124-development-anchor-check-01') {
        priorReportArtifacts(report);
        assert.deepStrictEqual({...report, artifacts: undefined}, {development_only: true, packet_accepted: false,
          source_map_sha256: p.source_map_sha256, exact_regression_tests: 158, strict_clippy: true, format: true,
          full_runs_executed: 0, runtime_mutations_executed: 0,
          helper_sha256: resumption.inputs['r124-development-check-v1.js'], artifacts: undefined});
        anchorCheck = result.record;
      } else if (spec.name === 'r124-development-gnu-check-v1') {
        priorReportArtifacts(report);
        assert.deepStrictEqual({...report, artifacts: undefined}, {development_only: true, packet_accepted: false,
          mode: 'baseline', kind: 'gnu', full_passed: 2882, source_map_sha256: p.source_map_sha256, artifacts: undefined});
      } else {
        assert.strictEqual(spec.name, resumption.source_snapshot);
        assert.deepStrictEqual(report, {source_snapshot_only: true, output: path.join(root, resumption.source_bundle),
          files: 12, new_files: 3, sha256: resumption.inputs[resumption.source_bundle], source_map_sha256: p.source_map_sha256});
      }
    }
  }
  assert.strictEqual(calibrationCount, 175);
  assert(anchorCheck);
  E.ordered(anchorCheck.clock.source_verified, json('r124-development-gnu-all-v1.json').clock.start);
  const bundle = json(resumption.source_bundle);
  assert.strictEqual(bundle.source_parent, p.source_parent);
  assert.strictEqual(bundle.source_map_sha256, p.source_map_sha256);
  assert.strictEqual(bundle.source_identities, p.source_count);
  assert.strictEqual(bundle.files.length, 12);
  assert.strictEqual(new Set(bundle.files.map(file => file.path)).size, 12);
  for (const file of bundle.files) {
    assert.strictEqual(file.sha256, map[file.path]);
    assert.strictEqual(hash(file.text), file.sha256);
    assert.strictEqual(bytes(path.join(repo, file.path)).toString(), file.text);
  }
  const interruption = json(resumption.interruption_observation);
  assert.strictEqual(interruption.classification, 'interrupted_no_completion_record');
  assert.strictEqual(interruption.qualification, false);
  assert.strictEqual(interruption.run, resumption.interrupted);
  for (const field of ['observed_returncode', 'observed_signal', 'observed_finish', 'observed_child_close', 'observed_elapsed_seconds'])
    assert.strictEqual(interruption[field], null);
  return {development_only: true, packet_accepted: false, contract: resumption.contract,
    source_parent: p.source_parent, source_map_sha256: p.source_map_sha256, source_identities: p.source_count,
    prior_boot: resumption.prior_boot, current_boot: resumption.current_boot, completed_gnu_tests: 2882,
    retained_calibration_cases: 175, interrupted_run: resumption.interrupted, interrupted_qualified: false,
    prerequisite_hashes: resumption.inputs,
    environment: {node: process.version, platform: process.platform, arch: process.arch, kernel: require('os').release()},
    exclusions: resumption.exclusions};
}

function stable() {
  for (const [file, value] of captured) assert.deepStrictEqual(fs.readFileSync(file), value, 'stable consumed bytes: ' + file);
}
function artifacts() {
  return [...captured].map(([file, value]) => ({file, sha256: hash(value)})).sort((a, b) => a.file.localeCompare(b.file));
}
module.exports = {resumption, resumptionInputs, observeClosure, readmission, p, E, root, repo, hash, bytes, json, pin, git, map, identities, completed, summaries, accepted, assertFull, anchorRoster, anchors, baseline, stable, artifacts};
if (require.main === module) {
  bytes(__filename);
  const [mode = 'baseline', kind = 'gnu'] = process.argv.slice(2);
  assert(['baseline', 'restored', 'readmit'].includes(mode));
  if (mode === 'readmit') {
    const report = readmission();
    assert.deepStrictEqual(identities(), map); stable();
    console.log(JSON.stringify(report));
    process.exit(0);
  }
  const result = mode === 'baseline' ? baseline(kind) : null;
  assert.deepStrictEqual(identities(), map);
  stable();
  console.log(JSON.stringify({development_only: true, packet_accepted: false, mode, kind,
    full_passed: result ? p.counts.full : null, source_map_sha256: p.source_map_sha256, artifacts: artifacts()}));
}
