// Observe an interrupted run without manufacturing a completion or cleanup event.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const helper = root + 'r124-development-check-v1.js';
assert.strictEqual(hash(fs.readFileSync(helper)), '1caef52ee3c10a2d8efd49d82debadfbe227822fb575fc88ea32f99cd0bb82ad');
const C = require(helper);
assert.deepStrictEqual(C.identities(), C.map);
const name = 'r124-development-musl-all-v1';
const logPath = root + name + '.log', sourcePath = root + name + '-source.json';
const log = fs.readFileSync(logPath), source = fs.readFileSync(sourcePath);
assert.strictEqual(hash(log), '8410dc099e9020d2882767ab2161360f5acfac9792ee6e6820fd19167333e306');
assert.strictEqual(hash(source), 'dcb514f96f92c948914e640035f853ad02f2663109963aaf8135fae4a3247f2e');
assert.deepStrictEqual(JSON.parse(source), C.map);
const missing = [name + '.json', name + '-source-after.json', name + '-gate-failure.json'];
for (const file of missing) assert(!fs.existsSync(root + file), 'missing original completion evidence');
const priorPath = root + 'r124-development-gnu-all-v1.json', priorBytes = fs.readFileSync(priorPath);
assert.strictEqual(hash(priorBytes), '7dcfc8a2edd044899ff6bb5ee07fd869e5f8624e66e54198e6631a0817969b2c');
const prior = JSON.parse(priorBytes), boot = fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim();
assert.strictEqual(prior.clock.source_verified.boot_id, 'f78ad468-d752-4634-8e79-45a154b73909');
assert.strictEqual(boot, '74856d82-79a2-4377-b6c4-aaf01159fd19');
assert.notStrictEqual(boot, prior.clock.source_verified.boot_id);
const matching = [];
for (const pid of fs.readdirSync('/proc').filter(value => /^[1-9][0-9]*$/.test(value))) {
  let argv;
  try { argv = fs.readFileSync('/proc/' + pid + '/cmdline').toString().split('\0').filter(Boolean); }
  catch (error) { if (['ENOENT', 'ESRCH'].includes(error.code)) continue; throw error; }
  if (argv.includes(name) || argv.some(arg => arg.startsWith(C.repo + '/target/') && /fe2o3_kfd-/.test(arg))) {
    matching.push({pid: Number(pid), argv});
  }
}
assert.deepStrictEqual(matching, [], 'no current matching musl runner or KFD executable');
const observation = {utc: new Date().toISOString(), monotonic_ns: process.hrtime.bigint().toString(),
  boot_id: boot, clock_source: 'node-process-hrtime-linux-monotonic'};
const report = {classification: 'interrupted_no_completion_record', qualification: false, run: name,
  observation, prior_completed_prerequisite: {path: priorPath, sha256: hash(priorBytes),
    boot_id: prior.clock.source_verified.boot_id},
  artifacts: [{path: logPath, bytes: log.length, sha256: hash(log)},
    {path: sourcePath, bytes: source.length, sha256: hash(source)}], missing_completion_artifacts: missing,
  current_source_map_sha256: C.p.source_map_sha256, current_source_identities: Object.keys(C.map).length,
  current_matching_processes: matching, observed_returncode: null, observed_signal: null,
  observed_finish: null, observed_child_close: null, observed_elapsed_seconds: null,
  exclusions: ['test acceptance', 'normal original cleanup', 'verified descendant containment',
    'original start or finish reconstruction', 'cross-boot monotonic duration']};
assert.deepStrictEqual(fs.readFileSync(logPath), log);
assert.deepStrictEqual(fs.readFileSync(sourcePath), source);
assert.deepStrictEqual(C.identities(), C.map); C.stable();
const output = root + 'r124-development-musl-interruption-observation-v1.json';
fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({output, sha256: hash(fs.readFileSync(output)), qualification: false,
  classification: report.classification, current_boot: boot}));
