import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { assess, bounds, cargoArguments, cases, leaf } from './source-storage-root-custody-ui.mjs';

const source = cases.map(name => `// UI-BEGIN ${name}\nfn case_${name}() {}\n// UI-END ${name}`).join('\n');
const target = { name: 'fe2o3_lower_mir_kernel', kind: ['lib'] };
const terminal = success => ({ reason: 'build-finished', success });
const run = (status, rows) => ({ status, signal: null, stdout: rows.map(JSON.stringify).join('\n') });
const diagnostic = (name, changes = {}) => ({
  reason: 'compiler-message', target,
  message: { level: 'error', message: 'lifetime may not live long enough', code: null,
    spans: [{ is_primary: true, file_name: `src/${leaf}`, line_start: bounds(source, name)[0] + 1, line_end: bounds(source, name)[1] - 1 }], ...changes },
});

test('probe selectors are final-package flags after the cargo rustc separator', () => {
  for (const name of cases) {
    const args = cargoArguments(name);
    const separator = args.indexOf('--');
    assert.deepEqual(args.slice(0, separator), ['rustc', '--offline', '--locked', '--lib', '-p', 'fe2o3-lower-mir-kernel', '--message-format=json']);
    assert.deepEqual(args.slice(separator + 1), ['--emit=metadata', '--cfg', 'fe2o3_source_storage_root_custody_ui', '--cfg', `fe2o3_source_storage_root_custody_ui_case="${name}"`]);
  }
  assert.throws(() => cargoArguments('unknown'));
});

test('real positive artifact and terminal record are both required', () => {
  const artifact = { reason: 'compiler-artifact', target };
  assert.doesNotThrow(() => assess('positive', source, run(0, [artifact, terminal(true)])));
  assert.throws(() => assess('positive', source, run(0, [terminal(true)])));
  assert.throws(() => assess('positive', source, run(0, [artifact])));
  assert.throws(() => assess('positive', source, run(101, [diagnostic('state'), terminal(false)])));
});

test('negative success, absent diagnostics, crashes and timeouts never qualify', () => {
  assert.throws(() => assess('state', source, run(0, [terminal(true)])));
  assert.throws(() => assess('state', source, run(101, [terminal(false)])));
  assert.throws(() => assess('state', source, { ...run(101, [diagnostic('state'), terminal(false)]), signal: 'SIGKILL' }));
  assert.throws(() => assess('state', source, { ...run(101, [diagnostic('state'), terminal(false)]), error: new Error('timeout') }));
});

test('only the selected actual leaf and selected boundary diagnostic count', () => {
  assert.doesNotThrow(() => assess('state', source, run(101, [diagnostic('state'), terminal(false)])));
  assert.throws(() => assess('state', source, run(101, [diagnostic('path'), terminal(false)])));
  assert.throws(() => assess('state', source, run(101, [diagnostic('state', { spans: [] }), terminal(false)])));
  assert.throws(() => assess('state', source, run(101, [diagnostic('state', { code: { code: 'E0425' }, message: 'cannot find value' }), terminal(false)])));
  const other = diagnostic('state'); other.target = { name: 'unrelated_dependency' };
  assert.throws(() => assess('state', source, run(101, [other, terminal(false)])));
  assert.throws(() => assess('state', source, run(101, [diagnostic('state'), other, terminal(false)])));
});

test('owned backing and consuming install require their corresponding type or move rejection', () => {
  assert.doesNotThrow(() => assess('owned', source, run(101, [diagnostic('owned', { code: { code: 'E0308' }, message: 'incompatible types' }), terminal(false)])));
  assert.doesNotThrow(() => assess('install', source, run(101, [diagnostic('install', { code: { code: 'E0505' }, message: 'cannot move out because it is borrowed' }), terminal(false)])));
  assert.throws(() => assess('owned', source, run(101, [diagnostic('owned'), terminal(false)])));
});

test('duplicate markers, malformed records and contradictory terminals fail closed', () => {
  assert.throws(() => bounds(`${source}\n// UI-BEGIN state`, 'state'));
  assert.throws(() => assess('state', source, { status: 1, signal: null, stdout: '{bad json' }));
  assert.throws(() => assess('state', source, run(101, [diagnostic('state'), terminal(true)])));
  assert.throws(() => assess('state', source, run(101, [diagnostic('state'), terminal(false), terminal(false)])));
});
