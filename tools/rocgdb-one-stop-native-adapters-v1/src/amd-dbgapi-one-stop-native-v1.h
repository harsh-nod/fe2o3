/* SPDX-License-Identifier: GPL-3.0-or-later
   Included in the existing amd-dbgapi target translation unit only. No second
   client, public owner constructor, raw pointer permit or JSON importer. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_NATIVE_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_NATIVE_V1_H
#include "amd-dbgapi-owned-one-stop-v1.h"
#include "amd-dbgapi-one-stop-activation-v1.h"
#include "amd-dbgapi-one-stop-profile-v1.h"
#include "amd-dbgapi-one-stop-locator-v1.h"
#include "amd-dbgapi-one-stop-sha256-v1.h"
#include <sys/stat.h>
namespace amd_owned_one_stop_v1 {
class owned_checkpoint_breakpoint;
class native_adapter final {
public:
  native_adapter (const native_adapter &) = delete;
  native_adapter &operator= (const native_adapter &) = delete;
  native_adapter (native_adapter &&) = delete;
  native_adapter &operator= (native_adapter &&) = delete;
  static native_adapter &instance () noexcept;
  bool selected () const noexcept { return m_owner.selected (); }
  /* Existing callbacks delegate byte-equivalently while no selected bounded
     output scope is active. These return no owner/permission to the caller. */
  void *allocate_client (std::size_t);
  void deallocate_client (void *);
  void select_at_entry (); // No user arguments; all inputs read from GDB.
  void before_generic_resume (ptid_t, int, gdb_signal);
  void after_generic_resume ();
  void after_generic_commit ();
  void native_unwind () noexcept;
  void infrun_begin (thread_info *,ptid_t,bool,gdb_signal,bool,bool,bool,bool);
  void infrun_end () noexcept;
  void before_finish_step (thread_info *,const target_waitstatus &,bool,bool,std::uint64_t);
  void mi_input (const char *);
  void nested_mi ();
  void after_amd_cpu_resume ();
  void after_amd_wave_resume (amd_dbgapi_wave_id_t, amd_dbgapi_status_t);
  void after_amd_resume ();
  void before_generic_commit ();
  void after_amd_commit ();
  void after_auto_delete ();
  void deleting_breakpoint (breakpoint *);
  void after_host_or_gpu_stop (thread_info *);
  void about_to_proceed ();
  void callback_begin (amd_dbgapi_inferior_info &, breakpoint *,
                       amd_dbgapi_breakpoint_id_t, thread_info *);
  void callback_end (amd_dbgapi_inferior_info &, breakpoint *,
                     amd_dbgapi_breakpoint_id_t, thread_info *,
                     amd_dbgapi_event_id_t, amd_dbgapi_breakpoint_action_t,
                     amd_dbgapi_status_t);
  void runtime_ack (amd_dbgapi_inferior_info &, amd_dbgapi_event_id_t,
                    amd_dbgapi_runtime_state_t, amd_dbgapi_status_t);
  void objects_ack (amd_dbgapi_inferior_info &, amd_dbgapi_event_id_t,
                    amd_dbgapi_status_t);
  void wave_ack (amd_dbgapi_inferior_info &, amd_dbgapi_event_id_t,
                 thread_info *, amd_dbgapi_status_t);
  void disappearing (inferior *);
  void invalidate (failure);
  void flush ();
  /* Existing solib loop calls this only in selected profile. It obtains the
     real sole handle and bounded URI before its ordinary solib bookkeeping. */
  bool selected_object (amd_dbgapi_inferior_info &, amd_dbgapi_code_object_id_t &,
                        std::ptrdiff_t &, const char *&);
private:
  friend class owned_checkpoint_breakpoint;
  native_adapter () noexcept = default;
  [[noreturn]] void reject (failure);
  void require (bool, failure);
  void debit (counter, std::uint64_t);
  void api ();
  identity current_identity ();
  amd_dbgapi_inferior_info &info ();
  void thread_roster (std::size_t &, std::size_t &);
  void checkpoint_hit (owned_checkpoint_breakpoint *, bpstat *);
  void inspect_checkpoint (bool recheck);
  void read (std::uint64_t, std::uint8_t *, std::size_t);
  void hash_inferior (std::uint64_t, std::uint64_t, const digest_bytes &);
  void hash_executable ();
  std::uint64_t read_start_ticks ();
  void current_executable ();
  void hash_input (const std::uint8_t *, std::size_t, bool file);
  void hash_finish (const digest_bytes &, bool file);
  void read_u64 (std::uint64_t, std::uint64_t &);
  void read_runtime_root (std::uint64_t);
  void check_owned_breakpoint_reset (const owned_checkpoint_breakpoint *);
  void check_owned_breakpoint_owner ();
  void inspect_empty_transport (const checkpoint_view &);
  void sole_queue (const checkpoint_view &, bool);
  gpu_stop query_stop (thread_info *, bool require_gdb_current);
  template<typename Get, typename Id, typename Query, typename Value>
  void query (Get, Id, Query, Value &);
  template<typename Handle, typename Call>
  Handle list_one (Call);
  void client_scope_failed () noexcept;
  static constexpr std::uint64_t constant_and_scratch = 4096;
  static constexpr std::uint64_t logical_storage () noexcept;
  owner m_owner;
  workspace m_work;
  sha256 m_hash;
  digest_bytes m_digest {};
  checkpoint m_candidate {};
  gpu_stop m_candidate_stop {};
  runtime_root m_root_read {};
  std::array<char,1024> m_proc {};
  std::array<char,128> m_proc_path {};
  struct stat m_exec_identity {};
  ui *m_ui = nullptr;
  interp *m_interpreter = nullptr;
  std::array<char,128> m_output {};
  inferior *m_inferior = nullptr;
  inferior_ref m_inferior_ref;
  amd_dbgapi_inferior_info *m_info = nullptr;
  thread_info *m_host = nullptr;
  thread_info_ref m_host_ref;
  thread_info *m_gpu_thread = nullptr;
  thread_info_ref m_gpu_thread_ref;
  owned_checkpoint_breakpoint *m_breakpoint = nullptr; // Cleared in deleted hook.
  std::uintptr_t m_breakpoint_identity = 0;
  std::uint64_t m_entry_pc = 0, m_checkpoint_pc = 0, m_start_ticks = 0;
  amd_dbgapi_code_object_id_t m_object {};
  std::ptrdiff_t m_object_bias = 0;
  std::uint64_t m_original_address = 0;
  std::uint64_t m_callback_pc = 0, m_callback_number = 0;
  std::uintptr_t m_callback_object = 0;
  int m_exec_fd = -1;
  unsigned m_file_passes = 0, m_checkpoint_passes = 0;
  bool m_query_output = false, m_in_allocate = false;
  bool m_origin_pending = false, m_origin_complete = false;
  enum class path : std::uint8_t { none,entry,callback_step,callback_continue,publication,completion };
  enum class callback_progress : std::uint8_t { none,step_ready,step_inflight,continue_ready };
  path m_path=path::none;
  callback_progress m_callback_progress=callback_progress::none;
  bool m_infrun_origin=false,m_infrun_step=false,m_infrun_inline=false;
  bool m_infrun_trap=false,m_infrun_software=false,m_infrun_user_step=false;
  bool m_generic_returned=false,m_generic_commit=false;
  bool m_amd_body_returned=false,m_amd_commit_returned=false;
  bool m_command_continue=false,m_exit_requested=false,m_stop_retired=false;
  ptid_t m_infrun_scope;
  gdb_signal m_infrun_signal=GDB_SIGNAL_0;
};
constexpr std::uint64_t native_adapter::logical_storage () noexcept {
  /* sizeof includes owner + every concrete scratch/data member. Additional
     4096 prepays fixed constants, parser/scalar stack scratch and fixed RAII
     guards, with the sole128-byte library callback allocation separate. */
  return sizeof (native_adapter) + constant_and_scratch + limits::client_output_bytes;
}
static_assert (sizeof (native_adapter) + 4096 + limits::client_output_bytes
               <= limits::logical_bytes, "all new fixed retained data is prepaid");
static_assert (limits::file_requested_bytes == target_file_read_cap
               && limits::file_rounds == target_file_round_cap,
               "measured executable and file ledger must remain joined");
}
#endif
