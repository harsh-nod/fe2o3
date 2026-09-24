// SPDX-License-Identifier: GPL-3.0-or-later
// In-memory package controls. No writes, subprocess, native API or debugger.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {manifest,validateManifest,checkedText,readSourceStage,readApiHeader,PATHS,MAX_FILE,MAX_TOTAL} from './source-files.mjs';
const clone=()=>structuredClone(manifest());
function reject(change){const m=clone();validateManifest(m);change(m);assert.throws(()=>validateManifest(m));}
test('exact disabled R4 successor roster and unchanged predecessor',()=>{
 const m=manifest();assert.equal(PATHS.length,48);assert.equal(m.patch.changes.length,22);
 assert.equal(m.standalone.length,16);assert.equal(m.unchanged_context.length,26);
 assert.equal(m.stages[0].files.filter(r=>r.absent).length,15);
 assert.equal(m.postimages.reduce((n,r)=>n+r.bytes,0),1963940);
});
test('schema unknown fields and source activation refuse',()=>{
 reject(m=>{m.extra=true});reject(m=>{m.schema+='-other'});
 reject(m=>{m.activation_available=true});reject(m=>{m.source_checks_are_native_authority=true});
});
test('upstream license and exact predecessor cannot change',()=>{
 reject(m=>{m.upstream.commit='0'.repeat(40)});reject(m=>{m.upstream.repository='https://example.invalid'});
 reject(m=>{m.license='MIT'});reject(m=>{m.copying.sha256='0'.repeat(64)});
 reject(m=>{m.predecessor.directory='../../other'});reject(m=>{m.predecessor.stage='lifecycle'});
 reject(m=>{m.predecessor.files.pop()});reject(m=>{m.predecessor.files[0].sha256='0'.repeat(64)});
});
test('reused bounded reader is exact and cannot redirect',()=>{
 reject(m=>{m.reused_reader.path='/tmp/other'});reject(m=>{m.reused_reader.sha256='0'.repeat(64)});
});
test('both stages and complete ordered roster required',()=>{
 reject(m=>{m.stages.pop()});reject(m=>{m.stages.reverse()});reject(m=>{m.stages[1].files.pop()});
 reject(m=>{m.stages[0].files.reverse()});reject(m=>{m.postimages.pop()});
});
test('absent new leaves cannot become permissive missing source',()=>{
 reject(m=>{m.stages[0].files[0]=structuredClone(m.postimages[0])});
 reject(m=>{m.stages[1].files[0]={path:PATHS[0],absent:true}});
 reject(m=>{m.stages[0].files[16]={path:PATHS[16],absent:true}});
});
test('bounded row fields lengths and hashes refuse',()=>{
 for(const bytes of [0,-1,MAX_FILE+1,1.5,'1'])reject(m=>{m.stages[1].files[0].bytes=bytes});
 for(const hash of ['', 'a'.repeat(63),'A'.repeat(64)])reject(m=>{m.stages[1].files[0].sha256=hash});
 reject(m=>{m.stages[1].files[0].extra=true});
});
test('whole source payload has a finite aggregate bound',()=>{
 assert(1963940<MAX_TOTAL);reject(m=>{for(const r of m.stages[1].files)r.bytes=MAX_FILE});
});
test('coordinated foreign source pin cannot redefine disabled contract',()=>{
 reject(m=>{m.stages[1].files[0].sha256='0'.repeat(64);m.postimages[0].sha256='0'.repeat(64);
 m.patch.changes[0].postimage.sha256='0'.repeat(64);m.standalone[0].sha256='0'.repeat(64)});
});
test('patch and standalone files cannot be redirected or omitted',()=>{
 reject(m=>{m.patch.path='../foreign'});reject(m=>{m.patch.changes.pop()});reject(m=>{m.patch.bytes--});
 reject(m=>{m.standalone.pop()});reject(m=>{m.standalone[0].source=PATHS[1]});
 reject(m=>{m.series.sha256='0'.repeat(64)});
});
test('unchanged contexts and external API pins remain exact',()=>{
 reject(m=>{m.unchanged_context.pop()});reject(m=>{m.stages[0].files[16].sha256='0'.repeat(64)});
 reject(m=>{m.external_api_header.bytes--});reject(m=>{m.external_api_header.sha256='0'.repeat(64)});
});
test('exact bytes pass but truncation foreign bytes and invalid UTF8 refuse',()=>{
 const b=Buffer.from('fixture\n'),r={path:'fixture',bytes:b.length,sha256:crypto.createHash('sha256').update(b).digest('hex')};
 assert.equal(checkedText(r,b),'fixture\n');assert.throws(()=>checkedText(r,b.subarray(1)));
 assert.throws(()=>checkedText(r,Buffer.from('foreign\n')));
 const bad=Buffer.from([255]);assert.throws(()=>checkedText({path:'fixture',bytes:1,sha256:crypto.createHash('sha256').update(bad).digest('hex')},bad));
});
test('source input and stage never default or admit a relative route',()=>{
 for(const p of [undefined,'','relative','/x\0y','x'.repeat(4097)])assert.throws(()=>readSourceStage(p,'one-stop-disabled-r4'));
 assert.throws(()=>readSourceStage('/','enabled-r5'));assert.throws(()=>readSourceStage('/'));
});
test('external header path is explicit bounded and read only',()=>{
 for(const p of [undefined,'','relative','/x\0y','x'.repeat(4097)])assert.throws(()=>readApiHeader(p));
});
