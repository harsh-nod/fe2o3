// SPDX-License-Identifier: GPL-3.0-or-later
// In-memory fixture controls only; no filesystem mutation or native process.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {manifest,validateManifest,checkedText,MAX_FILE} from './source-files.mjs';
const clone=()=>structuredClone(manifest());
function reject(change){const value=clone();validateManifest(value);change(value);assert.throws(()=>validateManifest(value));}
test('fixed source manifest has the exact four stages and seven final files',()=>validateManifest(manifest()));
test('wrong upstream revision refuses',()=>reject(v=>{v.upstream.commit='a'.repeat(40)}));
test('wrong license or authority claim refuses',()=>{reject(v=>{v.license='MIT'});reject(v=>{v.source_checks_are_native_authority=true});});
test('missing extra or reordered final source roster refuses',()=>{
 reject(v=>{v.postimages.pop()});reject(v=>{v.postimages.push(v.postimages[0])});
 reject(v=>{[v.postimages[0],v.postimages[1]]=[v.postimages[1],v.postimages[0]]});
});
test('stale or missing source stage refuses',()=>{reject(v=>{v.stages.pop()});reject(v=>{v.stages[2].name='native'});});
test('absent final source or invented old-file absence refuses',()=>{
 reject(v=>{v.postimages[0]={path:v.postimages[0].path,absent:true}});
 reject(v=>{v.stages[0].files[2]={path:v.stages[0].files[2].path,absent:true}});
 reject(v=>{v.stages[0].files[0]={...v.postimages[0]}});
});
test('source byte and digest shapes stay bounded',()=>{
 for(const bytes of [0,-1,MAX_FILE+1,1.5,'4'])reject(v=>{v.postimages[0].bytes=bytes});
 for(const sha of ['','a'.repeat(63),'A'.repeat(64)])reject(v=>{v.postimages[0].sha256=sha});
});
test('exact source content passes but substituted bytes refuse',()=>{
 const bytes=Buffer.from('fixture\n'),row={path:'fixture',bytes:bytes.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')};
 assert.equal(checkedText(row,bytes),'fixture\n');
 assert.throws(()=>checkedText(row,Buffer.from('changed\n')));
 assert.throws(()=>checkedText(row,Buffer.from('fixture')));
});
test('invalid UTF-8 cannot become source text even with matching digest',()=>{
 const bytes=Buffer.from([255]),row={path:'fixture',bytes:1,sha256:crypto.createHash('sha256').update(bytes).digest('hex')};
 assert.throws(()=>checkedText(row,bytes));
});

test('patch roster paths counts and stage correspondence are closed',()=>{
 reject(v=>{v.patches.pop()});reject(v=>{v.patches[0].path='../foreign.patch'});
 reject(v=>{v.patches[2].changes[0].preimage.sha256='0'.repeat(64)});
 reject(v=>{v.patches[1].changes.reverse()});
});
test('license identity and size cannot become an omitted or unrelated file',()=>{
 reject(v=>{v.upstream.license_source='COPYING'});
 reject(v=>{v.upstream.license_bytes=MAX_FILE+1});
 reject(v=>{v.upstream.license_sha256='invalid'});
});
