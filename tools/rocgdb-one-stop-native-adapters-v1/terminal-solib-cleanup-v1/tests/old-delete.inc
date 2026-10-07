void native_adapter::deleting_breakpoint_before (breakpoint *bp) {
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
  breakpoint_check (false,bp,breakpoint_site::unrelated_delete);
}
