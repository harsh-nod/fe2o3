/* SPDX-License-Identifier: GPL-3.0-or-later
   Called solely at the existing client's actual callback/sole-ACK sites.
   No synthetic event, second ACK or serialized owner construction. */
namespace amd_owned_one_stop_v1 {
void native_adapter::callback_begin (amd_dbgapi_inferior_info &actual,breakpoint *bp,
                                    amd_dbgapi_breakpoint_id_t id,thread_info *thread) {
  if (!selected ()) return;
  retire_entry_maintenance ();
  require (&actual==&info () && thread==m_host && thread==inferior_thread ()
           && m_path==path::none && m_callback_progress==callback_progress::none
           && bp!=nullptr && bp->pspace==m_inferior->pspace
           && bp->has_single_location () && bp->disposition==disp_donttouch,
           failure::callback_identity);
  const auto found=actual.breakpoint_map.find (id.handle);
  require (found!=actual.breakpoint_map.end () && found->second==bp
           && bp->first_loc ().pspace==m_inferior->pspace
           && bp->first_loc ().address!=0
           && regcache_read_pc (get_thread_regcache (thread))==bp->first_loc ().address,
           failure::callback_identity);
  const auto object=reinterpret_cast<std::uintptr_t> (bp);
  if (m_callback_object==0) {
    require (m_owner.state ()==phase::host_running,failure::callback_identity);
    m_callback_object=object; m_callback_pc=bp->first_loc ().address;
    m_callback_number=static_cast<std::uint64_t> (bp->number);
  } else {
    require (object==m_callback_object && bp->first_loc ().address==m_callback_pc
             && static_cast<std::uint64_t> (bp->number)==m_callback_number,
             failure::callback_identity);
  }
  if (m_owner.state ()==phase::awaiting_target_completion) {
    read_runtime_root (m_owner.m_root_address);
    require (m_root_read.breakpoint==m_callback_pc
             && m_owner.terminal_callback_begin
                  (current_identity (),object,id.handle,thread->global_num,
                   m_owner.m_root_address,m_root_read),failure::callback_identity);
  } else {
    require (m_owner.callback_begin (current_identity (),object,id.handle,thread->global_num),
             failure::callback_identity);
  }
}
void native_adapter::callback_end (amd_dbgapi_inferior_info &actual,breakpoint *bp,
                                  amd_dbgapi_breakpoint_id_t id,thread_info *thread,
                                  amd_dbgapi_event_id_t event,amd_dbgapi_breakpoint_action_t action,
                                  amd_dbgapi_status_t status) {
  if (!selected ()) return;
  require (&actual==&info () && thread==m_host && bp!=nullptr
           && reinterpret_cast<std::uintptr_t> (bp)==m_callback_object
           && bp->has_single_location () && bp->first_loc ().address==m_callback_pc,
           failure::callback_identity);
  const auto actual_owner=current_identity ();
  const bool accepted=m_owner.state ()==phase::awaiting_target_completion
    ? m_owner.terminal_callback_complete (actual_owner,m_callback_object,id.handle,
                                          thread->global_num,event.handle,action,status)
    : m_owner.callback_complete (actual_owner,m_callback_object,id.handle,
                                 thread->global_num,event.handle,action,status);
  require (accepted,failure::callback_identity);
  m_callback_progress=callback_progress::step_ready;
  // This admits no effect here. Actual infrun/target fences still consume the
  // one precisely identified internal host step and subsequent continuation.
  flush ();
}
void native_adapter::runtime_ack (amd_dbgapi_inferior_info &actual,amd_dbgapi_event_id_t event,
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
void native_adapter::objects_ack (amd_dbgapi_inferior_info &actual,amd_dbgapi_event_id_t event,
                                 amd_dbgapi_status_t status) {
  if (!selected ()) return;
  require (&actual==&info (),failure::owner_changed);
  if (m_owner.state ()==phase::awaiting_target_completion) {
    require (m_owner.terminal_object_completed (current_identity (),event.handle,status),
             failure::acknowledgement);
  } else {
    require (m_origin_pending && !m_origin_complete
             && m_owner.code_object_completed (current_identity (),event.handle,m_object.handle,status),
             failure::acknowledgement);
    m_origin_pending=false; m_origin_complete=true;
  }
}
void native_adapter::wave_ack (amd_dbgapi_inferior_info &actual,amd_dbgapi_event_id_t event,
                              thread_info *thread,amd_dbgapi_status_t status) {
  if (!selected ()) return;
  require (&actual==&info () && thread!=nullptr && status==AMD_DBGAPI_STATUS_SUCCESS
           && m_gpu_thread==nullptr && m_owner.state ()==phase::awaiting_gpu_stop
           && m_path==path::none,failure::stop_changed);
  // THREAD came from the actual handled WAVE_STOP before its sole ACK.
  // Never query the event handle after ACK, when the native event is invalid.
  m_candidate_stop=query_stop (thread,false);
  require (m_owner.stopped_after_ack (current_identity (),event.handle,m_candidate_stop,status),
           failure::stop_changed);
  m_gpu_thread=thread; m_gpu_thread_ref=thread_info_ref::new_reference (thread);
  capture_physical_snapshot (); // Source-disabled; private provisional bytes only.
  // No diagnostic output until actual MI stop and post-auto-delete safe point.
}
void native_adapter::after_host_or_gpu_stop (thread_info *thread) {
  if (!selected ()) return;
  retire_entry_maintenance ();
  if (m_owner.state ()==phase::lifecycle_disappeared && thread==nullptr) return;
  // Lifecycle disappearance is not a claim that the separate MI exit was normal.
  require (thread!=nullptr && thread==inferior_thread (),failure::stop_changed);
  if (m_owner.state ()==phase::provisional_checkpoint) {
    require (thread==m_host
             && m_owner.host_stop_output_completed
                  (current_identity (),regcache_read_pc (get_thread_regcache (thread))),
             failure::checkpoint_changed);
  } else if (m_owner.state ()==phase::provisional_gpu_stop) {
    require (thread==m_gpu_thread,failure::stop_changed);
    m_candidate_stop=query_stop (thread,true);
    require (m_owner.gpu_stop_output_completed (current_identity (),m_candidate_stop),
             failure::stop_changed);
    confirm_physical_snapshot (); // Uses the unchanged second full native tuple.
  } else reject (failure::order);
  // MI *stopped alone never means checkpoint retirement. Do not flush here.
}
void native_adapter::deleting_breakpoint (breakpoint *bp) {
  if (!selected () || m_owner.invalid ()) return;
  if (bp==m_breakpoint) {
    const auto object=reinterpret_cast<std::uintptr_t> (bp);
    const auto number=static_cast<std::uint64_t> (bp->number);
    require (m_owner.deleting_breakpoint (object,number),failure::breakpoint_changed);
    m_breakpoint=nullptr; // Observer precedes deletion; never dereference it later.
    return;
  }
  if (reinterpret_cast<std::uintptr_t> (bp)==m_callback_object) {
    require (m_owner.state ()==phase::awaiting_target_completion && !m_owner.m_callback
             && m_owner.m_terminal==owner::terminal_stage::withdrawal_complete
             && m_callback_progress==callback_progress::none,
             failure::breakpoint_changed);
    // This notification is not a retirement proof and grants no further resume.
    return;
  }
  reject (failure::breakpoint_changed);
}
void native_adapter::after_auto_delete () {
  if (!selected ()) return;
  if (m_owner.state ()==phase::provisional_checkpoint) {
    require (m_breakpoint==nullptr && inferior_thread ()==m_host,
             failure::retirement_incomplete);
    std::size_t objects=0,locations=0,count=0;
    for (auto &bp : all_breakpoints ()) {
      debit (counter::work,1);
      require (++count<=64,failure::budget);
      if (reinterpret_cast<std::uintptr_t> (&bp)==m_breakpoint_identity) ++objects;
    }
    count=0;
    for (auto &location : all_bp_locations ()) {
      debit (counter::work,1); require (++count<=64,failure::budget);
      if (location->pspace==m_inferior->pspace && location->address==m_checkpoint_pc) ++locations;
    }
    require (m_owner.retirement_finished (current_identity (),
               regcache_read_pc (get_thread_regcache (m_host)),objects,locations),
             failure::retirement_incomplete);
  } else {
    require (m_owner.state ()==phase::current_gpu_stop && inferior_thread ()==m_gpu_thread,
             failure::stop_changed);
    m_stop_retired=true;
    seal_physical_snapshot (); // No public value accessor or diagnostic is added.
  }
  flush (); // After actual breakpoint_auto_delete has fully returned.
}
void native_adapter::disappearing (inferior *actual) {
  if (!selected () || m_owner.invalid ()) return;
  retire_entry_maintenance ();
  // exit_inferior clears threads before this observer, but has not cleared PID.
  // Retained strong references prevent address reuse; do not pretend a live
  // stopped host thread still exists or query target memory here.
  const bool same_owner=actual==m_inferior && m_info!=nullptr
    && amd_dbgapi_inferior_data.get (actual)==m_info && m_info->inf==actual
    && actual->pspace==reinterpret_cast<program_space *> (m_owner.m_identity.program_space)
    && actual->num==static_cast<int> (m_owner.m_identity.inferior_number)
    && actual->pid==static_cast<int> (m_owner.m_identity.pid)
    && m_info->process_id.handle==m_owner.m_identity.process
    && m_inferior_ref.get ()==actual && m_host_ref.get ()==m_host;
  if (!same_owner || !m_owner.lifecycle_disappeared (m_owner.m_identity)) {
    m_owner.poison (failure::lifecycle);
    return; // Never interrupt ordinary known-owner teardown with a thrown error.
  }
  flush ();
}
void native_adapter::invalidate (failure why) {
  if (selected ()) { m_snapshot.revoke (snapshot_issue::owner_changed); m_owner.poison (why); retire_entry_maintenance (); }
}
void native_adapter::native_unwind () noexcept {
  if (selected ()) { m_snapshot.revoke (snapshot_issue::native_error); m_owner.poison (failure::native_result); retire_entry_maintenance (); }
}
} // namespace amd_owned_one_stop_v1
