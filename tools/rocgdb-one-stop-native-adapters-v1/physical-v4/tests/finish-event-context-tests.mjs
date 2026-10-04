// SPDX-License-Identifier: GPL-3.0-or-later
// Source and exact-body fixture checks only; no native process or owner creation.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {finalSource,packageText,manifest,falseGates} from './source-files.mjs';
const files=finalSource(),spec=JSON.parse(packageText('tests/finish-event-context-extraction.json'));
const transforms=JSON.parse(packageText('ordinary-transforms.json'));
const sha=s=>crypto.createHash('sha256').update(s).digest('hex');
const tokens=s=>s.replace(/\/\*[\s\S]*?\*\//g,'').replace(/\/\/[^\n]*/g,'').replace(/\s+/g,'');
function section(s){const a=s.indexOf(spec.start),b=s.indexOf(spec.end);assert(a>=0&&b>a);return s.slice(a,b);}
const resume=files['gdb/amd-dbgapi-one-stop-native-resume-v1.inc'];
const header=files['gdb/amd-dbgapi-one-stop-native-v1.h'];
const body=section(resume);
function old(name){
 let result=files['gdb/'+name];
 for(const op of transforms.operations.filter(x=>x.file===name&&x.stage==='finish-event-context').reverse()){
  assert.equal(result.split(op.after).length,2);result=result.replace(op.after,op.before);
 }
 return result;
}
test('public preimages independently equal the failed runtime method/header owners',()=>{
 for(const row of spec.public_preimages){
  const before=old(row.file);assert.equal(Buffer.byteLength(before),row.bytes);assert.equal(sha(before),row.sha256);
 }
});
test('readable public method has exact reviewed private method tokens',()=>{
 assert.equal(sha(tokens(body)),spec.reviewed_method_token_sha256);
 assert.equal(sha(tokens(header)),spec.reviewed_folded_header_token_sha256);
});
test('all16 CPU mocks compile the exact public body and renamed original body',()=>{
 const expected=spec.fixture_prefix+body+section(old(spec.public_preimages[0].file)).replace('::before_finish_step (','::before_finish_step_legacy (');
 assert.equal(packageText('tests/actual-finish-step-bodies.inc'),expected);
 assert.equal(sha(packageText('tests/finish-event-context.cc')),spec.cpp_fixture_sha256);
});
test('original native predicate remains exact and follows authenticated context loan',()=>{
 const original=section(old(spec.public_preimages[0].file));
 const a=original.indexOf('  require (m_path'),b=original.indexOf('  m_callback_progress=callback_progress::continue_ready;');
 assert(a>=0&&b>a);assert(body.includes(original.slice(a,b)));
 const order=['if (!selected ()) return;','if (m_callback_progress!=callback_progress::step_inflight) return;',
 'debit (counter::work,1);','require (thread!=nullptr','std::optional<scoped_restore_current_thread> event_context;',
 'if (inferior_ptid==null_ptid) {','event_context.emplace ();','switch_to_thread (thread);','require (m_path'];
 let p=-1;for(const x of order){const n=body.indexOf(x);assert(n>p,x);p=n;}
 assert(body.includes('current_inferior ()==m_inferior && thread->inf==m_inferior'));
 assert(body.includes('(inferior_ptid==null_ptid || inferior_thread ()==thread)'));
 assert(!body.includes('dont_restore'));
});
test('fixed guard charge and original work/cap declarations are not bypassed',()=>{
 assert.equal((header.match(/\+ sizeof \(std::optional<scoped_restore_current_thread>\)/g)||[]).length,2);
 assert(header.includes('<= limits::logical_bytes'));
 assert(header.includes('constant_and_scratch = 4096;'));
 assert.equal((body.match(/debit \(counter::work,1\);/g)||[]).length,1);
 const limits=files['gdb/amd-dbgapi-owned-one-stop-v1.h'];
 assert.equal(sha(limits),spec.owner_core_sha256);
 assert.equal(manifest().caps.combined_selected_stage,2195456);
});
test('missing predicate, guard or debit mutants cannot match reviewed source',()=>{
 for(const [needle,replacement] of [
  ['thread==m_host','true'],
  ['current_inferior ()==m_inferior','true'],
  ['inferior_ptid==null_ptid || inferior_thread ()==thread','true'],
  ['debit (counter::work,1);',''],
  ['event_context.emplace ();',''],
  ['switch_to_thread (thread);',''],
  ['had_inline && trap_expected','true'],
  ['inline_address==m_callback_pc','true'],
  ['m_owner.check_owner (current_identity ())','true'],
 ]){
  assert(body.includes(needle));assert.notEqual(sha(tokens(body.replace(needle,replacement))),spec.reviewed_method_token_sha256);
 }
});
test('all three native profiles remain disabled and source checks grant no authority',()=>{
 falseGates(files);
 for(const key of ['activation_available','capture_available','publication_available','source_checks_are_native_authority'])assert.equal(manifest()[key],false);
 const input=resume.slice(resume.indexOf('void native_adapter::mi_input ('),resume.indexOf('void native_adapter::nested_mi'));
 assert.equal(sha(input),spec.unchanged_mi_input_sha256);
});
