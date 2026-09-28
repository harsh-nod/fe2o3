// SPDX-License-Identifier: GPL-3.0-or-later
// Read-only selected source; no process execution, activation or native authority.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {readBounded} from '../../../rocgdb-runtime-observation-v1/tests/source-files.mjs';
import {manifest as parentManifest} from '../../physical-v2/tests/source-files.mjs';
export {readBounded};
export const MAX_FILE=512*1024, MAX_METADATA=64*1024, MAX_TOTAL=2144*1024;
export const PATHS=Object.freeze([
  "gdb/amd-dbgapi-one-stop-activation-v1.h",
  "gdb/amd-dbgapi-one-stop-checkpoint-v1.h",
  "gdb/amd-dbgapi-one-stop-locator-v1.h",
  "gdb/amd-dbgapi-one-stop-native-events-v1.inc",
  "gdb/amd-dbgapi-one-stop-native-io-v1.inc",
  "gdb/amd-dbgapi-one-stop-native-object-v1.inc",
  "gdb/amd-dbgapi-one-stop-native-owner-v1.inc",
  "gdb/amd-dbgapi-one-stop-native-query-v1.inc",
  "gdb/amd-dbgapi-one-stop-native-resume-v1.inc",
  "gdb/amd-dbgapi-one-stop-native-snapshot-v1.inc",
  "gdb/amd-dbgapi-one-stop-native-v1.h",
  "gdb/amd-dbgapi-one-stop-native-wrappers-v1.inc",
  "gdb/amd-dbgapi-one-stop-observation-v1.h",
  "gdb/amd-dbgapi-one-stop-output-v2.h",
  "gdb/amd-dbgapi-one-stop-profile-v1.h",
  "gdb/amd-dbgapi-one-stop-publication-v2.h",
  "gdb/amd-dbgapi-one-stop-sha256-v1.h",
  "gdb/amd-dbgapi-one-stop-snapshot-v1.h",
  "gdb/amd-dbgapi-owned-one-stop-v1.h",
  "gdb/amd-dbgapi-runtime-observation-hooks-v1.inc",
  "gdb/amd-dbgapi-runtime-observation-v1.h",
  "gdb/amd-dbgapi-stopped-wave-observation-hooks-v1.inc",
  "gdb/amd-dbgapi-stopped-wave-observation-v1.h",
  "gdb/amd-dbgapi-stopped-wave-query-v1.inc",
  "gdb/amd-dbgapi-target.c",
  "gdb/amd-dbgapi-target.h",
  "gdb/amd64-tdep.h",
  "gdb/breakpoint.c",
  "gdb/breakpoint.h",
  "gdb/gdbthread.h",
  "gdb/inferior.c",
  "gdb/inferior.h",
  "gdb/infrun.c",
  "gdb/interps.c",
  "gdb/interps.h",
  "gdb/mi/amd-stopped-wave-observation-mi-v1.inc",
  "gdb/mi/mi-console.c",
  "gdb/mi/mi-interp.c",
  "gdb/mi/mi-interp.h",
  "gdb/mi/mi-main.c",
  "gdb/mi/mi-out.c",
  "gdb/mi/mi-out.h",
  "gdb/minsyms.h",
  "gdb/observable.h",
  "gdb/pager.h",
  "gdb/posix-hdep.c",
  "gdb/process-stratum-target.h",
  "gdb/progspace.h",
  "gdb/regcache.h",
  "gdb/solib-rocm.c",
  "gdb/symfile.h",
  "gdb/symtab.h",
  "gdb/target.c",
  "gdb/target.h",
  "gdb/ui-file.c",
  "gdb/ui-file.h",
  "gdb/ui-out.c",
  "gdb/ui-out.h",
  "gdb/ui.c",
  "gdb/ui.h",
  "gdb/utils.c",
  "gdbsupport/gdb_ref_ptr.h",
  "gdbsupport/refcounted-object.h"
]);
export const STAGES=Object.freeze(['physical-publication-disabled-v2','physical-host-entry-maintenance-disabled-v3']);
const EXPECTED_MANIFEST='b5c46e9a98f72badbc90deb1740481665ef6d76bea1b9d3adac5574ea738c48f';
const digest=b=>crypto.createHash('sha256').update(b).digest('hex');
const keys=(v,k)=>assert.deepEqual(Object.keys(v).sort(),k.slice().sort());
const own=p=>fileURLToPath(new URL('../'+p,import.meta.url));
function row(r,expected){
 assert(r&&typeof r==='object'&&!Array.isArray(r));keys(r,['path','bytes','sha256']);assert.equal(r.path,expected);
 assert(Number.isSafeInteger(r.bytes)&&r.bytes>0&&r.bytes<=MAX_FILE);
 assert.equal(typeof r.sha256,'string');assert.match(r.sha256,/^[0-9a-f]{64}$/);
}
export function validateManifest(v){
 assert(v&&typeof v==='object'&&!Array.isArray(v));
 keys(v,['schema','license','upstream','activation_available','capture_available','publication_available','source_checks_are_native_authority','caps','predecessor','reused_reader','stages','postimages','patches','series','standalone','copying','license_notice','external_api_header','transforms','test_payloads']);
 assert.equal(v.schema,'fe2o3-rocgdb-one-stop-physical-v3-source-v1');assert.equal(v.license,'GPL-3.0-or-later');
 for(const key of ['activation_available','capture_available','publication_available','source_checks_are_native_authority'])assert.equal(v[key],false);
 assert.equal(v.upstream.repository,'https://github.com/ROCm/ROCgdb');assert.equal(v.upstream.commit,'48b1d324e389d2ed5e19822d377ff9050770233d');
 assert.deepEqual(v.caps,{file:MAX_FILE,metadata:MAX_METADATA,combined_selected_stage:MAX_TOTAL,unchanged_parent_v2_selected_stage:2112*1024});
 assert.equal(v.predecessor.directory,'../physical-v2');assert.equal(v.predecessor.stage,STAGES[0]);
 assert(Array.isArray(v.stages)&&v.stages.length===2);
 v.stages.forEach((s,i)=>{keys(s,['name','files']);assert.equal(s.name,STAGES[i]);assert(Array.isArray(s.files)&&s.files.length===PATHS.length);let total=0;
  s.files.forEach((r,j)=>{row(r,PATHS[j]);total+=r.bytes;});assert(total<=MAX_TOTAL,'one combined selected source cap');
 });
 assert.deepEqual(v.postimages,v.stages[1].files);
 assert.equal(v.patches.length,1);const patch=v.patches[0];keys(patch,['path','bytes','sha256','changes']);
 row({path:patch.path,bytes:patch.bytes,sha256:patch.sha256},'patches/0001-disabled-host-entry-maintenance.patch');
 const changed=PATHS.filter((_,j)=>JSON.stringify(v.stages[0].files[j])!==JSON.stringify(v.stages[1].files[j]));
 assert.deepEqual(changed,['gdb/amd-dbgapi-one-stop-native-events-v1.inc','gdb/amd-dbgapi-one-stop-native-io-v1.inc','gdb/amd-dbgapi-one-stop-native-resume-v1.inc','gdb/amd-dbgapi-one-stop-native-v1.h']);
 assert.equal(patch.changes.length,4);
 patch.changes.forEach((r,j)=>{keys(r,['path','preimage','postimage']);assert.equal(r.path,changed[j]);const n=PATHS.indexOf(r.path);assert.deepEqual(r.preimage,v.stages[0].files[n]);assert.deepEqual(r.postimage,v.stages[1].files[n]);});
 assert.equal(v.standalone.length,4);
 v.standalone.forEach((r,i)=>{keys(r,['path','source','bytes','sha256']);assert.equal(r.source,changed[i]);row({path:r.path,bytes:r.bytes,sha256:r.sha256},'src/'+changed[i].slice(4));const actual=v.postimages.find(x=>x.path===r.source);assert.equal(r.bytes,actual.bytes);assert.equal(r.sha256,actual.sha256);});
 assert.equal(digest(JSON.stringify(v)),EXPECTED_MANIFEST,'exact immutable disabled physical-v3 source contract');
 return v;
}
export function checkedText(spec,bytes){
 row(spec,spec.path);assert(Buffer.isBuffer(bytes));assert.equal(bytes.length,spec.bytes);assert.equal(digest(bytes),spec.sha256);
 const text=bytes.toString('utf8');assert(Buffer.from(text).equals(bytes),'strict UTF8');return text;
}
const check=(r,p,cap=MAX_FILE)=>checkedText(r,readBounded(p,cap));
export function manifest(){
 const bytes=readBounded(own('source-manifest.json'),MAX_METADATA),text=bytes.toString('utf8');assert(Buffer.from(text).equals(bytes));
 const v=validateManifest(JSON.parse(text));assert.equal(JSON.stringify(v,null,2)+'\n',text,'canonical metadata encoding');
 for(const r of v.predecessor.files)check(r,own('../physical-v2/'+r.path));
 check(v.reused_reader,own(v.reused_reader.path));
 const parent=parentManifest();assert.equal(parent.schema,'fe2o3-rocgdb-one-stop-physical-v2-source-v1');
 for(const r of parent.postimages)assert.deepEqual(v.stages[0].files.find(x=>x.path===r.path),r);
 for(const r of v.standalone)check({path:r.path,bytes:r.bytes,sha256:r.sha256},own(r.path));
 for(const r of v.patches)check({path:r.path,bytes:r.bytes,sha256:r.sha256},own(r.path));
 for(const r of [v.series,v.copying,v.license_notice,v.transforms,...v.test_payloads])check(r,own(r.path),r.path.endsWith('.json')?MAX_METADATA:MAX_FILE);
 return v;
}
function absolute(p){
 assert(typeof p==='string'&&p.length>1&&p.length<=4096&&!/[\x00-\x1f\x7f]/.test(p)&&path.isAbsolute(p),'explicit bounded absolute source path');
 assert.equal(fs.realpathSync(p),p,'canonical path required');
}
export function falseGates(files){
 for(const [name,fn]of [['activation','selection_available'],['snapshot','snapshot_capture_available'],['publication','snapshot_publication_available']]){
  const leaf=name==='publication'?'gdb/amd-dbgapi-one-stop-publication-v2.h':'gdb/amd-dbgapi-one-stop-'+name+'-v1.h';
  const expected=fn+' () noexcept { return false; }';
  assert.equal(files[leaf].split(expected).length,2,'literal disabled '+name+' gate');
 }
}
export function readSourceStage(root,stage){
 assert(STAGES.includes(stage),'closed explicit source stage');absolute(root);
 const spec=manifest().stages.find(s=>s.name===stage),files={};let total=0;
 for(const r of spec.files){const bytes=readBounded(path.join(root,r.path),MAX_FILE);
  total+=bytes.length;assert(total<=MAX_TOTAL,'one combined 63-row source boundary');files[r.path]=checkedText(r,bytes);
 }
 falseGates(files);
 return Object.freeze({stage,bytes:total,files:Object.freeze(files)});
}
export function finalSource(){return readSourceStage(process.env.FE2O3_ROCGDB_TEST_SOURCE,STAGES[1]).files;}
export function readApiHeader(filename){absolute(filename);const r=manifest().external_api_header;check(r,filename);return Object.freeze({...r});}
export function packageText(name){
 const v=manifest();const r=[...v.test_payloads,v.transforms].find(r=>r.path===name);assert(r,'closed source-test payload');
 return check(r,own(name),name.endsWith('.json')?MAX_METADATA:MAX_FILE);
}
