// Synthetic transcript calibration only. Never writes or substitutes run evidence.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const helper = '/home/harsh/.codex-tmp/r121-development-gate-check-v4.js';
const source = fs.readFileSync(helper, 'utf8');
assert.strictEqual(crypto.createHash('sha256').update(source).digest('hex'), '1f1cb46d0a77385a4af82bca13af9bbfb312a4b698af1d933791ec39f2792217');
const marker = "const [mode = 'all', selected] = process.argv.slice(2);";
assert.strictEqual(source.split(marker).length, 2);
const exercise = String.raw`const cases = [];
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
const originalLinux = accepted(linuxSpec);
good('accepted Linux split normalization', () => linux(originalLinux));
bad('Linux malformed extra row', () => linux(originalLinux + 'test calibration-not-a-test ... unexpected status\n'));
bad('Linux unknown partial row', () => linux(originalLinux + 'test calibration-not-a-test ... \nrunning 1 test\nok\n'));
bad('Linux whitespace orphan', () => linux(originalLinux + ' ok \n'));
const partial = /^test (.+) \.\.\. \nrunning 1 test\nok$/m.exec(originalLinux);
assert(partial, 'retained accepted split');
bad('Linux intervening named result', () => linux(replaceOne(originalLinux, partial[0],
  partial[0].replace('\nrunning 1 test\nok', '\nrunning 1 test\ntest ' + linuxNames[0] + ' ... ok\nok'))));
const correctedLinux = replaceOne(originalLinux, '0 measured; 1129 filtered out;', '0 measured; 1152 filtered out;');
good('Linux synthetic corrected-cohort filtered count', () => leaf(linuxSpec, correctedLinux));
bad('Linux stale filtered count', () => leaf(linuxSpec, originalLinux));
const docsSpec = gates.gates.find(s => s.name === 'gnu-docs');
const relocated = relocate(accepted(docsSpec));
good('GNU exact relocated doctest roster', () => leaf(docsSpec, relocated));
const doc = gates.doc_relocations[0];
const currentName = E.passing(relocated).find(n => n.startsWith(doc.path + ' - ') && n.includes('(line ' + doc.to + ')'));
assert(currentName);
bad('GNU stale doctest location', () => leaf(docsSpec, replaceOne(relocated,
  currentName + ' ... ok', currentName.replace('(line ' + doc.to + ')', '(line ' + doc.from + ')') + ' ... ok')));
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
const lockLog = accepted(locks), waitLine = '    Blocking waiting for file lock on package cache\n';
const lockRow = /^checking standalone lockfile: .+\n/m.exec(lockLog)[0];
const observedLockLog = bytes(locks.run + '.log').toString();
assert.strictEqual(observedLockLog.split(waitLine).length, 2);
assert.strictEqual(observedLockLog.replace(waitLine, ''), lockLog);
good('standalone exact lockfile roster', () => leaf(locks, lockLog));
good('standalone observed cache wait', () => leaf(locks, observedLockLog));
good('standalone repeated exact cache waits', () => leaf(locks, waitLine + observedLockLog + waitLine));
bad('standalone unknown cache wait suffix', () => leaf(locks, observedLockLog.replace(waitLine, waitLine.trimEnd() + ' unexpected\n')));
bad('standalone missing lockfile with cache wait', () => leaf(locks, replaceOne(observedLockLog, lockRow, '')));
bad('standalone duplicate lockfile with cache wait', () => leaf(locks, replaceOne(observedLockLog, lockRow, lockRow + lockRow)));
bad('standalone wrong completion count with cache wait', () => leaf(locks, replaceOne(observedLockLog, 'OK (32 checked)', 'OK (31 checked)')));
bad('standalone completion without roster', () => leaf(locks, waitLine + 'standalone lockfiles: OK (32 checked)\n'));
bad('standalone error with cache wait', () => leaf(locks, observedLockLog + 'error: calibration-only failure\n'));
bad('standalone warning with cache wait', () => leaf(locks, observedLockLog + 'warning: calibration-only warning\n'));
assert.strictEqual(cases.length, 30);
for (const [file, b] of captured) assert.deepStrictEqual(fs.readFileSync(file), b);
console.log(JSON.stringify({development_only: true, helper_calibration_only: true, runtime_mutation_count: 0,
  helper_sha256: sha(fs.readFileSync(__filename)), passed: cases.length, cases}));
`;
// Isolate the unchanged parser definitions; the CLI/record chain has its own real GNU check.
new Function('require', '__filename', source.split(marker)[0] + exercise)(require, helper);




