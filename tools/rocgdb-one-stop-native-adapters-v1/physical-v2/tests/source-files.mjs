// SPDX-License-Identifier: GPL-3.0-or-later
// Read-only selected source. No subprocess, source activation or native authority.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {readBounded} from '../../../rocgdb-runtime-observation-v1/tests/source-files.mjs';
export {readBounded};
// Versioned combined source-only bound. Old V1 and ALL native caps unchanged.
export const MAX_FILE=512*1024, MAX_TOTAL=2112*1024, MAX_METADATA=64*1024;
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
  "gdb/utils.c"
]);
export const STAGES=Object.freeze(['one-stop-disabled-r4','physical-snapshot-disabled-v1','physical-publication-disabled-v2']);
const EXPECTED_MANIFEST='4cc36a83a976284a0fdd2cca5dd5a22b94b34b61631c093d8a269fcf63efcaa4';
const digest=b=>crypto.createHash('sha256').update(b).digest('hex');
const keys=(v,k)=>assert.deepEqual(Object.keys(v).sort(),k.slice().sort());
const own=p=>fileURLToPath(new URL('../'+p,import.meta.url));
function row(r,expected,absent=false){
 assert(r&&typeof r==='object'&&!Array.isArray(r));assert.equal(r.path,expected);
 if(absent&&r.absent===true){keys(r,['path','absent']);return;}
 keys(r,['path','bytes','sha256']);assert(Number.isSafeInteger(r.bytes)&&r.bytes>0&&r.bytes<=MAX_FILE);
 assert.equal(typeof r.sha256,'string');assert.match(r.sha256,/^[0-9a-f]{64}$/);
}
export function validateManifest(v){
 assert(v&&typeof v==='object'&&!Array.isArray(v));
 keys(v,['schema','license','upstream','activation_available','capture_available','publication_available','source_checks_are_native_authority','caps','predecessor','reused_reader','stages','postimages','patches','series','standalone','copying','external_api_header']);
 assert.equal(v.schema,'fe2o3-rocgdb-one-stop-physical-v2-source-v1');assert.equal(v.license,'GPL-3.0-or-later');
 for(const flag of ['activation_available','capture_available','publication_available','source_checks_are_native_authority'])assert.equal(v[flag],false);
 assert.equal(v.upstream.repository,'https://github.com/ROCm/ROCgdb');assert.equal(v.upstream.commit,'48b1d324e389d2ed5e19822d377ff9050770233d');
 assert.deepEqual(v.caps,{file:MAX_FILE,metadata:MAX_METADATA,combined_selected_stage:MAX_TOTAL,old_parent_selected_stage:2*1024*1024});
 assert.equal(v.predecessor.directory,'..');assert.equal(v.predecessor.stage,STAGES[0]);
 assert(Array.isArray(v.stages)&&v.stages.length===3);
 v.stages.forEach((s,i)=>{keys(s,['name','files']);assert.equal(s.name,STAGES[i]);assert(Array.isArray(s.files)&&s.files.length===PATHS.length);let total=0;
  s.files.forEach((r,j)=>{row(r,PATHS[j],i<2);if(!r.absent)total+=r.bytes;});assert(total<=MAX_TOTAL,'combined selected source cap');
 });
 assert.deepEqual(v.postimages,v.stages[2].files);
 assert.equal(v.patches.length,2);
 v.patches.forEach((p,i)=>{
  keys(p,['path','bytes','sha256','changes']);row({path:p.path,bytes:p.bytes,sha256:p.sha256},p.path);
  const changed=PATHS.filter((_,j)=>JSON.stringify(v.stages[i].files[j])!==JSON.stringify(v.stages[i+1].files[j]));
  assert.equal(p.changes.length,changed.length);
  p.changes.forEach((r,j)=>{keys(r,['path','preimage','postimage']);assert.equal(r.path,changed[j]);const n=PATHS.indexOf(r.path);assert.deepEqual(r.preimage,v.stages[i].files[n]);assert.deepEqual(r.postimage,v.stages[i+1].files[n]);});
 });
 assert.equal(digest(JSON.stringify(v)),EXPECTED_MANIFEST,'exact immutable disabled physical-V2 source contract');
 return v;
}
export function checkedText(spec,bytes){
 row(spec,spec.path);assert(Buffer.isBuffer(bytes));assert.equal(bytes.length,spec.bytes);assert.equal(digest(bytes),spec.sha256);
 const text=bytes.toString('utf8');assert(Buffer.from(text).equals(bytes),'strict UTF8');return text;
}
const check=(r,p)=>checkedText(r,readBounded(p,MAX_FILE));
export function manifest(){
 const bytes=readBounded(own('source-manifest.json'),MAX_METADATA),text=bytes.toString('utf8');assert(Buffer.from(text).equals(bytes));
 const v=validateManifest(JSON.parse(text));assert.equal(JSON.stringify(v,null,2)+'\n',text,'canonical metadata encoding');
 for(const r of v.predecessor.files)check(r,own('../'+r.path));
 check(v.reused_reader,own(v.reused_reader.path));
 const parent=JSON.parse(check(v.predecessor.files[0],own('../source-manifest.json')));
 assert.equal(parent.schema,'fe2o3-rocgdb-one-stop-disabled-source-v1');
 for(const r of parent.postimages)assert.deepEqual(v.stages[0].files.find(x=>x.path===r.path),r);
 for(const r of v.standalone)check({path:r.path,bytes:r.bytes,sha256:r.sha256},own(r.path));
 for(const r of v.patches)check({path:r.path,bytes:r.bytes,sha256:r.sha256},own(r.path));
 check(v.series,own(v.series.path));check(v.copying,own('COPYING'));return v;
}
function absolute(p){
 assert(typeof p==='string'&&p.length>1&&p.length<=4096&&!/[\x00-\x1f\x7f]/.test(p)&&path.isAbsolute(p),'explicit bounded absolute source path');
 assert.equal(fs.realpathSync(p),p,'canonical path required');
}
export function readSourceStage(root,stage){
 assert(STAGES.includes(stage),'closed source stage');absolute(root);
 const spec=manifest().stages.find(s=>s.name===stage),files={};let total=0;
 for(const r of spec.files){const filename=path.join(root,r.path);
  if(r.absent){let missing=false;try{fs.lstatSync(filename);}catch(e){if(e.code==='ENOENT')missing=true;else throw e;}assert(missing,'new leaf must be absent');continue;}
  const bytes=readBounded(filename,MAX_FILE);total+=bytes.length;assert(total<=MAX_TOTAL,'one combined source boundary');files[r.path]=checkedText(r,bytes);
 }
 return Object.freeze({stage,bytes:total,files:Object.freeze(files)});
}
export function finalSource(){return readSourceStage(process.env.FE2O3_ROCGDB_TEST_SOURCE,STAGES[2]).files;}
export function readApiHeader(filename){absolute(filename);const r=manifest().external_api_header;check(r,filename);return Object.freeze({...r});}
