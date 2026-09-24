// SPDX-License-Identifier: MIT OR Apache-2.0
// Inert source-package controls only. Never invokes cargo, the controller or GDB.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {PATHS,MANIFEST_CAP,FILE_CAP,TOTAL_CAP,manifest,verifyPayload,readPackage,readRegular} from './package-files.mjs';
const packageRoot=path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const encode=v=>Buffer.from(JSON.stringify(v,null,2)+'\n');
function fixture(){
 const data=new Map(PATHS.map(p=>[p,Buffer.from('inert source-shaped fixture '+p+'\n')]));
 const value={schema:'fe2o3-gfx950-one-stop-controller-source-v1',license:'MIT OR Apache-2.0',
  runtime_profile:'unbound',rust_files:18,files:PATHS.map(p=>({path:p,bytes:data.get(p).length,sha256:hash(data.get(p))}))};
 return {data,value};
}
function diskFixture(t){
 const root=fs.mkdtempSync(path.join(fs.realpathSync(os.tmpdir()),'one-stop-package-'));
 t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const f=fixture();fs.mkdirSync(root+'/native');
 for(const [p,b]of f.data)fs.writeFileSync(root+'/'+p,b,{flag:'wx'});
 fs.writeFileSync(root+'/source-manifest.json',encode(f.value),{flag:'wx'});
 return{...f,root};
}
test('checked-in selected package bytes verify without runtime claims',()=>{
 const r=readPackage(packageRoot);assert.equal(r.files,22);assert(r.bytes>0&&r.bytes<TOTAL_CAP);
 for(const key of ['complete_tree_verified','source_authenticated','built_binary_verified','startup_qualified','native_authority'])assert.equal(r[key],false);
 assert.equal(r.processes_started,0);assert.equal(r.runtime_profile,'unbound');
});
test('native source is present but exact static activation remains None',()=>{
 const s=fs.readFileSync(packageRoot+'/native/profile.rs','utf8');
 assert(s.includes('const PROFILE: Option<&Profile> = None;'));
 assert(s.includes('PROFILE.ok_or(Refusal::State)'));
 const lib=fs.readFileSync(packageRoot+'/lib.rs','utf8');
 assert(lib.includes('Generic one-stop MI2 protocol library'));
 assert(!lib.includes('This packet provides NO native implementation'));
});
test('separate unpublished workspace and fixed binary do not join parent workspace',()=>{
 const s=fs.readFileSync(packageRoot+'/Cargo.toml','utf8');
 assert(s.includes('publish = false'));assert(s.includes('[workspace]'));
 assert(s.includes('path = "native/main.rs"'));assert(s.includes('name = "fe2o3-private-one-stop-controller"'));
 assert(!s.includes('workspace = true'));assert(!s.includes('build ='));
 const lock=fs.readFileSync(packageRoot+'/Cargo.lock','utf8');assert(lock.includes('name = "fe2o3-private-one-stop-protocol"'));
});
test('selected source roster has exact project and dual license leaves',()=>{
 assert.equal(PATHS.filter(p=>p.endsWith('.rs')).length,18);
 assert(PATHS.includes('LICENSE-MIT'));assert(PATHS.includes('LICENSE-APACHE'));
 assert(!PATHS.some(p=>/\.(inc|cc|h)$/.test(p)));
 assert.equal(manifest(fs.readFileSync(packageRoot+'/source-manifest.json')).license,'MIT OR Apache-2.0');
});
test('wrong domain license profile and source count refuse',()=>{
 for(const [k,v]of [['schema','other'],['license','GPL-3.0-or-later'],['runtime_profile','enabled'],['rust_files',20]]){
  const f=fixture();f.value[k]=v;assert.throws(()=>manifest(encode(f.value)));
 }
});
test('closed keys roster duplicates omission reorder and traversal refuse',()=>{
 for(const change of [
  v=>v.extra=false,v=>v.files.pop(),v=>v.files.reverse(),v=>v.files[1]=v.files[0],
  v=>v.files[0].path='../Cargo.toml',v=>v.files[0].extra=false]){
  const f=fixture();change(f.value);assert.throws(()=>manifest(encode(f.value)));
 }
});
test('file and aggregate limits refuse before payload reads',()=>{
 for(const n of [0,-1,FILE_CAP+1,1.5,Number.MAX_SAFE_INTEGER]){
  const f=fixture();f.value.files[0].bytes=n;assert.throws(()=>manifest(encode(f.value)));
 }
 const f=fixture();for(const row of f.value.files)row.bytes=FILE_CAP;
 let reads=0;assert.throws(()=>verifyPayload(f.value,()=>{reads++;return Buffer.alloc(0)}));assert.equal(reads,0);
});
test('malformed hashes and missing fields refuse before callback',()=>{
 for(const change of [v=>v.files[0].sha256='A'.repeat(64),v=>delete v.files[0].bytes,v=>v.files=[]]){
  const f=fixture();change(f.value);let reads=0;
  assert.throws(()=>verifyPayload(f.value,()=>{reads++;return Buffer.alloc(0)}));assert.equal(reads,0);
 }
});
test('changed payload or byte length cannot match the source pins',()=>{
 const f=fixture();assert.equal(verifyPayload(manifest(encode(f.value)),p=>f.data.get(p)).files,22);
 const p=PATHS[0];for(const b of [Buffer.from('changed'),Buffer.alloc(f.data.get(p).length,0)]){
  const data=new Map(f.data);data.set(p,b);assert.throws(()=>verifyPayload(f.value,n=>data.get(n)));
 }
});
test('UTF8 refusal remains even with matching raw hash',()=>{
 const f=fixture(),b=Buffer.from([0xff,10]);f.data.set(PATHS[0],b);
 f.value.files[0].bytes=b.length;f.value.files[0].sha256=hash(b);
 assert.throws(()=>verifyPayload(f.value,p=>f.data.get(p)));
});
test('manifest encoding denies duplicate keys trailing values and excessive bytes',()=>{
 const f=fixture(),s=encode(f.value).toString();
 for(const b of [Buffer.from(s.replace('"schema":','"schema": "duplicate",\n  "schema":')),
  Buffer.from(s+'{}'),Buffer.from(s.trimEnd()),Buffer.from([0xff]),Buffer.alloc(MANIFEST_CAP+1)]){
  assert.throws(()=>manifest(b));
 }
});
test('inert temporary selected-file package reads without execution',t=>{
 const f=diskFixture(t);assert.equal(readPackage(f.root).files,22);
});
test('noncanonical relative and symlink package roots refuse',t=>{
 const f=diskFixture(t),alias=f.root+'-alias';fs.symlinkSync(f.root,alias);t.after(()=>fs.unlinkSync(alias));
 for(const root of ['.',f.root+'/',f.root+'/../'+path.basename(f.root),alias])assert.throws(()=>readPackage(root));
});
test('symlink source files and parent directories refuse',t=>{
 const f=diskFixture(t),p=f.root+'/Cargo.toml';fs.unlinkSync(p);fs.symlinkSync('Cargo.lock',p);
 assert.throws(()=>readPackage(f.root));
 fs.unlinkSync(p);fs.writeFileSync(p,f.data.get('Cargo.toml'));
 fs.renameSync(f.root+'/native',f.root+'/native-real');fs.symlinkSync('native-real',f.root+'/native');
 assert.throws(()=>readPackage(f.root));
});
test('directory in place of source leaf refuses before read',t=>{
 const f=diskFixture(t),p=f.root+'/Cargo.toml';fs.unlinkSync(p);fs.mkdirSync(p);
 assert.throws(()=>readRegular(f.root,'Cargo.toml',FILE_CAP));
});
test('regular file exact cap and one-short ceiling are enforced',t=>{
 const f=diskFixture(t),b=Buffer.alloc(FILE_CAP,120);fs.writeFileSync(f.root+'/Cargo.toml',b);
 assert.equal(readRegular(f.root,'Cargo.toml',FILE_CAP).length,FILE_CAP);
 assert.throws(()=>readRegular(f.root,'Cargo.toml',FILE_CAP-1));
 fs.appendFileSync(f.root+'/Cargo.toml','x');assert.throws(()=>readRegular(f.root,'Cargo.toml',FILE_CAP));
});
test('reader rejects caller path expansion and invalid allocation cap',t=>{
 const f=diskFixture(t);
 for(const p of ['../Cargo.toml','/etc/passwd','native/../Cargo.toml','native/missing.rs'])
  assert.throws(()=>readRegular(f.root,p,FILE_CAP));
 for(const n of [0,-1,FILE_CAP+1,NaN,Infinity])assert.throws(()=>readRegular(f.root,'Cargo.toml',n));
});
test('truncated source and modified source-manifest refuse',t=>{
 const f=diskFixture(t);fs.writeFileSync(f.root+'/Cargo.toml','x');assert.throws(()=>readPackage(f.root));
 fs.writeFileSync(f.root+'/source-manifest.json','{}\n');assert.throws(()=>readPackage(f.root));
});
test('verification entry points import no child or native launch helper',()=>{
 for(const file of ['verify-source.mjs','tests/package-files.mjs']){
  const s=fs.readFileSync(packageRoot+'/'+file,'utf8');
  assert(!s.includes('child_process'));assert(!s.includes('execFile'));assert(!s.includes('process.env'));
 }
});
test('historical 72 controls are not a new package or native qualification',()=>{
 const v=JSON.parse(fs.readFileSync(packageRoot+'/historical-evidence.json','utf8'));
 assert.equal(v.tests.library,43);assert.equal(v.tests.native_binary_cpu,29);assert.equal(v.tests.total,72);
 assert.equal(v.native_executed,false);assert.equal(v.package_helpers_qualified,false);
 assert.equal(v.runtime_profile,'unbound');assert.equal(v.receipt.sha256,'359e6c322b0a73470d1d77a7e4c9c73d37bcf3171a5f6841b46b1045d01013a8');
});
