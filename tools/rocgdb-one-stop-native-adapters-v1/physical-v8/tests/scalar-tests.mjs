// SPDX-License-Identifier: GPL-3.0-or-later
// Self-contained source controls; full GDB and compiled CPU fixtures are separate.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {SourceSession} from './source-files.mjs';
const s=new SourceSession(),m=s.manifest();
const contract=JSON.parse(s.packageText('tests/scalar-contract.json'));
const before=s.packageText(contract.intermediate.owner.path),after=s.packageText(contract.production.path);
const digest=x=>crypto.createHash('sha256').update(x).digest('hex');
const pin=(text,p)=>{assert.equal(Buffer.byteLength(text),p.bytes);assert.equal(digest(text),p.sha256);};
test('complete intermediate owner and successor differ by exactly the one reviewed byte',()=>{
 pin(before,contract.intermediate.owner);pin(after,contract.production);
 const e=contract.edit;
 assert.equal(before.split(e.before).length,2);assert.equal(after.split(e.after).length,2);
 assert.equal(before.replace(e.before,e.after),after);assert.equal(after.replace(e.after,e.before),before);
 const a=Buffer.from(before),b=Buffer.from(after),changes=[];
 assert.equal(a.length,b.length);
 for(let i=0;i<a.length;i++)if(a[i]!==b[i])changes.push([a[i],b[i]]);
 assert.deepEqual(changes,[[48,49]]);
});
test('both full CPU predicates are exact corresponding production lambda bodies',()=>{
 for(const [text,p]of [[before,contract.extract.legacy],[after,contract.extract.current]]){
  const head='checkpoint_require (checkpoint_site::checkpoint_presentation, [&] { return ';
  const start=text.indexOf(head),end=text.indexOf('; });',start);
  assert(start>=0&&end>start);
  const extracted=s.packageText(p.path);pin(extracted,p);
  assert.equal(text.slice(start+head.length,end)+'\n',extracted);
 }
 assert.equal(s.packageText(contract.extract.legacy.path).replace('status->simd_lane_mask==0','status->simd_lane_mask==1'),s.packageText(contract.extract.current.path));
});
test('packaged complete thread method preserves scalar default and architecture hook',()=>{
 const p=contract.extract.thread,text=s.packageText(p.path);pin(text,p);
 assert(text.startsWith('simd_lanes_mask_t\nthread_info::active_simd_lanes_mask ()\n{'));
 assert(text.endsWith('\n}\n'));
 assert(text.includes('gdb_assert (this->inf != nullptr);'));
 assert(text.includes('if (gdbarch_active_lanes_mask_p (arch))\n    return gdbarch_active_lanes_mask (arch, this);'));
 assert(text.includes('/* Default: only one lane is active, lane 0.  */\n  return 1;'));
 assert.equal(contract.upstream.files.find(x=>x.role==='thread').sha256,'ef168539705ce1252786b0b9a105f107417a5a637b7b4acadf9ea04427b8d005');
 // This pin describes upstream provenance; no undeclared external file is read.
});
test('site11 alone composes its functional correction inside the diagnostic range',()=>{
 const sites=JSON.parse(s.packageText('tests/checkpoint-sites.json')).sites;
 const row=sites.find(x=>x.id===11);
 assert.equal(row.label,'checkpoint_presentation');
 assert.equal(row.replacement.split(contract.edit.before).length,2);
 assert.equal(before.split(row.replacement).length,2);
 assert.equal(after.split(row.replacement.replace(contract.edit.before,contract.edit.after)).length,2);
 const delta=JSON.parse(s.packageText('transforms/amd-dbgapi-one-stop-native-owner-v1.inc.json'));
 assert.equal(delta.changes.filter(x=>x.after.includes(contract.edit.after)).length,1);
 assert.equal(m.changed.flatMap(x=>JSON.parse(s.packageText(x.delta)).changes).length,44);
});
test('all other v7 source postimages and the selected roster remain independently exact',()=>{
 assert.equal(contract.intermediate.changed_postimages.length,7);
 for(const p of contract.intermediate.changed_postimages){
  const c=m.changed.find(x=>x.source===p.path);assert(c);
  if(p.path.endsWith('native-owner-v1.inc'))pin(before,p);
  else pin(s.packageText(c.postimage),p);
 }
 assert.equal(m.selected.files.length,63);assert.equal(m.selected.bytes,2254118);
 assert.equal(m.predecessor.directory,'../physical-v6');
});
test('host architecture checks and no-added-call-or-storage contract are preserved',()=>{
 for(const text of [before,after]){
  assert(text.includes('gdbarch_bfd_arch_info (arch)->arch==bfd_arch_i386'));
  assert(text.includes('gdbarch_bfd_arch_info (arch)->mach==bfd_mach_x86_64'));
  assert(text.includes('gdbarch_byte_order (arch)==BFD_ENDIAN_LITTLE'));
 }
 assert.equal(contract.new_native_calls,0);assert.equal(contract.new_file_reads,0);assert.equal(contract.new_storage_fields,0);
 assert.equal(contract.required_layout_types,18);
});
test('CPU fixture explicitly covers scalar masks every retained clause hooks and assignment ordering',()=>{
 const t=s.packageText('tests/scalar-presentation-test.cc');
 for(const n of ['thread-method.inc','presentation-condition.inc','legacy-condition.inc'])assert(t.includes('#include "'+n+'"'));
 assert(t.includes('mask<256'));assert(t.includes('std::array<void(*)(fixture&),17>'));
 assert(t.includes('if(checks!=300)return 2;'));
 assert(t.includes('f.storage.simd_lane_mask=t.active_simd_lanes_mask();'));
 assert(t.includes('catch(gdbarch*p){same=p==&arch;}'));
 assert.equal(contract.controls.cpp_checks,300);assert.equal(contract.controls.node_groups,8);
});
test('functional correction neither grants authority nor proves the other compound operands',()=>{
 assert.equal(contract.authority,false);assert.equal(contract.native_execution,false);
 assert.equal(contract.unique_runtime_conjunct_proven,false);
 for(const k of ['activation_available','capture_available','publication_available','source_checks_are_native_authority'])assert.equal(m[k],false);
 assert.equal(m.native_caps.logical_bytes,65536);assert.equal(m.native_caps.api_calls,192);
 assert.equal(m.native_caps.read_bytes,65536);assert.equal(m.native_caps.read_calls,256);
});
