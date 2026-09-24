// SPDX-License-Identifier: GPL-3.0-or-later
import assert from 'node:assert/strict';
import {test} from 'node:test';
import {finalSource} from './source-files.mjs';
const files=finalSource();
const source=files['gdb/amd-dbgapi-one-stop-native-owner-v1.inc'];
const header=files['gdb/amd-dbgapi-one-stop-native-v1.h'];
function before(s,a,b){const x=s.indexOf(a),y=s.indexOf(b);assert(x>=0&&y>x, a+' before '+b);}
function validate(s,h){
 const start=s.indexOf('class owned_checkpoint_breakpoint final : public code_breakpoint {');
 const end=s.indexOf('static_assert (sizeof (owned_checkpoint_breakpoint)',start);
 assert(start>=0&&end>start);const c=s.slice(start,end);
 assert(c.includes('owned_checkpoint_breakpoint (struct gdbarch *arch,CORE_ADDR pc)'));
 assert(c.includes(': code_breakpoint (arch,bp_breakpoint)'));
 assert(c.includes('disposition=disp_del;'));
 assert(c.includes('void re_set (program_space *) override {'));
 assert(c.includes('native_adapter::instance ().check_owned_breakpoint_reset (this);'));
 assert(c.includes('void check_status (bpstat *status) override {'));
 assert(c.includes('native_adapter::instance ().checkpoint_hit (this,status);'));
 assert(c.includes('enum print_stop_action print_it (const bpstat *status) const override {'));
 before(c,'adapter.debit (counter::work,64);','adapter.check_owned_breakpoint_reset (this);');
 before(c,'adapter.require (adapter.m_owner.state ()==phase::provisional_checkpoint','struct ui_out *out=current_uiout;');
 before(c,'adapter.check_owned_breakpoint_owner ();','adapter.require (adapter.m_owner.state ()==phase::provisional_checkpoint');
 assert(s.includes('void native_adapter::check_owned_breakpoint_owner () {\n  require (m_owner.check_owner (current_identity ()),failure::owner_changed);\n}'));
 assert(h.includes('void check_owned_breakpoint_owner ();'));
 for(const guard of [
  'inferior_thread ()==adapter.m_host','!adapter.m_host->executing ()',
  '!adapter.m_host->resumed ()','status!=nullptr && status->breakpoint_at==this',
  'status->bp_location_at.get ()==&first_loc ()','status->stop && status->simd_lane_mask==0',
  'type==bp_breakpoint && number<0 && enable_state==bp_enabled',
  '!silent && ignore_count==0 && !commands','cond_string==nullptr && extra_string==nullptr'])
  assert(c.includes(guard),guard);
 for(const row of ['out->field_string ("reason","breakpoint-hit");',
  'out->field_string ("disp","del");','out->field_signed ("bkptno",number);',
  'return PRINT_SRC_AND_LOC;','catch (...) { adapter.invalidate (failure::output); throw; }'])
  assert(c.includes(row),row);
 for(const signature of ['void print_mention () const override {',
  'void print_recreate (struct ui_file *) const override {',
  'int resources_needed (const struct bp_location *) override {']){
  const at=c.indexOf(signature);assert(at>=0);
  const close=c.indexOf('\n  }',at);assert(close>at);
  assert(c.slice(at,close).includes('native_adapter::instance ().reject (failure::command);'));
 }
 assert(!c.includes('locspec->')&&!c.includes('ordinary_breakpoint'));
 assert(s.includes('install_breakpoint (true,std::move (pending),1)'));
 assert(h.includes('check_owned_breakpoint_reset (const owned_checkpoint_breakpoint *);'));
 assert(s.includes('check_owned_breakpoint_reset (const owned_checkpoint_breakpoint *bp)'));
}
test('baseline exact fixed software breakpoint presentation',()=>validate(source,header));
for(const [name,from,to] of [
 ['missing override','enum print_stop_action print_it (const bpstat *status) const override {','enum print_stop_action print_other (const bpstat *status) const {'],
 ['unsigned internal ID','out->field_signed ("bkptno",number);','out->field_unsigned ("bkptno",number);'],
 ['owned location missing','status->bp_location_at.get ()==&first_loc ()','true'],
 ['wrong phase','phase::provisional_checkpoint\n                       &&','phase::current_gpu_stop\n                       &&'],
 ['foreign thread allowed','inferior_thread ()==adapter.m_host','true'],
 ['commands allowed','!silent && ignore_count==0 && !commands','!silent && ignore_count==0'],
 ['work not charged','adapter.debit (counter::work,64);','/* omitted */'],
 ['silent retirement','return PRINT_SRC_AND_LOC;','return PRINT_NOTHING;'],
 ['recreate not closed','// No locspec exists and selected MI admits no save/recreate command.\n    native_adapter::instance ().reject (failure::command);','return;'],
 ['internal numbering lost','install_breakpoint (true,std::move (pending),1)','install_breakpoint (false,std::move (pending),1)'],
 ['reset check lost','native_adapter::instance ().check_owned_breakpoint_reset (this);','/* no check */'],
 ['ordinary inherited','final : public code_breakpoint','final : public ordinary_breakpoint']
]) test(name,()=>{validate(source,header);assert(source.includes(from));const changed=source.replace(from,to);assert.notEqual(changed,source);assert.throws(()=>validate(changed,header));});
