// SPDX-License-Identifier: GPL-3.0-or-later
// Pure source-placement controls only. These do not execute GDB or prove MI delivery.
import assert from 'node:assert/strict';
import test from 'node:test';
import {finalSource} from './source-files.mjs';
const source=finalSource();
const original=source['gdb/mi/mi-interp.c'];
const header=source['gdb/amd-dbgapi-runtime-observation-v1.h'];
const hooks=source['gdb/amd-dbgapi-runtime-observation-hooks-v1.inc'];
const call = '  amd_runtime_observation_v1::flush_safe_point ();';
const guarded = '#if HAVE_AMD_DBGAPI\n' + call + '\n#endif';
const stopGuarded = '#if HAVE_AMD_DBGAPI\n  /* The complete *stopped record is flushed and mi_uiout rewound first. */\n' + call + '\n#endif';

function section(source, from, to) {
  const start = source.indexOf(from);
  const end = source.indexOf(to, start + from.length);
  assert.ok(start >= 0 && end > start);
  return source.slice(start, end);
}
function check(source) {
  assert.equal(source.split(call).length - 1, 2);
  assert.ok(!source.includes('before_prompt.notify'));
  const prompt = section(source, 'display_mi_prompt (struct mi_interp *mi)', '\nvoid\nmi_interp::on_command_error');
  assert.ok(prompt.includes(guarded));
  assert.ok(prompt.indexOf(guarded) < prompt.indexOf('gdb_puts ("(gdb) \\n", mi->raw_stdout);'));
  const stop = section(source, 'mi_interp::on_normal_stop (struct bpstat *bs, int print_frame)', '\nvoid\nmi_interp::on_about_to_proceed');
  assert.ok(stop.includes(stopGuarded));
  const marks = ['gdb_puts ("*stopped"', 'mi_out_put (mi_uiout', 'mi_out_rewind (mi_uiout)',
    'gdb_puts ("\\n", this->raw_stdout)', 'gdb_flush (this->raw_stdout)', stopGuarded];
  let prior = -1;
  for (const mark of marks) {
    const at = stop.indexOf(mark);
    assert.ok(at > prior, 'ordered complete stop record before flush: ' + mark);
    prior = at;
  }
  assert.match(header, /void flush_safe_point \(\);/);
  assert.match(hooks, /void\namd_runtime_observation_v1::flush_safe_point \(\)\n\{\n  flush_runtime_observation \(\);\n\}/);
}

function refuse(changed){check(original);assert.notEqual(changed,original);assert.throws(()=>check(changed));}

test('two explicitly guarded MI safe points reuse the existing bounded flush', () => check(original));
test('missing prompt call refuses', () => refuse(original.replace(guarded, '')));
test('unguarded prompt call refuses', () => refuse(original.replace(guarded, call)));
test('missing stop call refuses', () => refuse(original.replace(stopGuarded, '')));
test('early stop flush refuses', () => {
  const changed = original.replace(stopGuarded, '').replace('  gdb_puts ("*stopped"', stopGuarded + '\n  gdb_puts ("*stopped"');
  refuse(changed);
});
test('duplicated callback refuses', () => refuse(original.replace(guarded, guarded + '\n' + guarded)));
test('arbitrary CLI observer dispatch refuses', () => refuse(original.replace(guarded, guarded + '\n  gdb::observers::before_prompt.notify ("");')));
