// SPDX-License-Identifier: GPL-3.0-or-later
// Selected source only. No subprocess, writes, debugger, authority or activation.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {readBounded} from '../../rocgdb-runtime-observation-v1/tests/source-files.mjs';
export {readBounded};
export const MAX_FILE=512*1024, MAX_TOTAL=2*1024*1024, MAX_METADATA=64*1024;
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
  "gdb/amd-dbgapi-one-stop-native-v1.h",
  "gdb/amd-dbgapi-one-stop-native-wrappers-v1.inc",
  "gdb/amd-dbgapi-one-stop-observation-v1.h",
  "gdb/amd-dbgapi-one-stop-profile-v1.h",
  "gdb/amd-dbgapi-one-stop-sha256-v1.h",
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
  "gdb/minsyms.h",
  "gdb/observable.h",
  "gdb/progspace.h",
  "gdb/regcache.h",
  "gdb/solib-rocm.c",
  "gdb/symfile.h",
  "gdb/symtab.h",
  "gdb/target.c",
  "gdb/target.h",
  "gdb/ui-out.c",
  "gdb/ui.h"
]);
export const STAGES=Object.freeze(['stopped-wave','one-stop-disabled-r4']);
const EXPECTED_MANIFEST='4a1c4db3c620c52233863bf978d731811cad8bf0fd65a53f695dc149a5dee4ff';

const digest=b=>crypto.createHash('sha256').update(b).digest('hex');
const keys=(value,expected)=>assert.deepEqual(Object.keys(value).sort(),expected.slice().sort());
const own=p=>fileURLToPath(new URL('../'+p,import.meta.url));
function row(row,expected,absent=false){
 assert(row&&typeof row==='object'&&!Array.isArray(row));assert.equal(row.path,expected);
 if(absent&&row.absent===true){keys(row,['path','absent']);return;}
 keys(row,['path','bytes','sha256']);
 assert(Number.isSafeInteger(row.bytes)&&row.bytes>0&&row.bytes<=MAX_FILE);
 assert.equal(typeof row.sha256,'string');assert.match(row.sha256,/^[0-9a-f]{64}$/);
}
export function validateManifest(value){
 assert(value&&typeof value==='object'&&!Array.isArray(value));
 keys(value,['schema','license','upstream','activation_available','source_checks_are_native_authority',
  'predecessor','reused_reader','stages','postimages','unchanged_context','patch','series','standalone','copying','external_api_header']);
 assert.equal(value.schema,'fe2o3-rocgdb-one-stop-disabled-source-v1');
 assert.equal(value.license,'GPL-3.0-or-later');assert.equal(value.activation_available,false);
 assert.equal(value.source_checks_are_native_authority,false);
 assert.equal(value.upstream.repository,'https://github.com/ROCm/ROCgdb');
 assert.equal(value.upstream.commit,'48b1d324e389d2ed5e19822d377ff9050770233d');
 assert.equal(value.predecessor.directory,'../rocgdb-stopped-wave-observation-v1');
 assert.equal(value.predecessor.stage,'stopped-wave');
 assert(Array.isArray(value.stages)&&value.stages.length===2);
 value.stages.forEach((stage,i)=>{
  keys(stage,['name','files']);assert.equal(stage.name,STAGES[i]);
  assert(Array.isArray(stage.files)&&stage.files.length===PATHS.length);let total=0;
  stage.files.forEach((r,j)=>{row(r,PATHS[j],i===0);if(!r.absent)total+=r.bytes;});
  assert(total<=MAX_TOTAL,'selected-source aggregate bound');
 });
 assert.deepEqual(value.postimages,value.stages[1].files);
 // All exact stage, absence, predecessor, patch and context pins are closed.
 // This is an integrity selector, never a source/native permission constructor.
 assert.equal(digest(JSON.stringify(value)),EXPECTED_MANIFEST,'exact immutable R4 source contract');
 return value;
}
export function checkedText(spec,bytes){
 row(spec,spec.path);assert(Buffer.isBuffer(bytes));assert.equal(bytes.length,spec.bytes);
 assert.equal(digest(bytes),spec.sha256);const text=bytes.toString('utf8');
 assert(Buffer.from(text).equals(bytes),'strict UTF-8');return text;
}
function checkFile(spec,filename){return checkedText(spec,readBounded(filename,MAX_FILE));}
export function manifest(){
 const bytes=readBounded(own('source-manifest.json'),MAX_METADATA),text=bytes.toString('utf8');
 assert(Buffer.from(text).equals(bytes),'manifest UTF-8');
 const value=validateManifest(JSON.parse(text));
 for(const r of value.predecessor.files)checkFile(r,own(value.predecessor.directory+'/'+r.path));
 checkFile(value.reused_reader,own(value.reused_reader.path));
 const predecessor=JSON.parse(checkFile(value.predecessor.files[0],own(value.predecessor.directory+'/source-manifest.json')));
 assert.equal(predecessor.schema,'fe2o3-rocgdb-stopped-wave-source-v1');
 for(const r of predecessor.postimages)assert.deepEqual(value.stages[0].files.find(x=>x.path===r.path),r);
 for(const r of value.standalone)checkFile({path:r.path,bytes:r.bytes,sha256:r.sha256},own(r.path));
 checkFile({path:value.patch.path,bytes:value.patch.bytes,sha256:value.patch.sha256},own(value.patch.path));
 checkFile(value.series,own(value.series.path));checkFile(value.copying,own('COPYING'));
 return value;
}
function absolute(p){
 assert(typeof p==='string'&&p.length>1&&p.length<=4096&&!p.includes('\0')&&path.isAbsolute(p),'explicit bounded absolute path required');
 assert.equal(fs.realpathSync(p),p,'canonical path required');
}
export function readSourceStage(root,stage){
 // No default input or success-by-skipping absent environment variables.
 assert(STAGES.includes(stage),'closed source stage');absolute(root);
 const spec=manifest().stages.find(x=>x.name===stage),files={};let total=0;
 for(const r of spec.files){
  const filename=path.join(root,r.path);
  if(r.absent){
   let missing=false;try{fs.lstatSync(filename);}catch(e){if(e.code==='ENOENT')missing=true;else throw e;}
   assert(missing,'new R4 source must be absent at predecessor stage');continue;
  }
  const bytes=readBounded(filename,MAX_FILE);total+=bytes.length;assert(total<=MAX_TOTAL);
  files[r.path]=checkedText(r,bytes);
 }
 return Object.freeze({stage,bytes:total,files:Object.freeze(files)});
}
export function finalSource(){
 return readSourceStage(process.env.FE2O3_ROCGDB_TEST_SOURCE,'one-stop-disabled-r4').files;
}
export function readApiHeader(filename){
 absolute(filename);const r=manifest().external_api_header;checkFile(r,filename);
 return Object.freeze({...r});
}
