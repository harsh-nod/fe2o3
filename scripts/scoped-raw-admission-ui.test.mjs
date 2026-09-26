import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { cases, assess, bounds, cargoArguments, leaf } from './scoped-raw-admission-ui.mjs';

const source = readFileSync(fileURLToPath(new URL(`../crates/fe2o3-lower-mir-kernel/src/${leaf}`, import.meta.url)), 'utf8');
const target = { name: 'fe2o3_lower_mir_kernel', kind: ['lib'] };
const run = (status, messages) => ({ status, signal: null, stderr: '', stdout:
  messages.concat({ reason: 'build-finished', success: status === 0 }).map(JSON.stringify).join('\n') });

test('positive control and every negative require only their exact production diagnostic', () => {
  assess('positive', source, run(0, [{ reason: 'compiler-artifact', target }]));
  for (const name of cases.slice(1)) {
    const [start] = bounds(source, name);
    const message = { reason: 'compiler-message', target, message: {
      level: 'error', code: { code: name.startsWith('physical_') ? 'E0521' : name === 'checked_path' ? 'E0308'
        : name === 'forged_completion' ? 'E0451' : 'E0616' },
      message: 'expected production refusal', spans: [{ is_primary: true,
        file_name: `crates/fe2o3-lower-mir-kernel/src/${leaf}`, line_start: start + 2, line_end: start + 2 }],
    } };
    assess(name, source, run(101, [message]));
    assert.throws(() => assess(name, source, run(0, [])));
    assert.throws(() => assess(name, source, run(101, [{ ...message, message: { ...message.message, code: { code: 'E0425' } } }])));
    assert.throws(() => assess(name, source, run(101, [{ ...message, target: { name: 'dependency', kind: ['lib'] } }])));
    assert.throws(() => assess(name, source, run(101, [{ ...message, message: { ...message.message,
      spans: [{ is_primary: true, file_name: 'unrelated.rs', line_start: start + 2, line_end: start + 2 }] } }])));
    assert.throws(() => assess(name, source, { ...run(101, [message]), stderr: 'internal compiler error' }));
  }
});

test('probe flags belong only to the final crate and preserve caller environment', () => {
  for (const name of cases) {
    const args = cargoArguments(name);
    const separator = args.indexOf('--');
    assert.equal(args[0], 'rustc');
    assert(args.slice(0, separator).includes('--offline'));
    assert(args.slice(0, separator).includes('--locked'));
    assert(!args.slice(0, separator).includes('--cfg'));
    assert(args.slice(separator + 1).includes('--emit=metadata'));
    assert.equal(args.filter(arg => arg === '--cfg').length, name === 'positive' ? 0 : 1);
  }
});
