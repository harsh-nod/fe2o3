// SPDX-License-Identifier: GPL-3.0-or-later
// Exact old owner-binding and proposed real helper/commit bodies.
void native_adapter::require (bool yes, failure why) {
  if (!yes || m_owner.invalid ()) reject (why);
}
amd_dbgapi_inferior_info &native_adapter::info () {
  require (m_owner.selected () && !m_owner.invalid () && m_inferior!=nullptr
           && current_inferior ()==m_inferior && m_info!=nullptr,
           failure::owner_changed);
  require (amd_dbgapi_inferior_data.get (m_inferior)==m_info
           && m_info->inf==m_inferior && m_info->process_id.handle!=0,
           failure::owner_changed); // Never get-or-create a replacement registry owner.
  return *m_info;
}
identity native_adapter::current_identity () {
  auto &i=info ();
  require (m_host!=nullptr && m_host->inf==m_inferior && m_host->global_num>0
           && m_host->state!=THREAD_EXITED && !ptid_is_gpu (m_host->ptid)
           && m_inferior->pid>0 && m_host->ptid.pid ()==m_inferior->pid
           && m_inferior->process_target ()->find_thread (m_host->ptid)==m_host,
           failure::owner_changed);
  return {reinterpret_cast<std::uintptr_t> (m_inferior),
          reinterpret_cast<std::uintptr_t> (m_inferior->pspace),
          reinterpret_cast<std::uintptr_t> (&i),
          static_cast<std::uint64_t> (m_inferior->num),
          static_cast<std::uint64_t> (m_inferior->pid),m_start_ticks,
          i.process_id.handle,static_cast<std::uint64_t> (m_host->global_num)};
}
void native_adapter::thread_roster (std::size_t &host,std::size_t &gpu) {
  host=0; gpu=0; std::size_t inferiors=0,total=0;
  for (auto *inf : all_non_exited_inferiors ()) {
    debit (counter::work,1);
    require (++inferiors<=1 && inf==m_inferior,failure::resume_scope);
  }
  require (inferiors==1,failure::resume_scope);
  for (auto *thread : all_non_exited_threads (m_inferior->process_target (),minus_one_ptid)) {
    debit (counter::work,1);
    require (++total<=16 && thread->inf==m_inferior
             && thread->ptid.pid ()==m_inferior->pid,failure::resume_scope);
    if (ptid_is_gpu (thread->ptid)) ++gpu;
    else { require (thread==m_host,failure::resume_scope); ++host; }
  }
}
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
