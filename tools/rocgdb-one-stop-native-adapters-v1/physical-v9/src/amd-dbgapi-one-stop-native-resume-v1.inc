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
// Only the pre-publication host callback interval is covered here. This is a
// distinct lineage from the permanently retired entry/loaded epochs. The
// already-retained loaded refs are lifetime holders, never their old permit.
void native_adapter::callback_maintenance_current (bool progress_required) {
  debit (counter::work,192); // Fixed checks prepaid; roster traversal separately paid.
  require (m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
           && current_inferior ()==m_inferior && m_host!=nullptr
           && m_host_ref.get ()==m_host && m_loaded_process_ref.get ()!=nullptr
           && m_loaded_base_ref.get ()!=nullptr && m_loaded_top_ref.get ()!=nullptr,
           failure::owner_changed);
  auto *const proc=m_inferior->process_target ();
  auto *const base=m_loaded_base_ref.get ();
  auto *const top=m_inferior->top_target ();
  require (proc!=nullptr && proc==m_loaded_process_ref.get () && proc==get_native_target ()
           && top==m_loaded_top_ref.get () && top==&the_amd_dbgapi_target
           && m_inferior->target_is_pushed (proc) && m_inferior->target_is_pushed (base)
           && m_inferior->target_is_pushed (top) && m_inferior->find_target_beneath (top)==base
           && (base==proc || (base->stratum ()==thread_stratum
                             && m_inferior->find_target_beneath (base)==proc)),
           failure::owner_changed);
  require (m_owner.check_owner (current_identity ()),failure::owner_changed);
  require (m_entry_epoch==entry_epoch::retired && !m_entry_maintenance_inflight
           && m_loaded_epoch==loaded_epoch::retired
           && m_owner.state ()==phase::host_running && m_owner.m_runtime
           && !m_owner.m_callback && m_owner.m_callbacks!=0
           && m_owner.m_callbacks<=limits::callbacks
           && m_owner.m_callbacks==m_owner.m_completed_callbacks
           && m_callback_generation==m_owner.m_completed_callbacks
           && m_callback_object!=0 && m_callback_object==m_owner.m_origin_callback_object
           && m_owner.m_origin_callback_id!=0 && m_callback_pc!=0 && m_callback_number!=0
           && info ().runtime_state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS
           && (!progress_required || info ().forward_progress_required)
           && m_path==path::none && !m_generic_returned && !m_generic_commit
           && !m_amd_body_returned && !m_amd_commit_returned
           && !m_command_continue && !m_exit_requested && !m_infrun_origin
           && m_gpu_thread==nullptr && m_gpu_thread_ref.get ()==nullptr,
           failure::commit,commit_site::callback_environment);
  // No inferior_thread() or PC access: generic commit deliberately selects no
  // thread and the original retained host may currently be executing.
  require (m_host->state==THREAD_RUNNING && m_host->executing () && m_host->resumed ()
           && !m_host->has_pending_waitstatus () && proc->threads_executing
           && proc->commit_resumed_state && !proc->has_resumed_with_pending_wait_status ()
           && m_infrun_signal==GDB_SIGNAL_0 && !m_infrun_software && !m_infrun_user_step,
           failure::commit,commit_site::callback_thread_state);
  if (m_callback_stepping) {
    require (m_callback_progress==callback_progress::step_inflight
             && m_host->control.trap_expected && m_infrun_step
             && m_infrun_trap && m_infrun_inline && m_infrun_scope==m_host->ptid,
             failure::commit,commit_site::callback_thread_state);
  } else {
    require (m_callback_progress==callback_progress::none
             && !m_host->control.trap_expected && !m_infrun_step
             && !m_infrun_trap && !m_infrun_inline
             && (m_infrun_scope==m_host->ptid || m_infrun_scope==minus_one_ptid
                 || m_infrun_scope==ptid_t (m_inferior->pid)),
             failure::commit,commit_site::callback_thread_state);
  }
  std::size_t host=0,gpu=0; thread_roster (host,gpu);
  require (host==1 && gpu==0,failure::resume_scope);
}
void native_adapter::arm_callback_maintenance (bool stepping) {
  debit (counter::work,32);
  require (m_callback_epoch==callback_epoch::inactive
           && (stepping
               ? m_owner.m_completed_callbacks>m_callback_generation
               : (m_callback_stepping
                  && m_owner.m_completed_callbacks==m_callback_generation)),
           failure::commit,commit_site::callback_arm);
  m_callback_generation=m_owner.m_completed_callbacks;
  m_callback_stepping=stepping;
  callback_maintenance_current (true);
  // Called only after the actual original generic and AMD resume/commit pair
  // returned and flushed. No callback record or serialized state can arm this.
  m_callback_epoch=callback_epoch::armed;
}
void native_adapter::retire_callback_maintenance () noexcept {
  if (m_callback_epoch==callback_epoch::inactive) return;
  if (!m_owner.invalid ()) m_owner.debit (counter::work,32);
  if (m_callback_epoch==callback_epoch::in_flight
      || m_callback_epoch==callback_epoch::amd_returned)
    m_owner.poison (failure::commit,commit_site::callback_retirement);
  m_callback_epoch=callback_epoch::inactive;
  // Preserve generation/kind for the one actual continuation join. No target
  // refs are released, counters refunded or retired earlier epochs rearmed.
}
// A terminal maintenance commit borrows one original completed wave owner.
// Raw-list membership and active-map membership are deliberately different:
// set_thread_exited erases only the latter while our strong ref keeps the
// original object and wave cache alive. Absence never proves GPU completion.
void native_adapter::terminal_gpu_current () {
  debit (counter::work,160);
  require (m_gpu_thread!=nullptr && m_gpu_thread_ref.get ()==m_gpu_thread
           && m_gpu_thread->inf==m_inferior && m_gpu_thread->global_num>0
           && static_cast<std::uint64_t> (m_gpu_thread->global_num)==m_owner.m_stop.thread
           && ptid_is_gpu (m_gpu_thread->ptid)
           && m_gpu_thread->ptid.pid ()==m_inferior->pid
           && get_amd_dbgapi_wave_id (m_gpu_thread->ptid).handle==m_owner.m_stop.wave
           && m_owner.m_stop.wave!=0 && m_owner.m_stop.queue!=0
           && m_owner.m_stop.dispatch!=0 && m_owner.m_stop.agent!=0,
           failure::commit,commit_site::terminal_wave_identity);
  // Both maps are bounded BEFORE any lookup/iterator acquisition. No native
  // query, cached fetch(), temporary roster or owner reconstruction is used.
  require (m_inferior->ptid_thread_map.size ()<=2 && info ().wave_info_map.size ()==1,
           failure::commit,commit_site::terminal_wave_identity);
  const auto active=m_inferior->ptid_thread_map.find (m_gpu_thread->ptid);
  const auto cached=info ().wave_info_map.begin ();
  require (cached->first==m_owner.m_stop.wave
           && cached->second.coords.wave_id.handle==m_owner.m_stop.wave
           && cached->second.coords.queue_id.handle==m_owner.m_stop.queue
           && cached->second.coords.dispatch_id.handle==m_owner.m_stop.dispatch
           && cached->second.coords.agent_id.handle==m_owner.m_stop.agent
           && cached->second.last_resume_mode==AMD_DBGAPI_RESUME_MODE_NORMAL
           && !cached->second.stopping,
           failure::commit,commit_site::terminal_wave_identity);
  if (m_gpu_thread->state==THREAD_EXITED) {
    require (m_inferior->ptid_thread_map.size ()==1
             && active==m_inferior->ptid_thread_map.end (),
             failure::commit,commit_site::terminal_wave_membership);
    // Executing/resumed/pending bits are not reset by set_thread_exited.
    // This is retained bookkeeping only, not a currently running/stopped wave.
  } else {
    require (m_inferior->ptid_thread_map.size ()==2
             && active!=m_inferior->ptid_thread_map.end () && active->second==m_gpu_thread
             && m_gpu_thread->state==THREAD_RUNNING && m_gpu_thread->executing ()
             && m_gpu_thread->resumed () && !m_gpu_thread->has_pending_waitstatus (),
             failure::commit,commit_site::terminal_wave_membership);
  }
  std::size_t inferiors=0,total=0,host=0,gpu=0;
  // Do not use the non-exited filter: its hidden skips would leave unmetered
  // traversal and would conceal the still-held exited GPU object.
  for (auto *inf : all_inferiors ()) {
    debit (counter::work,8);
    require (++inferiors<=1 && inf==m_inferior,failure::resume_scope);
  }
  require (inferiors==1,failure::resume_scope);
  for (auto *thread : m_inferior->threads ()) {
    debit (counter::work,32);
    require (++total<=16 && thread->inf==m_inferior
             && thread->ptid.pid ()==m_inferior->pid,failure::resume_scope);
    if (thread==m_host) ++host;
    else {
      require (thread==m_gpu_thread && ptid_is_gpu (thread->ptid),failure::resume_scope);
      ++gpu;
    }
  }
  require (host==1 && gpu==1,failure::resume_scope); // Includes the held exited object.
}
void native_adapter::terminal_maintenance_current (bool progress_required) {
  debit (counter::work,256); // Fixed checks prepaid; roster traversal separately paid.
  require (m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
           && current_inferior ()==m_inferior && m_host!=nullptr
           && m_host_ref.get ()==m_host && m_loaded_process_ref.get ()!=nullptr
           && m_loaded_base_ref.get ()!=nullptr && m_loaded_top_ref.get ()!=nullptr,
           failure::owner_changed);
  auto *const proc=m_inferior->process_target ();
  auto *const base=m_loaded_base_ref.get ();
  auto *const top=m_inferior->top_target ();
  require (proc!=nullptr && proc==m_loaded_process_ref.get () && proc==get_native_target ()
           && top==m_loaded_top_ref.get () && top==&the_amd_dbgapi_target
           && m_inferior->target_is_pushed (proc) && m_inferior->target_is_pushed (base)
           && m_inferior->target_is_pushed (top) && m_inferior->find_target_beneath (top)==base
           && (base==proc || (base->stratum ()==thread_stratum
                             && m_inferior->find_target_beneath (base)==proc)),
           failure::owner_changed);
  require (m_inferior->ptid_thread_map.size ()<=2,failure::resume_scope);
  require (m_owner.check_owner (current_identity ()),failure::owner_changed);
  require (m_entry_epoch==entry_epoch::retired && !m_entry_maintenance_inflight
           && m_loaded_epoch==loaded_epoch::retired
           && m_owner.state ()==phase::awaiting_target_completion && m_owner.m_runtime
           && !m_owner.m_callback && m_owner.m_callbacks!=0
           && m_owner.m_callbacks<=limits::callbacks
           && m_owner.m_callbacks==m_owner.m_completed_callbacks
           && m_terminal_generation==m_owner.m_completed_callbacks
           && m_callback_object!=0 && m_callback_object==m_owner.m_origin_callback_object
           && m_owner.m_origin_callback_id!=0 && m_callback_pc!=0 && m_callback_number!=0
           && info ().runtime_state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS
           && (!progress_required || info ().forward_progress_required)
           && m_path==path::none && !m_generic_returned && !m_generic_commit
           && !m_amd_body_returned && !m_amd_commit_returned
           && !m_command_continue && !m_exit_requested && !m_infrun_origin
           && m_callback_epoch==callback_epoch::inactive
           && m_owner.m_resume==resume_kind::completion
           && !m_owner.m_cpu_returned && !m_owner.m_wave_returned
           && !m_owner.m_body_returned && !m_owner.m_commit_started
           && m_terminal_stage==m_owner.m_terminal,
           failure::commit,commit_site::terminal_environment);
  // No inferior_thread() or PC access: generic commit deliberately selects no
  // thread and the original retained host may currently be executing.
  require (m_host->state==THREAD_RUNNING && m_host->executing () && m_host->resumed ()
           && !m_host->has_pending_waitstatus () && proc->threads_executing
           && proc->commit_resumed_state && !proc->has_resumed_with_pending_wait_status ()
           && m_infrun_signal==GDB_SIGNAL_0 && !m_infrun_software && !m_infrun_user_step,
           failure::commit,commit_site::terminal_thread_state);
  if (m_terminal_origin==terminal_origin::completion) {
    require (m_terminal_stage==terminal_stage::idle
             && !m_owner.m_empty_objects && m_callback_progress==callback_progress::none
             && !m_host->control.trap_expected && !m_infrun_step
             && !m_infrun_trap && !m_infrun_inline
             && m_completion.completed () && m_infrun_scope==m_gpu_thread->ptid,
             failure::commit,commit_site::terminal_thread_state);
  } else {
    // Two exact completed terminal callback stages, each retaining the source
    // owner's own empty-object acknowledgement state; no arbitrary phase OR.
    if (m_terminal_stage==terminal_stage::delete_complete)
      require (!m_owner.m_empty_objects,failure::commit,commit_site::terminal_environment);
    else
      require (m_terminal_stage==terminal_stage::withdrawal_complete
               && m_owner.m_empty_objects,failure::commit,commit_site::terminal_environment);
    require (m_infrun_scope==m_host->ptid,failure::commit,commit_site::terminal_thread_state);
    if (m_terminal_origin==terminal_origin::callback_step) {
      require (m_callback_progress==callback_progress::step_inflight
               && m_host->control.trap_expected && m_infrun_step
               && m_infrun_trap && m_infrun_inline,
               failure::commit,commit_site::terminal_thread_state);
    } else {
      require (m_terminal_origin==terminal_origin::callback_continue
               && m_callback_progress==callback_progress::none
               && !m_host->control.trap_expected && !m_infrun_step
               && !m_infrun_trap && !m_infrun_inline,
               failure::commit,commit_site::terminal_thread_state);
    }
  }
  terminal_gpu_current ();
}
void native_adapter::arm_terminal_maintenance (terminal_origin origin) {
  debit (counter::work,64);
  require (m_terminal_epoch==callback_epoch::inactive
           && m_callback_epoch==callback_epoch::inactive,
           failure::commit,commit_site::terminal_arm);
  if (origin==terminal_origin::completion) {
    require (m_terminal_origin==terminal_origin::none && m_terminal_generation==0
             && m_owner.m_terminal==terminal_stage::idle,
             failure::commit,commit_site::terminal_arm);
  } else if (origin==terminal_origin::callback_step) {
    require (m_terminal_generation<limits::callbacks
             && m_owner.m_completed_callbacks==m_terminal_generation+1
             && ((m_terminal_origin==terminal_origin::completion
                  && m_terminal_stage==terminal_stage::idle
                  && m_owner.m_terminal==terminal_stage::delete_complete)
                 || (m_terminal_origin==terminal_origin::callback_continue
                     && m_terminal_stage==terminal_stage::delete_complete
                     && m_owner.m_terminal==terminal_stage::withdrawal_complete)),
             failure::commit,commit_site::terminal_arm);
  } else {
    require (origin==terminal_origin::callback_continue
             && m_terminal_origin==terminal_origin::callback_step
             && m_owner.m_completed_callbacks==m_terminal_generation
             && m_owner.m_terminal==m_terminal_stage,
             failure::commit,commit_site::terminal_arm);
  }
  m_terminal_origin=origin;
  m_terminal_generation=m_owner.m_completed_callbacks;
  m_terminal_stage=m_owner.m_terminal;
  terminal_maintenance_current (true);
  m_terminal_epoch=callback_epoch::armed; // Only after the actual original commit/flush.
}
void native_adapter::retire_terminal_maintenance () noexcept {
  if (m_terminal_epoch==callback_epoch::inactive) return;
  if (!m_owner.invalid ()) m_owner.debit (counter::work,32);
  if (m_terminal_epoch==callback_epoch::in_flight
      || m_terminal_epoch==callback_epoch::amd_returned)
    m_owner.poison (failure::commit,commit_site::terminal_retirement);
  m_terminal_epoch=callback_epoch::inactive;
  // Origin/stage/generation persist only for the next exact original join.
}
// Armed only by the original accepted runtime-unload ACK after its flush.
// The AMD target has already been unpushed; this lineage expects ONLY the
// existing host base/process commit, never an AMD-after callback.
void native_adapter::unloaded_maintenance_current (bool commit_required) {
  debit (counter::work,256);
  require (m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
           && current_inferior ()==m_inferior && m_host!=nullptr
           && m_host_ref.get ()==m_host && m_loaded_process_ref.get ()!=nullptr
           && m_loaded_base_ref.get ()!=nullptr && m_loaded_top_ref.get ()==&the_amd_dbgapi_target,
           failure::owner_changed);
  auto *const proc=m_inferior->process_target ();
  auto *const base=m_loaded_base_ref.get ();
  require (proc!=nullptr && proc==m_loaded_process_ref.get () && proc==get_native_target ()
           && m_inferior->top_target ()==base
           && m_inferior->target_is_pushed (proc) && m_inferior->target_is_pushed (base)
           && !m_inferior->target_is_pushed (&the_amd_dbgapi_target)
           && (base==proc || (base->stratum ()==thread_stratum
                             && m_inferior->find_target_beneath (base)==proc))
           && m_inferior->ptid_thread_map.size ()<=2,
           failure::owner_changed);
  require (m_owner.check_owner (current_identity ()),failure::owner_changed);
  require (m_owner.state ()==phase::awaiting_target_completion && !m_owner.m_runtime
           && m_owner.m_terminal==terminal_stage::unloaded && !m_owner.m_callback
           && !m_owner.m_allocation_active && m_owner.m_empty_objects
           && m_owner.m_resume==resume_kind::completion
           && !m_owner.m_cpu_returned && !m_owner.m_wave_returned
           && !m_owner.m_body_returned && !m_owner.m_commit_started
           && m_owner.m_callbacks!=0 && m_owner.m_callbacks<=limits::callbacks
           && m_owner.m_callbacks==m_owner.m_completed_callbacks
           && m_terminal_generation==m_owner.m_completed_callbacks
           && m_terminal_stage==terminal_stage::withdrawal_complete
           && m_terminal_origin==terminal_origin::callback_continue
           && m_terminal_epoch==callback_epoch::inactive
           && m_callback_epoch==callback_epoch::inactive
           && m_entry_epoch==entry_epoch::retired && !m_entry_maintenance_inflight
           && m_loaded_epoch==loaded_epoch::retired
           && m_callback_object!=0 && m_callback_object==m_owner.m_origin_callback_object
           && m_owner.m_origin_callback_id!=0 && m_callback_pc!=0 && m_callback_number!=0
           && info ().runtime_state==AMD_DBGAPI_RUNTIME_STATE_UNLOADED
           && !info ().forward_progress_required
           && m_path==path::none && !m_generic_returned && !m_generic_commit
           && !m_amd_body_returned && !m_amd_commit_returned
           && !m_command_continue && !m_exit_requested && !m_infrun_origin
           && m_callback_progress==callback_progress::none,
           failure::commit,commit_site::unloaded_environment);
  require (m_host->state==THREAD_RUNNING && m_host->executing () && m_host->resumed ()
           && !m_host->has_pending_waitstatus () && proc->threads_executing
           && (!commit_required || proc->commit_resumed_state)
           && !proc->has_resumed_with_pending_wait_status ()
           && !m_host->control.trap_expected && m_infrun_scope==m_host->ptid
           && m_infrun_signal==GDB_SIGNAL_0 && !m_infrun_step && !m_infrun_trap
           && !m_infrun_inline && !m_infrun_software && !m_infrun_user_step,
           failure::commit,commit_site::unloaded_thread_state);
  // Same original retained wave/cache/roster checks, including the distinct
  // exited-but-held branch. No absent-wave or runtime-unload completion claim.
  terminal_gpu_current ();
}
void native_adapter::arm_unloaded_maintenance () {
  debit (counter::work,32);
  require (m_unloaded_epoch==unloaded_epoch::unused,
           failure::commit,commit_site::unloaded_arm);
  unloaded_maintenance_current (false);
  m_unloaded_epoch=unloaded_epoch::armed;
}
void native_adapter::retire_unloaded_maintenance () noexcept {
  if (m_unloaded_epoch==unloaded_epoch::unused || m_unloaded_epoch==unloaded_epoch::retired) return;
  if (!m_owner.invalid ()) m_owner.debit (counter::work,32);
  if (m_unloaded_epoch==unloaded_epoch::in_flight)
    m_owner.poison (failure::commit,commit_site::unloaded_retirement);
  m_unloaded_epoch=unloaded_epoch::retired; // Never rearm, release refs or grant resume.
}
// The selected completion profile admits exactly the actual host-first,
// one-host/one-wave roster used by proceed's all-stop-on-non-stop loop.
// Reverse order, extra/exited threads, stack drift, and pending statuses fail
// before the next effect. This is a narrow profile, not a global ordering claim.
void native_adapter::completion_current (bool cpu_running, bool wave_running,
                                         bool commit_required) {
  debit (counter::work,160);
  resume_require (m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
                  && current_inferior ()==m_inferior && m_host!=nullptr
                  && m_host_ref.get ()==m_host && m_gpu_thread!=nullptr
                  && m_gpu_thread_ref.get ()==m_gpu_thread
                  && !non_stop && target_is_non_stop_p ()
                  && m_loaded_process_ref.get ()!=nullptr
                  && m_loaded_base_ref.get ()!=nullptr
                  && m_loaded_top_ref.get ()==&the_amd_dbgapi_target,
                  resume_site::completion_environment);
  auto *const proc=m_inferior->process_target ();
  auto *const base=m_loaded_base_ref.get ();
  resume_require (proc==m_loaded_process_ref.get () && proc==get_native_target ()
                  && m_inferior->top_target ()==m_loaded_top_ref.get ()
                  && m_inferior->target_is_pushed (proc)
                  && m_inferior->target_is_pushed (base)
                  && m_inferior->find_target_beneath (&the_amd_dbgapi_target)==base
                  && (base==proc || (base->stratum ()==thread_stratum
                                    && m_inferior->find_target_beneath (base)==proc))
                  && info ().runtime_state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS
                  && m_owner.check_owner (current_identity ())
                  && !m_owner.m_callback && !m_owner.m_allocation_active
                  && m_callback_progress==callback_progress::none
                  && m_inferior->ptid_thread_map.size ()==2
                  && info ().wave_info_map.size ()==1,
                  resume_site::completion_environment);
  std::size_t total=0;
  for (auto *inf : all_inferiors ()) {
    debit (counter::work,8);
    resume_require (++total==1 && inf==m_inferior,resume_site::completion_roster);
  }
  resume_require (total==1,resume_site::completion_roster);
  total=0;
  for (auto *thread : m_inferior->threads ()) {
    debit (counter::work,32);
    resume_require (++total<=2 && thread==(total==1 ? m_host : m_gpu_thread)
                    && thread->inf==m_inferior && thread->state==THREAD_RUNNING
                    && thread->ptid.pid ()==m_inferior->pid
                    && !thread->has_pending_waitstatus (),
                    resume_site::completion_roster);
  }
  resume_require (total==2 && !ptid_is_gpu (m_host->ptid)
                  && ptid_is_gpu (m_gpu_thread->ptid)
                  && proc->find_thread (m_gpu_thread->ptid)==m_gpu_thread
                  && static_cast<std::uint64_t> (m_gpu_thread->global_num)==m_owner.m_stop.thread
                  && get_amd_dbgapi_wave_id (m_gpu_thread->ptid).handle==m_owner.m_stop.wave
                  && m_host->executing ()==cpu_running && m_host->resumed ()==cpu_running
                  && m_gpu_thread->executing ()==wave_running
                  && m_gpu_thread->resumed ()==wave_running
                  && !m_host->control.trap_expected && !m_gpu_thread->control.trap_expected
                  && !m_host->control.stepping_command && !m_gpu_thread->control.stepping_command
                  && proc->commit_resumed_state==commit_required
                  && !proc->has_resumed_with_pending_wait_status (),
                  resume_site::completion_thread_state);
  const auto cached=info ().wave_info_map.begin ();
  resume_require (cached->first==m_owner.m_stop.wave
                  && cached->second.coords.wave_id.handle==m_owner.m_stop.wave
                  && cached->second.coords.queue_id.handle==m_owner.m_stop.queue
                  && cached->second.coords.dispatch_id.handle==m_owner.m_stop.dispatch
                  && cached->second.coords.agent_id.handle==m_owner.m_stop.agent,
                  resume_site::completion_environment);
}
void native_adapter::infrun_begin (thread_info *thread,ptid_t scope,bool step,gdb_signal signal,
                                  bool trap,bool inline_step,bool software,bool user_step) {
  if (!selected ()) return;
  resume_require (!m_infrun_origin
                  && (m_path==path::none
                      || (m_path==path::completion
                          && m_completion.expects (completion_edge::wave_begin)))
                  && thread!=nullptr && thread==inferior_thread ()
                  && (thread==m_host || thread==m_gpu_thread)
                  && m_owner.check_owner (current_identity ()),resume_site::infrun_origin);
  m_infrun_origin=true; m_infrun_scope=scope; m_infrun_step=step; m_infrun_signal=signal;
  m_infrun_trap=trap; m_infrun_inline=inline_step; m_infrun_software=software;
  m_infrun_user_step=user_step;
}
void native_adapter::infrun_end () noexcept { m_infrun_origin=false; }
void native_adapter::before_generic_resume (ptid_t scope,int step,gdb_signal signal) {
  if (!selected ()) return;
  retire_unloaded_maintenance ();
  retire_terminal_maintenance ();
  retire_callback_maintenance ();
  retire_loaded_maintenance ();
  retire_entry_maintenance ();
  resume_require (m_infrun_origin
                  && (m_path==path::none
                      || (m_path==path::completion
                          && m_completion.expects (completion_edge::wave_begin)))
                  && !m_generic_returned && !m_generic_commit && !m_amd_body_returned
                  && !m_amd_commit_returned
                  && m_infrun_scope==scope && m_infrun_step==(step!=0)
                  && m_infrun_signal==signal && signal==GDB_SIGNAL_0
                  && !m_infrun_software && !m_infrun_user_step
                  && !inferior_thread ()->executing () && !inferior_thread ()->resumed ()
                  && m_owner.check_owner (current_identity ()),resume_site::generic_origin);
  std::size_t host=0,gpu=0; thread_roster (host,gpu);
  const bool all=scope==minus_one_ptid || scope==ptid_t (m_inferior->pid);
  const bool host_only=scope==m_host->ptid || (all && host==1 && gpu==0);
  if (m_path==path::completion) {
    resume_require (m_owner.state ()==phase::completion_consumed
                    && !m_command_continue && !m_stop_retired
                    && inferior_thread ()==m_gpu_thread && scope==m_gpu_thread->ptid
                    && host==1 && gpu==1 && step==0
                    && !m_infrun_trap && !m_infrun_inline
                    && same (m_candidate_stop,m_owner.m_stop),
                    resume_site::completion_wave_scope);
    completion_current (true,false,false);
    resume_require (m_completion.advance (completion_edge::wave_begin),
                    resume_site::completion_wave_scope);
  } else if (m_callback_progress==callback_progress::step_ready) {
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
    resume_require (m_command_continue && m_stop_retired
                    && inferior_thread ()==m_host && scope==m_host->ptid
                    && host==1 && gpu==1 && step==0
                    && !m_infrun_trap && !m_infrun_inline
                    && m_completion.expects (completion_edge::cpu_begin),
                    resume_site::completion_cpu_scope);
    completion_current (false,false,false);
    m_command_continue=false; m_stop_retired=false;
    // Exactly one actual same-stop requery, before EITHER target effect.
    // query_stop(false) reads the retained GPU, independent of selected host.
    m_candidate_stop=query_stop (m_gpu_thread,false);
    require (m_owner.consume_completion (current_identity (),m_candidate_stop,host,gpu,step,signal),
             failure::resume_scope);
    m_snapshot.revoke (snapshot_issue::resumed);
    resume_require (m_completion.advance (completion_edge::cpu_begin),
                    resume_site::completion_cpu_scope);
    m_path=path::completion;
  } else reject (failure::resume_scope);
}
void native_adapter::after_amd_cpu_resume () {
  if (!selected ()) return;
  require (m_path==path::publication || m_path==path::completion
           || m_path==path::callback_step || m_path==path::callback_continue,
           failure::resume_scope);
  if (m_path==path::completion)
    resume_require (m_completion.advance (completion_edge::cpu_return),
                    resume_site::cpu_return);
  if (m_path==path::publication || m_path==path::completion)
    require (m_owner.cpu_resume_returned (),failure::native_result);
}
void native_adapter::after_amd_wave_resume (amd_dbgapi_wave_id_t wave,amd_dbgapi_status_t status) {
  if (!selected ()) return;
  resume_require (m_path==path::completion
                  && m_completion.advance (completion_edge::wave_return),
                  resume_site::wave_return);
  require (m_owner.wave_resume_returned (wave.handle,status),failure::native_result);
  // Called before existing INVALID_WAVE_ID compatibility. No selected waiver.
}
void native_adapter::after_amd_resume () {
  if (!selected ()) return;
  require (!m_amd_body_returned && m_path!=path::none && m_path!=path::entry,
           failure::resume_scope);
  if (m_path==path::completion) {
    if (m_completion.expects (completion_edge::cpu_body))
      resume_require (m_completion.advance (completion_edge::cpu_body),resume_site::body_return);
    else {
      resume_require (m_completion.advance (completion_edge::wave_body),resume_site::body_return);
      require (m_owner.resume_body_returned (),failure::native_result);
    }
  } else if (m_path==path::publication)
    require (m_owner.resume_body_returned (),failure::native_result);
  m_amd_body_returned=true;
}
void native_adapter::after_generic_resume () {
  if (!selected ()) return;
  require (m_path!=path::none && !m_generic_returned
           && (m_path==path::entry ? !m_amd_body_returned : m_amd_body_returned),
           failure::native_result);
  if (m_path==path::completion) {
    if (m_completion.expects (completion_edge::cpu_generic)) {
      resume_require (m_completion.advance (completion_edge::cpu_generic),
                      resume_site::generic_return);
      m_amd_body_returned=false;
      return; // First complete call; retain transaction, await exact wave call.
    }
    resume_require (m_completion.advance (completion_edge::wave_generic),
                    resume_site::generic_return);
  }
  m_generic_returned=true;
}
void native_adapter::before_generic_commit () {
  if (!selected ()) return;
  if (m_unloaded_epoch==unloaded_epoch::armed || m_unloaded_epoch==unloaded_epoch::in_flight) {
    require (m_unloaded_epoch==unloaded_epoch::armed,failure::commit,commit_site::unloaded_before);
    unloaded_maintenance_current (true);
    m_unloaded_epoch=unloaded_epoch::in_flight;
    return; // Consume before original host-only commit; no resume/ACK/owner credit.
  }
  if (m_terminal_epoch!=callback_epoch::inactive) {
    require (m_terminal_epoch==callback_epoch::armed,failure::commit,commit_site::terminal_before);
    terminal_maintenance_current (false);
    m_terminal_epoch=callback_epoch::in_flight;
    return; // Original commit only; no owner resume credit, ACK or output.
  }
  if (m_path==path::none && m_callback_epoch!=callback_epoch::inactive) {
    require (m_callback_epoch==callback_epoch::armed,failure::commit,commit_site::callback_before);
    callback_maintenance_current (false);
    m_callback_epoch=callback_epoch::in_flight; // Before the original commit effect.
    return;
  }
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
  if (m_path==path::completion) {
    resume_require (!m_infrun_origin && !m_command_continue
                    && m_generic_returned && m_amd_body_returned
                    && m_completion.expects (completion_edge::commit_begin),
                    resume_site::commit_begin);
    completion_current (true,true,true);
    resume_require (m_completion.advance (completion_edge::commit_begin),
                    resume_site::commit_begin);
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
  require (m_unloaded_epoch==unloaded_epoch::unused || m_unloaded_epoch==unloaded_epoch::retired,
           failure::commit,commit_site::unloaded_unexpected_amd);
  if (m_terminal_epoch!=callback_epoch::inactive) {
    require (m_terminal_epoch==callback_epoch::in_flight,failure::commit,commit_site::terminal_after_amd);
    terminal_maintenance_current (true);
    m_terminal_epoch=callback_epoch::amd_returned;
    return; // Original commit only; no owner resume credit, ACK or output.
  }
  if (m_callback_epoch!=callback_epoch::inactive) {
    require (m_callback_epoch==callback_epoch::in_flight,failure::commit,commit_site::callback_after_amd);
    callback_maintenance_current (true);
    m_callback_epoch=callback_epoch::amd_returned;
    return; // No owner resume/commit credit.
  }
  if (m_loaded_epoch==loaded_epoch::in_flight || m_loaded_epoch==loaded_epoch::amd_returned) {
    require (m_loaded_epoch==loaded_epoch::in_flight,failure::commit,commit_site::loaded_after_amd);
    loaded_maintenance_current (true,true);
    m_loaded_epoch=loaded_epoch::amd_returned;
    return; // Original beneath/progress returned; no owner resume credit.
  }
  require (m_generic_commit && !m_amd_commit_returned && m_path!=path::entry
           && m_amd_body_returned,failure::commit, commit_site::after_amd);
  if (m_path==path::completion)
    resume_require (m_completion.advance (completion_edge::commit_amd),
                    resume_site::commit_amd);
  m_amd_commit_returned=true;
}
void native_adapter::after_generic_commit () {
  if (!selected ()) return;
  if (m_unloaded_epoch==unloaded_epoch::armed || m_unloaded_epoch==unloaded_epoch::in_flight) {
    require (m_unloaded_epoch==unloaded_epoch::in_flight,failure::commit,commit_site::unloaded_after);
    unloaded_maintenance_current (true);
    m_unloaded_epoch=unloaded_epoch::armed;
    return; // Original host base/process returned; no AMD-after is expected.
  }
  if (m_terminal_epoch!=callback_epoch::inactive) {
    require (m_terminal_epoch==callback_epoch::amd_returned,failure::commit,commit_site::terminal_after);
    terminal_maintenance_current (true);
    m_terminal_epoch=callback_epoch::armed;
    return; // Original commit only; no owner resume credit, ACK or output.
  }
  if (m_callback_epoch!=callback_epoch::inactive) {
    require (m_callback_epoch==callback_epoch::amd_returned,failure::commit,commit_site::callback_after);
    callback_maintenance_current (true);
    m_callback_epoch=callback_epoch::armed;
    return; // No flush, diagnostic, ACK, new resume or owner transition.
  }
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
  if (m_path==path::completion)
    resume_require (m_completion.advance (completion_edge::commit_generic),
                    resume_site::commit_generic);
  if (m_path==path::publication || m_path==path::completion)
    require (m_owner.commit_returned (),failure::commit, commit_site::owner_return_bridge);
  if (m_path==path::callback_continue) m_callback_progress=callback_progress::none;
  const bool entry=m_path==path::entry;
  const bool callback=m_owner.state ()==phase::host_running
    && (m_path==path::callback_step || m_path==path::callback_continue);
  const bool stepping=m_path==path::callback_step;
  const terminal_origin terminal=m_path==path::completion ? terminal_origin::completion
    : (m_owner.state ()==phase::awaiting_target_completion
       ? (m_path==path::callback_step ? terminal_origin::callback_step
          : (m_path==path::callback_continue ? terminal_origin::callback_continue
             : terminal_origin::none))
       : terminal_origin::none);
  m_path=path::none; m_generic_returned=m_generic_commit=false;
  m_amd_body_returned=m_amd_commit_returned=false;
  flush ();
  if (callback) arm_callback_maintenance (stepping);
  if (terminal!=terminal_origin::none) arm_terminal_maintenance (terminal);
  if (entry) {
    require (m_entry_epoch==entry_epoch::entry_commit,failure::commit, commit_site::after_entry_epoch);
    entry_maintenance_current ();
    m_entry_epoch=entry_epoch::armed; // Only after that actual first commit/flush.
  }
}
void native_adapter::before_finish_step (thread_info *thread,const target_waitstatus &wait,
                                        bool had_inline,bool trap_expected,std::uint64_t inline_address) {
  if (!selected ()) return;
  if (m_callback_progress!=callback_progress::step_inflight) return;
  // Prepay this bounded event-context visit in the original selection ledger.
  debit (counter::work,1);
  // infrun selects the event inferior, but intentionally no thread, before
  // finish_step_over. Never repair a foreign inferior or selected thread.
  require (thread!=nullptr && thread==m_host && m_inferior!=nullptr
           && current_inferior ()==m_inferior && thread->inf==m_inferior
           && (inferior_ptid==null_ptid || inferior_thread ()==thread),
           failure::resume_scope);
  std::optional<scoped_restore_current_thread> event_context;
  if (inferior_ptid==null_ptid) {
    event_context.emplace ();
    switch_to_thread (thread);
  }
  // The original full native predicate runs in that borrowed event context.
  // Destruction restores the original no-thread context on return or unwind.
  require (m_path==path::none && thread==m_host && thread==inferior_thread ()
           && wait.kind ()==TARGET_WAITKIND_STOPPED && wait.sig ()==GDB_SIGNAL_TRAP
           && had_inline && trap_expected && inline_address==m_callback_pc
           && !thread->executing () && !thread->resumed ()
           && m_owner.check_owner (current_identity ()),failure::resume_scope);
  retire_unloaded_maintenance ();
  retire_terminal_maintenance ();
  retire_callback_maintenance (); // Actual stopped step ends the running interval.
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
  retire_unloaded_maintenance ();
  retire_terminal_maintenance ();
  retire_callback_maintenance ();
  retire_loaded_maintenance ();
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
