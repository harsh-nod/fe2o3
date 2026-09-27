// SPDX-License-Identifier: GPL-3.0-or-later
// Exact owner-binding and proposed diagnostic-tagged real helper/commit bodies.
void native_adapter::require (bool yes, failure why, commit_site site) {
  if (!yes || m_owner.invalid ()) reject (why, site);
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
           failure::commit, commit_site::entry_environment);
  require (m_host->state==THREAD_RUNNING && m_host->executing () && m_host->resumed ()
           && !m_host->has_pending_waitstatus () && proc->threads_executing
           && proc->commit_resumed_state && !proc->has_resumed_with_pending_wait_status (),
           failure::commit, commit_site::entry_thread_state);
  std::size_t host=0,gpu=0; thread_roster (host,gpu);
  require (host==1 && gpu==0,failure::resume_scope);
}
void native_adapter::retain_entry_commit_targets () {
  debit (counter::work,32); // Retention/scalar setup before any commit effect.
  require (m_entry_epoch==entry_epoch::unused && !m_entry_maintenance_inflight
           && m_entry_process_ref.get ()==nullptr && m_entry_top_ref.get ()==nullptr
           && m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
           && current_inferior ()==m_inferior,failure::commit, commit_site::entry_retention);
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
    m_owner.poison (failure::commit, commit_site::entry_retirement); // Reentry cannot acquire a resume permit.
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
void native_adapter::prepare_loaded_maintenance (amd_dbgapi_inferior_info &actual) {
  debit (counter::work,128); // Fixed prospective lineage checks, before ref copies.
  require (m_loaded_epoch==loaded_epoch::unused
           && m_entry_epoch==entry_epoch::armed && !m_entry_maintenance_inflight
           && m_path==path::none && !m_generic_returned && !m_generic_commit
           && !m_amd_body_returned && !m_amd_commit_returned,
           failure::commit,commit_site::loaded_prepare);
  require (m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
           && current_inferior ()==m_inferior
           && m_entry_process_ref.get ()!=nullptr && m_entry_top_ref.get ()!=nullptr
           && m_loaded_process_ref.get ()==nullptr && m_loaded_base_ref.get ()==nullptr
           && m_loaded_top_ref.get ()==nullptr,failure::owner_changed);
  auto *const proc=m_inferior->process_target ();
  auto *const base=m_entry_top_ref.get ();
  auto *const top=m_inferior->top_target ();
  require (proc!=nullptr && proc==m_entry_process_ref.get () && proc==get_native_target ()
           && top==&the_amd_dbgapi_target && m_inferior->target_is_pushed (proc)
           && m_inferior->target_is_pushed (base) && m_inferior->target_is_pushed (top)
           && m_inferior->find_target_beneath (top)==base
           && (base==proc || (base->stratum ()==thread_stratum
                             && m_inferior->find_target_beneath (base)==proc)),
           failure::owner_changed);
  require (&actual==&info () && actual.runtime_state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS
           && m_owner.check_owner (current_identity ()),failure::owner_changed);
  require (m_owner.state ()==phase::host_running && !m_owner.m_runtime
           && !m_owner.m_callback && m_owner.m_callbacks==0 && !m_owner.m_object
           && !m_command_continue && !m_exit_requested && !m_infrun_origin
           && m_callback_progress==callback_progress::none && m_callback_object==0
           && m_gpu_thread==nullptr && m_gpu_thread_ref.get ()==nullptr,
           failure::commit,commit_site::loaded_prepare);
  // Prospective holders preserve the old process/host stack lifetime across
  // original entry retirement. They grant no commit authority until arm below.
  m_loaded_process_ref=target_ops_ref::new_reference (proc);
  m_loaded_base_ref=target_ops_ref::new_reference (base);
  m_loaded_top_ref=target_ops_ref::new_reference (top);
  m_loaded_epoch=loaded_epoch::prospective;
}
void native_adapter::loaded_maintenance_current (bool commit_required,bool progress_required) {
  debit (counter::work,160); // Fixed checks; actual roster traversal is separately paid.
  require (m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
           && current_inferior ()==m_inferior && m_host!=nullptr
           && m_host_ref.get ()==m_host && m_loaded_process_ref.get ()!=nullptr
           && m_loaded_base_ref.get ()!=nullptr && m_loaded_top_ref.get ()!=nullptr,
           failure::owner_changed);
  auto *const proc=m_inferior->process_target ();
  auto *const base=m_loaded_base_ref.get ();
  auto *const top=m_inferior->top_target ();
  require (proc!=nullptr && proc==m_loaded_process_ref.get () && proc==get_native_target ()
           && base!=nullptr && top==m_loaded_top_ref.get () && top==&the_amd_dbgapi_target
           && m_inferior->target_is_pushed (proc) && m_inferior->target_is_pushed (base)
           && m_inferior->target_is_pushed (top) && m_inferior->find_target_beneath (top)==base
           && (base==proc || (base->stratum ()==thread_stratum
                             && m_inferior->find_target_beneath (base)==proc)),
           failure::owner_changed);
  require (m_owner.check_owner (current_identity ()),failure::owner_changed);
  require (m_entry_epoch==entry_epoch::retired && !m_entry_maintenance_inflight
           && m_owner.state ()==phase::host_running && m_owner.m_runtime
           && !m_owner.m_callback && m_owner.m_callbacks==0 && !m_owner.m_object
           && info ().runtime_state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS
           && (!progress_required || info ().forward_progress_required)
           && m_path==path::none && !m_generic_returned && !m_generic_commit
           && !m_amd_body_returned && !m_amd_commit_returned
           && !m_command_continue && !m_exit_requested && !m_infrun_origin
           && m_callback_progress==callback_progress::none && m_callback_object==0
           && m_gpu_thread==nullptr && m_gpu_thread_ref.get ()==nullptr,
           failure::commit,commit_site::loaded_environment);
  require (m_host->state==THREAD_RUNNING && m_host->executing () && m_host->resumed ()
           && !m_host->has_pending_waitstatus () && proc->threads_executing
           && (!commit_required || proc->commit_resumed_state)
           && !proc->has_resumed_with_pending_wait_status (),
           failure::commit,commit_site::loaded_thread_state);
  std::size_t host=0,gpu=0; thread_roster (host,gpu);
  require (host==1 && gpu==0,failure::resume_scope);
}
void native_adapter::arm_loaded_maintenance () {
  debit (counter::work,32);
  require (m_loaded_epoch==loaded_epoch::prospective,
           failure::commit,commit_site::loaded_arm);
  loaded_maintenance_current (false,false);
  m_loaded_epoch=loaded_epoch::armed; // Only after actual accepted first ACK/flush.
}
void native_adapter::retire_loaded_maintenance () noexcept {
  if (m_loaded_epoch==loaded_epoch::unused || m_loaded_epoch==loaded_epoch::retired) return;
  if (!m_owner.invalid ()) m_owner.debit (counter::work,32);
  if (m_loaded_epoch==loaded_epoch::in_flight || m_loaded_epoch==loaded_epoch::amd_returned)
    m_owner.poison (failure::commit,commit_site::loaded_retirement);
  m_loaded_epoch=loaded_epoch::retired;
  // Inert refs remain physically retained through debugger teardown. No reset,
  // last-reference close, allocation, or authority reconstruction on this path.
}
void native_adapter::before_generic_commit () {
  if (!selected ()) return;
  if (m_path==path::none && m_loaded_epoch!=loaded_epoch::unused
      && m_loaded_epoch!=loaded_epoch::retired) {
    require (m_loaded_epoch==loaded_epoch::armed,failure::commit,commit_site::loaded_before);
    loaded_maintenance_current (true,false);
    m_loaded_epoch=loaded_epoch::in_flight; // Before original top/beneath/progress effects.
    return;
  }
  if (m_path==path::none) {
    require (m_entry_epoch==entry_epoch::armed && !m_entry_maintenance_inflight
             && !m_generic_returned && !m_generic_commit
             && !m_amd_body_returned && !m_amd_commit_returned,failure::commit, commit_site::before_maintenance);
    entry_maintenance_current ();
    m_entry_maintenance_inflight=true; // Consume before the one existing commit.
    return; // No target_resume credit, owner phase change, ACK, or output.
  }
  require (m_path!=path::none && m_generic_returned && !m_generic_commit,
           failure::commit, commit_site::before_pending);
  if (m_path==path::entry) retain_entry_commit_targets ();
  if (m_path==path::publication || m_path==path::completion)
    require (m_owner.commit_started (current_identity ()),failure::commit, commit_site::owner_start_bridge);
  m_generic_commit=true; // Before beneath/forward-progress effects.
}
void native_adapter::after_amd_commit () {
  if (!selected ()) return;
  if (m_loaded_epoch==loaded_epoch::in_flight || m_loaded_epoch==loaded_epoch::amd_returned) {
    require (m_loaded_epoch==loaded_epoch::in_flight,failure::commit,commit_site::loaded_after_amd);
    loaded_maintenance_current (true,true);
    m_loaded_epoch=loaded_epoch::amd_returned;
    return; // Original beneath/progress returned; no owner resume credit.
  }
  require (m_generic_commit && !m_amd_commit_returned && m_path!=path::entry
           && m_amd_body_returned,failure::commit, commit_site::after_amd);
  m_amd_commit_returned=true;
}
void native_adapter::after_generic_commit () {
  if (!selected ()) return;
  if (m_loaded_epoch==loaded_epoch::in_flight || m_loaded_epoch==loaded_epoch::amd_returned) {
    require (m_loaded_epoch==loaded_epoch::amd_returned,failure::commit,commit_site::loaded_after);
    loaded_maintenance_current (true,true);
    m_loaded_epoch=loaded_epoch::armed;
    return; // Maintenance only: no new ACK, row, stop or packet transition.
  }
  if (m_entry_maintenance_inflight) {
    require (m_entry_epoch==entry_epoch::armed && m_path==path::none
             && !m_generic_returned && !m_generic_commit
             && !m_amd_body_returned && !m_amd_commit_returned,failure::commit, commit_site::after_maintenance);
    entry_maintenance_current ();
    m_entry_maintenance_inflight=false;
    return; // Do not repeat an owner transition row or change stop/packet counts.
  }
  require (m_generic_commit && m_generic_returned
           && (m_path==path::entry ? !m_amd_commit_returned : m_amd_commit_returned),
           failure::commit, commit_site::after_pending);
  if (m_path==path::publication || m_path==path::completion)
    require (m_owner.commit_returned (),failure::commit, commit_site::owner_return_bridge);
  if (m_path==path::callback_continue) m_callback_progress=callback_progress::none;
  const bool entry=m_path==path::entry;
  m_path=path::none; m_generic_returned=m_generic_commit=false;
  m_amd_body_returned=m_amd_commit_returned=false;
  flush ();
  if (entry) {
    require (m_entry_epoch==entry_epoch::entry_commit,failure::commit, commit_site::after_entry_epoch);
    entry_maintenance_current ();
    m_entry_epoch=entry_epoch::armed; // Only after that actual first commit/flush.
  }
}
void native_adapter::runtime_ack (amd_dbgapi_inferior_info &actual,amd_dbgapi_event_id_t event,
                                 amd_dbgapi_runtime_state_t state,amd_dbgapi_status_t status) {
  if (!selected ()) return;
  if (state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS) prepare_loaded_maintenance (actual);
  else retire_loaded_maintenance ();
  retire_entry_maintenance ();
  require (&actual==&info () && actual.runtime_state==state
           && m_path==path::none && m_callback_progress==callback_progress::none,
           failure::owner_changed);
  if (state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS) {
    require (m_owner.runtime_loaded (current_identity (),event.handle,status),failure::acknowledgement);
  } else if (state==AMD_DBGAPI_RUNTIME_STATE_UNLOADED) {
    require (m_owner.runtime_unloaded (current_identity (),event.handle,status),failure::acknowledgement);
  } else reject (failure::native_result);
  flush ();
  if (state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS) arm_loaded_maintenance ();
}
void native_adapter::runtime_ack_legacy (amd_dbgapi_inferior_info &actual,amd_dbgapi_event_id_t event,
                                 amd_dbgapi_runtime_state_t state,amd_dbgapi_status_t status) {
  if (!selected ()) return;
  retire_entry_maintenance ();
  require (&actual==&info () && actual.runtime_state==state
           && m_path==path::none && m_callback_progress==callback_progress::none,
           failure::owner_changed);
  if (state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS) {
    require (m_owner.runtime_loaded (current_identity (),event.handle,status),failure::acknowledgement);
  } else if (state==AMD_DBGAPI_RUNTIME_STATE_UNLOADED) {
    require (m_owner.runtime_unloaded (current_identity (),event.handle,status),failure::acknowledgement);
  } else reject (failure::native_result);
  flush ();
}
void native_adapter::invalidate (failure why) {
  if (selected ()) { m_snapshot.revoke (snapshot_issue::owner_changed); m_owner.poison (why); retire_loaded_maintenance (); retire_entry_maintenance (); }
}
void native_adapter::native_unwind () noexcept {
  if (selected ()) { m_snapshot.revoke (snapshot_issue::native_error); m_owner.poison (failure::native_result); retire_loaded_maintenance (); retire_entry_maintenance (); }
}
