// SPDX-License-Identifier: GPL-3.0-or-later
// Source controls only. Read C++ as text; never import/execute target code.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import test from 'node:test';
const root=path.resolve(process.argv[2]);
const names=['amd-dbgapi-one-stop-native-resume-v1.inc','amd-dbgapi-one-stop-native-v1.h',
'amd-dbgapi-owned-one-stop-v1.h','amd-dbgapi-one-stop-native-io-v1.inc','amd-dbgapi-target.c','target.c','infrun.c'];
const rows=Object.fromEntries(names.map(n=>[n,fs.readFileSync(path.join(root,n),'utf8')]));
function section(s,start,end) { const a=s.indexOf(start);assert(a>=0,start);
const b=s.indexOf(end,a+start.length);assert(b>a,end);return s.slice(a,b); }
function order(s,...parts) { let at=-1;for(const p of parts) { at=s.indexOf(p,at+1);assert(at>=0,p); } }
function validate(r) {
  const resume=r[names[0]],header=r[names[1]];
  const begin=section(resume,'void native_adapter::before_generic_resume','void native_adapter::after_amd_cpu_resume');
  assert.equal(begin.match(/query_stop \(m_gpu_thread,false\)/g)?.length,1);
  const cpu=section(begin,'} else if (m_owner.state ()==phase::current_gpu_stop)', '} else reject');
  order(cpu,'m_command_continue && m_stop_retired','inferior_thread ()==m_host && scope==m_host->ptid',
    'host==1 && gpu==1 && step==0','!m_infrun_trap && !m_infrun_inline',
    'completion_current (false,false,false)','m_command_continue=false; m_stop_retired=false;',
    'query_stop (m_gpu_thread,false)','m_owner.consume_completion',
    'm_snapshot.revoke','advance (completion_edge::cpu_begin)','m_path=path::completion');
  assert(!cpu.includes('&& all'));
  const wave=section(begin,'if (m_path==path::completion)', '} else if (m_callback_progress');
  order(wave,'m_owner.state ()==phase::completion_consumed','!m_command_continue && !m_stop_retired',
    'inferior_thread ()==m_gpu_thread && scope==m_gpu_thread->ptid','host==1 && gpu==1 && step==0',
    'same (m_candidate_stop,m_owner.m_stop)','completion_current (true,false,false)',
    'advance (completion_edge::wave_begin)');
  assert(!wave.includes('query_stop'));
  const current=section(resume,'void native_adapter::completion_current','void native_adapter::infrun_begin');
  for(const s of ['!non_stop && target_is_non_stop_p ()','m_inferior_ref.get ()==m_inferior',
    'm_host_ref.get ()==m_host','m_gpu_thread_ref.get ()==m_gpu_thread',
    'm_loaded_top_ref.get ()==&the_amd_dbgapi_target','m_owner.check_owner (current_identity ())',
    'm_inferior->ptid_thread_map.size ()==2','info ().wave_info_map.size ()==1',
    '++total<=2 && thread==(total==1 ? m_host : m_gpu_thread)',
    '!thread->has_pending_waitstatus ()','m_host->executing ()==cpu_running',
    'm_gpu_thread->executing ()==wave_running','proc->commit_resumed_state==commit_required',
    'cached->first==m_owner.m_stop.wave']) assert(current.includes(s),s);
  const generic=section(resume,'void native_adapter::after_generic_resume','void native_adapter::before_generic_commit');
  order(generic,'advance (completion_edge::cpu_generic)','m_amd_body_returned=false;',
    'return;','advance (completion_edge::wave_generic)','m_generic_returned=true;');
  const commit=section(resume,'void native_adapter::before_generic_commit','void native_adapter::after_amd_commit');
  order(commit,'m_completion.expects (completion_edge::commit_begin)',
    'completion_current (true,true,true)','advance (completion_edge::commit_begin)','m_owner.commit_started');
  const after=section(resume,'void native_adapter::after_generic_commit','void native_adapter::before_finish_step');
  order(after,'advance (completion_edge::commit_generic)','m_owner.commit_returned','arm_terminal_maintenance');
  assert(resume.includes('m_completion.completed () && m_infrun_scope==m_gpu_thread->ptid'));
  assert(header.includes('sizeof (completion_resume_scratch)'));
  assert(r[names[2]].includes('static constexpr std::uint64_t api = 192;'));
  assert(r[names[2]].includes('static constexpr std::uint64_t logical_bytes = 64 * 1024;'));
  assert(r[names[3]].includes('resume-site=%u'));
  const amd=section(r['amd-dbgapi-target.c'],'amd_dbgapi_target::resume (','amd_dbgapi_target::commit_resumed ()');
  order(amd,'beneath ()->resume (scope_ptid, step, signo)','after_amd_cpu_resume ();',
    'if (scope_ptid == inferior_ptid)','after_amd_resume','amd_dbgapi_wave_resume',
    'after_amd_wave_resume (wave_id, status)','AMD_DBGAPI_STATUS_ERROR_INVALID_WAVE_ID');
  order(section(r['target.c'],'target_resume (ptid_t','target_commit_resumed ()'),
    'before_resume','->resume (scope_ptid, step, signal)','after_resume','set_executing');
  order(section(r['infrun.c'],'else if (!non_stop && target_is_non_stop_p ())',
    'finish_state.release ();'),'all_non_exited_threads','proceed_resume_thread_checked (tp)',
    'disable_commit_resumed.reset_and_commit ();');
}
test('paired completion actual source placement',()=>validate(rows));
const mutations=[
  [names[0],'scope==m_gpu_thread->ptid','true'],
  [names[0],'scope==m_host->ptid\n                    && host==1','true\n                    && host==1'],
  [names[0],'host==1 && gpu==1 && step==0','true'],
  [names[0],'same (m_candidate_stop,m_owner.m_stop)','true'],
  [names[0],'m_completion.expects (completion_edge::commit_begin)','true'],
  [names[0],'++total<=2 && thread==(total==1 ? m_host : m_gpu_thread)','true'],
  [names[0],'m_gpu_thread->executing ()==wave_running','true'],
  [names[0],'completion_current (true,true,true)','completion_current_removed ()'],
  [names[0],'m_completion.completed () && m_infrun_scope==m_gpu_thread->ptid','true'],
  [names[3],'resume-site=%u','resume-site-removed'],
  ['amd-dbgapi-target.c','after_amd_cpu_resume','after_amd_cpu_resume_removed'],
  ['amd-dbgapi-target.c','after_amd_wave_resume (wave_id, status)','after_amd_wave_resume_removed ()']
];
for (const [file,from,to] of mutations) test('refuses deleted contract: '+from,()=>{
  validate(rows);assert(rows[file].includes(from));
  assert.throws(()=>validate({...rows,[file]:rows[file].replace(from,to)}));
});
