// SPDX-License-Identifier: GPL-3.0-or-later
// Read-only CPU fixture input. No subprocess, debugger, attachment or authority.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';

export const COMMIT='48b1d324e389d2ed5e19822d377ff9050770233d';
export const MAX_FILE=256*1024, MAX_TOTAL=1024*1024;
export const PATHS=Object.freeze([
 'gdb/amd-dbgapi-runtime-observation-hooks-v1.inc',
 'gdb/amd-dbgapi-runtime-observation-v1.h',
 'gdb/amd-dbgapi-target.c','gdb/interps.c','gdb/interps.h',
 'gdb/mi/mi-interp.c','gdb/mi/mi-interp.h',
]);
const STAGES=['upstream','native','mi-safe-points','lifecycle'];
const PATCHES=['patches/0001-native-observation.patch','patches/0002-mi-safe-points.patch','patches/0003-lifecycle-causes.patch'];
const digest=b=>crypto.createHash('sha256').update(b).digest('hex');
const keys=(value,expected)=>assert.deepEqual(Object.keys(value).sort(),expected.slice().sort());
function rowShape(row,expected,allowAbsent=false){
 assert(row&&typeof row==='object'&&!Array.isArray(row));
 assert.equal(row.path,expected);
 if(allowAbsent&&row.absent===true){keys(row,['path','absent']);return;}
 keys(row,['path','bytes','sha256']);
 assert(Number.isSafeInteger(row.bytes)&&row.bytes>0&&row.bytes<=MAX_FILE);
 assert.equal(typeof row.sha256,'string');assert.match(row.sha256,/^[0-9a-f]{64}$/);
}
export function validateManifest(value){
 assert(value&&typeof value==='object'&&!Array.isArray(value));
 assert.equal(value.schema,'fe2o3-rocgdb-producer-source-v1');
 assert.equal(value.upstream.repository,'https://github.com/ROCm/ROCgdb');
 assert.equal(value.upstream.commit,COMMIT);
 assert.equal(value.license,'GPL-3.0-or-later');
 assert.equal(value.source_checks_are_native_authority,false);
 assert(Array.isArray(value.postimages)&&value.postimages.length===PATHS.length);
 value.postimages.forEach((row,i)=>rowShape(row,PATHS[i]));
 assert(Array.isArray(value.stages)&&value.stages.length===STAGES.length);
 value.stages.forEach((stage,i)=>{
  keys(stage,['name','files']);assert.equal(stage.name,STAGES[i]);
  assert(Array.isArray(stage.files)&&stage.files.length===PATHS.length);
  stage.files.forEach((row,j)=>{
   if(i===0&&j<2)assert.equal(row.absent,true,'new headers absent upstream');
   rowShape(row,PATHS[j],i===0&&j<2);
  });
 });
 assert.deepEqual(value.stages[3].files,value.postimages);
 assert.equal(value.upstream.license_source,'COPYING3');
 rowShape({path:'COPYING',bytes:value.upstream.license_bytes,sha256:value.upstream.license_sha256},'COPYING');
 assert(Array.isArray(value.patches)&&value.patches.length===PATCHES.length);
 value.patches.forEach((patch,i)=>{
  keys(patch,['path','bytes','sha256','changes']);
  rowShape({path:patch.path,bytes:patch.bytes,sha256:patch.sha256},PATCHES[i]);
  const changed=PATHS.filter((_,j)=>JSON.stringify(value.stages[i].files[j])!==JSON.stringify(value.stages[i+1].files[j]));
  assert(Array.isArray(patch.changes)&&patch.changes.length===changed.length);
  patch.changes.forEach((row,j)=>{
   keys(row,['path','preimage','postimage']);assert.equal(row.path,changed[j]);
   const index=PATHS.indexOf(row.path);
   assert.deepEqual(row.preimage,value.stages[i].files[index]);
   assert.deepEqual(row.postimage,value.stages[i+1].files[index]);
  });
 });
 return value;
}
export function checkedText(row,bytes){
 rowShape(row,row.path);assert(Buffer.isBuffer(bytes));
 assert.equal(bytes.length,row.bytes);assert.equal(digest(bytes),row.sha256);
 const text=bytes.toString('utf8');assert(Buffer.from(text).equals(bytes),'strict UTF-8');
 return text;
}
const stamp=s=>[s.dev,s.ino,s.size,s.mtimeNs,s.ctimeNs,s.mode].map(String);
export function readBounded(file,cap=MAX_FILE){
 assert.equal(fs.realpathSync(file),file,'canonical file path');
 const fd=fs.openSync(file,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW|fs.constants.O_NONBLOCK);
 try{
  const before=fs.fstatSync(fd,{bigint:true});
  assert(before.isFile()&&before.size>0n&&before.size<=BigInt(cap),'regular bounded source');
  const bytes=Buffer.alloc(Number(before.size)+1);let size=0;
  for(;;){const n=fs.readSync(fd,bytes,size,bytes.length-size,null);if(n===0)break;size+=n;assert(size<=Number(before.size),'source grew');}
  assert.equal(size,Number(before.size));assert.deepEqual(stamp(before),stamp(fs.fstatSync(fd,{bigint:true})));
  assert.deepEqual(stamp(before),stamp(fs.statSync(file,{bigint:true})));
  assert.equal(fs.realpathSync(file),file);return bytes.subarray(0,size);
 }finally{fs.closeSync(fd);}
}
export function manifest(){
 const file=fileURLToPath(new URL('../source-manifest.json',import.meta.url));
 const bytes=readBounded(file,32768),text=bytes.toString('utf8');
 assert(Buffer.from(text).equals(bytes),'manifest UTF-8');
 const value=validateManifest(JSON.parse(text));
 for(const name of ['amd-dbgapi-runtime-observation-v1.h','amd-dbgapi-runtime-observation-hooks-v1.inc']){
  const row=value.postimages.find(r=>r.path==='gdb/'+name);
  checkedText(row,readBounded(fileURLToPath(new URL('../src/'+name,import.meta.url))));
 }
 for(const row of value.patches){
  const pin={path:row.path,bytes:row.bytes,sha256:row.sha256};
  checkedText(pin,readBounded(fileURLToPath(new URL('../'+row.path,import.meta.url))));
 }
 checkedText({path:'COPYING',bytes:value.upstream.license_bytes,sha256:value.upstream.license_sha256},
  readBounded(fileURLToPath(new URL('../COPYING',import.meta.url))));
 return value;
}
export function readSourceStage(root,stage='lifecycle'){
 assert(typeof root==='string'&&root.length>1&&root.length<=4096&&!root.includes('\0')&&path.isAbsolute(root),'explicit absolute read-only source root required');
 assert.equal(fs.realpathSync(root),root,'canonical source root');
 assert(STAGES.includes(stage),'closed source stage');
 const spec=manifest().stages.find(s=>s.name===stage),files={};let total=0;
 for(const row of spec.files){
  const file=path.join(root,row.path);
  if(row.absent===true){
   let absent=false;try{fs.lstatSync(file);}catch(error){if(error.code==='ENOENT')absent=true;else throw error;}
   assert(absent,'upstream new file must be absent');continue;
  }
  const bytes=readBounded(file);total+=bytes.length;assert(total<=MAX_TOTAL);
  files[row.path]=checkedText(row,bytes);
 }
 return Object.freeze({stage,bytes:total,files:Object.freeze(files)});
}
export function finalSource(){
 // Read-only placement-test fixture location, never an executable/profile override.
 return readSourceStage(process.env.FE2O3_ROCGDB_TEST_SOURCE,'lifecycle').files;
}
