// SPDX-License-Identifier: GPL-3.0-or-later
// Static contract controls only; no compiler, AMD API or GDB subprocess.
import assert from 'node:assert/strict';
import test from 'node:test';
import {finalSource} from './source-files.mjs';
const files=finalSource();
const read=p=>files[p];
const target=read('gdb/amd-dbgapi-target.c');
const query=read('gdb/amd-dbgapi-stopped-wave-query-v1.inc');
const hooks=read('gdb/amd-dbgapi-runtime-observation-hooks-v1.inc');
const mi=read('gdb/mi/mi-interp.c');
const main=read('gdb/mi/mi-main.c');
const output=read('gdb/mi/amd-stopped-wave-observation-mi-v1.inc');
const ledger=read('gdb/amd-dbgapi-stopped-wave-observation-v1.h');
function between(s,a,b){const i=s.indexOf(a);assert(i>=0,a);const j=s.indexOf(b,i+a.length);assert(j>i,b);return s.slice(i,j);}
function ordered(s,...items){let n=-1;for(const item of items){const next=s.indexOf(item,n+1);assert(next>n,item);n=next;}}
function normalStop(s){const body=between(s,'mi_interp::on_normal_stop (','mi_interp::on_about_to_proceed');
  ordered(body,'gdb_puts ("*stopped"','mi_out_rewind','gdb_flush (this->raw_stdout)',
    'amd_runtime_observation_v1::flush_safe_point','amd_stopped_wave_observation_v1::after_normal_stop');
}
test('MI completion strictly follows complete raw stopped row',()=>normalStop(mi));
test('premature MI observation fails static ordering',()=>{
  const old='  amd_stopped_wave_observation_v1::after_normal_stop\n    (print_frame ? inferior_thread () : nullptr);';
  assert.throws(()=>normalStop(mi.replace(old,'').replace('  gdb_puts ("*stopped"',old+'\n  gdb_puts ("*stopped"')));
});
test('both real command entries invalidate before any dispatch or selection',()=>{
  for(const signature of ['mi_execute_command (const char *cmd, int from_tty)','mi_execute_command (mi_parse *context)']){
    const at=main.indexOf(signature+'\n{');assert(at>=0);
    const prefix=main.slice(at,at+650);
    ordered(prefix,signature,'amd_stopped_wave_observation_v1::change','reason::stop_changed','#endif');
  }
  assert(!main.includes('stopped-wave-read'));
});
test('event staging follows actual existing ACK',()=>{
  const body=between(target,'// Stage only from the actual handled WAVE_STOP','  flush_runtime_observation ();');
  ordered(body,'query_stopped_wave_event_v1','mark_event_processed.finish ()','stopped_wave_observation.stage');
});
test('sole native acknowledgement call remains original helper',()=>{
  assert.equal((hooks.match(/amd_dbgapi_event_processed \(event\)/g)||[]).length,1);
  assert(!query.includes('amd_dbgapi_event_processed'));
  assert(hooks.includes('m_stop_scope.done ()'));
  assert(hooks.includes('runtime_obs::event_ack<native_event_ack_v1> m_ack;'));
});
test('old noqueue explicit WAVE_STOP refusal remains exact',()=>{
  assert(target.includes('if (event_kind != AMD_DBGAPI_EVENT_KIND_CODE_OBJECT_LIST_UPDATED\n      && event_kind != AMD_DBGAPI_EVENT_KIND_RUNTIME)\n    runtime_observation.invalidate (runtime_obs::reason::unsupported_event);'));
});
test('real owner and current GPU thread remain mandatory',()=>{
  for(const text of ['info.process_id.handle == stopped_wave_observation.process ()','ptid_is_gpu (thread->ptid)',
    'thread == inferior_thread ()','thread->ptid == inferior_ptid','!thread->executing ()','!thread->resumed ()',
    'info.inf->process_target ()->find_thread (thread->ptid) == thread','get_amd_dbgapi_inferior_info (thread->inf)'])
    assert(query.includes(text),text);
});
test('native write and resume defenses remain before target access',()=>{
  for(const [a,b,guard] of [
    ['amd_dbgapi_target::resume (','beneath ()->resume','reason::resume_attempt'],
    ['amd_dbgapi_target::store_registers (','if (!ptid_is_gpu','reason::stop_changed'],
    ['amd_dbgapi_target::xfer_partial (','if (!ptid_is_gpu','reason::stop_changed']]){
    const start=target.indexOf(a);assert(start>=0);const section=target.slice(start);
    ordered(section,a,guard,b);
  }
});
test('all typed native identity families queried without register or memory samples',()=>{
  for(const text of ['amd_dbgapi_wave_get_info','amd_dbgapi_workgroup_get_info','amd_dbgapi_dispatch_get_info',
    'amd_dbgapi_queue_get_info','amd_dbgapi_agent_get_info','amd_dbgapi_architecture_get_info'])
    assert(query.includes(text));
  for(const text of ['amd_dbgapi_read_memory','amd_dbgapi_read_register','AMD_DBGAPI_WAVE_INFO_PC',
    'AMD_DBGAPI_WAVE_INFO_EXEC_MASK','AMD_DBGAPI_QUEUE_INFO_ADDRESS','AMD_DBGAPI_DISPATCH_INFO_KERNEL_CODE_ENTRY_ADDRESS'])
    assert(!query.includes(text));
  assert(query.includes('required_identity_unavailable'));
});
test('all output fields are numeric or the three exact diagnostic constants',()=>{
  const strings=[...output.matchAll(/field_string \("([^"]+)", "([^"]+)"\)/g)].map(m=>[m[1],m[2]]);
  assert.deepEqual(strings,[
    ['schema','fe2o3-gfx950-native-stopped-wave-observation-v1'],
    ['profile','fe2o3.gfx950.same-client-stopped-wave-observation.v1'],
    ['queue-provenance','unavailable']]);
  const keys=[...output.matchAll(/field_(?:string|unsigned|signed) \("([^"]+)"/g)].map(m=>m[1]);
  assert.equal(new Set(keys).size,keys.length);
  assert.equal(keys.length,47);
  for(const k of ['wave','workgroup','dispatch','queue','agent','architecture','os-agent','packet','os-queue','stop-generation'])
    assert(keys.includes(k));
});
test('worst numeric MI row is bounded independently of values',()=>{
  const keys=[...output.matchAll(/field_(?:unsigned|signed) \("([^"]+)"/g)].map(m=>m[1]);
  const maximum='=amd-stopped-wave-observation-v1'+keys.map(k=>','+k+'="'+('-'+ '9'.repeat(20))+'"').join('')
    + ',schema="fe2o3-gfx950-native-stopped-wave-observation-v1",profile="fe2o3.gfx950.same-client-stopped-wave-observation.v1",queue-provenance="unavailable"\n';
  assert(Buffer.byteLength(maximum)<=4096);
});
test('fixed storage no allocator or runtime token in new ledger/query leaves',()=>{
  assert(ledger.includes('std::array<record, 64>'));
  assert(ledger.includes('sizeof (record) <= 256'));
  assert(ledger.includes('sizeof (ledger) <= 24 * 1024'));
  assert(!/\b(?:malloc|calloc|realloc|new|std::vector)\s*[<(]/.test(ledger+query));
  assert(!ledger.includes('reset ('));
});
test('full exact requery precedes publication and never renews generation',()=>{
  ordered(query,'auto observed = query_stopped_wave_v1 (*info, thread);',
    'stopped_wave_observation.publish_normal_stop','flush_safe_point ();');
  assert(ledger.includes('static constexpr std::uint64_t generation = 1'));
  assert(ledger.includes('m_pending.stop_generation = 1'));
  assert(ledger.includes('if (stop_started ()) { invalidate (reason::stop_changed); return; }'));
});
test('cross-schema reason mapping is closed at allocation',()=>{
  const names=[...ledger.matchAll(/([a-z_]+) = (\d+)/g)].filter(m=>+m[2]>=20&&+m[2]<=25).map(m=>m[1]);
  assert.deepEqual(names,['resume_attempt','thread_changed','wave_identity','unsupported_profile','stop_changed','required_identity_unavailable']);
});
