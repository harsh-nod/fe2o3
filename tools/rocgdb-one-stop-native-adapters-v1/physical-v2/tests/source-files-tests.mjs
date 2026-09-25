// SPDX-License-Identifier: GPL-3.0-or-later
// Inert metadata/byte controls only; no subprocess or source activation.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import {manifest,validateManifest,checkedText,readSourceStage,readApiHeader,PATHS,STAGES,MAX_FILE,MAX_TOTAL} from './source-files.mjs';
const clone=()=>structuredClone(manifest());
function reject(change){const m=clone();validateManifest(m);change(m);assert.throws(()=>validateManifest(m));}
test('complete combined three-stage source census, not the old2MiB claim',()=>{
 const m=manifest();assert.equal(PATHS.length,60);assert.equal(m.standalone.length,9);
 assert.deepEqual(m.patches.map(p=>p.changes.length),[7,9]);
 assert.deepEqual(m.stages.map(s=>s.files.filter(r=>!r.absent).length),[56,58,60]);
 assert.deepEqual(m.stages.map(s=>s.files.reduce((n,r)=>n+(r.bytes||0),0)),[2135571,2148853,2162093]);
 assert.equal(MAX_TOTAL,2112*1024);assert.equal(MAX_TOTAL-2162093,595);
 assert(2162093>2*1024*1024);assert.equal(2162093-2*1024*1024,64941);
});
test('allthree false gates and no source authority stay literal',()=>{
 for(const k of ['activation_available','capture_available','publication_available','source_checks_are_native_authority'])reject(m=>{m[k]=true;});
 reject(m=>{m.extra=true;});reject(m=>{m.schema+='other';});
});
test('exact upstream license predecessor and reused reader cannot be redirected',()=>{
 reject(m=>{m.upstream.commit='0'.repeat(40);});reject(m=>{m.license='MIT';});
 reject(m=>{m.predecessor.directory='../other';});reject(m=>{m.predecessor.files.pop();});
 reject(m=>{m.reused_reader.path='/tmp/other';});reject(m=>{m.reused_reader.sha256='0'.repeat(64);});
});
test('three exact stage names and ordered complete rosters are mandatory',()=>{
 reject(m=>{m.stages.pop();});reject(m=>{m.stages.reverse();});reject(m=>{m.stages[1].files.pop();});
 reject(m=>{m.stages[0].files.reverse();});reject(m=>{m.postimages.pop();});
});
test('only exact new leaves are absent and never absent in final stage',()=>{
 const n=PATHS.indexOf('gdb/amd-dbgapi-one-stop-output-v2.h'),c=PATHS.indexOf('gdb/ui.c');assert(n>=0&&c>=0);
 reject(m=>{m.stages[0].files[n]=structuredClone(m.postimages[n]);});
 reject(m=>{m.stages[2].files[n]={path:PATHS[n],absent:true};});
 reject(m=>{m.stages[0].files[c]={path:PATHS[c],absent:true};});
});
test('new source-only aggregate is explicit, fixed and still fail closed',()=>{
 reject(m=>{m.caps.combined_selected_stage=2*1024*1024;});reject(m=>{m.caps.combined_selected_stage++;});
 reject(m=>{m.caps.old_parent_selected_stage++;});reject(m=>{m.caps.file++;});
 reject(m=>{for(const r of m.stages[2].files)r.bytes=MAX_FILE;});
});
test('six actual sink-proof contexts remain within the SAME source roster',()=>{
 const m=manifest();for(const p of ['gdb/ui-file.c','gdb/utils.c','gdb/ui.c','gdb/posix-hdep.c','gdb/ui-out.h','gdb/mi/mi-out.h']){
  assert(PATHS.includes(p));assert(m.stages.every(s=>s.files.find(r=>r.path===p)?.bytes>0));
  reject(v=>{v.stages[2].files=v.stages[2].files.filter(r=>r.path!==p);});
 }
});
test('exact initializer is a changed upstream header, not a standalone vendor copy',()=>{
 const m=manifest(),row=m.postimages.find(r=>r.path==='gdb/mi/mi-interp.h');
 assert.equal(row.bytes,4402);assert.equal(row.sha256,'93eb1f87f6c29f2f64e352af8ff74aa0f55b16c9352989d31a9aee65e2e7fd9d');
 assert(!m.standalone.some(r=>r.source===row.path));
 reject(v=>{v.postimages.find(r=>r.path===row.path).sha256='0'.repeat(64);});
});
test('row fields, cap boundaries and malformed hashes refuse',()=>{
 for(const bytes of [0,-1,MAX_FILE+1,1.5,'1'])reject(m=>{m.stages[2].files[0].bytes=bytes;});
 for(const sha of ['', 'a'.repeat(63),'A'.repeat(64)])reject(m=>{m.stages[2].files[0].sha256=sha;});
 reject(m=>{m.stages[2].files[0].extra=true;});
});
test('coordinated pin changes cannot redefine this immutable disabled contract',()=>{
 reject(m=>{m.stages[2].files[0].sha256='0'.repeat(64);m.postimages[0].sha256='0'.repeat(64);});
 reject(m=>{m.patches[1].changes.pop();});reject(m=>{m.patches[1].path='../foreign';});
 reject(m=>{m.standalone.pop();});reject(m=>{m.series.sha256='0'.repeat(64);});
});
test('exact bytes pass; truncation substitution and invalid UTF8 refuse',()=>{
 const b=Buffer.from('fixture\n'),r={path:'fixture',bytes:b.length,sha256:crypto.createHash('sha256').update(b).digest('hex')};
 assert.equal(checkedText(r,b),'fixture\n');assert.throws(()=>checkedText(r,b.subarray(1)));
 assert.throws(()=>checkedText(r,Buffer.from('foreign\n')));
 const bad=Buffer.from([255]);assert.throws(()=>checkedText({path:'fixture',bytes:1,sha256:crypto.createHash('sha256').update(bad).digest('hex')},bad));
});
test('no implicit source input or arbitrary stage is admitted',()=>{
 for(const p of [undefined,'','relative','/x\0y','x'.repeat(4097)])assert.throws(()=>readSourceStage(p,STAGES[2]));
 assert.throws(()=>readSourceStage('/','enabled'));assert.throws(()=>readSourceStage('/'));
});
test('external API header is exact bounded read-only evidence, not library authority',()=>{
 const m=manifest();assert(m.external_api_header.bytes>0&&m.external_api_header.bytes<=MAX_FILE);
 for(const p of [undefined,'','relative','/x\0y','x'.repeat(4097)])assert.throws(()=>readApiHeader(p));
 reject(v=>{v.external_api_header.sha256='0'.repeat(64);});
});
test('verification entry points contain no execution or source-writing route',()=>{
 for(const n of ['verify-source.mjs','verify-api-header.mjs','tests/source-files.mjs']){
  const s=fs.readFileSync(new URL('../'+n,import.meta.url),'utf8');
  assert(!/child_process|execFile|spawnSync|writeFile|apply_patch/.test(s));
 }
});
