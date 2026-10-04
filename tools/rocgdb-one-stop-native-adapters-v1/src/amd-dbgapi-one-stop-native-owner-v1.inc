/* SPDX-License-Identifier: GPL-3.0-or-later
   Actual GDB owners, measured executable and PID-start binding. No imported
   identity is accepted. The setup command has no caller-supplied parameters. */
#include "ui-out.h"
namespace amd_owned_one_stop_v1 {
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
std::uint64_t native_adapter::read_start_ticks () {
  require (m_inferior!=nullptr && m_inferior->pid>0,failure::owner_changed);
  const auto count=std::snprintf (m_proc_path.data (),m_proc_path.size (),
                                 "/proc/%d/stat",m_inferior->pid);
  require (count>0 && static_cast<std::size_t> (count)<m_proc_path.size (),failure::budget);
  debit (counter::proc_requested_bytes,m_proc.size ());
  debit (counter::work,m_proc.size ());
  scoped_fd fd (::open (m_proc_path.data (),O_RDONLY|O_CLOEXEC|O_NOFOLLOW));
  require (fd.get ()>=0,failure::owner_changed);
  const auto got=::read (fd.get (),m_proc.data (),m_proc.size ());
  require (got>0 && static_cast<std::size_t> (got)<m_proc.size ()
           && m_proc[static_cast<std::size_t> (got)-1]=='\n',failure::owner_changed);
  const auto end=static_cast<std::size_t> (got);
  std::size_t at=0; std::uint64_t pid=0;
  while (at<end && m_proc[at]>='0' && m_proc[at]<='9') {
    const unsigned digit=static_cast<unsigned> (m_proc[at++]-'0');
    require (pid<=(UINT64_MAX-digit)/10,failure::owner_changed); pid=pid*10+digit;
  }
  require (pid==static_cast<std::uint64_t> (m_inferior->pid)
           && at+2<end && m_proc[at]==' ' && m_proc[at+1]=='(',failure::owner_changed);
  std::size_t close=0;
  for (std::size_t n=at+2;n<end;++n) if (m_proc[n]==')') close=n;
  require (close>at+1 && close+4<end && m_proc[close+1]==' ',failure::owner_changed);
  at=close+2; // Field3 state; advance exactly to field22 starttime.
  for (unsigned field=3;field<22;++field) {
    require (at<end && m_proc[at]!=' ',failure::owner_changed);
    while (at<end && m_proc[at]!=' ') ++at;
    require (at<end,failure::owner_changed); ++at;
  }
  std::uint64_t ticks=0; const auto first=at;
  while (at<end && m_proc[at]>='0' && m_proc[at]<='9') {
    const unsigned digit=static_cast<unsigned> (m_proc[at++]-'0');
    require (ticks<=(UINT64_MAX-digit)/10,failure::owner_changed); ticks=ticks*10+digit;
  }
  require (at>first && at<end && m_proc[at]==' ' && ticks!=0,failure::owner_changed);
  return ticks;
}
void native_adapter::current_executable () {
  require (m_inferior!=nullptr && m_inferior->pid>0
           && m_inferior->pspace->exec_filename ()!=nullptr
           && std::strcmp (m_inferior->pspace->exec_filename (),target_path)==0,
           failure::checkpoint_changed);
  const auto count=std::snprintf (m_proc_path.data (),m_proc_path.size (),
                                 "/proc/%d/exe",m_inferior->pid);
  require (count>0 && static_cast<std::size_t> (count)<m_proc_path.size (),failure::budget);
  scoped_fd actual (::open (m_proc_path.data (),O_RDONLY|O_CLOEXEC));
  struct stat process_file {},fixed_file {};
  require (actual.get ()>=0 && ::fstat (actual.get (),&process_file)==0
           && ::lstat (target_path,&fixed_file)==0
           && S_ISREG (fixed_file.st_mode) && S_ISREG (process_file.st_mode)
           && fixed_file.st_dev==process_file.st_dev && fixed_file.st_ino==process_file.st_ino
           && fixed_file.st_size==static_cast<off_t> (target_bytes)
           && process_file.st_size==fixed_file.st_size,failure::checkpoint_changed);
  if (m_exec_fd<0) {
    m_exec_identity=process_file; m_exec_fd=actual.release ();
  } else {
    require (process_file.st_dev==m_exec_identity.st_dev
             && process_file.st_ino==m_exec_identity.st_ino
             && process_file.st_size==m_exec_identity.st_size
             && process_file.st_mtim.tv_sec==m_exec_identity.st_mtim.tv_sec
             && process_file.st_mtim.tv_nsec==m_exec_identity.st_mtim.tv_nsec
             && process_file.st_ctim.tv_sec==m_exec_identity.st_ctim.tv_sec
             && process_file.st_ctim.tv_nsec==m_exec_identity.st_ctim.tv_nsec,
             failure::checkpoint_changed);
  }
}
class owned_checkpoint_breakpoint final : public code_breakpoint {
public:
  owned_checkpoint_breakpoint (struct gdbarch *arch,CORE_ADDR pc)
    : code_breakpoint (arch,bp_breakpoint) {
    symtab_and_line location;
    location.pc=pc; location.section=find_pc_overlay (pc); location.pspace=current_program_space;
    add_location (location); pspace=current_program_space; disposition=disp_del;
  }
  void re_set (program_space *) override {
    native_adapter::instance ().check_owned_breakpoint_reset (this);
  }
  void check_status (bpstat *status) override {
    status->stop=1;
    try { native_adapter::instance ().checkpoint_hit (this,status); }
    catch (...) { native_adapter::instance ().invalidate (failure::checkpoint_changed); throw; }
  }
  enum print_stop_action print_it (const bpstat *status) const override {
    auto &adapter=native_adapter::instance ();
    try {
      // This is presentation of the same retained native stop, not an input
      // parser or a new proof. MI and console may each print the stop once.
      adapter.debit (counter::work,64);
      adapter.check_owned_breakpoint_reset (this);
      adapter.check_owned_breakpoint_owner ();
      adapter.require (adapter.m_owner.state ()==phase::provisional_checkpoint
                       && inferior_thread ()==adapter.m_host
                       && !adapter.m_host->executing () && !adapter.m_host->resumed ()
                       && status!=nullptr && status->breakpoint_at==this
                       && status->bp_location_at.get ()==&first_loc ()
                       && status->stop && status->simd_lane_mask==0
                       && type==bp_breakpoint && number<0 && enable_state==bp_enabled
                       && !silent && ignore_count==0 && !commands
                       && cond_string==nullptr && extra_string==nullptr,
                       failure::checkpoint_changed);
      struct ui_out *out=current_uiout;
      if (out->is_mi_like_p ()) {
        out->field_string ("reason","breakpoint-hit");
        out->field_string ("disp","del");
        out->field_signed ("bkptno",number);
      } else {
        out->text ("Owned one-stop checkpoint, ");
      }
      return PRINT_SRC_AND_LOC;
    } catch (...) { adapter.invalidate (failure::output); throw; }
  }
  void print_mention () const override {
    // install_breakpoint(internal=true) never invokes mention.
    native_adapter::instance ().reject (failure::command);
  }
  void print_recreate (struct ui_file *) const override {
    // No locspec exists and selected MI admits no save/recreate command.
    native_adapter::instance ().reject (failure::command);
  }
  int resources_needed (const struct bp_location *) override {
    // This exact object is a software breakpoint, never a hardware query.
    native_adapter::instance ().reject (failure::command);
  }
};
static_assert (sizeof (owned_checkpoint_breakpoint)<=1024,
               "fixed breakpoint object is within the prepaid logical envelope");
void native_adapter::select_at_entry () {
  if (!selection_available ())
    error (_("owned one-stop native activation unavailable: compile candidate only"));
  require (!m_owner.selected (),failure::already_selected);
  require (m_owner.reserve_selection (logical_storage (),limits::logical_bytes),failure::budget);
  try {
    m_inferior=current_inferior (); m_host=inferior_thread ();
    require (m_inferior!=nullptr && m_host!=nullptr && m_host->inf==m_inferior
             && m_inferior->pid>0 && m_inferior->num>0 && !non_stop
             && top_level_interpreter ()!=nullptr
             && std::strcmp (top_level_interpreter ()->name (),"mi2")==0
             && m_inferior->pspace->cbfd==nullptr
             && m_host->state==THREAD_STOPPED && !m_host->executing ()
             && !m_host->resumed () && !ptid_is_gpu (m_host->ptid),failure::owner_changed);
    m_inferior_ref=inferior_ref::new_reference (m_inferior);
    m_ui=current_ui; m_interpreter=top_level_interpreter ();
    m_host_ref=thread_info_ref::new_reference (m_host);
    m_info=amd_dbgapi_inferior_data.get (m_inferior);
    require (m_info!=nullptr && m_info->inf==m_inferior && m_info->process_id.handle!=0
             && m_info->runtime_state==AMD_DBGAPI_RUNTIME_STATE_UNLOADED
             && runtime_observation.owns (reinterpret_cast<std::uintptr_t> (m_info),
                                          m_info->process_id.handle)
             && !runtime_observation.invalid () && !runtime_observation.attaching (),
             failure::owner_changed);
    auto *arch=m_inferior->arch ();
    require (gdbarch_bfd_arch_info (arch)->arch==bfd_arch_i386
             && gdbarch_bfd_arch_info (arch)->mach==bfd_mach_x86_64
             && gdbarch_byte_order (arch)==BFD_ENDIAN_LITTLE
             && gdbarch_ptr_bit (arch)==64,failure::checkpoint_changed);
    current_executable (); hash_executable (); m_start_ticks=read_start_ticks ();
    require (m_owner.bind_selection (current_identity ()),failure::owner_changed);
    auto *obj=m_inferior->pspace->symfile_object_file;
    require (obj!=nullptr && obj->obfd!=nullptr
             && std::strcmp (bfd_get_filename (obj->obfd.get ()),target_path)==0,
             failure::checkpoint_changed);
    const auto entry=lookup_minimal_symbol (m_inferior->pspace,entry_symbol,obj);
    const auto checkpoint=lookup_minimal_symbol (m_inferior->pspace,checkpoint_symbol,obj);
    require (entry.minsym!=nullptr && checkpoint.minsym!=nullptr
             && entry.objfile==obj && checkpoint.objfile==obj
             && entry.minsym->has_size () && entry.minsym->size ()==entry_symbol_size
             && checkpoint.minsym->has_size () && checkpoint.minsym->size ()==checkpoint_symbol_size
             && entry.minsym->type ()==mst_text && checkpoint.minsym->type ()==mst_text,
             failure::checkpoint_changed);
    std::uint64_t expected_entry=0,expected_checkpoint=0;
    require (checked_sum (obj->text_section_offset (),entry_symbol_value,expected_entry)
             && checked_sum (obj->text_section_offset (),checkpoint_symbol_value,expected_checkpoint)
             && entry.value_address ()==expected_entry
             && checkpoint.value_address ()==expected_checkpoint
             && regcache_read_pc (get_thread_regcache (m_host))==expected_entry,
             failure::checkpoint_changed);
    m_entry_pc=expected_entry; m_checkpoint_pc=expected_checkpoint;
    std::size_t host=0,gpu=0; thread_roster (host,gpu);
    require (host==1 && gpu==0,failure::resume_scope);
    std::size_t count=0;
    for (auto &location : all_bp_locations ()) {
      debit (counter::work,1);
      require (++count<=64 && !(location->pspace==m_inferior->pspace
               && (location->address==m_entry_pc || location->address==m_checkpoint_pc)),
               failure::breakpoint_changed);
    }
    auto pending=std::make_unique<owned_checkpoint_breakpoint> (m_inferior->arch (),m_checkpoint_pc);
    m_breakpoint=pending.get (); m_breakpoint_identity=reinterpret_cast<std::uintptr_t> (m_breakpoint);
    auto *installed=install_breakpoint (true,std::move (pending),1);
    require (installed==m_breakpoint && installed->number!=0,failure::breakpoint_changed);
    m_candidate.breakpoint_object=m_breakpoint_identity;
    m_candidate.breakpoint_number=static_cast<std::uint64_t> (installed->number);
    flush ();
  } catch (...) { m_owner.poison (failure::owner_changed); throw; }
}
void native_adapter::check_owned_breakpoint_owner () {
  require (m_owner.check_owner (current_identity ()),failure::owner_changed);
}
void native_adapter::check_owned_breakpoint_reset (const owned_checkpoint_breakpoint *bp) {
  require (m_owner.selected () && bp==m_breakpoint
           && bp->pspace==m_inferior->pspace && bp->has_single_location ()
           && bp->first_loc ().address==m_checkpoint_pc
           && bp->first_loc ().pspace==m_inferior->pspace
           && bp->disposition==disp_del,failure::breakpoint_changed);
  // Fixed address remains unchanged through the legitimate selected solib refresh.
}
void native_adapter::checkpoint_hit (owned_checkpoint_breakpoint *bp,bpstat *status) {
  require (m_owner.selected () && bp==m_breakpoint && status!=nullptr
           && status->breakpoint_at==bp && status->bp_location_at!=nullptr
           && status->bp_location_at->address==m_checkpoint_pc
           && bp->disposition==disp_del,failure::breakpoint_changed);
  inspect_checkpoint (false);
  require (m_owner.checkpoint_hit (current_identity (),m_candidate),failure::checkpoint_changed);
}
}
