// Exact source extraction. Namespace supplied by the inert harness.
void native_adapter::breakpoint_refresh_current (refresh_kind kind,bool observed) {
  debit (counter::work,256);
  require (kind!=refresh_kind::none && m_inferior!=nullptr
           && m_inferior_ref.get ()==m_inferior && current_inferior ()==m_inferior
           && current_program_space==m_inferior->pspace && m_host!=nullptr
           && m_host_ref.get ()==m_host && m_loaded_process_ref.get ()!=nullptr
           && m_loaded_base_ref.get ()!=nullptr && m_loaded_top_ref.get ()!=nullptr,
           failure::owner_changed);
  auto *const proc=m_inferior->process_target ();
  auto *const base=m_loaded_base_ref.get ();
  auto *const top=m_inferior->top_target ();
  require (proc==m_loaded_process_ref.get () && proc==get_native_target ()
           && top==m_loaded_top_ref.get () && top==&the_amd_dbgapi_target
           && m_inferior->target_is_pushed (proc) && m_inferior->target_is_pushed (base)
           && m_inferior->target_is_pushed (top) && m_inferior->find_target_beneath (top)==base
           && (base==proc || (base->stratum ()==thread_stratum
                             && m_inferior->find_target_beneath (base)==proc)),
           failure::owner_changed);
  require (m_owner.check_owner (current_identity ()),failure::owner_changed);
  require (m_owner.m_runtime && m_owner.m_callback
           && m_owner.m_callbacks!=0 && m_owner.m_callbacks<=limits::callbacks
           && m_owner.m_completed_callbacks+1==m_owner.m_callbacks
           && m_callback_object!=0 && m_callback_object==m_owner.m_callback_object
           && m_callback_object==m_owner.m_origin_callback_object
           && m_owner.m_callback_id!=0
           && m_owner.m_callback_id==m_owner.m_origin_callback_id
           && m_callback_pc!=0 && m_callback_number!=0
           && info ().runtime_state==AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS
           && !info ().forward_progress_required && !m_owner.m_allocation_active
           && !m_query_output && !m_in_allocate
           && m_path==path::none && m_callback_progress==callback_progress::none
           && !m_generic_returned && !m_generic_commit && !m_amd_body_returned
           && !m_amd_commit_returned && !m_command_continue && !m_exit_requested
           && !m_infrun_origin && m_entry_epoch==entry_epoch::retired
           && !m_entry_maintenance_inflight && m_loaded_epoch==loaded_epoch::retired
           && m_callback_epoch==callback_epoch::inactive
           && m_terminal_epoch==callback_epoch::inactive,
           failure::callback_identity);
  // infrun marks execution/resume false before bpstat check_status. Frontend
  // state may still be THREAD_RUNNING until a user-visible stop is reported.
  require (m_host==inferior_thread () && m_host->state!=THREAD_EXITED
           && !m_host->executing () && !m_host->resumed ()
           && !m_host->has_pending_waitstatus ()
           && regcache_read_pc (get_thread_regcache (m_host))==m_callback_pc,
           failure::callback_identity);
  require (!info ().breakpoint_map.empty () && info ().breakpoint_map.size ()<=64,
           failure::callback_identity);
  const auto callback=info ().breakpoint_map.find (m_owner.m_callback_id);
  require (callback!=info ().breakpoint_map.end () && callback->second!=nullptr
           && reinterpret_cast<std::uintptr_t> (callback->second)==m_callback_object
           && static_cast<std::uint64_t> (callback->second->number)==m_callback_number
           && callback->second->pspace==m_inferior->pspace
           && callback->second->has_single_location ()
           && callback->second->first_loc ().pspace==m_inferior->pspace
           && callback->second->first_loc ().address==m_callback_pc
           && callback->second->disposition==disp_donttouch,
           failure::callback_identity);
  if (kind==refresh_kind::initial) {
    require (m_owner.state ()==phase::host_running && !m_owner.m_object
             && !m_origin_complete && !m_owner.m_empty_objects
             && (observed ? (m_origin_pending && m_object.handle!=0)
                          : (!m_origin_pending && m_object.handle==0))
             && m_gpu_thread==nullptr && m_gpu_thread_ref.get ()==nullptr,
             failure::order);
    std::size_t host=0,gpu=0; thread_roster (host,gpu);
    require (host==1 && gpu==0,failure::resume_scope);
  } else {
    require (kind==refresh_kind::terminal
             && m_owner.state ()==phase::awaiting_target_completion
             && m_owner.m_terminal==terminal_stage::consistent
             && m_owner.m_resume==resume_kind::completion
             && m_owner.m_object!=0 && m_owner.m_object==m_object.handle
             && m_origin_complete && !m_origin_pending
             && m_owner.m_empty_objects==observed,
             failure::order);
    // Same retained wave/cache identity, not a fresh wave or a completion
    // claim. At this host callback live threads have all-stop bookkeeping.
    require (m_gpu_thread!=nullptr && m_gpu_thread_ref.get ()==m_gpu_thread
             && m_gpu_thread->inf==m_inferior && m_gpu_thread->global_num>0
             && static_cast<std::uint64_t> (m_gpu_thread->global_num)==m_owner.m_stop.thread
             && ptid_is_gpu (m_gpu_thread->ptid)
             && m_gpu_thread->ptid.pid ()==m_inferior->pid
             && get_amd_dbgapi_wave_id (m_gpu_thread->ptid).handle==m_owner.m_stop.wave
             && m_owner.m_stop.wave!=0 && m_owner.m_stop.queue!=0
             && m_owner.m_stop.dispatch!=0 && m_owner.m_stop.agent!=0
             && m_inferior->ptid_thread_map.size ()<=2
             && info ().wave_info_map.size ()==1,failure::stop_changed);
    const auto active=m_inferior->ptid_thread_map.find (m_gpu_thread->ptid);
    const auto cached=info ().wave_info_map.begin ();
    require (cached->first==m_owner.m_stop.wave
             && cached->second.coords.wave_id.handle==m_owner.m_stop.wave
             && cached->second.coords.queue_id.handle==m_owner.m_stop.queue
             && cached->second.coords.dispatch_id.handle==m_owner.m_stop.dispatch
             && cached->second.coords.agent_id.handle==m_owner.m_stop.agent
             && cached->second.last_resume_mode==AMD_DBGAPI_RESUME_MODE_NORMAL
             && !cached->second.stopping,failure::stop_changed);
    if (m_gpu_thread->state==THREAD_EXITED) {
      require (m_inferior->ptid_thread_map.size ()==1
               && active==m_inferior->ptid_thread_map.end (),failure::stop_changed);
      // Exited-but-held flags are not reset by set_thread_exited.
    } else {
      require (m_inferior->ptid_thread_map.size ()==2
               && active!=m_inferior->ptid_thread_map.end () && active->second==m_gpu_thread
               && m_gpu_thread->state==THREAD_RUNNING && !m_gpu_thread->executing ()
               && !m_gpu_thread->resumed () && !m_gpu_thread->has_pending_waitstatus (),
               failure::stop_changed);
    }
    std::size_t inferiors=0,total=0,host=0,gpu=0;
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
      else { require (thread==m_gpu_thread,failure::resume_scope); ++gpu; }
    }
    require (host==1 && gpu==1,failure::resume_scope);
  }
}
void native_adapter::before_breakpoint_refresh (amd_dbgapi_inferior_info &actual,
                                                amd_dbgapi_event_id_t event) {
  if (!selected ()) return;
  breakpoint_check (m_refresh_kind==refresh_kind::none && m_reset_state==reset_state::idle
                    && event.handle!=0,nullptr,breakpoint_site::refresh_begin);
  require (&actual==&info (),failure::owner_changed);
  const auto kind=m_owner.state ()==phase::host_running
    ? refresh_kind::initial : refresh_kind::terminal;
  breakpoint_refresh_current (kind,false);
  debit (counter::work,64); // Includes fixed retirement/error bookkeeping.
  m_refresh_kind=kind; m_refresh_event=event.handle;
  m_refresh_generation=m_owner.m_callbacks; m_reset_count=0;
}
void native_adapter::after_breakpoint_refresh (amd_dbgapi_inferior_info &actual,
                                               amd_dbgapi_event_id_t event) {
  if (!selected ()) return;
  breakpoint_check (m_refresh_kind!=refresh_kind::none && m_reset_state==reset_state::idle
                    && event.handle==m_refresh_event
                    && m_refresh_generation==m_owner.m_callbacks,
                    nullptr,breakpoint_site::refresh_end);
  require (&actual==&info (),failure::owner_changed);
  breakpoint_refresh_current (m_refresh_kind,true);
  clear_breakpoint_refresh ();
}
void native_adapter::clear_breakpoint_refresh () noexcept {
  m_refresh_kind=refresh_kind::none; m_refresh_event=0; m_refresh_generation=0;
  m_reset_state=reset_state::idle; m_reset_object=nullptr; m_reset_location=nullptr;
  m_reset_number=0; m_reset_type=0; m_reset_address=0; m_reset_count=0;
}
void native_adapter::abort_breakpoint_refresh () noexcept {
  if (!selected ()) return;
  // Fixed cleanup was prepaid at entry; never allocate, refund or throw.
  invalidate (failure::breakpoint_changed);
  clear_breakpoint_refresh ();
}
bool native_adapter::before_internal_breakpoint_reset (breakpoint *bp,program_space *pspace) {
  if (!selected ()) return false;
  breakpoint_check (m_refresh_kind!=refresh_kind::none && m_refresh_event!=0
                    && m_refresh_generation==m_owner.m_callbacks
                    && m_reset_state==reset_state::idle,
                    bp,breakpoint_site::reset_begin);
  breakpoint_refresh_current (m_refresh_kind,true);
  debit (counter::work,128);
  breakpoint_check (m_reset_count<64,bp,breakpoint_site::reset_bound);
  ++m_reset_count; // Attempt consumed before any candidate can be admitted.
  breakpoint_check (bp!=nullptr && bp!=m_breakpoint
                    && reinterpret_cast<std::uintptr_t> (bp)!=m_callback_object
                    && bp->number<0 && bp->enable_state==bp_disabled
                    && (bp->type==bp_overlay_event || bp->type==bp_longjmp_master
                        || bp->type==bp_std_terminate_master || bp->type==bp_exception_master)
                    && pspace==m_inferior->pspace && bp->pspace==pspace
                    && bp->has_single_location () && bp->first_loc ().owner==bp
                    && bp->first_loc ().pspace==pspace && bp->first_loc ().address!=0
                    && bp->first_loc ().address!=m_entry_pc
                    && bp->first_loc ().address!=m_checkpoint_pc
                    && bp->first_loc ().address!=m_callback_pc
                    && bp->disposition==disp_donttouch && bp->related_breakpoint==bp
                    && !bp->commands && bp->cond_string==nullptr && bp->extra_string==nullptr,
                    bp,breakpoint_site::reset_candidate);
  m_reset_object=bp; m_reset_location=&bp->first_loc ();
  m_reset_number=bp->number; m_reset_type=static_cast<unsigned> (bp->type);
  m_reset_address=bp->first_loc ().address; m_reset_state=reset_state::armed;
  return true;
}
void native_adapter::consume_internal_breakpoint_reset (breakpoint *bp) {
  breakpoint_check (m_refresh_kind!=refresh_kind::none && m_reset_state==reset_state::armed
                    && m_refresh_generation==m_owner.m_callbacks && bp!=nullptr
                    && bp==m_reset_object,bp,breakpoint_site::reset_consume);
  breakpoint_refresh_current (m_refresh_kind,true);
  debit (counter::work,64);
  breakpoint_check (bp->number==m_reset_number && static_cast<unsigned> (bp->type)==m_reset_type
                    && bp->enable_state==bp_disabled && bp->pspace==m_inferior->pspace
                    && bp->has_single_location () && &bp->first_loc ()==m_reset_location
                    && bp->first_loc ().owner==bp && bp->first_loc ().pspace==m_inferior->pspace
                    && bp->first_loc ().address==m_reset_address
                    && bp->disposition==disp_donttouch && bp->related_breakpoint==bp
                    && !bp->commands && bp->cond_string==nullptr && bp->extra_string==nullptr,
                    bp,breakpoint_site::reset_mutated);
  // Observer precedes delete. No retained pointer may be dereferenced afterward.
  m_reset_object=nullptr; m_reset_location=nullptr; m_reset_state=reset_state::consumed;
}
void native_adapter::after_internal_breakpoint_reset () {
  breakpoint_check (m_refresh_kind!=refresh_kind::none && m_reset_state==reset_state::consumed
                    && m_refresh_generation==m_owner.m_callbacks,
                    nullptr,breakpoint_site::reset_end);
  m_reset_state=reset_state::idle; m_reset_number=0; m_reset_type=0; m_reset_address=0;
}
void native_adapter::remember_breakpoint_failure (const breakpoint *bp,breakpoint_site site,
                                                  bool was_valid) noexcept {
  if (!was_valid || m_first_breakpoint_site!=breakpoint_site::none) return;
  m_first_breakpoint_site=site;
  m_first_breakpoint_type=bp==nullptr ? 0u : static_cast<unsigned> (bp->type);
  m_first_breakpoint_number=bp==nullptr ? 0 : bp->number;
}
void native_adapter::breakpoint_check (bool yes,const breakpoint *bp,breakpoint_site site) {
  if (!yes && !m_owner.invalid ()) remember_breakpoint_failure (bp,site,true);
  require (yes,failure::breakpoint_changed);
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
  breakpoint_check (false,bp,breakpoint_site::unrelated_delete);
}
void native_adapter::check_owned_breakpoint_reset (const owned_checkpoint_breakpoint *bp) {
  breakpoint_check (m_owner.selected () && bp==m_breakpoint && bp!=nullptr
                    && bp->pspace==m_inferior->pspace && bp->has_single_location ()
                    && bp->first_loc ().address==m_checkpoint_pc
                    && bp->first_loc ().pspace==m_inferior->pspace
                    && bp->disposition==disp_del,bp,breakpoint_site::owned_reset);
  // Fixed address remains unchanged through the legitimate selected solib refresh.
}
[[noreturn]] void native_adapter::reject (failure why, commit_site site) {
  m_snapshot.revoke (snapshot_issue::owner_changed);
  m_owner.poison (why, site);
  clear_breakpoint_refresh ();
  retire_loaded_maintenance ();
  retire_entry_maintenance ();
  if (m_first_breakpoint_site!=breakpoint_site::none)
    error ("fixed owned one-stop native relation refused (%u); commit-site=%u; breakpoint-site=%u; breakpoint-type=%u; breakpoint-number=%d",
           static_cast<unsigned> (m_owner.why ()),
           static_cast<unsigned> (m_owner.first_commit_site ()),
           static_cast<unsigned> (m_first_breakpoint_site),
           m_first_breakpoint_type,m_first_breakpoint_number);
  error (_("fixed owned one-stop native relation refused (%u); commit-site=%u"),
         static_cast<unsigned> (m_owner.why ()),
         static_cast<unsigned> (m_owner.first_commit_site ()));
}
void native_adapter::require (bool yes, failure why, commit_site site) {
  if (!yes || m_owner.invalid ()) reject (why, site);
}
void native_adapter::debit (counter kind, std::uint64_t amount) {
  if (!m_owner.debit (kind, amount)) reject (failure::budget);
}
