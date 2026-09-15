// Synthetic transcript calibration only. Never substitutes runtime execution evidence.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const helper = '/home/harsh/.codex-tmp/r123-development-gate-check-v2.js';
const helperHash = '10ceeaf5d5635ea08646bc399f72a4c9bcf28d8097ff07b167d852b1eee7fd65';
assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(helper)).digest('hex'), helperHash);
const G = require(helper);
const {C, gates, extraDocRelocations, accepted, full, leaf, relocate, linux, linuxNames, fullGnu} = G;
const {E, bytes, summaries} = C;
C.pin(helper, helperHash); C.bytes(__filename);
C.baseline();
const cases = [];
function good(name, run) { run(); cases.push({name, expected: 'accept'}); }
function bad(name, run) {
  assert.throws(run, error => error.code === 'ERR_ASSERTION', name);
  cases.push({name, expected: 'reject'});
}
function replaceOne(text, before, after) {
  if (text.split(before).length !== 2) throw new Error('ambiguous calibration target');
  return text.replace(before, after);
}
const currentGnu = bytes(fullGnu.name + '.log').toString();
good('actual GNU target and complete summary roster', () => full(fullGnu, currentGnu));
const first = summaries(currentGnu)[0];
assert.deepStrictEqual(first, ['ok', 17, 0, 0, 0, 0]);
const row = 'test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;';
bad('GNU measured count', () => full(fullGnu, replaceOne(currentGnu, row, row.replace('0 measured', '1 measured'))));
bad('GNU filtered count', () => full(fullGnu, replaceOne(currentGnu, row, row.replace('0 filtered', '1 filtered'))));
const running = /^     Running (.+)$/m.exec(currentGnu)[0];
bad('GNU executable identity', () => full(fullGnu, replaceOne(currentGnu, running, '     Running calibration-not-a-target')));
const firstPass = /^test .+ \.\.\. ok$/m.exec(currentGnu)[0];
bad('GNU unknown successful row', () => full(fullGnu, replaceOne(currentGnu, firstPass, firstPass + '\ntest calibration-not-a-test ... ok')));
const linuxSpec = gates.gates.find(s => s.name === 'linux-helpers');
const originalLinux = C.pin('r119-integrated-linux-helpers.log',
  'b78cb48f28f17aaad884d876e8fce4b4cae038155be715c5b678d77113956815').toString();
const currentLinux = accepted(linuxSpec);
good('historical observed Linux split normalization', () => linux(originalLinux));
bad('Linux malformed extra row', () => linux(originalLinux + 'test calibration-not-a-test ... unexpected status\n'));
bad('Linux unknown partial row', () => linux(originalLinux + 'test calibration-not-a-test ... \nrunning 1 test\nok\n'));
bad('Linux whitespace orphan', () => linux(originalLinux + ' ok \n'));
const partial = /^test (.+) \.\.\. \nrunning 1 test\nok$/m.exec(originalLinux);
assert(partial, 'retained accepted split');
bad('Linux intervening named result', () => linux(replaceOne(originalLinux, partial[0],
  partial[0].replace('\nrunning 1 test\nok', '\nrunning 1 test\ntest ' + linuxNames[0] + ' ... ok\nok'))));
const correctedLinux = replaceOne(currentLinux, '0 measured; 1170 filtered out;', '0 measured; 1189 filtered out;');
good('Linux synthetic corrected-cohort filtered count', () => leaf(linuxSpec, correctedLinux));
bad('Linux stale filtered count', () => leaf(linuxSpec, currentLinux));
const docsSpec = gates.gates.find(s => s.name === 'gnu-docs');
const relocated = relocate(accepted(docsSpec));
good('GNU exact relocated doctest roster', () => leaf(docsSpec, relocated));
const doc = gates.doc_relocations[0];
const currentName = E.passing(relocated).find(n => n.startsWith(doc.path + ' - ') && n.includes('(line ' + doc.to + ')'));
assert(currentName);
bad('GNU stale doctest location', () => leaf(docsSpec, replaceOne(relocated,
  currentName + ' ... ok', currentName.replace('(line ' + doc.to + ')', '(line ' + doc.from + ')') + ' ... ok')));
assert.strictEqual(extraDocRelocations.length, 4);
for (const relocation of extraDocRelocations) {
  const name = E.passing(relocated).find(n => n.startsWith(relocation.path + ' - ') &&
    n.includes('(line ' + relocation.to + ')'));
  assert(name, 'declared dispatch-binding doctest');
  bad('GNU stale dispatch-binding doctest ' + relocation.to, () => leaf(docsSpec, replaceOne(relocated,
    name + ' ... ok', name.replace('(line ' + relocation.to + ')', '(line ' + relocation.from + ')') + ' ... ok')));
  bad('GNU missing dispatch-binding doctest ' + relocation.to, () => leaf(docsSpec, replaceOne(relocated,
    'test ' + name + ' ... ok\n', '')));
}
const python = gates.gates.find(s => s.name === 'python'), pythonLog = accepted(python);
good('Python declared test count', () => leaf(python, pythonLog));
bad('Python wrong test count', () => leaf(python, replaceOne(pythonLog, 'Ran 151 tests', 'Ran 150 tests')));
const clippy = gates.gates.find(s => s.name === 'clippy-all'), clippyLog = accepted(clippy);
good('strict Clippy completion', () => leaf(clippy, clippyLog));
bad('Clippy warning', () => leaf(clippy, 'warning: calibration-only warning\n' + clippyLog));
const fmt = gates.gates.find(s => s.name === 'fmt');
good('empty formatting transcript', () => leaf(fmt, ''));
bad('nonempty formatting transcript', () => leaf(fmt, '\n'));
const locks = gates.gates.find(s => s.name === 'standalone-lockfiles');
const acceptedLockLog = accepted(locks), waitLine = '    Blocking waiting for file lock on package cache\n';
assert.strictEqual(acceptedLockLog.split(waitLine).length, 1, 'accepted R122 has no cache-wait row');
const lockLog = acceptedLockLog;
const observedLockLog = waitLine + acceptedLockLog; // Explicit synthetic exact-wait fixture.
const lockRow = /^checking standalone lockfile: .+\n/m.exec(lockLog)[0];
good('standalone exact lockfile roster', () => leaf(locks, lockLog));
good('standalone synthetic exact cache wait', () => leaf(locks, observedLockLog));
good('standalone repeated exact cache waits', () => leaf(locks, waitLine + observedLockLog + waitLine));
bad('standalone unknown cache wait suffix', () => leaf(locks, observedLockLog.replace(waitLine, waitLine.trimEnd() + ' unexpected\n')));
bad('standalone missing lockfile with cache wait', () => leaf(locks, replaceOne(observedLockLog, lockRow, '')));
bad('standalone duplicate lockfile with cache wait', () => leaf(locks, replaceOne(observedLockLog, lockRow, lockRow + lockRow)));
bad('standalone wrong completion count with cache wait', () => leaf(locks, replaceOne(observedLockLog, 'OK (32 checked)', 'OK (31 checked)')));
bad('standalone completion without roster', () => leaf(locks, waitLine + 'standalone lockfiles: OK (32 checked)\n'));
bad('standalone error with cache wait', () => leaf(locks, observedLockLog + 'error: calibration-only failure\n'));
bad('standalone warning with cache wait', () => leaf(locks, observedLockLog + 'warning: calibration-only warning\n'));
assert.strictEqual(cases.length, 38);
assert.deepStrictEqual(C.identities(), C.map);
C.stable();
console.log(JSON.stringify({development_only: true, helper_calibration_only: true, runtime_mutation_count: 0,
  helper_sha256: helperHash, passed: cases.length, cases}));
