import {readLoadedReviewFixtures} from './loaded-fixture-reader.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import crypto from 'node:crypto';
import {PROFILE} from './loaded-profile-binding.mjs';
import {reviewLoadedStartup,reviewUnqualifiedStartup} from './loaded-profile.mjs';
const {profile_records:records}=readLoadedReviewFixtures();
const by=new Map(records.map(r=>[r.role,r.bytes]));
const values={};
for(const [name,role]of Object.entries(PROFILE.valueRoles))values[name]=PROFILE.textRoles.includes(name)?by.get(role).toString('utf8'):JSON.parse(by.get(role));
const clone=()=>structuredClone(values);
const encode=v=>Buffer.from(JSON.stringify(v));
const synthetic=v=>reviewUnqualifiedStartup(encode(v));
function syncObservation(v){v.collectorFrame='FE2O3_CUSTOM_STARTUP_V1 '+JSON.stringify(v.observation)+'\n';}
test('complete actual historical buffers derive every loaded role, without runtime authority',()=>{
 const out=reviewLoadedStartup(records);
 assert.equal(out.qualified_retained_input_bytes,true);
 assert.deepEqual(out.counts,{files:193,present:170,absent:23,bytes:398626885,initial_modules:104,mapped_elf_paths:50,newly_observed_files:123,static_candidates_not_observed:37});
 assert.equal(out.rows.length,193);assert.equal(out.modules.length,216);
 assert.equal(out.named_aliases.length,1);assert.equal(out.collector_added_modules.length,8);
 assert.ok(out.rows.every(r=>r.references.length>0));
 assert.ok(out.rows.filter(r=>!r.observation.exists).every(r=>r.classification==='absent-cache-path'));
 assert.equal(out.failed_attempt.full_gate_passed,false);
 for(const n of ['complete_import_history','cache_execution_provenance','runtime_acceptance','physical_capture','gpu_dispatch'])assert.equal(out.limitations[n],false);
 assert.equal(out.original_selected_roles,1024);assert.equal(out.historical_duties_changed,false);
 assert.equal(out.physical_files_reread,false);assert.equal(out.live_activation,false);
 assert.ok(Object.isFrozen(out)&&Object.isFrozen(out.rows[0].references));
});
test('pure semantic entry is deterministic and can never qualify bytes',()=>{
 const a=synthetic(clone()),b=synthetic(clone());
 assert.deepEqual(a,b);assert.equal(a.qualified_retained_input_bytes,false);
 assert.equal(a.source_authority,'none');
 assert.equal(a.rows.some(r=>r.classification==='cache-path-presence-not-execution'),true);
});
test('caller mutation cannot change a completed immutable profile',()=>{
 const v=clone(),out=synthetic(v),before=JSON.stringify(out);
 v.observation.files[0].sha256='0'.repeat(64);v.summary.newly_observed_files.length=0;
 assert.equal(JSON.stringify(out),before);
 assert.throws(()=>{out.rows.pop();});
});
for(const [name,mutate]of [
 ['omit complete input',a=>a.pop()],
 ['duplicate complete input',a=>a[1]=a[0]],
 ['reorder complete inputs',a=>[a[0],a[1]]=[a[1],a[0]]],
 ['extra complete input',a=>a.push(a[0])],
 ['record additional key',a=>a[0]={...a[0],accepted:true}],
 ['role substitution',a=>a[0]={...a[0],role:'other'}],
 ['not a complete Buffer',a=>a[0]={...a[0],bytes:a[0].bytes.toString()}],
 ['truncated input',a=>a[0]={...a[0],bytes:a[0].bytes.subarray(1)}],
 ['trailing input byte',a=>a[0]={...a[0],bytes:Buffer.concat([a[0].bytes,Buffer.from(' ')])}],
 ['same-size payload mutation',a=>{const b=Buffer.from(a[0].bytes);b[1]^=1;a[0]={...a[0],bytes:b};}],
 ['historical failure omission',a=>a.splice(3,1)],
 ['collector source omission',a=>a.splice(29,1)],
 ['last historical source omission',a=>a.splice(66,1)],
 ['outer gate raw receipt omission',a=>a.splice(67,1)],
 ['outer request substitution',a=>a[68]=a[67]],
 ['actual build-pins omission',a=>a.pop()],
])test('qualified admission refuses '+name,()=>{const a=records.slice();mutate(a);assert.throws(()=>reviewLoadedStartup(a));});
const negatives=[
 ['file omitted',v=>v.observation.files.pop()],
 ['file duplicated',v=>v.observation.files.push(v.observation.files[0])],
 ['file reordered',v=>v.observation.files.reverse()],
 ['file path traversal',v=>v.observation.files[0].path='/usr/lib/../tmp/bad'],
 ['file deleted spelling',v=>v.observation.files[0].path+=' (deleted)'],
 ['file nonregular',v=>v.observation.files[0].mode=0o040755],
 ['file excessive bytes',v=>v.observation.files[0].bytes=256*1024*1024+1],
 ['file malformed digest',v=>v.observation.files[0].sha256='a'.repeat(63)],
 ['file malformed inode',v=>v.observation.files[0].ino='01'],
 ['file post identity mismatch',v=>v.post.files[0].ino='9'],
 ['post read count wrong',v=>v.post.read_accounting.files--],
 ['post EOF reserve omitted',v=>v.post.read_accounting.reserved_bytes--],
 ['post changed to prelaunch admission',v=>v.post.not_prelaunch_admission=false],
 ['alias target mismatch',v=>v.observation.files.find(f=>f.path!==f.realpath).realpath='/usr/lib/python3.12/sitecustomize.py'],
 ['alias named pin changed',v=>v.observation.files.find(f=>f.path!==f.realpath).sha256='0'.repeat(64)],
 ['absent file hash invented',v=>v.observation.files.find(f=>!f.exists).sha256='0'.repeat(64)],
 ['absent cache promoted present',v=>v.observation.files.find(f=>!f.exists).exists=true],
 ['initial module omitted',v=>v.observation.initial_modules.pop()],
 ['initial module duplicate',v=>v.observation.initial_modules.push(v.observation.initial_modules[0])],
 ['initial module metadata drift',v=>v.observation.initial_modules.find(r=>r.name==='gdb').loader_type='other'],
 ['collector module omission',v=>v.observation.collection_modules.pop()],
 ['custom package inherited installed path',v=>v.observation.collection_modules.find(r=>r.name==='gdb').file='/opt/rocm-7.2.1/share/rocgdb/python/gdb/__init__.py'],
 ['custom search prefix changed',v=>v.observation.sys_path[0]='/usr/lib/python3.12'],
 ['custom cache unrelated source',v=>v.observation.collection_modules.find(r=>r.name==='gdb').cached='/usr/lib/python3.12/__pycache__/os.cpython-312.pyc'],
 ['module path missing observed pin',v=>v.observation.collection_modules.find(r=>r.name==='json').file='/usr/lib/python3.12/never.py'],
 ['module unknown kind',v=>v.observation.collection_modules[0].kind='unknown'],
 ['map unterminated',v=>v.observation.initial_maps=v.observation.initial_maps.trimEnd()],
 ['map anonymous unknown spelling',v=>v.observation.initial_maps+='1-2 r--p 00000000 00:00 0 invalid\n'],
 ['mapped inode changed',v=>v.observation.collection_maps=v.observation.collection_maps.replace(/ ([0-9]+) +\//,' 9 /')],
 ['final mapped roster omitted',v=>v.observation.final_maps=''],
 ['observed byte count false',v=>v.observation.file_bytes_hashed--],
 ['static post drift',v=>v.candidatesAfter.files[0].sha256='0'.repeat(64)],
 ['static file roster omission',v=>{v.candidatesBefore.files.pop();v.candidatesAfter=structuredClone(v.candidatesBefore);}],
 ['static not-loaded limitation removed',v=>{v.candidatesBefore.static_not_loaded=false;v.candidatesAfter=structuredClone(v.candidatesBefore);}],
 ['summary new files silently omitted',v=>v.summary.newly_observed_files.pop()],
 ['summary unobserved static silently omitted',v=>v.summary.static_candidates_not_observed.pop()],
 ['summary collector attribution changed',v=>v.summary.collector_added_modules.push('sys')],
 ['complete import history promoted',v=>v.summary.complete_import_history=true],
 ['cache execution promoted',v=>v.summary.cache_execution_provenance=true],
 ['observation no-history limitation removed',v=>v.observation.startup_snapshot_not_import_history=false],
 ['observation cache provenance limitation removed',v=>v.observation.cache_path_not_execution_provenance=false],
 ['runtime acceptance promoted',v=>v.observation.runtime_acceptance=true],
 ['physical capture promoted',v=>v.observation.physical_capture=true],
 ['GPU dispatch promoted',v=>v.observation.gpu_dispatch=true],
 ['inferior activated',v=>v.observation.inferior_pids=[1]],
 ['inferior objfile loaded',v=>v.observation.objfile_count=1],
 ['effective Python writes enabled',v=>v.observation.flags.dont_write_bytecode=0],
 ['environment key appended',v=>v.observation.environment_keys.push('PYTHONPATH')],
 ['configured Python runtime changed',v=>v.observation.python_executable='/opt/venv/bin/python'],
 ['family cleanup false',v=>v.family.cleanup_complete=false],
 ['family ECHILD false',v=>v.family.wait_echild=false],
 ['family controller PID drift',v=>v.family.controller_pid++],
 ['family stream hash mismatch',v=>v.family.mi2_startup.stdout_sha256='0'.repeat(64)],
 ['raw MI2 trailing byte',v=>v.controllerStdout+='x'],
 ['collector frame trailing byte',v=>v.collectorFrame+='x'],
 ['unexpected stderr',v=>v.controllerStderr+='x'],
 ['outer cleanup false',v=>v.outer.exact_service_cgroup_empty=false],
 ['manager invocation drift',v=>v.managerBefore.InvocationID='0'.repeat(32)],
 ['manager memory cap changed',v=>v.managerBefore.MemoryMax='infinity'],
 ['manager final loaded',v=>v.managerAfter.LoadState='loaded'],
 ['release owner changed',v=>v.releaseText+='x'],
 ['failed attempt silently promoted',v=>v.failure.full_gate_passed=true],
 ['failure reason rewritten',v=>v.failure.post_census_reason='none'],
 ['marker runtime promoted',v=>v.marker.runtime_acceptance=true],
 ['marker pins detached',v=>v.marker.build_pins_sha256='0'.repeat(64)],
 ['root readback absent count wrong',v=>v.readback.loaded.absent--],
 ['root gate digest detached',v=>v.readback.gate.sha256='0'.repeat(64)],
 ['root historical duties omitted',v=>v.gateRequest.inputs.pop()],
 ['gate full request mismatch',v=>v.gate.configuration.timeout_ms--],
 ['gate after input changed',v=>v.gate.inputs_after[0].ino='9'],
 ['gate authority promoted',v=>v.gate.authority='runtime'],
 ['gate tools after changed',v=>v.gate.tools_after[0].sha256='0'.repeat(64)],
 ['gate stderr invented',v=>v.gateStderr='x'],
 ['gate raw output omitted',v=>v.gateStdout=''],
 ['source pin omitted',v=>v.sourceAfter.build.files.pop()],
 ['generated Python changed',v=>v.artifactsAfter.data.files[0].sha256='0'.repeat(64)],
 ['readback records omitted',v=>v.readback.records.pop()],
 ['census zero reference false',v=>v.censusAfter.rows[0].references=1],
 ['census writer exclusion promoted',v=>v.censusAfter.writer_exclusion_proved=true],
 ['census full-helper cleanup promoted',v=>v.censusAfter.privileged_reader.whole_helper_family_cleanup_proved=true],
 ['extra envelope field',v=>v.permission=true],
];
for(const[name,mutate]of negatives)test('semantic review refuses '+name,()=>{const v=clone();mutate(v);assert.throws(()=>synthetic(v));});
test('malformed and excessive pure JSON refused before semantic processing',()=>{
 for(const b of [Buffer.from('{'),Buffer.from([0xff]),Buffer.from('null'),Buffer.alloc(16*1024*1024+1)])assert.throws(()=>reviewUnqualifiedStartup(b));
 assert.throws(()=>reviewUnqualifiedStartup(Buffer.from('{"__proto__":{}}')));
});

for(let i=0;i<PROFILE.records.length;i++)test('complete pin mutation refuses source/data role '+PROFILE.records[i].role,()=>{
 const a=records.slice(),b=Buffer.from(a[i].bytes);
 if(b.length){b[b.length-1]^=1;a[i]={...a[i],bytes:b};}else a[i]={...a[i],bytes:Buffer.from('x')};
 assert.throws(()=>reviewLoadedStartup(a));
});
