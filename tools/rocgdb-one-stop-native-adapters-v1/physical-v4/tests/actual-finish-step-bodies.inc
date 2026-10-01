// SPDX-License-Identifier: GPL-3.0-or-later
// Exact new body followed by exact original body with method name only changed.
void native_adapter::before_finish_step (thread_info *thread,const target_waitstatus &wait,
                                        bool had_inline,bool trap_expected,std::uint64_t inline_address) {
  if (!selected ()) return;
  if (m_callback_progress!=callback_progress::step_inflight) return;
  // Borrow only the null-thread event context; retain every native check.
  debit (counter::work,1);
  require (thread!=nullptr && thread==m_host && m_inferior!=nullptr
           && current_inferior ()==m_inferior && thread->inf==m_inferior
           && (inferior_ptid==null_ptid || inferior_thread ()==thread),
           failure::resume_scope);
  std::optional<scoped_restore_current_thread> event_context;
  if (inferior_ptid==null_ptid) {
    event_context.emplace ();
    switch_to_thread (thread);
  }
  require (m_path==path::none && thread==m_host && thread==inferior_thread ()
           && wait.kind ()==TARGET_WAITKIND_STOPPED && wait.sig ()==GDB_SIGNAL_TRAP
           && had_inline && trap_expected && inline_address==m_callback_pc
           && !thread->executing () && !thread->resumed ()
           && m_owner.check_owner (current_identity ()),failure::resume_scope);
  m_callback_progress=callback_progress::continue_ready;
}
void native_adapter::before_finish_step_legacy (thread_info *thread,const target_waitstatus &wait,
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
