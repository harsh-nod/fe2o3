/* SPDX-License-Identifier: GPL-3.0-or-later
   The generic fence runs before dcache invalidation and top_target()->resume.
   The selected path consumes native state BEFORE any host or wave resume. */
namespace amd_owned_one_stop_v1 {
void native_adapter::infrun_begin (thread_info *thread,ptid_t scope,bool step,gdb_signal signal,
                                  bool trap,bool inline_step,bool software,bool user_step) {
  if (!selected ()) return;
  require (!m_infrun_origin && m_path==path::none && thread!=nullptr
           && thread==inferior_thread () && (thread==m_host || thread==m_gpu_thread)
           && m_owner.check_owner (current_identity ()),failure::resume_scope);
  m_infrun_origin=true; m_infrun_scope=scope; m_infrun_step=step; m_infrun_signal=signal;
  m_infrun_trap=trap; m_infrun_inline=inline_step; m_infrun_software=software;
  m_infrun_user_step=user_step;
}
void native_adapter::infrun_end () noexcept { m_infrun_origin=false; }
void native_adapter::before_generic_resume (ptid_t scope,int step,gdb_signal signal) {
  if (!selected ()) return;
  require (m_infrun_origin && m_path==path::none && !m_generic_returned
           && !m_generic_commit && m_infrun_scope==scope && m_infrun_step==(step!=0)
           && m_infrun_signal==signal && signal==GDB_SIGNAL_0
           && !m_infrun_software && !m_infrun_user_step
           && !inferior_thread ()->executing () && !inferior_thread ()->resumed ()
           && m_owner.check_owner (current_identity ()),failure::resume_scope);
  std::size_t host=0,gpu=0; thread_roster (host,gpu);
  const bool all=scope==minus_one_ptid || scope==ptid_t (m_inferior->pid);
  const bool host_only=scope==m_host->ptid || (all && host==1 && gpu==0);
  if (m_callback_progress==callback_progress::step_ready) {
    require (!m_command_continue && inferior_thread ()==m_host
             && scope==m_host->ptid && step==1 && m_infrun_trap && m_infrun_inline
             && regcache_read_pc (get_thread_regcache (m_host))==m_callback_pc
             && (m_owner.state ()==phase::host_running
                 || m_owner.state ()==phase::awaiting_target_completion),failure::resume_scope);
    m_callback_progress=callback_progress::step_inflight;
    m_path=path::callback_step; // Consume before actual hardware host step.
  } else if (m_callback_progress==callback_progress::continue_ready) {
    require (!m_command_continue && inferior_thread ()==m_host && host_only
             && step==0 && !m_infrun_trap && !m_infrun_inline,failure::resume_scope);
    m_path=path::callback_continue; // Exactly one continuation from actual step completion.
  } else if (m_owner.state ()==phase::armed_entry) {
    require (m_command_continue && inferior_thread ()==m_host && host==1 && gpu==0
             && host_only && step==0 && !m_infrun_trap && !m_infrun_inline
             && regcache_read_pc (get_thread_regcache (m_host))==m_entry_pc,
             failure::resume_scope);
    m_command_continue=false;
    require (m_owner.entry_continuation_consumed (current_identity ()),failure::resume_scope);
    m_path=path::entry;
  } else if (m_owner.state ()==phase::retired_checkpoint) {
    require (m_command_continue && inferior_thread ()==m_host && host_only
             && step==0 && !m_infrun_trap && !m_infrun_inline
             && m_breakpoint==nullptr,failure::resume_scope);
    m_command_continue=false;
    inspect_checkpoint (true); // Actual PC, bytes, queue, file/root; same ledger.
    require (m_owner.consume_publication (current_identity (),m_candidate,host,gpu,step,signal),
             failure::resume_scope);
    m_path=path::publication;
  } else if (m_owner.state ()==phase::current_gpu_stop) {
    require (m_command_continue && m_stop_retired && inferior_thread ()==m_gpu_thread
             && all && step==0 && !m_infrun_trap && !m_infrun_inline,
             failure::resume_scope);
    m_command_continue=false; m_stop_retired=false;
    // Frontend state is now RUNNING intent. Native STOP/PC and actual
    // !executing/!resumed still hold before the target effect.
    m_candidate_stop=query_stop (m_gpu_thread,false);
    require (m_owner.consume_completion (current_identity (),m_candidate_stop,host,gpu,step,signal),
             failure::resume_scope);
    m_path=path::completion;
  } else reject (failure::resume_scope);
}
void native_adapter::after_amd_cpu_resume () {
  if (!selected ()) return;
  require (m_path==path::publication || m_path==path::completion
           || m_path==path::callback_step || m_path==path::callback_continue,
           failure::resume_scope);
  if (m_path==path::publication || m_path==path::completion)
    require (m_owner.cpu_resume_returned (),failure::native_result);
}
void native_adapter::after_amd_wave_resume (amd_dbgapi_wave_id_t wave,amd_dbgapi_status_t status) {
  if (!selected ()) return;
  require (m_path==path::completion
           && m_owner.wave_resume_returned (wave.handle,status),failure::native_result);
  // Called before existing INVALID_WAVE_ID compatibility. No selected waiver.
}
void native_adapter::after_amd_resume () {
  if (!selected ()) return;
  require (!m_amd_body_returned && m_path!=path::none && m_path!=path::entry,
           failure::resume_scope);
  if (m_path==path::publication || m_path==path::completion)
    require (m_owner.resume_body_returned (),failure::native_result);
  m_amd_body_returned=true;
}
void native_adapter::after_generic_resume () {
  if (!selected ()) return;
  require (m_path!=path::none && !m_generic_returned
           && (m_path==path::entry ? !m_amd_body_returned : m_amd_body_returned),
           failure::native_result);
  m_generic_returned=true;
}
void native_adapter::before_generic_commit () {
  if (!selected ()) return;
  require (m_path!=path::none && m_generic_returned && !m_generic_commit,
           failure::commit);
  if (m_path==path::publication || m_path==path::completion)
    require (m_owner.commit_started (current_identity ()),failure::commit);
  m_generic_commit=true; // Before beneath/forward-progress effects.
}
void native_adapter::after_amd_commit () {
  if (!selected ()) return;
  require (m_generic_commit && !m_amd_commit_returned && m_path!=path::entry
           && m_amd_body_returned,failure::commit);
  m_amd_commit_returned=true;
}
void native_adapter::after_generic_commit () {
  if (!selected ()) return;
  require (m_generic_commit && m_generic_returned
           && (m_path==path::entry ? !m_amd_commit_returned : m_amd_commit_returned),
           failure::commit);
  if (m_path==path::publication || m_path==path::completion)
    require (m_owner.commit_returned (),failure::commit);
  if (m_path==path::callback_continue) m_callback_progress=callback_progress::none;
  m_path=path::none; m_generic_returned=m_generic_commit=false;
  m_amd_body_returned=m_amd_commit_returned=false;
  flush ();
}
void native_adapter::before_finish_step (thread_info *thread,const target_waitstatus &wait,
                                        bool had_inline,bool trap_expected,std::uint64_t inline_address) {
  if (!selected ()) return;
  if (m_callback_progress!=callback_progress::step_inflight) return;
  require (m_path==path::none && thread==m_host && thread==inferior_thread ()
           && wait.kind ()==TARGET_WAITKIND_STOPPED && wait.sig ()==GDB_SIGNAL_TRAP
           && had_inline && trap_expected && inline_address==m_callback_pc
           && !thread->executing () && !thread->resumed ()
           && m_owner.check_owner (current_identity ()),failure::resume_scope);
  m_callback_progress=callback_progress::continue_ready;
  // Actual native stopped event, not a CLI step claim. The source hook precedes
  // finish_step_over clearing the actual inline/trap identity.
}
void native_adapter::about_to_proceed () {
  if (!selected ()) return;
  require (m_command_continue && !m_infrun_origin && m_path==path::none
           && m_callback_progress==callback_progress::none,failure::resume_scope);
  if (m_owner.state ()==phase::armed_entry)
    require (inferior_thread ()==m_host && m_owner.check_owner (current_identity ()),
             failure::resume_scope);
  else require (m_owner.early_proceed (current_identity ()),failure::resume_scope);
}
void native_adapter::mi_input (const char *text) {
  if (!selected ()) return;
  require (text!=nullptr && current_ui==m_ui && top_level_interpreter ()==m_interpreter,
           failure::command);
  std::size_t size=0;
  while (size<128 && text[size]!=0) ++size;
  debit (counter::work,size);
  require (size>0 && size<128,failure::command);
  std::size_t token=0;
  while (token<size && text[token]>='0' && text[token]<='9') ++token;
  require (token>0 && token<=20 && token<size && !m_command_continue && !m_exit_requested,
           failure::command);
  if (std::strcmp (text+token,"-exec-continue")==0) {
    require (m_path==path::none && m_callback_progress==callback_progress::none
             && (m_owner.state ()==phase::armed_entry
                 || m_owner.state ()==phase::retired_checkpoint
                 || m_owner.state ()==phase::current_gpu_stop),failure::command);
    m_command_continue=true;
  } else if (std::strcmp (text+token,"-gdb-exit")==0) {
    require (m_owner.state ()==phase::lifecycle_disappeared && m_path==path::none,
             failure::command);
    m_exit_requested=true;
  } else reject (failure::command);
}
void native_adapter::nested_mi () { if (selected ()) reject (failure::command); }
void native_adapter::flush () {
  if (!selected () || m_owner.invalid ()) return;
  if (m_owner.state ()==phase::provisional_checkpoint
      || m_owner.state ()==phase::provisional_gpu_stop
      || (m_owner.state ()==phase::current_gpu_stop && !m_stop_retired)) return;
  try {
    require (current_ui==m_ui && top_level_interpreter ()==m_interpreter,failure::output);
    auto *mi=as_mi_interp (m_interpreter);
    require (mi!=nullptr && mi->raw_stdout!=nullptr,failure::output);
    while (const auto *row=m_owner.next_diagnostic ()) {
      const auto count=std::snprintf (m_output.data (),m_output.size (),
        "=fe2o3-owned-one-stop-v1,s=\"%llu\",p=\"%u\",pid=\"%llu\",i=\"%llu\",a=\"%llu\",t=\"%llu\"\n",
        static_cast<unsigned long long> (row->sequence),static_cast<unsigned> (row->state),
        static_cast<unsigned long long> (m_owner.m_identity.pid),
        static_cast<unsigned long long> (m_owner.m_identity.inferior_number),
        static_cast<unsigned long long> (m_owner.m_identity.process),
        static_cast<unsigned long long> (m_owner.m_identity.host_thread));
      require (count>0 && static_cast<std::size_t> (count)<m_output.size (),failure::output);
      debit (counter::work,static_cast<std::uint64_t> (count));
      gdb_puts (m_output.data (),mi->raw_stdout); gdb_flush (mi->raw_stdout);
    }
  } catch (...) { m_owner.poison (failure::output); throw; }
}
} // namespace amd_owned_one_stop_v1
