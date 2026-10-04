// SPDX-License-Identifier: GPL-3.0-or-later
// Inert metadata and byte controls. No external source default or process launch.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import {manifest,validateManifest,checkedText,readSourceStage,readApiHeader,falseGates,packageText,PATHS,STAGES,MAX_FILE,MAX_TOTAL} from './source-files.mjs';
const clone=()=>structuredClone(manifest());
const reject=change=>{const m=clone();change(m);assert.throws(()=>validateManifest(m));};
test('one measured 63-row two-stage boundary with explicit new cap',()=>{
 const m=manifest();assert.equal(PATHS.length,63);assert.equal(new Set(PATHS).size,63);
 assert.deepEqual(PATHS,[...PATHS].sort());assert.equal(m.standalone.length,4);
 assert.deepEqual(m.stages.map(s=>s.files.reduce((a,r)=>a+r.bytes,0)),[2177182,2183132]);
 assert.equal(MAX_TOTAL,2144*1024);assert.equal(MAX_TOTAL-2183132,12324);
 assert.equal(2183132-2112*1024,20444);assert.equal(m.patches[0].changes.length,4);
});
test('the three lifetime headers are in BOTH same cumulative rosters',()=>{
 const m=manifest();for(const p of ['gdb/process-stratum-target.h','gdbsupport/gdb_ref_ptr.h','gdbsupport/refcounted-object.h']){
  assert(PATHS.includes(p));assert(m.stages.every(s=>s.files.find(r=>r.path===p)?.bytes>0));
  reject(v=>{v.stages[1].files=v.stages[1].files.filter(r=>r.path!==p);});
 }
});
test('all three false gates and source nonauthority remain mandatory',()=>{
 for(const k of ['activation_available','capture_available','publication_available','source_checks_are_native_authority'])reject(m=>{m[k]=true;});
 reject(m=>{m.extra=true;});reject(m=>{m.schema+='changed';});
});
test('parent source contract and bounded reader cannot be redirected',()=>{
 reject(m=>{m.predecessor.directory='../foreign';});reject(m=>{m.predecessor.stage='enabled';});
 reject(m=>{m.predecessor.files.pop();});reject(m=>{m.reused_reader.sha256='0'.repeat(64);});
 reject(m=>{m.upstream.commit='0'.repeat(40);});reject(m=>{m.license='MIT';});
});
test('no omitted duplicate reordered absent or invented source rows',()=>{
 reject(m=>{m.stages.reverse();});reject(m=>{m.stages.pop();});
 reject(m=>{m.stages[0].files.reverse();});reject(m=>{m.stages[1].files.pop();});
 reject(m=>{m.stages[1].files[0]=structuredClone(m.stages[1].files[1]);});
 reject(m=>{m.stages[1].files[0]={path:PATHS[0],absent:true};});
 reject(m=>{m.postimages.pop();});reject(m=>{m.stages[1].name='enabled';});
});
test('source caps cannot be changed or bypassed by partition',()=>{
 for(const key of ['file','metadata','combined_selected_stage','unchanged_parent_v2_selected_stage'])reject(m=>{m.caps[key]++;});
 reject(m=>{for(const r of m.stages[1].files)r.bytes=MAX_FILE;});
 reject(m=>{m.caps.overlay_selected_stage=1024;});
});
test('four exact hook changes only; ordinary 59 rows remain identical',()=>{
 const m=manifest(),different=m.stages[0].files.filter((r,i)=>JSON.stringify(r)!==JSON.stringify(m.stages[1].files[i]));
 assert.equal(different.length,4);assert.equal(PATHS.length-different.length,59);
 assert(different.every(r=>r.path.startsWith('gdb/amd-dbgapi-one-stop-native-')));
 reject(v=>{v.patches[0].changes.pop();});reject(v=>{v.patches[0].changes.reverse();});
 reject(v=>{v.stages[1].files.find(r=>r.path==='gdb/target.c').sha256='0'.repeat(64);});
});
test('coordinated metadata/test/source pin changes cannot redefine contract',()=>{
 reject(m=>{m.stages[1].files[0].sha256='0'.repeat(64);m.postimages[0].sha256='0'.repeat(64);});
 reject(m=>{m.standalone.pop();});reject(m=>{m.series.sha256='0'.repeat(64);});
 reject(m=>{m.transforms.sha256='0'.repeat(64);});reject(m=>{m.test_payloads.pop();});
});
test('per-file hash size type and field refusals',()=>{
 for(const bytes of [0,-1,MAX_FILE+1,1.5,'1'])reject(m=>{m.stages[1].files[0].bytes=bytes;});
 for(const sha of ['','a'.repeat(63),'A'.repeat(64)])reject(m=>{m.stages[1].files[0].sha256=sha;});
 reject(m=>{m.stages[1].files[0].extra=true;});
});
test('exact bytes pass; truncation substitution and invalid UTF8 refuse',()=>{
 const b=Buffer.from('fixture\n'),r={path:'fixture',bytes:b.length,sha256:crypto.createHash('sha256').update(b).digest('hex')};
 assert.equal(checkedText(r,b),'fixture\n');assert.throws(()=>checkedText(r,b.subarray(1)));assert.throws(()=>checkedText(r,Buffer.from('foreign\n')));
 const bad=Buffer.from([255]);assert.throws(()=>checkedText({path:'fixture',bytes:1,sha256:crypto.createHash('sha256').update(bad).digest('hex')},bad));
});
test('no default source or unknown stage/API path selector',()=>{
 for(const p of [undefined,'','relative','/x\0y','x'.repeat(4097)]){assert.throws(()=>readSourceStage(p,STAGES[1]));assert.throws(()=>readApiHeader(p));}
 assert.throws(()=>readSourceStage('/','enabled'));assert.throws(()=>readSourceStage('/'));
 assert.throws(()=>packageText('../foreign'));
});
test('literal false gates reject true missing and duplicate forms',()=>{
 const files={'gdb/amd-dbgapi-one-stop-activation-v1.h':'selection_available () noexcept { return false; }',
  'gdb/amd-dbgapi-one-stop-snapshot-v1.h':'snapshot_capture_available () noexcept { return false; }',
  'gdb/amd-dbgapi-one-stop-publication-v2.h':'snapshot_publication_available () noexcept { return false; }'};
 falseGates(files);
 for(const key of Object.keys(files)){
  assert.throws(()=>falseGates({...files,[key]:files[key].replace('false','true')}));
  assert.throws(()=>falseGates({...files,[key]:''}));assert.throws(()=>falseGates({...files,[key]:files[key]+files[key]}));
 }
});
test('public CPU harness changes only two includes and binds exact donor digest',()=>{
 const x=JSON.parse(packageText('tests/helper-extraction.json'));let source=packageText('tests/maintenance.cc');
 assert.equal(x.include_only_transforms.length,2);
 for(const op of [...x.include_only_transforms].reverse()){assert.equal(source.split(op.after).length,2);source=source.replace(op.after,op.before);}
 assert.equal(Buffer.byteLength(source),x.private_fixture.bytes);
 assert.equal(crypto.createHash('sha256').update(source).digest('hex'),x.private_fixture.sha256);
});
test('public verification routes remain read-only and never launch a child',()=>{
 for(const n of ['verify-source.mjs','verify-api-header.mjs','tests/source-files.mjs']){
  const source=fs.readFileSync(new URL('../'+n,import.meta.url),'utf8');
  assert(!/child_process|execFile|spawnSync|writeFile|apply_patch/.test(source));
 }
});
