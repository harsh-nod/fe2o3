// Local R125 gate validation only. Mutation, native and formal acceptance remain separate.
const fs = require('fs'), path = require('path'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp', checker = path.join(root, 'r125-development-check-v1.js');
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const checkerHash = 'e37aab2693167424232e925bec843e3a187fcf7192add1f78244cf52ecbd8981';
assert.strictEqual(sha(fs.readFileSync(checker)), checkerHash);
const C = require(checker), {E, repo, bytes, json, pin, git, map, summaries} = C;
pin(checker, checkerHash);
const gates = C.p;
assert.strictEqual(gates.doc_relocations.length, 10);
assert.strictEqual(new Set(gates.doc_relocations.map(r => r.path + ':' + r.from)).size, 10);
assert.strictEqual(gates.gates.length, 25);
assert.strictEqual(gates.full_runs.length, 2);
function accepted(spec) {
  const value = git(['show', gates.accepted_commit + ':' + spec.accepted_log]);
  assert.strictEqual(sha(value), spec.accepted_log_sha256);
  return value;
}
function record(spec) {
  return C.completed(spec.run || spec.name, spec.command, spec.predecessor, map, 0, spec.runner || C.p.runner).log;
}
function roster(log) {
  E.assertPassing(log);
  const passes = E.passing(log), ignored = E.ignored(log), totals = E.totals(log);
  assert.strictEqual(passes.length, totals.passed);
  assert.strictEqual(ignored.length, totals.ignored);
  assert.strictEqual(new Set([...passes, ...ignored]).size, passes.length + ignored.length);
  return {passes, ignored, summaries: summaries(log)};
}
function groups(log, docs) {
  const re = docs ? /^   Doc-tests (.+)$/gm : /^     Running (.+)$/gm;
  const markers = [...log.matchAll(re)];
  assert(markers.length > 0);
  assert(!/^test result:/m.test(log.slice(0, markers[0].index)));
  return markers.map((m, i) => ({name: m[1].replace(/-[0-9a-f]{16}\)$/, ')'),
    ...roster(log.slice(m.index, markers[i + 1]?.index ?? log.length))}));
}
function full(spec, log) { C.assertFull(log, accepted(spec)); }
function fence(text, line) {
  const rows = text.split('\n'), start = line - 1;
  assert(/^\s*\/\/\/ \x60{3}/.test(rows[start]));
  let end = start + 1;
  while (end < rows.length && !/^\s*\/\/\/ \x60{3}\s*$/.test(rows[end])) end++;
  assert(end < rows.length, 'closed doc fence');
  return rows.slice(start, end + 1).join('\n');
}
function relocate(log) {
  for (const r of gates.doc_relocations) {
    const current = bytes(path.join(repo, r.path)).toString();
    assert.strictEqual(sha(current), map[r.path]);
    const original = fence(git(['show', gates.source_parent + ':' + r.path]), r.from);
    assert.strictEqual(original, fence(current, r.to));
    if (r.fence_sha256) assert.strictEqual(sha(original), r.fence_sha256);
    const token = '(line ' + r.from + ')', names = E.passing(log).filter(n => n.startsWith(r.path + ' - ') && n.includes(token));
    if (!/^   Doc-tests fe2o3_kfd$/m.test(log)) { assert.strictEqual(names.length, 0); continue; }
    assert.strictEqual(names.length, 1);
    log = log.replace('test ' + names[0] + ' ... ok', 'test ' + names[0].replace(token, '(line ' + r.to + ')') + ' ... ok');
  }
  return log;
}
const fullGnu = gates.full_runs.find(s => s.kind === 'gnu');
const linuxNames = E.executables(accepted(fullGnu)).find(t => /fe2o3_kfd\)/.test(t.name)).passing
  .filter(n => n.startsWith('queue_linux::tests::'));
assert.strictEqual(linuxNames.length, 20);
function linux(log) {
  const lines = log.split('\n'), normalized = [];
  for (let i = 0; i < lines.length; i++) {
    const partial = /^test (.+) \.\.\. $/.exec(lines[i]);
    if (partial) {
      assert(linuxNames.includes(partial[1]), 'known split result');
      assert.strictEqual(lines[i + 1], 'running 1 test');
      assert.strictEqual(lines[i + 2], 'ok');
      normalized.push(lines[i] + 'ok', lines[i + 1]); i += 2;
    } else {
      assert(!/^\s*ok\s*$/.test(lines[i]), 'no orphan result');
      normalized.push(lines[i]);
    }
  }
  const result = normalized.join('\n');
  for (const line of normalized) {
    if (/^test /.test(line) && !/^test result:/.test(line)) {
      assert(linuxNames.some(name => line === 'test ' + name + ' ... ok'), 'exact allowlisted result row');
    }
  }
  assert.deepStrictEqual(E.passing(result), linuxNames);
  assert.strictEqual((result.match(/^running 20 tests$/gm) || []).length, 1);
  assert.strictEqual((result.match(/^running 1 test$/gm) || []).length, 3);
  return result;
}
const auxCounts = gates.auxiliary_counts;
const leafCounts = {'gnu-docs': [108, 0, 7], 'musl-docs': [91, 0, 5], 'musl-host-docs': [16, 0, 2],
  'gnu-host': [258, 4, 1], 'musl-host': [141, 0, 1], 'macro-fixtures': [7, 0, 1]};
function leaf(spec, log) {
  let old = accepted(spec);
  if (spec.name in leafCounts || spec.name in auxCounts) {
    const docs = spec.name.endsWith('docs');
    if (docs) old = relocate(old);
    if (spec.name === 'linux-helpers') { old = linux(old); log = linux(log); }
    const wanted = groups(old, docs), actual = groups(log, docs);
    if (spec.name in auxCounts) {
      assert.strictEqual(wanted.length, 1);
      assert.strictEqual(wanted[0].summaries.length, 1);
      wanted[0].summaries[0][5] = gates.counts.kfd - auxCounts[spec.name];
    }
    assert.deepStrictEqual(actual, wanted, spec.name + ' exact target/roster/summary');
    const [passed, ignored, harnesses] = leafCounts[spec.name] || [auxCounts[spec.name], 0, 1];
    assert.deepStrictEqual(E.totals(log), {harnesses, passed, failed: 0, ignored});
    if (!docs) assert.strictEqual((log.match(/^\s*Finished \x60test\x60 profile.*$/gm) || []).length, 1);
  } else if (spec.name.startsWith('clippy-')) {
    assert.strictEqual((log.match(/^\s*Finished \x60dev\x60 profile.*$/gm) || []).length, 1);
    assert(/^\s*Finished \x60dev\x60 profile.*$/.test(log.trimEnd().split('\n').at(-1)));
    assert(!/^(?:warning|error)(?:\[|:)/m.test(log));
  } else if (['python', 'dependency-tests'].includes(spec.name)) {
    const expected = spec.name === 'python' ? 151 : 8;
    const rows = [...log.matchAll(/^Ran (\d+) tests? in [0-9.]+s$/gm)];
    assert.strictEqual(rows.length, 1); assert.strictEqual(Number(rows[0][1]), expected);
    assert.strictEqual(log.trimEnd().split('\n').at(-1), 'OK');
    assert(!/^FAILED(?: |\()|^ERROR:|^FAIL:/m.test(log));
  } else if (spec.name === 'production-metadata') {
    assert.strictEqual(bytes(spec.stderr_log).length, 0);
    const metadata = JSON.parse(log);
    assert.deepStrictEqual(metadata, JSON.parse(old));
    assert(metadata.packages.some(p => p.name === 'fe2o3-runtime'));
    assert.strictEqual(metadata.workspace_root, repo);
  } else if (spec.name === 'standalone-lockfiles') {
    // Cargo may report contention on its shared package cache without changing the checks.
    const normalized = text => text.split('\n')
      .filter(line => line !== '    Blocking waiting for file lock on package cache').join('\n');
    assert.strictEqual(normalized(log), normalized(old), 'standalone-lockfiles exact roster and completion');
  } else assert.strictEqual(log, old, spec.name + ' exact deterministic log');
}

module.exports = {C, gates, accepted, record, full, leaf, roster, groups, fence, relocate, linux, linuxNames, fullGnu};
if (require.main === module) {
  bytes(__filename);
  const [mode = 'all', selected] = process.argv.slice(2);
  assert(['all', 'full', 'leaf'].includes(mode));
  if (mode === 'full') assert(['gnu', 'musl'].includes(selected));
  if (mode === 'leaf') assert(gates.gates.some(spec => spec.name === selected));
  const checked = [];
  const baseline = C.baseline();
  full(fullGnu, baseline.log); checked.push(fullGnu.name);
  if (mode !== 'full' || selected !== 'gnu') {
    const musl = gates.full_runs.find(spec => spec.kind === 'musl');
    full(musl, record(musl)); checked.push(musl.name);
    if (mode !== 'full') {
      for (const spec of gates.gates) {
        const previous = json(spec.predecessor);
        assert.strictEqual(previous.returncode, 0);
        assert.strictEqual(previous.child_closed, true);
        leaf(spec, record(spec)); checked.push(spec.run);
        if (mode === 'leaf' && spec.name === selected) break;
      }
    }
  }
  assert.deepStrictEqual(C.identities(), map, 'unchanged closing gate source map');
  C.stable();
  console.log(JSON.stringify({development_only: true, packet_accepted: false, checked,
    source_map_sha256: gates.source_map_sha256, artifacts: C.artifacts()}));
}
