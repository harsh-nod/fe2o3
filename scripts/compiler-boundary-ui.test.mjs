import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import * as cursor from './execution-cursor-ui.mjs';
import * as custody from './source-storage-root-custody-ui.mjs';
import { compilerBoundarySuite } from './compiler-boundary-ui.mjs';

const repo = fileURLToPath(new URL('../', import.meta.url));
const target = { name: 'fe2o3_lower_mir_kernel', kind: ['lib'] };
const artifact = { reason: 'compiler-artifact', target };
const terminal = success => ({ reason: 'build-finished', success });
const run = (status, rows) => ({ status, signal: null, stdout: rows.map(JSON.stringify).join('\n') });
const leafSource = suite => readFileSync(resolve(repo, 'crates/fe2o3-lower-mir-kernel/src', suite.leaf), 'utf8');
function diagnostic(suite, source, name) {
  const [start, end] = suite.bounds(source, name);
  const code = name.startsWith('buffer_') ? 'E0509' : name === 'owned' ? 'E0308' : name === 'install' ? 'E0505' : 'E0521';
  return { reason: 'compiler-message', target, message: {
    level: 'error', message: 'expected ownership refusal', code: { code },
    spans: [{ is_primary: true, file_name: `src/${suite.leaf}`, line_start: start + 1, line_end: end - 1 }],
  } };
}

test('cursor selectors only affect the final production library', () => {
  assert.deepEqual(cursor.cases, ['positive', 'direct', 'nested', 'buffer_direct', 'buffer_nested']);
  for (const name of cursor.cases) {
    const args = cursor.cargoArguments(name);
    const split = args.indexOf('--');
    assert.deepEqual(args.slice(0, split), ['rustc', '--offline', '--locked', '--lib', '-p', 'fe2o3-lower-mir-kernel', '--message-format=json']);
    assert.equal(args[split + 1], '--emit=metadata');
    assert.deepEqual(args.slice(split + 2), name === 'positive' ? [] : ['--cfg', `fe2o3_cursor_${name.startsWith('buffer_') ? 'buffer_escape_' + name.slice(7) : 'escape_' + name}_v1`]);
  }
  assert.throws(() => cursor.cargoArguments('unknown'));
  assert.throws(() => compilerBoundarySuite({ leaf: 'unused', selectors: { negative: [] } }));
});

for (const [label, suite] of [['cursor', cursor], ['custody', custody]]) {
  const source = leafSource(suite);
  test(`${label}: the real marked source accepts only its exact selected diagnostic`, () => {
    assert.doesNotThrow(() => suite.assess('positive', source, run(0, [artifact, terminal(true)])));
    for (const name of suite.cases.slice(1)) {
      const expected = diagnostic(suite, source, name);
      const valid = () => run(101, [expected, terminal(false)]);
      assert.doesNotThrow(() => suite.assess(name, source, valid()));
      for (const mutate of [
        row => { row.target = { ...target, name: 'other_crate' }; },
        row => { row.target = { ...target, kind: ['bin'] }; },
        row => { row.message.code = { code: 'E0425' }; row.message.message = 'lifetime may not live long enough'; },
        row => { row.message.spans[0].is_primary = false; },
        row => { row.message.spans[0].file_name = 'src/other.rs'; },
        row => { row.message.spans[0].line_start = 1; },
        row => { row.message.spans[0].line_end = source.split('\n').length + 1; },
      ]) {
        const unrelated = structuredClone(expected);
        mutate(unrelated);
        assert.throws(() => suite.assess(name, source, run(101, [expected, unrelated, terminal(false)])));
      }
      assert.throws(() => suite.assess(name, source, run(1, [expected, terminal(false)])));
      assert.throws(() => suite.assess(name, source, run(0, [artifact, terminal(true)])));
      assert.throws(() => suite.assess(name, source, { ...valid(), signal: 'SIGTERM' }));
      assert.throws(() => suite.assess(name, source, { ...valid(), error: new Error('timeout') }));
      assert.throws(() => suite.assess(name, source, { ...valid(), stderr: 'error: internal compiler error' }));
      assert.throws(() => suite.assess(name, source, { ...valid(), stderr: 'process failed (signal: 9, SIGKILL)' }));
      assert.throws(() => suite.assess(name, source, run(101, [expected])));
      assert.throws(() => suite.assess(name, source, run(101, [expected, terminal(true)])));
      assert.throws(() => suite.assess(name, source, run(101, [expected, terminal(false), terminal(false)])));
      assert.throws(() => suite.assess(name, `${source}\n// UI-BEGIN ${name}`, valid()));
    }
    assert.throws(() => suite.assess('positive', source, run(0, [terminal(true)])));
    assert.throws(() => suite.assess('positive', source, run(0, [{ ...artifact, target: { ...target, kind: ['bin'] } }, terminal(true)])));
    assert.throws(() => suite.assess(suite.cases[1], source, { status: 101, stdout: '{bad json' }));
  });

  test(`${label}: bounded runner visits every case and preserves the caller environment`, () => {
    const env = Object.freeze({ CARGO: '/bounded/cargo', RUSTFLAGS: '-C link-arg=-Wl,--threads=1' });
    const visits = [];
    const reports = [];
    const results = suite.runSuite(repo, env, {
      runCompiler(command, args, options) {
        const name = suite.cases[visits.length];
        visits.push(name);
        assert.equal(command, env.CARGO);
        assert.deepEqual(args, suite.cargoArguments(name));
        assert.equal(options.cwd, repo);
        assert.equal(options.timeout, 900_000);
        assert.equal(options.maxBuffer, 64 * 1024 * 1024);
        assert.deepEqual(options.env, env);
        assert.notEqual(options.env, env);
        return name === 'positive' ? run(0, [artifact, terminal(true)]) : run(101, [diagnostic(suite, source, name), terminal(false)]);
      },
      report: row => reports.push(row),
    });
    assert.deepEqual(visits, suite.cases);
    assert.deepEqual(results, reports);
    assert.deepEqual(results.map(row => row.status), suite.cases.map(name => name === 'positive' ? 'compiled' : 'rejected'));
    let calls = 0;
    assert.throws(() => suite.runSuite(repo, env, { runCompiler() { calls++; return run(101, [terminal(false)]); } }));
    assert.equal(calls, 1);
    assert.throws(() => suite.runSuite(repo, env, {
      runCompiler() {
        return { ...run(101, [terminal(false)]), stdout: 'dependency metadata'.repeat(10_000), stderr: 'actual compiler failure' };
      },
    }), error => error.message.includes('actual compiler failure') && error.message.length < 17_000);
  });
}
