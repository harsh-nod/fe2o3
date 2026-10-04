// SPDX-License-Identifier: GPL-3.0-or-later
// Source tests only; actual C++ diagnostic execution is separate.
import test from 'node:test';
import assert from 'node:assert/strict';
import {SourceSession} from './source-files.mjs';
import {createRequire} from 'node:module';
const {parseCheckpointCalls}=createRequire(import.meta.url)('./parse-checkpoint-calls.cjs');
const s=new SourceSession(),m=s.manifest();
const sites=JSON.parse(s.packageText('tests/checkpoint-sites.json')).sites;
test('checkpoint core and guard helper are exact full production sections',()=>{
 const h=s.packageText('src/amd-dbgapi-one-stop-native-v1.h');
 for(const p of ['tests/checkpoint-core.inc','tests/checkpoint-helper.inc']) assert.equal(h.split(s.packageText(p)).length,2);
});
test('all38 checkpoint sites retain exact expressions except the explicit scalar site11 correction',()=>{
 assert.equal(sites.length,38);
 assert.deepEqual(sites.map(x=>x.id),Array.from({length:38},(_,i)=>i+1));
 assert.equal(new Set(sites.map(x=>x.label)).size,38);
 for(const r of sites) {
  const before=s.packageText('preimages/'+r.file),after=s.packageText('src/'+r.file);
  const c=parseCheckpointCalls(before).find(x=>x.line===r.line);
  assert(c);assert.equal(c.before,r.original);assert.equal(c.condition,r.condition);
  let expected=r.replacement;
  if(r.id===11){
   assert.equal(r.label,'checkpoint_presentation');
   assert.equal(expected.split('status->simd_lane_mask==0').length,2);
   expected=expected.replace('status->simd_lane_mask==0','status->simd_lane_mask==1');
  }
  assert.equal(after.split(expected).length,2);
 }
});
test('all checkpoint guard calls accounted without changing their refusal family',()=>{
 let n=0;
 for(const file of new Set(sites.map(x=>x.file))) {
  n+=parseCheckpointCalls(s.packageText('preimages/'+file)).length;
  assert.equal(parseCheckpointCalls(s.packageText('src/'+file)).length,0);
 }
 assert.equal(n,38);
 assert(s.packageText('tests/checkpoint-helper.inc').includes('require (yes,failure::checkpoint_changed)'));
});
test('pre-expression owner validity and original exception propagation remain explicit',()=>{
 const helper=s.packageText('tests/checkpoint-helper.inc'),core=s.packageText('tests/checkpoint-core.inc');
 const capture=helper.indexOf('const bool was_valid=m_owner.selected () && !m_owner.invalid ();');
 assert(capture>=0&&helper.indexOf('m_checkpoint_note.evaluate (was_valid,site,predicate)')>capture);
 assert(core.includes('const bool yes = predicate ();'));
 assert(core.includes('if (was_valid && !yes && site == checkpoint_site::none'));
 assert(!core.includes('catch')&&!helper.includes('catch'));
});
test('two-byte note and separate bounded diagnostic scratch preserve native caps',()=>{
 const h=s.packageText('src/amd-dbgapi-one-stop-native-v1.h');
 assert(h.includes('sizeof (checkpoint_note) == 2'));
 assert(h.includes('std::array<const void *,32> predicate_reference_storage'));
 assert.equal(h.split('+ sizeof (checkpoint_diagnostic_scratch)').length,3);
 assert.equal(m.native_caps.logical_bytes,65536);
 assert.equal(m.native_caps.api_calls,192);
 assert.equal(m.native_caps.read_bytes,65536);
 assert.equal(m.native_caps.read_calls,256);
});
test('checkpoint diagnostic output remains gated by sticky failure and cannot grant authority',()=>{
 const io=s.packageText('src/amd-dbgapi-one-stop-native-io-v1.inc');
 assert(io.includes('if (m_owner.why ()==failure::checkpoint_changed\n      && m_checkpoint_note.site!=checkpoint_site::none)'));
 assert(io.includes('checkpoint-site=%u'));
 assert(io.includes('m_owner.poison (why, site)'));
 assert.equal(m.activation_available,false);assert.equal(m.capture_available,false);assert.equal(m.publication_available,false);
});
test('portable fixture uses exact core and helper with explicit mocked owner/refusal only',()=>{
 const t=s.packageText('tests/checkpoint-core-test.cc');
 assert(t.includes('#include "checkpoint-core.inc"'));
 assert(t.includes('#include "checkpoint-helper.inc"'));
 assert(t.includes('struct mock_owner'));
 assert(t.includes('if(checks!=56) return 2;'));
 const contract=JSON.parse(s.packageText('tests/fixture-contracts.json'));
 assert.equal(contract.checkpoint.controls,56);
 assert.equal(contract.native_result.controls,12);
});
test('seven selected leaves descend from public v6 and 44 normalized transforms compose the diagnostic step and scalar correction',()=>{
 assert.equal(m.predecessor.directory,'../physical-v6');
 assert.equal(m.changed.length,7);
 const changes=m.changed.flatMap(c=>JSON.parse(s.packageText(c.delta)).changes);
 assert.equal(changes.length,44);
 assert.equal(m.changed.some(x=>/activation|publication-v2|snapshot-v1\.h/.test(x.source)),false);
});
