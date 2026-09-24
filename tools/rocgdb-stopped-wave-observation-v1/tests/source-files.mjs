// SPDX-License-Identifier: GPL-3.0-or-later
// Bounded read-only selected-source input; never launches checkout code.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
import {readBounded} from '../../rocgdb-runtime-observation-v1/tests/source-files.mjs';
export {readBounded};
export const COMMIT='48b1d324e389d2ed5e19822d377ff9050770233d';
export const MAX_FILE=512*1024, MAX_TOTAL=2*1024*1024;
export const PATHS=Object.freeze([
 'gdb/amd-dbgapi-runtime-observation-hooks-v1.inc',
 'gdb/amd-dbgapi-runtime-observation-v1.h',
 'gdb/amd-dbgapi-stopped-wave-observation-hooks-v1.inc',
 'gdb/amd-dbgapi-stopped-wave-observation-v1.h',
 'gdb/amd-dbgapi-stopped-wave-query-v1.inc',
 'gdb/amd-dbgapi-target.c','gdb/amd-dbgapi-target.h','gdb/gdbthread.h',
 'gdb/infrun.c','gdb/interps.c','gdb/interps.h',
 'gdb/mi/amd-stopped-wave-observation-mi-v1.inc',
 'gdb/mi/mi-console.c','gdb/mi/mi-interp.c','gdb/mi/mi-interp.h',
 'gdb/mi/mi-main.c','gdb/mi/mi-out.c','gdb/observable.h','gdb/ui-out.c',
]);
const ADDED=[2,3,4,11];
const CHANGED=[0,2,3,4,5,9,10,11,13,14,15];
const STANDALONE=[0,2,3,4,11];
const PRIOR=['source-manifest.json','tests/source-files.mjs',
 'src/amd-dbgapi-runtime-observation-v1.h','patches/0001-native-observation.patch',
 'patches/0002-mi-safe-points.patch','patches/0003-lifecycle-causes.patch','patches/series'];
const digest=b=>crypto.createHash('sha256').update(b).digest('hex');
const keys=(v,n)=>assert.deepEqual(Object.keys(v).sort(),n.slice().sort());
function shape(row,expected,absent=false){
 assert(row&&typeof row==='object'&&!Array.isArray(row));
 assert.equal(row.path,expected);
 if(absent){keys(row,['path','absent']);assert.equal(row.absent,true);return;}
 keys(row,['path','bytes','sha256']);
 assert(Number.isSafeInteger(row.bytes)&&row.bytes>0&&row.bytes<=MAX_FILE);
 assert.equal(typeof row.sha256,'string');assert.match(row.sha256,/^[0-9a-f]{64}$/);
}
export function validateManifest(v){
 keys(v,['schema','upstream','license','source_checks_are_native_authority','predecessor',
  'stages','postimages','patch','standalone','copying','external_api_header']);
 assert.equal(v.schema,'fe2o3-rocgdb-stopped-wave-source-v1');
 keys(v.upstream,['repository','commit']);
 assert.equal(v.upstream.repository,'https://github.com/ROCm/ROCgdb');
 assert.equal(v.upstream.commit,COMMIT);assert.equal(v.license,'GPL-3.0-or-later');
 assert.equal(v.source_checks_are_native_authority,false);
 keys(v.predecessor,['directory','stage','files']);
 assert.equal(v.predecessor.directory,'../rocgdb-runtime-observation-v1');
 assert.equal(v.predecessor.stage,'lifecycle');
 assert.equal(v.predecessor.files.length,PRIOR.length);
 v.predecessor.files.forEach((r,i)=>shape(r,PRIOR[i]));
 assert.equal(v.stages.length,2);
 v.stages.forEach((s,i)=>{
  keys(s,['name','files']);assert.equal(s.name,['lifecycle','stopped-wave'][i]);
  assert.equal(s.files.length,PATHS.length);
  s.files.forEach((r,j)=>shape(r,PATHS[j],i===0&&ADDED.includes(j)));
  assert(s.files.reduce((n,r)=>n+(r.bytes??0),0)<=MAX_TOTAL);
 });
 assert.equal(v.postimages.length,PATHS.length);
 v.postimages.forEach((r,i)=>shape(r,PATHS[i]));
 assert.deepEqual(v.postimages,v.stages[1].files);
 keys(v.patch,['path','bytes','sha256','changes']);
 shape({path:v.patch.path,bytes:v.patch.bytes,sha256:v.patch.sha256},'patches/0001-stopped-wave-observation.patch');
 assert.equal(v.patch.changes.length,CHANGED.length);
 for(let i=0;i<PATHS.length;i++){
  const pre=v.stages[0].files[i],post=v.stages[1].files[i];
  if(!CHANGED.includes(i))assert.deepEqual(pre,post,'unchanged context remains exact');
 }
 v.patch.changes.forEach((c,i)=>{
  const n=CHANGED[i];keys(c,['path','preimage','postimage']);assert.equal(c.path,PATHS[n]);
  assert.deepEqual(c.preimage,v.stages[0].files[n]);assert.deepEqual(c.postimage,v.postimages[n]);
  assert.notDeepEqual(c.preimage,c.postimage);
 });
 assert.equal(v.standalone.length,STANDALONE.length);
 v.standalone.forEach((r,i)=>{
  const n=STANDALONE[i],p=v.postimages[n];keys(r,['source','path','bytes','sha256']);
  assert.equal(r.source,p.path);
  shape({path:r.path,bytes:r.bytes,sha256:r.sha256},'src/'+path.posix.basename(p.path));
  assert.equal(r.bytes,p.bytes);assert.equal(r.sha256,p.sha256);
 });
 shape(v.copying,'COPYING');
 assert.equal(v.copying.bytes,35147);
 assert.equal(v.copying.sha256,'8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903');
 shape(v.external_api_header,'amd-dbgapi/amd-dbgapi.h');
 assert.equal(v.external_api_header.bytes,312312);
 assert.equal(v.external_api_header.sha256,'2d0f9629299ecd8c0e72ff292f127573db499a3ab559ebe40ae66d366bde176a');
 return v;
}
export function checkedText(row,bytes){
 shape(row,row.path);assert(Buffer.isBuffer(bytes));assert.equal(bytes.length,row.bytes);
 assert.equal(digest(bytes),row.sha256);
 const s=bytes.toString('utf8');assert(Buffer.from(s).equals(bytes),'strict UTF-8');return s;
}
const local=p=>fileURLToPath(new URL('../'+p,import.meta.url));
export function manifest(){
 const raw=readBounded(local('source-manifest.json'),32768);
 const text=raw.toString('utf8');assert(Buffer.from(text).equals(raw));
 const v=validateManifest(JSON.parse(text));
 // Reused read helper and CPU fixture dependency remain exact predecessor bytes.
 for(const row of v.predecessor.files)
  checkedText(row,readBounded(local('../rocgdb-runtime-observation-v1/'+row.path),MAX_FILE));
 const prior=JSON.parse(readBounded(local('../rocgdb-runtime-observation-v1/source-manifest.json'),32768).toString('utf8'));
 assert.equal(prior.upstream.commit,COMMIT);
 for(const row of prior.postimages)
  assert.deepEqual(v.stages[0].files.find(r=>r.path===row.path),row);
 for(const row of v.standalone)
  checkedText({path:row.path,bytes:row.bytes,sha256:row.sha256},readBounded(local(row.path),MAX_FILE));
 checkedText({path:v.patch.path,bytes:v.patch.bytes,sha256:v.patch.sha256},
  readBounded(local(v.patch.path),MAX_FILE));
 assert.equal(readBounded(local('patches/series'),4096).toString('utf8'),'0001-stopped-wave-observation.patch\n');
 checkedText(v.copying,readBounded(local('COPYING'),MAX_FILE));
 return v;
}
export function readSourceStage(root,stage){
 assert(typeof root==='string'&&root.length>1&&root.length<=4096&&!root.includes('\0')&&path.isAbsolute(root));
 assert.equal(fs.realpathSync(root),root,'canonical read-only source root');
 assert(['lifecycle','stopped-wave'].includes(stage),'closed selected source stage');
 const v=manifest(),rows=v.stages.find(s=>s.name===stage).files,files={};let bytes=0;
 for(const row of rows){
  const file=path.join(root,row.path);
  if(row.absent===true){
   let absent=false;try{fs.lstatSync(file);}catch(e){if(e.code==='ENOENT')absent=true;else throw e;}
   assert(absent,'new leaf must be absent in lifecycle base');continue;
  }
  const b=readBounded(file,MAX_FILE);bytes+=b.length;assert(bytes<=MAX_TOTAL);
  files[row.path]=checkedText(row,b);
 }
 return Object.freeze({stage,bytes,files:Object.freeze(files)});
}
export function finalSource(){
 return readSourceStage(process.env.FE2O3_ROCGDB_STOPPED_WAVE_TEST_SOURCE,'stopped-wave').files;
}
export function readApiHeader(file){
 assert(typeof file==='string'&&file.length>1&&file.length<=4096&&!file.includes('\0')&&path.isAbsolute(file));
 const row=manifest().external_api_header;
 checkedText(row,readBounded(file,MAX_FILE));return row;
}
