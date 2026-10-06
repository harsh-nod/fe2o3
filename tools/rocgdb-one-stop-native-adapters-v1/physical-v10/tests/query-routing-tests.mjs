// SPDX-License-Identifier: GPL-3.0-or-later
// Inert source/package controls; never builds or runs the C++ fixture.
import test from 'node:test';
import assert from 'node:assert/strict';
import {SourceSession} from './source-files.mjs';
const s=new SourceSession(),m=s.manifest(),c=m.changed[0];
const before=s.packageText(c.preimage),after=s.packageText(c.postimage);
const d=JSON.parse(s.packageText(c.delta)),edit=d.changes[0];
test('one exact reversible target span',()=>{assert.equal(before.split(edit.before).length,2);assert.equal(after,before.replace(edit.before,edit.after));assert.equal(after.replace(edit.after,edit.before),before);});
test('inert C++ route exactly equals production postimage span',()=>assert.equal(s.packageText('tests/routing-under-test.inc'),edit.after+'\n'));
test('only selected WAVE_STOP rejects simultaneous legacy profile',()=>{assert(edit.after.includes('event_kind == AMD_DBGAPI_EVENT_KIND_WAVE_STOP\n          && amd_owned_one_stop_native_v1::selected ()'));assert(edit.after.indexOf('reason::unsupported_profile')<edit.after.indexOf('&& !stopped_wave_observation.invalid ()'));});
test('unselected legacy query condition remains exact',()=>assert(edit.after.endsWith(edit.before.slice(edit.before.indexOf('      if (')))));
test('provider fixture preserves ten permits and five bytes',()=>{const t=s.packageText('tests/queue-health-v1.h');assert(t.includes('query_limit = 10'));assert(t.includes('sizeof (queue_health_v1) == 5'));assert(t.includes('++m_queries; // Consume before the sole driver call'));});
test('legacy rejection cannot be cached or synthesized publication',()=>{for(const n of ['publish_normal_stop','m_candidate_stop','take_query'])assert(!edit.after.includes(n));assert(edit.after.includes('stopped_obs::invalidate'));});
test('fixture preserves production bound guard and inert binding',()=>{const t=s.packageText('tests/query-routing.cc');assert(t.includes('if (stopped_wave_observation.bound())\n    stopped_wave_observation.invalidate(r,status);'));assert(t.includes('assert(stopped_wave_observation.bind(1,1,1234,1));'));});
test('closed fixture contract retains all owned pairs and disabled qualification',()=>{const v=JSON.parse(s.packageText('tests/query-contract.json'));assert.deepEqual(v,{schema:'fe2o3-owned-legacy-query-separation-source-contract-v1',source_changes:1,owned_health_pairs:5,legacy_health_pairs_selected:0,legacy_health_pairs_unselected_unchanged:true,provider_query_limit:10,provider_ledger_bytes:5,legacy_rejection:'unsupported_profile',paired_completion_edges:11,activation_available:false,native_qualified:false});});
