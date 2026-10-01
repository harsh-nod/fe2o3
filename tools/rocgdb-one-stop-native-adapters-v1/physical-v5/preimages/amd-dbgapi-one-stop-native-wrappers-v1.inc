/* SPDX-License-Identifier: GPL-3.0-or-later
   Single-instance wrappers compiled in the existing AMD client TU. */
namespace amd_owned_one_stop_native_v1 {
using adapter=amd_owned_one_stop_v1::native_adapter;
bool selected () noexcept { return adapter::instance ().selected (); }
void unwind () noexcept { adapter::instance ().native_unwind (); }
void before_resume (ptid_t p,int step,gdb_signal sig) { adapter::instance ().before_generic_resume (p,step,sig); }
void after_resume () { adapter::instance ().after_generic_resume (); }
void before_commit () { adapter::instance ().before_generic_commit (); }
void after_commit () { adapter::instance ().after_generic_commit (); }
void infrun_begin (thread_info *t,ptid_t p,bool step,gdb_signal sig,bool trap,bool inl,bool soft,bool user) {
  adapter::instance ().infrun_begin (t,p,step,sig,trap,inl,soft,user);
}
void infrun_end () noexcept { adapter::instance ().infrun_end (); }
void before_finish_step (thread_info *t,const target_waitstatus &s,bool inl,bool trap,std::uint64_t address) {
  adapter::instance ().before_finish_step (t,s,inl,trap,address);
}
void after_auto_delete () { adapter::instance ().after_auto_delete (); }
void after_normal_stop (thread_info *t) { adapter::instance ().after_host_or_gpu_stop (t); }
void flush_safe_point () { adapter::instance ().flush (); }
void mi_input (const char *s) { adapter::instance ().mi_input (s); }
void nested_mi () { adapter::instance ().nested_mi (); }
bool selected_object (inferior *inf,amd_dbgapi_code_object_id_t &id,
                      std::ptrdiff_t &bias,const char *&uri) {
  unwind_guard guard;
  if (!selected () || inf!=current_inferior ())
    error (_("owned one-stop selected object request has no actual owner"));
  auto *info=amd_dbgapi_inferior_data.get (inf);
  if (info==nullptr) {
    unwind ();
    error (_("owned one-stop actual registry owner disappeared"));
  }
  return adapter::instance ().selected_object (*info,id,bias,uri);
}
}
static void one_stop_select_command_v1 (const char *args,int) {
  if (args!=nullptr && *args!=0)
    error (_("owned one-stop selector accepts no arguments"));
  // The real method is compiled; its first source-level gate always refuses.
  amd_owned_one_stop_v1::native_adapter::instance ().select_at_entry ();
}
