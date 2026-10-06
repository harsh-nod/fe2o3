// SPDX-License-Identifier: GPL-3.0-or-later
// Inert source controls; this module does not compile or execute the C++ fixture.
import test from 'node:test';
import assert from 'node:assert/strict';
import {SourceSession} from './source-files.mjs';
const s=new SourceSession(), m=s.manifest();
const owner=s.packageText('src/amd-dbgapi-owned-one-stop-v1.h');
const native=s.packageText('src/amd-dbgapi-one-stop-native-v1.h');
const io=s.packageText('src/amd-dbgapi-one-stop-native-io-v1.inc');
const c=JSON.parse(s.packageText('tests/budget-contract.json'));
test('three retained diagnostic leaves preserve all selected roles',()=>{for(const leaf of ['amd-dbgapi-one-stop-native-io-v1.inc','amd-dbgapi-one-stop-native-v1.h','amd-dbgapi-owned-one-stop-v1.h']){const selected=m.selected.files.find(r=>r.path==='gdb/'+leaf),payload=m.payloads.find(r=>r.path==='src/'+leaf);assert.deepEqual({bytes:selected.bytes,sha256:selected.sha256},{bytes:payload.bytes,sha256:payload.sha256});}assert.equal(m.selected.files.length,63);assert.equal(m.selected.bytes,2267086);});
test('first-denial note remains private and fixed storage',()=>{assert(owner.includes('static_assert (sizeof (budget_failure_note) <= 32'));assert(owner.includes('  budget_failure_note m_budget_failure {};'));assert.equal(owner.split('m_budget_failure =').length,2);});
test('denied debit uses no-wrap and preserves sticky refusal',()=>{assert(owner.includes('if (used == nullptr) return fail (failure::budget);'));assert(owner.includes('if (*used > cap || amount > cap - *used) {'));assert(owner.includes('m_budget_failure = {kind, m_phase, true, *used, amount, cap};'));assert(owner.includes('*used += amount; return true;'));});
test('error fragment exactly equals production and is budget-only',()=>{const fragment=s.packageText('tests/budget-error.inc').trimEnd();assert(io.includes(fragment));assert(fragment.startsWith('  if (m_owner.why ()==failure::budget && m_owner.m_budget_failure.present)'));assert(fragment.includes('budget-counter=%u; budget-phase=%u; budget-used=%llu; budget-request=%llu; budget-cap=%llu'));});
test('separate frame is charged through existing selected scratch type',()=>{assert(native.includes('struct budget_frames final'));assert(native.includes('char fatal_error_payload[256]'));assert(native.includes('sizeof (native_result_diagnostic_scratch::budget_frames) <= 320'));assert(native.includes('sizeof (native_result_diagnostic_scratch)'));});
test('C++ fixture uses actual owner and retains all boundary lanes',()=>{const t=s.packageText('tests/budget-diagnostic.cc');assert(t.includes('#include "../src/amd-dbgapi-owned-one-stop-v1.h"'));assert(t.includes('#include "budget-error.inc"'));assert(t.includes('UINT64_MAX'));assert(t.includes('232'));assert(t.includes('384'));});
test('fixed decimal error upper bound is 209 plus NUL',()=>{const max='18446744073709551615';const out='fixed owned one-stop native relation refused (255); commit-site=255; budget-counter=255; budget-phase=255; budget-used='+max+'; budget-request='+max+'; budget-cap='+max;assert.equal(Buffer.byteLength(out),209);assert(Buffer.byteLength(out)+1<=256);});
test('diagnostic is not a counter observation or behavioral fix',()=>{assert.equal(c.behavioral_fix,false);assert.equal(c.actual_denied_counter,null);assert.equal(c.actual_new_layout,null);assert.equal(c.native_authority,false);assert.equal(c.unchanged_logical_cap,65536);assert.equal(c.selected_type_count,20);assert.equal(c.expected_lp64_reservation,20312);});
