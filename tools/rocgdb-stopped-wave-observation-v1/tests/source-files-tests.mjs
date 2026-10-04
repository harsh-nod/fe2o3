// SPDX-License-Identifier: GPL-3.0-or-later
// CPU/package-input controls; no subprocess and no filesystem mutation.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {manifest,validateManifest,checkedText,readSourceStage,readApiHeader,MAX_FILE,PATHS} from './source-files.mjs';
const clone=()=>structuredClone(manifest());
function reject(f){const v=clone();validateManifest(v);f(v);assert.throws(()=>validateManifest(v));}
test('exact lifecycle successor and nineteen selected paths',()=>{
 const v=manifest();validateManifest(v);assert.equal(v.postimages.length,19);
 assert.equal(v.patch.changes.length,11);assert.equal(v.standalone.length,5);
});
test('unknown metadata and mismatched upstream refuse',()=>{
 reject(v=>{v.unknown=true});reject(v=>{v.upstream.commit='0'.repeat(40)});
 reject(v=>{v.upstream.repository='https://example.invalid'});reject(v=>{v.schema+='-other'});
});
test('license and native authority cannot change',()=>{
 reject(v=>{v.license='MIT'});reject(v=>{v.source_checks_are_native_authority=true});
 reject(v=>{v.copying.sha256='0'.repeat(64)});reject(v=>{v.copying.bytes--});
});
test('predecessor location stage and bounded file roster stay closed',()=>{
 reject(v=>{v.predecessor.directory='../../foreign'});
 reject(v=>{v.predecessor.stage='native'});reject(v=>{v.predecessor.files.pop()});
 reject(v=>{v.predecessor.files[0].path='../other'});
});
test('source stages cannot be omitted duplicated or reordered',()=>{
 reject(v=>{v.stages.pop()});reject(v=>{v.stages.reverse()});
 reject(v=>{v.stages[0].files.reverse()});reject(v=>{v.stages[1].files.pop()});
});
test('exact added leaves must be absent only at lifecycle',()=>{
 reject(v=>{v.stages[0].files[2]=v.postimages[2]});
 reject(v=>{v.stages[0].files[0]={path:PATHS[0],absent:true}});
 reject(v=>{v.postimages[2]={path:PATHS[2],absent:true}});
});
test('final roster and unchanged context remain exact',()=>{
 reject(v=>{v.postimages.pop()});reject(v=>{v.postimages.reverse()});
 reject(v=>{v.stages[0].files[8].sha256='0'.repeat(64)});
});
test('bounded row shapes reject malformed lengths and hashes',()=>{
 for(const n of [0,-1,MAX_FILE+1,1.5,'10'])reject(v=>{v.postimages[0].bytes=n});
 for(const h of ['', '0'.repeat(63),'A'.repeat(64)])reject(v=>{v.postimages[0].sha256=h});
});
test('patch cannot redirect omit or substitute a preimage',()=>{
 reject(v=>{v.patch.path='../changed'});reject(v=>{v.patch.changes.pop()});
 reject(v=>{v.patch.changes[0].preimage.sha256='0'.repeat(64)});
 reject(v=>{v.patch.changes.reverse()});
});
test('standalone source must match exact corresponding postimage',()=>{
 reject(v=>{v.standalone.pop()});reject(v=>{v.standalone[0].source=PATHS[2]});
 reject(v=>{v.standalone[0].bytes++});reject(v=>{v.standalone[0].sha256='0'.repeat(64)});
});
test('installed API remains an exact external header',()=>{
 reject(v=>{v.external_api_header.path='../api.h'});
 reject(v=>{v.external_api_header.bytes--});reject(v=>{v.external_api_header.sha256='0'.repeat(64)});
});
test('exact text accepted and foreign or truncated bytes denied',()=>{
 const b=Buffer.from('fixture\n'),r={path:'fixture',bytes:b.length,sha256:crypto.createHash('sha256').update(b).digest('hex')};
 assert.equal(checkedText(r,b),'fixture\n');
 assert.throws(()=>checkedText(r,Buffer.from('foreign\n')));
 assert.throws(()=>checkedText(r,b.subarray(0,b.length-1)));
});
test('matching digest does not permit invalid UTF8',()=>{
 const b=Buffer.from([255]),r={path:'fixture',bytes:1,sha256:crypto.createHash('sha256').update(b).digest('hex')};
 assert.throws(()=>checkedText(r,b));
});
test('selected-source input never defaults to a path or stage',()=>{
 for(const p of [undefined,'','relative','/x\0y','x'.repeat(4097)])
  assert.throws(()=>readSourceStage(p,'stopped-wave'));
 assert.throws(()=>readSourceStage('/', 'upstream'));
});
test('API header input is explicit bounded absolute and read only',()=>{
 for(const p of [undefined,'','relative','/x\0y','x'.repeat(4097)])
  assert.throws(()=>readApiHeader(p));
});
test('aggregate source size guard applies to every stage',()=>{
 reject(v=>{for(const row of v.stages[0].files)if(!row.absent)row.bytes=MAX_FILE});
});
