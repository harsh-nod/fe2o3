/* SPDX-License-Identifier: GPL-3.0-or-later
   The generic fence runs before dcache invalidation and top_target()->resume.
   The selected path consumes native state BEFORE any host or wave resume. */
namespace amd_owned_one_stop_v1 {
void native_adapter::entry_maintenance_current () {
  debit (counter::work,128); // Fixed added checks; roster debits remain separate.
  require (m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
           && current_inferior ()==m_inferior && m_host!=nullptr
           && m_host_ref.get ()==m_host && m_entry_process_ref.get ()!=nullptr
           && m_entry_top_ref.get ()!=nullptr,failure::owner_changed);
  auto *const proc=m_inferior->process_target ();
  auto *const top=m_inferior->top_target ();
  require (proc!=nullptr && proc==m_entry_process_ref.get ()
           && proc==get_native_target () && top==m_entry_top_ref.get ()
           && m_inferior->target_is_pushed (proc)
           && m_inferior->target_is_pushed (top)
           && (top==proc || (top->stratum ()==thread_stratum
                            && m_inferior->find_target_beneath (top)==proc)),
           failure::owner_changed);
  require (m_owner.check_owner (current_identity ()),failure::owner_changed);
  require (m_owner.state ()==phase::host_running && !m_owner.m_runtime
           && !m_owner.m_callback && info ().runtime_state==AMD_DBGAPI_RUNTIME_STATE_UNLOADED
           && !m_inferior->target_is_pushed (&the_amd_dbgapi_target)
           && !m_command_continue && !m_exit_requested && !m_infrun_origin
           && m_callback_progress==callback_progress::none
           && m_gpu_thread==nullptr && m_gpu_thread_ref.get ()==nullptr,
           failure::commit);
  require (m_host->state==THREAD_RUNNING && m_host->executing () && m_host->resumed ()
           && !m_host->has_pending_waitstatus () && proc->threads_executing
           && proc->commit_resumed_state && !proc->has_resumed_with_pending_wait_status (),
           failure::commit);
  std::size_t host=0,gpu=0; thread_roster (host,gpu);
  require (host==1 && gpu==0,failure::resume_scope);
}
void native_adapter::retain_entry_commit_targets () {
  debit (counter::work,32); // Retention/scalar setup before any commit effect.
  require (m_entry_epoch==entry_epoch::unused && !m_entry_maintenance_inflight
           && m_entry_process_ref.get ()==nullptr && m_entry_top_ref.get ()==nullptr
           && m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
           && current_inferior ()==m_inferior,failure::commit);
  auto *const proc=m_inferior->process_target ();
  auto *const top=m_inferior->top_target ();
  require (proc!=nullptr && top!=nullptr && proc==get_native_target ()
           && m_inferior->target_is_pushed (proc)
           && m_inferior->target_is_pushed (top),failure::owner_changed);
  // Same real ref policy used by target_detach; no raw-pointer permit.
  m_entry_process_ref=target_ops_ref::new_reference (proc);
  m_entry_top_ref=target_ops_ref::new_reference (top);
  m_entry_epoch=entry_epoch::entry_commit; // Never re-armed from serialized state.
  entry_maintenance_current ();
}
void native_adapter::retire_entry_maintenance () noexcept {
  if (m_entry_epoch==entry_epoch::unused || m_entry_epoch==entry_epoch::retired) return;
  if (!m_owner.invalid ()) m_owner.debit (counter::work,32);
  if (m_entry_maintenance_inflight
      || (m_entry_epoch==entry_epoch::entry_commit && m_generic_commit))
    m_owner.poison (failure::commit); // Reentry cannot acquire a resume permit.
  m_entry_epoch=entry_epoch::retired; // Monotonic; no reset/reselection.
  // Never drop the last ref or run target close from this noexcept path.
  // During an effect, or after ownership/stack drift, retain the bounded refs
  // through debugger teardown. They grant no authority once retired/poisoned.
  if (!m_entry_maintenance_inflight && !m_generic_commit
      && m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
      && current_inferior ()==m_inferior && m_entry_process_ref.get ()!=nullptr
      && m_entry_top_ref.get ()!=nullptr
      && m_inferior->target_is_pushed (m_entry_process_ref.get ())
      && m_inferior->target_is_pushed (m_entry_top_ref.get ())) {
    m_entry_top_ref.reset (nullptr);
    m_entry_process_ref.reset (nullptr);
  }
}
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
  retire_entry_maintenance ();
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
    m_snapshot.revoke (snapshot_issue::resumed); // Before any existing resume effect.
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
  if (m_path==path::none) {
    require (m_entry_epoch==entry_epoch::armed && !m_entry_maintenance_inflight
             && !m_generic_returned && !m_generic_commit
             && !m_amd_body_returned && !m_amd_commit_returned,failure::commit);
    entry_maintenance_current ();
    m_entry_maintenance_inflight=true; // Consume before the one existing commit.
    return; // No target_resume credit, owner phase change, ACK, or output.
  }
  require (m_path!=path::none && m_generic_returned && !m_generic_commit,
           failure::commit);
  if (m_path==path::entry) retain_entry_commit_targets ();
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
  if (m_entry_maintenance_inflight) {
    require (m_entry_epoch==entry_epoch::armed && m_path==path::none
             && !m_generic_returned && !m_generic_commit
             && !m_amd_body_returned && !m_amd_commit_returned,failure::commit);
    entry_maintenance_current ();
    m_entry_maintenance_inflight=false;
    return; // Do not repeat an owner transition row or change stop/packet counts.
  }
  require (m_generic_commit && m_generic_returned
           && (m_path==path::entry ? !m_amd_commit_returned : m_amd_commit_returned),
           failure::commit);
  if (m_path==path::publication || m_path==path::completion)
    require (m_owner.commit_returned (),failure::commit);
  if (m_path==path::callback_continue) m_callback_progress=callback_progress::none;
  const bool entry=m_path==path::entry;
  m_path=path::none; m_generic_returned=m_generic_commit=false;
  m_amd_body_returned=m_amd_commit_returned=false;
  flush ();
  if (entry) {
    require (m_entry_epoch==entry_epoch::entry_commit,failure::commit);
    entry_maintenance_current ();
    m_entry_epoch=entry_epoch::armed; // Only after that actual first commit/flush.
  }
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
  retire_entry_maintenance ();
  m_snapshot.revoke (snapshot_issue::resumed); // EVERY next input, before parsing.
  m_output.forget (); // Serialized history is never a live value accessor.
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
      const bool physical=row->state==phase::current_gpu_stop;
      // Prepay every added sink check BEFORE taking the formatter's exact floor.
      debit (counter::work,publication_output::work_per_row);
      auto *const observed_ui=m_ui;
      auto *const observed_interpreter=m_interpreter;
      auto *const raw=mi->raw_stdout;
      FILE *const output=observed_ui->outstream;
      const auto *const pager=dynamic_cast<const pager_file *> (raw);
      const auto sink_current=[&] () {
        return current_ui==observed_ui && m_ui==observed_ui
          && top_level_interpreter ()==observed_interpreter
          && m_interpreter==observed_interpreter
          && mi==as_mi_interp (observed_interpreter) && mi->raw_stdout==raw
          && observed_ui->outstream==output && mi->saved_raw_stdout==nullptr
          && !mi->logfile_holder && !mi->stdout_holder
          && mi->mi_uiout!=nullptr && mi->interp_ui_out ()==mi->mi_uiout
          && mi->mi_uiout->is_mi_like_p () && pager!=nullptr
          && pager->one_stop_borrowed_output_matches (output);
      };
      const auto before=m_owner.consumed ();
      debit (counter::work,snapshot_publication::work_for (physical));
      const auto id=physical ? current_identity () : m_owner.m_identity;
      if (physical) {
        require (m_stop_retired && m_gpu_thread!=nullptr
                 && m_gpu_thread_ref.get ()==m_gpu_thread
                 && inferior_thread ()==m_gpu_thread && m_gpu_thread->inf==m_inferior
                 && m_gpu_thread->state==THREAD_STOPPED && !m_gpu_thread->executing ()
                 && !m_gpu_thread->resumed () && m_path==path::none
                 && m_callback_progress==callback_progress::none,failure::stop_changed);
      }
      require (m_output.render (m_owner,before,id,*row,m_snapshot,
                                m_candidate_stop,m_owner.m_checkpoint),failure::output);
      // No native call/read is introduced: actual retained owners and already
      // confirmed native tuple fence both sides of the one bounded output.
      if (physical)
        require (same (current_identity (),id)
                 && m_output.still_current (m_owner,id,m_snapshot,m_candidate_stop)
                 && m_stop_retired && inferior_thread ()==m_gpu_thread
                 && !m_gpu_thread->executing () && !m_gpu_thread->resumed (),failure::stop_changed);
      // The pinned stdio methods ignore fputs/fflush return values. Check their
      // same FILE sticky error before and after the ONE original submission.
      // A clean flush is submission only, never proof the receiver consumed it.
      require (publication_output::submit_once (output,sink_current,
        [&] () { gdb_puts (m_output.data (),raw); },
        [&] () { gdb_flush (raw); }),failure::output);
      if (physical)
        require (same (current_identity (),id)
                 && m_output.still_current (m_owner,id,m_snapshot,m_candidate_stop)
                 && m_stop_retired && inferior_thread ()==m_gpu_thread
                 && !m_gpu_thread->executing () && !m_gpu_thread->resumed (),failure::stop_changed);
      m_output.forget ();
    }
  } catch (...) {
    m_output.forget (); m_snapshot.revoke (snapshot_issue::owner_changed);
    m_owner.poison (failure::output); throw;
  }
}
} // namespace amd_owned_one_stop_v1
