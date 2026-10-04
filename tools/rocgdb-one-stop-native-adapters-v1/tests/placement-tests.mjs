// SPDX-License-Identifier: GPL-3.0-or-later
import assert from 'node:assert/strict';
import test from 'node:test';
import {finalSource} from './source-files.mjs';
const source=finalSource();
const names=['amd-dbgapi-one-stop-activation-v1.h','amd-dbgapi-one-stop-native-owner-v1.inc',
'amd-dbgapi-one-stop-native-events-v1.inc','amd-dbgapi-one-stop-native-resume-v1.inc',
'amd-dbgapi-one-stop-native-io-v1.inc','amd-dbgapi-one-stop-native-query-v1.inc',
'amd-dbgapi-one-stop-native-object-v1.inc','amd-dbgapi-one-stop-native-wrappers-v1.inc',
'amd-dbgapi-target.c','amd-dbgapi-runtime-observation-hooks-v1.inc',
'target.c','infrun.c','mi/mi-interp.c','mi/mi-main.c','solib-rocm.c'];
const original=Object.fromEntries(names.map(n=>[n,source['gdb/'+n]]));
function section(s,start,end){const at=s.indexOf(start);assert(at>=0,start);const stop=s.indexOf(end,at+start.length);assert(stop>at,end);return s.slice(at,stop);}
function order(s,...parts){let at=-1;for(const p of parts){at=s.indexOf(p,at+1);assert(at>=0,p);}}
export function validate(rows){
  assert(rows['amd-dbgapi-one-stop-activation-v1.h'].includes('constexpr bool selection_available () noexcept { return false; }'));
  order(section(rows['amd-dbgapi-one-stop-native-owner-v1.inc'],'void native_adapter::select_at_entry ()','void native_adapter::check_owned_breakpoint_reset'),
    'if (!selection_available ())','error (_("owned one-stop native activation unavailable: compile candidate only"))','reserve_selection','current_executable (); hash_executable ();','bind_selection','install_breakpoint');
  const generic=section(rows['target.c'],'target_resume (ptid_t','/* See target.h.  */');
  order(generic,'before_resume (scope_ptid, step, signal)','target_dcache_invalidate','->resume (scope_ptid, step, signal)','after_resume ()','set_executing');
  order(section(rows['target.c'],'target_commit_resumed ()','/* See target.h.  */'),'before_commit ()','->commit_resumed ()','after_commit ()');
  const run=section(rows['infrun.c'],'const bool one_stop_inline =','/* Resume the inferior.');
  order(run,'selected ()','step_over_info.thread == tp->global_num','step_over_info.aspace','step_over_info.address == regcache_read_pc','!displaced_step_in_progress_thread','infrun_begin','target_resume');
  const finish=section(rows['infrun.c'],'\nstatic int\nfinish_step_over (struct execution_control_state *ecs)\n{','if (!target_is_non_stop_p ())');
  order(finish,'selected ()','step_over_info.thread == ecs->event_thread->global_num','before_finish_step','step_over_info.address','displaced_step_finish','clear_step_over_info');
  order(rows['infrun.c'],'breakpoint_auto_delete (inferior_thread ()->control.stop_bpstat);','amd_owned_one_stop_native_v1::after_auto_delete ();');
  const amd=section(rows['amd-dbgapi-target.c'],'amd_dbgapi_target::resume (','/* Return a string version of RESUME_MODE');
  order(amd,'beneath ()->resume','after_amd_cpu_resume','amd_dbgapi_wave_resume','one_stop_native::native_adapter::instance ().after_amd_wave_resume (wave_id, status);','AMD_DBGAPI_STATUS_ERROR_INVALID_WAVE_ID','after_amd_commit');
  const callback=section(rows['amd-dbgapi-target.c'],'amd_dbgapi_target_breakpoint::check_status','amd_dbgapi_target::thread_alive');
  order(callback,'one_stop_native::native_adapter::instance ().callback_begin\n    (*info, this, breakpoint_id, callback_thread);','amd_dbgapi_report_breakpoint_hit','callback_end','auto ack_status = resume_event.finish ();','static_cast<amd_dbgapi_status_t> (ack_status)');
  const event=section(rows['amd-dbgapi-target.c'],'process_one_event (','/* Return a textual version of KIND.');
  order(event,'one_stop_event_thread = thread','auto acknowledged_status = mark_event_processed.finish ();','wave_ack','static_cast<amd_dbgapi_status_t> (acknowledged_status)');
  const ack=section(rows['amd-dbgapi-runtime-observation-hooks-v1.inc'],'std::int64_t complete (','std::int64_t finish ()');
  assert.equal(ack.match(/finish \(\)/g)?.length,1);
  order(ack,'auto status = finish ();','stopped_obs::completed','return status;');
  const mi=section(rows['mi/mi-interp.c'],'gdb_puts ("*stopped"','void\nmi_interp::');
  order(mi,'mi_out_rewind','gdb_puts ("\\n"','gdb_flush','amd_owned_one_stop_native_v1::after_normal_stop');
  order(section(rows['amd-dbgapi-target.c'],'amd_dbgapi_inferior_exited (','/* inferior_pre_detach'),
    '.disappearing (inf)','invalidate_runtime_observation','detach_amd_dbgapi (inf)');
  order(section(rows['amd-dbgapi-one-stop-native-events-v1.inc'],'void native_adapter::deleting_breakpoint','void native_adapter::after_auto_delete'),
    'deleting_breakpoint (object,number)','m_breakpoint=nullptr');
  order(section(rows['amd-dbgapi-one-stop-native-events-v1.inc'],'void native_adapter::after_auto_delete','void native_adapter::disappearing'),
    'all_breakpoints ()','all_bp_locations ()','retirement_finished','flush ()');
  const resume=rows['amd-dbgapi-one-stop-native-resume-v1.inc'];
  assert(resume.includes('!inferior_thread ()->executing () && !inferior_thread ()->resumed ()'));
  assert(resume.includes('inline_address==m_callback_pc'));
  order(resume,'m_candidate_stop=query_stop (m_gpu_thread,false)','m_owner.consume_completion','m_path=path::completion');
  order(resume,'m_command_continue=false;','inspect_checkpoint (true)','m_owner.consume_publication');
  const io=rows['amd-dbgapi-one-stop-native-io-v1.inc'];
  order(io,'m_owner.request_client_output (bytes)','if (m_owner.m_allocation_empty) return nullptr','void *const result=::malloc (bytes)');
  order(io,'debit (counter::read_calls,1)','debit (counter::read_bytes,bytes)','target_read_memory');
  const solib=section(rows['solib-rocm.c'],'\nstatic void\nrocm_update_solib_list ()\n{','rocm_solib_target_inferior_created');
  order(solib,'if (amd_owned_one_stop_native_v1::selected ())','::selected_object','else','amd_dbgapi_process_code_object_list');
  const object=rows['amd-dbgapi-one-stop-native-object-v1.inc'];
  order(object,'begin_client_empty_objects','m_query_output=true','amd_dbgapi_process_code_object_list','client_empty_objects_returned');
  order(rows['amd-dbgapi-one-stop-native-query-v1.inc'],'AMD_DBGAPI_DISPATCH_INFO_KERNEL_COMPLETION_ADDRESS','completion==cp.u64 (128)');
}
test('actual complete source-placement baseline',()=>validate(original));
const mutations=[
 ['activation','amd-dbgapi-one-stop-activation-v1.h','return false;','return true;'],
 ['selection gate','amd-dbgapi-one-stop-native-owner-v1.inc','if (!selection_available ())','if (false)'],
 ['pre-effect resume','target.c','before_resume (scope_ptid, step, signal)','before_resume_removed ()'],
 ['commit fence','target.c','before_commit ()','before_commit_removed ()'],
 ['inactive inline guard','infrun.c','const bool one_stop_inline = amd_owned_one_stop_native_v1::selected ()','const bool one_stop_inline = true'],
 ['step identity','infrun.c','step_over_info.thread == tp->global_num','true'],
 ['retirement hook','infrun.c','amd_owned_one_stop_native_v1::after_auto_delete ();','/* no post-retirement hook */'],
 ['invalid wave result','amd-dbgapi-target.c','after_amd_wave_resume (wave_id, status)','after_amd_wave_resume_removed ()'],
 ['callback actual origin','amd-dbgapi-target.c','().callback_begin','().callback_begin_removed'],
 ['same sole ACK','amd-dbgapi-runtime-observation-hooks-v1.inc','return status;','return 0;'],
 ['retained exit owner','amd-dbgapi-target.c','.disappearing (inf)','.disappearing_removed (inf)'],
 ['postdelete census','amd-dbgapi-one-stop-native-events-v1.inc','all_bp_locations ()','no_location_census ()'],
 ['pre-resume actual state','amd-dbgapi-one-stop-native-resume-v1.inc','!inferior_thread ()->executing () && !inferior_thread ()->resumed ()','true'],
 ['callback step address','amd-dbgapi-one-stop-native-resume-v1.inc','inline_address==m_callback_pc','true'],
 ['zero output scope','amd-dbgapi-one-stop-native-object-v1.inc','begin_client_empty_objects','unscoped_empty_objects'],
 ['allocator debit','amd-dbgapi-one-stop-native-io-v1.inc','m_owner.request_client_output (bytes)','true'],
 ['actual base query','amd-dbgapi-one-stop-native-query-v1.inc','completion==cp.u64 (128)','completion==cp.u64 (136)'],
 ['selected list replacement','solib-rocm.c','if (amd_owned_one_stop_native_v1::selected ())','if (false)']
];
for(const [name,file,from,to] of mutations)test('source mutation refuses: '+name,()=>{
  validate(original); // No mutant can pass because an unrelated baseline check already fails.
  assert(original[file].includes(from));const changed={...original,[file]:original[file].replace(from,to)};
  assert.throws(()=>validate(changed));
});

test('definition anchors exclude retained forward declarations',()=>{
  validate(original);
  assert(original['infrun.c'].includes('static int finish_step_over (struct execution_control_state *ecs);'));
  assert(original['solib-rocm.c'].includes('static void rocm_update_solib_list ();'));
  const finish=section(original['infrun.c'],'\nstatic int\nfinish_step_over (struct execution_control_state *ecs)\n{','if (!target_is_non_stop_p ())');
  const solib=section(original['solib-rocm.c'],'\nstatic void\nrocm_update_solib_list ()\n{','rocm_solib_target_inferior_created');
  assert(!finish.includes('finish_step_over (struct execution_control_state *ecs);'));
  assert(!solib.includes('static void rocm_update_solib_list ();'));
});
