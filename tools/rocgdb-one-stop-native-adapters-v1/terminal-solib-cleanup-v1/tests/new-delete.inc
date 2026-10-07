void native_adapter::terminal_delete_check (bool yes,const breakpoint *bp,std::uint8_t field) {
  if (!yes && !m_owner.invalid () && m_terminal_delete_field==0)
    m_terminal_delete_field=field; // Fixed first failed predicate; no raw data.
  breakpoint_check (yes,bp,breakpoint_site::unrelated_delete);
}
void native_adapter::deleting_breakpoint (breakpoint *bp) {
  if (!selected () || m_owner.invalid ()) return;
  if (bp==m_breakpoint) {
    const auto object=reinterpret_cast<std::uintptr_t> (bp);
    const auto number=static_cast<std::uint64_t> (bp->number);
    const bool was_valid=!m_owner.invalid ();
    const bool accepted=m_owner.deleting_breakpoint (object,number);
    if (!accepted) remember_breakpoint_failure (bp,breakpoint_site::owned_delete,was_valid);
    require (accepted,failure::breakpoint_changed);
    m_breakpoint=nullptr; // Observer precedes deletion; never dereference it later.
    return;
  }
  if (reinterpret_cast<std::uintptr_t> (bp)==m_callback_object) {
    breakpoint_check (m_owner.state ()==phase::awaiting_target_completion && !m_owner.m_callback
                      && m_owner.m_terminal==owner::terminal_stage::withdrawal_complete
                      && m_callback_progress==callback_progress::none,
                      bp,breakpoint_site::callback_delete);
    // This notification is not a retirement proof and grants no further resume.
    return;
  }
  if (m_reset_state==reset_state::armed) {
    consume_internal_breakpoint_reset (bp);
    return;
  }
  if (m_owner.state ()==phase::lifecycle_disappeared
      && bp!=nullptr && bp->type==bp_shlib_event) {
    debit (counter::work,128); // Fixed logical check/error work, never refunded.
    terminal_delete_check (complete (m_disappeared_identity)
      && same (m_disappeared_identity,m_owner.m_identity)
      && m_start_ticks==m_disappeared_identity.pid_start,bp,1);
    terminal_delete_check (m_inferior!=nullptr && m_inferior_ref.get ()==m_inferior
      && current_inferior ()==m_inferior
      && reinterpret_cast<std::uintptr_t> (m_inferior)==m_disappeared_identity.inferior,bp,2);
    terminal_delete_check (m_inferior->pid==0 && m_inferior->thread_list.empty ()
      && m_inferior->num==static_cast<int> (m_disappeared_identity.inferior_number)
      && reinterpret_cast<std::uintptr_t> (m_inferior->pspace)==m_disappeared_identity.program_space
      && current_program_space==m_inferior->pspace,bp,3);
    terminal_delete_check (m_host!=nullptr && m_host_ref.get ()==m_host
      && m_host->inf==m_inferior && m_host->state==THREAD_EXITED
      && static_cast<std::uint64_t> (m_host->global_num)==m_disappeared_identity.host_thread
      && reinterpret_cast<std::uintptr_t> (m_info)==m_disappeared_identity.process_owner,bp,4);
    terminal_delete_check (m_owner.m_sent!=0 && m_owner.m_sent==m_owner.m_usage.rows
      && m_owner.m_sent<=m_owner.m_rows.size ()
      && m_owner.m_rows[m_owner.m_sent-1].state==phase::lifecycle_disappeared
      && current_ui==m_ui && top_level_interpreter ()==m_interpreter,bp,5);
    terminal_delete_check (m_owner.m_terminal==terminal_stage::unloaded
      && !m_owner.m_runtime && !m_owner.m_callback && !m_owner.m_allocation_active
      && m_refresh_kind==refresh_kind::none && m_reset_state==reset_state::idle
      && m_path==path::none && m_callback_progress==callback_progress::none
      && !m_command_continue && !m_exit_requested && !m_infrun_origin,bp,6);
    terminal_delete_check (bp->number<0,bp,7);
    terminal_delete_check (bp->pspace==m_inferior->pspace,bp,8);
    terminal_delete_check (bp->has_single_location (),bp,9);
    terminal_delete_check (bp->first_loc ().owner==bp
      && bp->first_loc ().pspace==m_inferior->pspace,bp,10);
    terminal_delete_check (!bp->first_loc ().inserted,bp,11);
    terminal_delete_check (bp->disposition==disp_donttouch && !bp->commands
      && bp->cond_string==nullptr && bp->extra_string==nullptr,bp,12);
    m_disappeared_identity={}; // Consume once, BEFORE original deletion proceeds.
    return; // Not a normal-exit proof; the controller still requires MI *stopped.
  }
  breakpoint_check (false,bp,breakpoint_site::unrelated_delete);
}
void native_adapter::disappearing (inferior *actual) {
  if (!selected () || m_owner.invalid ()) return;
  retire_unloaded_maintenance ();
  retire_terminal_maintenance ();
  retire_callback_maintenance ();
  retire_loaded_maintenance ();
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
  // A thrown/failed submission never arms cleanup. This is not receiver ACK.
  m_disappeared_identity=m_owner.m_identity;
}
