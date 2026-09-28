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
 const value={schema:'fe2o3-gfx950-one-stop-controller-source-v2',license:'MIT OR Apache-2.0',
  runtime_profile:'unbound',rust_files:23,files:PATHS.map(p=>({path:p,bytes:data.get(p).length,sha256:hash(data.get(p))}))};
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
 const r=readPackage(packageRoot);assert.equal(r.files,27);assert(r.bytes>0&&r.bytes<TOTAL_CAP);
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
 assert.equal(PATHS.filter(p=>p.endsWith('.rs')).length,23);
 assert(PATHS.includes('LICENSE-MIT'));assert(PATHS.includes('LICENSE-APACHE'));
 assert(!PATHS.some(p=>/\.(inc|cc|h)$/.test(p)));
 assert.equal(manifest(fs.readFileSync(packageRoot+'/source-manifest.json')).license,'MIT OR Apache-2.0');
});
test('wrong domain license profile and source count refuse',()=>{
 for(const [k,v]of [['schema','other'],['license','GPL-3.0-or-later'],['runtime_profile','enabled'],['rust_files',18]]){
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
 const f=fixture();assert.equal(verifyPayload(manifest(encode(f.value)),p=>f.data.get(p)).files,27);
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
 const f=diskFixture(t);assert.equal(readPackage(f.root).files,27);
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
test('historical 58+30 V2 and one separate V1 control do not transfer acceptance',()=>{
 const v=JSON.parse(fs.readFileSync(packageRoot+'/historical-evidence.json','utf8'));
 assert.equal(v.tests.v2_library,58);assert.equal(v.tests.v2_native_binary_cpu,30);
 assert.equal(v.tests.separate_v1_compatibility,1);assert.equal(v.tests.total,89);
 assert.equal(v.native_executed,false);assert.equal(v.package_helpers_qualified,false);
 assert.equal(v.runtime_profile,'unbound');
 assert.equal(v.receipt.sha256,'9b7183bd3a522667461310734213ca0ac283760aa1d9167db51101d430c63a20');
});
test('V2 domains and MI-only PC relation are retained, not a V1 option',()=>{
 const s=fs.readFileSync(packageRoot+'/controller.rs','utf8');
 assert(s.includes('fe2o3-gfx950-one-stop-controller-observation-v2'));
 assert(s.includes('fe2o3-owned-one-stop-v2'));
 const t=fs.readFileSync(packageRoot+'/physical_snapshot_v2_tests.rs','utf8');
 assert(t.includes('actual_mi_gpu_pc_must_match_unchanged_physical_row_before_continue'));
 const n=fs.readFileSync(packageRoot+'/native/publication.rs','utf8');
 assert(n.includes('fe2o3-one-stop-native-peer-observation-v2'));
});
test('unavailable values remain typed and no published runtime binding exists',()=>{
 const p=fs.readFileSync(packageRoot+'/physical_snapshot_v2.rs','utf8');
 assert(p.includes('UnavailableV2')&&p.includes('self.unavailable.is_none()'));
 const b=JSON.parse(fs.readFileSync(packageRoot+'/runtime-bindings.json','utf8'));
 assert.deepEqual(Object.keys(b).sort(),['controller','debugger','family','native_attempt','startup','target'].sort());
 for(const value of Object.values(b))assert.equal(value,null);
});

test('setup diagnostics preserve the first failure without becoming runtime binding',()=>{
 const wire=fs.readFileSync(packageRoot+'/native/wire.rs','utf8');
 const main=fs.readFileSync(packageRoot+'/native/main.rs','utf8');
 const start=wire.indexOf('if let Err(refusal) = setup {');
 const stop=wire.indexOf('        Ok(result)',start);
 assert(start>=0&&stop>start);
 const failure=wire.slice(start,stop);
 assert(failure.indexOf('let diagnostic = trace.freeze(result.sent);')>=0);
 assert(failure.indexOf('let diagnostic = trace.freeze(result.sent);')<failure.indexOf('let cleanup = result.teardown();'));
 assert(main.includes('e.refusal, e.cleanup, e.diagnostic'));
 assert(!failure.includes('eof =')&&!failure.includes('may_have_inferior ='));
});
test('diagnostics leaf has fixed setup stages and no acquisition interface',()=>{
 const text=fs.readFileSync(packageRoot+'/native/setup_diagnostic.rs','utf8');
 for(const stage of ['ExecutableLink','ExecutableMetadata','ExecutableRecheck','ScopeMember','Cmdline'])
  assert(text.includes(stage));
 assert(text.includes('ChildWaitObservation::Unobserved'));
 assert(text.includes('initial_stamp: None'));
 assert(text.includes('readers_started: 0'));
 assert(!text.includes('std::process')&&!text.includes('std::fs'));
 assert(!text.includes('Command::')&&!text.includes('process.env'));
});
test('old count or omitted diagnostic source refuses before any payload read',()=>{
 for(const change of [
  v=>v.rust_files=20,
  v=>v.files=v.files.filter(row=>row.path!=='native/setup_diagnostic.rs')
 ]){
  const f=fixture();change(f.value);let reads=0;
  assert.throws(()=>verifyPayload(f.value,()=>{reads++;return Buffer.alloc(0)}));
  assert.equal(reads,0);
 }
});

test('failure capture is inserted only after first refusal and before unchanged cleanup',()=>{
 const wire=fs.readFileSync(packageRoot+'/native/wire.rs','utf8');
 const start=wire.indexOf('if let Err(refusal) = setup {'),stop=wire.indexOf('        Ok(result)',start);
 const failure=wire.slice(start,stop);
 assert(failure.indexOf('let diagnostic = trace.freeze(result.sent);')<failure.indexOf('FailureDiagnostic::observe('));
 assert(failure.indexOf('FailureDiagnostic::observe(')<failure.indexOf('let cleanup = result.teardown();'));
 assert(failure.includes('result.readers.is_empty()')&&failure.includes('result.input.is_none()')&&failure.includes('result.sent == 0'));
 assert(!failure.includes('eof =')&&!failure.includes('may_have_inferior ='));
});
test('failure capture keeps bounded owned-pipe diagnostics separate from admission',()=>{
 const s=fs.readFileSync(packageRoot+'/native/failure_diagnostic.rs','utf8');
 assert(s.includes('const RETAIN: usize = 256;')&&s.includes('const READ_CALLS: u8 = 4;'));
 assert(s.includes('child.try_wait()')&&s.includes('OFlags::NONBLOCK'));
 assert(s.includes('PipeState::OwnershipNotIntact')&&s.includes('Restore::Error'));
 assert(s.includes('diagnostic_only=true'));
 assert(!s.includes('Command::')&&!s.includes('thread::')&&!s.includes('fs::File')&&!s.includes('/proc/'));
 const main=fs.readFileSync(packageRoot+'/native/main.rs','utf8');
 assert(main.includes('failure_observation={}')&&main.includes('e.failure_observation'));
});
test('previous count or omitted failure capture source refuses before reads',()=>{
 for(const change of [v=>v.rust_files=21,v=>v.files=v.files.filter(p=>p.path!=='native/failure_diagnostic.rs')]){
  const f=fixture();change(f.value);let reads=0;
  assert.throws(()=>verifyPayload(f.value,()=>{reads++;return Buffer.alloc(0)}));
  assert.equal(reads,0);
 }
});

test('initial argv readiness has a closed bounded source seam and later ownership stays one-shot',()=>{
 const s=fs.readFileSync(packageRoot+'/native/argv_readiness.rs','utf8');
 assert(s.includes('const ATTEMPTS: u8 = 16;')&&s.includes('const MAX_CMDLINE: usize = 2048;'));
 const compact=s.replace(/\s+/g,'');
 assert(compact.includes('letsame=raw==expected;')&&compact.includes('if!raw.is_empty(){returntrace.step(SetupStage::Cmdline,||Err(Refusal::Changed));}'));
 assert(s.includes('CmdlineReadyLimit')&&s.includes('source.yield_once();'));
 assert(!s.includes('Command::')&&!s.includes('std::process')&&!s.includes('std::fs')&&!s.includes('/proc/'));
 const w=fs.readFileSync(packageRoot+'/native/wire.rs','utf8');
 const start=w.indexOf('    fn owner_current('),end=w.indexOf('    fn receive_until(',start);
 assert(start>=0&&end>start);const later=w.slice(start,end);
 assert(!later.includes('argv_readiness')&&!later.includes('yield'));
 assert(later.includes('custody::read_bounded(format!("/proc/{}/cmdline", self.child.id()), 2048)? != self.argv'));
});
test('initial readiness adapter retains original clock custody and complete scope guards',()=>{
 const w=fs.readFileSync(packageRoot+'/native/wire.rs','utf8');
 const start=w.indexOf("impl argv_readiness::Source for InitialArgv<'_>"),end=w.indexOf("impl<'a> NativePeer<'a>",start);
 assert(start>=0&&end>start);const adapter=w.slice(start,end);
 for(const text of ['self.clock.check()','self.scope.current(self.clock)?','self.debugger.check(self.child, self.executable)?','self.scope.member(self.child.id())','argv_readiness::MAX_CMDLINE'])assert(adapter.includes(text));
 const readiness=w.indexOf('argv_readiness::initial('),stdin=w.indexOf('SetupStage::TakeStdin');
 assert(readiness>=0&&readiness<stdin);
 const diagnostic=fs.readFileSync(packageRoot+'/native/setup_diagnostic.rs','utf8');
 for(const name of ['cmdline_attempts','cmdline_empty','cmdline_identity_checks','cmdline_yields'])assert(diagnostic.includes(name));
});
test('previous inventory or omitted readiness module refuses before payload reads',()=>{
 for(const change of [v=>v.rust_files=22,v=>v.files=v.files.filter(p=>p.path!=='native/argv_readiness.rs')]){
  const f=fixture();change(f.value);let reads=0;
  assert.throws(()=>verifyPayload(f.value,()=>{reads++;return Buffer.alloc(0)}));assert.equal(reads,0);
 }
});
