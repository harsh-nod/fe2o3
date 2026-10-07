/* SPDX-License-Identifier: GPL-3.0-or-later
   Included in the existing amd-dbgapi target translation unit only. No second
   client, public owner constructor, raw pointer permit or JSON importer. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_NATIVE_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_NATIVE_V1_H
#include "amd-dbgapi-owned-one-stop-v1.h"
#include "amd-dbgapi-one-stop-snapshot-v1.h"
#include "amd-dbgapi-one-stop-publication-v2.h"
#include "amd-dbgapi-one-stop-output-v2.h"
#include "pager.h"
#include "amd-dbgapi-one-stop-activation-v1.h"
#include "amd-dbgapi-one-stop-profile-v1.h"
#include "amd-dbgapi-one-stop-locator-v1.h"
#include "amd-dbgapi-one-stop-sha256-v1.h"
#include <sys/stat.h>
#include <optional>
#include <utility>
namespace amd_owned_one_stop_v1 {
class owned_checkpoint_breakpoint;
// Source-owned logical envelopes, not an optimized C++ stack/RSS assertion.
// Rows name the selected added/helper vertices; reference views are pointer-sized.
// No object of this type is constructed and no retained target is duplicated.
struct loaded_maintenance_scratch final {
  struct ack_frame { native_adapter *self; amd_dbgapi_inferior_info *actual;
    amd_dbgapi_event_id_t event; amd_dbgapi_runtime_state_t state; amd_dbgapi_status_t status; } ack;
  struct prepare_frame { native_adapter *self; amd_dbgapi_inferior_info *actual;
    process_stratum_target *proc; target_ops *base,*top; } prepare;
  struct current_frame { native_adapter *self; bool commit_required,progress_required;
    process_stratum_target *proc; target_ops *base,*top; std::size_t host,gpu; identity observed; } current;
  struct arm_frame { native_adapter *self; } arm;
  struct retirement_frame { native_adapter *self; } retirement;
  struct hook_frames { native_adapter *before,*amd_after,*generic_after; } hooks;
  struct identity_frame { native_adapter *self; amd_dbgapi_inferior_info *actual; identity result; } owner_identity;
  struct info_frame { native_adapter *self; amd_dbgapi_inferior_info *result; } info;
  struct roster_frame { native_adapter *self; std::size_t *host,*gpu;
    std::size_t inferiors,total; inferior *inf; thread_info *thread; } roster;
  struct require_frame { native_adapter *self; bool yes; failure why; commit_site site; } require;
  struct debit_frame { native_adapter *self; counter kind; std::uint64_t amount; } debit;
  struct core_frames { owner *check,*require,*debit,*poison;
    identity const *actual; bool yes; failure why; commit_site site;
    counter kind; std::uint64_t amount; } core;
  struct ref_frame { target_ops *source; target_ops_ref result;
    target_ops_ref *destination,*incoming; target_ops *incref,*decref; } refs[3];
};
// Debug-type observability only: no call or object construction.
[[gnu::used]] static const loaded_maintenance_scratch *
loaded_maintenance_debug_type (const loaded_maintenance_scratch *p) noexcept
{ return p; }
// Extra bounded borrowed/helper frames for callback-only maintenance. Retained
// fields are already in sizeof(native_adapter); existing targets are not copied.
struct callback_maintenance_scratch final {
  struct current_frame { native_adapter *self; bool progress_required;
    process_stratum_target *proc; target_ops *base,*top; identity observed;
    std::size_t host,gpu; } current;
  struct arm_frame { native_adapter *self; bool stepping; } arm;
  struct retire_frame { native_adapter *self; } retire;
  struct commit_frames { native_adapter *before,*amd_after,*generic_after;
    bool entry,callback,stepping; } hooks;
  loaded_maintenance_scratch::identity_frame owner_identity;
  loaded_maintenance_scratch::info_frame info;
  loaded_maintenance_scratch::roster_frame roster;
  loaded_maintenance_scratch::require_frame require;
  loaded_maintenance_scratch::debit_frame debit;
  loaded_maintenance_scratch::core_frames core;
};
[[gnu::used]] static const callback_maintenance_scratch *
callback_maintenance_debug_type (const callback_maintenance_scratch *p) noexcept
{ return p; }
// Separate terminal-only borrowed frames; no actual owner or cache is copied.
struct terminal_maintenance_scratch final {
  callback_maintenance_scratch common;
  struct gpu_frame { native_adapter *self; thread_info *retained;
    decltype (std::declval<inferior &> ().ptid_thread_map.begin ()) active;
    decltype (std::declval<amd_dbgapi_inferior_info &> ().wave_info_map.begin ()) cached;
    std::size_t inferiors,total,host,gpu;
    decltype (all_inferiors ()) inferior_range;
    decltype (all_inferiors ().begin ()) inferior_begin,inferior_end;
    decltype (std::declval<inferior &> ().threads ()) thread_range;
    decltype (std::declval<inferior &> ().threads ().begin ()) thread_begin,thread_end;
    inferior *inf; thread_info *thread; } gpu;
  struct origin_frames { native_adapter *arm,*retire; std::uint64_t generation;
    std::uint8_t origin,stage; } origins;
};
[[gnu::used]] static const terminal_maintenance_scratch *
terminal_maintenance_debug_type (const terminal_maintenance_scratch *p) noexcept
{ return p; }
// Unload-ACK-specific borrowed/helper frames, independently prepaid.
struct unloaded_maintenance_scratch final {
  callback_maintenance_scratch common;
  terminal_maintenance_scratch::gpu_frame gpu;
  struct frames { native_adapter *current,*arm,*retire,*before,*after;
    bool commit_required; process_stratum_target *proc; target_ops *base; } hooks;
};
[[gnu::used]] static const unloaded_maintenance_scratch *
unloaded_maintenance_debug_type (const unloaded_maintenance_scratch *p) noexcept
{ return p; }
// Separately prepaid typed helper frames for the exact solib/reset lineage.
// This is a logical source envelope, not a compiler stack/RSS claim.
struct breakpoint_refresh_scratch final {
  terminal_maintenance_scratch common;
  struct refresh_frames { native_adapter *before,*current,*after,*abort,*clear;
    amd_dbgapi_inferior_info *actual; amd_dbgapi_event_id_t event;
    program_space *pspace; std::uint8_t kind; bool observed; } refresh;
  struct reset_frames { native_adapter *begin,*consume,*end,*remember,*check;
    breakpoint *bp; const breakpoint *diagnostic; bp_location *location;
    program_space *pspace; std::uint64_t generation,address; int number;
    unsigned type,site; bool yes,was_valid; } reset;
  decltype (std::declval<amd_dbgapi_inferior_info &> ().breakpoint_map.begin ()) callback;
  amd_owned_one_stop_native_v1::internal_breakpoint_reset_guard guard;
  char fatal_error_payload[224]; // Separate bounded error text, not client output.
};
[[gnu::used]] static const breakpoint_refresh_scratch *
breakpoint_refresh_debug_type (const breakpoint_refresh_scratch *p) noexcept
{ return p; }
// BEGIN bounded native-result diagnostic core.
enum class native_result_site : std::uint8_t { none, target_memory, scalar_query };
enum class native_result_family : std::uint8_t {
  none, queue, wave, workgroup, dispatch, agent, architecture, other
};
// Diagnostic data only: no owner, permit, target pointer, allocation or retry.
struct native_result_note final {
  native_result_site site = native_result_site::none;
  native_result_family family = native_result_family::none;
  std::uint32_t selector = 0, bytes = 0;
  std::int32_t status = 0;
  void remember (bool selected, bool invalid, native_result_site actual_site,
                 native_result_family actual_family, std::uint32_t actual_selector,
                 std::uint32_t actual_bytes, std::int32_t actual_status) noexcept {
    if (!selected || invalid || site != native_result_site::none
        || actual_site == native_result_site::none || actual_status == 0) return;
    site = actual_site; family = actual_family; selector = actual_selector;
    bytes = actual_bytes; status = actual_status;
  }
};
static_assert (sizeof (native_result_note) == 16, "fixed diagnostic record");
static_assert (alignof (native_result_note) == 4, "fixed diagnostic alignment");
// END bounded native-result diagnostic core.
// Additional source-only logical envelope; no extra owner or target is copied.
struct native_result_diagnostic_scratch final {
  native_adapter *query_adapter,*remember_adapter;
  native_result_note *note;
  bool selected,invalid;
  native_result_site site;
  native_result_family family;
  std::uint32_t selector,bytes;
  std::int32_t status;
  char fatal_error_payload[224];
};
[[gnu::used]] static const native_result_diagnostic_scratch *
native_result_diagnostic_debug_type (const native_result_diagnostic_scratch *p) noexcept
{ return p; }
// BEGIN bounded checkpoint-site diagnostic core.
enum class checkpoint_site : std::uint16_t {
  none = 0,
  hash_finish = 1,
  hash_inferior_extent = 2,
  hash_inferior_address = 3,
  executable_pass = 4,
  executable_before = 5,
  executable_eof = 6,
  executable_after = 7,
  executable_name = 8,
  executable_identity = 9,
  executable_recheck = 10,
  checkpoint_presentation = 11,
  selection_architecture = 12,
  selection_symfile = 13,
  selection_symbols = 14,
  selection_addresses = 15,
  owner_checkpoint_hit = 16,
  object_locator = 17,
  runtime_root_address = 18,
  transport_write = 19,
  transport_read = 20,
  transport_signal = 21,
  transport_ring = 22,
  transport_kernarg = 23,
  transport_output_address = 24,
  transport_output_bytes = 25,
  checkpoint_entry = 26,
  checkpoint_pc_argument = 27,
  checkpoint_argument_recheck = 28,
  checkpoint_codec = 29,
  checkpoint_relocation = 30,
  checkpoint_repeat = 31,
  checkpoint_callback_root = 32,
  checkpoint_root_recheck = 33,
  checkpoint_root_bind = 34,
  sole_queue_fields = 35,
  sole_queue_recheck = 36,
  checkpoint_stop_output = 37,
  snapshot_extent = 38
};
// A diagnostic label only. It never replaces a predicate, owner or permit.
struct checkpoint_note final {
  checkpoint_site site = checkpoint_site::none;
  template<class Predicate>
  bool evaluate (bool was_valid, checkpoint_site actual_site,
                 const Predicate &predicate) {
    const bool yes = predicate (); // Exactly once; exceptions propagate unchanged.
    if (was_valid && !yes && site == checkpoint_site::none
        && actual_site != checkpoint_site::none) site = actual_site;
    return yes;
  }
};
static_assert (sizeof (checkpoint_note) == 2, "fixed checkpoint diagnostic record");
static_assert (alignof (checkpoint_note) == 2, "fixed checkpoint diagnostic alignment");
// END bounded checkpoint-site diagnostic core.
// Separate logical reservation: retained note is already in sizeof(native_adapter).
// The full closure bound is enforced at each instantiated guard; no heap storage.
struct checkpoint_diagnostic_scratch final {
  native_adapter *adapter;
  checkpoint_note *note;
  const void *predicate;
  checkpoint_site site;
  bool was_valid,yes;
  std::array<const void *,32> predicate_reference_storage;
  char fatal_error_payload[224];
};
[[gnu::used]] static const checkpoint_diagnostic_scratch *
checkpoint_diagnostic_debug_type (const checkpoint_diagnostic_scratch *p) noexcept
{ return p; }
// Separate logical reservation for the paired completion helpers and diagnostic.
// Existing targets, owners, maps, and thread objects are only borrowed.
struct completion_resume_scratch final {
  terminal_maintenance_scratch common;
  struct frames {
    native_adapter *current,*origin,*before,*cpu,*wave,*body,*generic,*commit;
    completion_sequence *sequence;
    completion_edge edge;
    thread_info *thread;
    process_stratum_target *proc;
    target_ops *base,*top;
    bool cpu_running,wave_running,commit_required,yes;
    std::size_t host,gpu,total;
    identity actual;
  } frames;
  char fatal_error_payload[224];
};
[[gnu::used]] static const completion_resume_scratch *
completion_resume_debug_type (const completion_resume_scratch *p) noexcept
{ return p; }
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
  void before_breakpoint_refresh (amd_dbgapi_inferior_info &,amd_dbgapi_event_id_t);
  void after_breakpoint_refresh (amd_dbgapi_inferior_info &,amd_dbgapi_event_id_t);
  void abort_breakpoint_refresh () noexcept;
  bool before_internal_breakpoint_reset (breakpoint *,program_space *);
  void after_internal_breakpoint_reset ();
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
  using terminal_stage = owner::terminal_stage;
  enum class resume_site : std::uint8_t {
    none, infrun_origin, generic_origin, completion_cpu_scope,
    completion_wave_scope, completion_environment, completion_roster,
    completion_thread_state, cpu_return, wave_return, body_return,
    generic_return, commit_begin, commit_amd, commit_generic
  };
  void resume_require (bool, resume_site);
  void completion_current (bool cpu_running, bool wave_running, bool commit_required);
  enum class refresh_kind : std::uint8_t { none,initial,terminal };
  enum class reset_state : std::uint8_t { idle,armed,consumed };
  enum class breakpoint_site : std::uint8_t {
    none,refresh_begin,refresh_end,reset_begin,reset_bound,reset_candidate,
    reset_consume,reset_mutated,reset_end,unrelated_delete,owned_delete,
    callback_delete,owned_reset,callback_boundary
  };
  void breakpoint_refresh_current (refresh_kind,bool);
  void clear_breakpoint_refresh () noexcept;
  void consume_internal_breakpoint_reset (breakpoint *);
  void remember_breakpoint_failure (const breakpoint *,breakpoint_site,bool) noexcept;
  void breakpoint_check (bool,const breakpoint *,breakpoint_site);
  enum class terminal_origin : std::uint8_t { none,completion,callback_step,callback_continue };
  native_adapter () noexcept = default;
  [[noreturn]] void reject (failure, commit_site = commit_site::none);
  void require (bool, failure, commit_site = commit_site::none);
  template<class Predicate>
  void checkpoint_require (checkpoint_site site, const Predicate &predicate) {
    static_assert (sizeof (Predicate) <= sizeof (checkpoint_diagnostic_scratch::predicate_reference_storage),
                   "checkpoint predicate closure exceeds its separate reservation");
    const bool was_valid=m_owner.selected () && !m_owner.invalid ();
    const bool yes=m_checkpoint_note.evaluate (was_valid,site,predicate);
    require (yes,failure::checkpoint_changed); // Original sticky refusal path.
  }
  void debit (counter, std::uint64_t);
  void api ();
  void entry_maintenance_current ();
  void retain_entry_commit_targets ();
  void retire_entry_maintenance () noexcept;
  void prepare_loaded_maintenance (amd_dbgapi_inferior_info &);
  void loaded_maintenance_current (bool,bool);
  void arm_loaded_maintenance ();
  void retire_loaded_maintenance () noexcept;
  void callback_maintenance_current (bool);
  void arm_callback_maintenance (bool);
  void retire_callback_maintenance () noexcept;
  void terminal_gpu_current ();
  void terminal_maintenance_current (bool);
  void arm_terminal_maintenance (terminal_origin);
  void retire_terminal_maintenance () noexcept;
  void unloaded_maintenance_current (bool);
  void arm_unloaded_maintenance ();
  void retire_unloaded_maintenance () noexcept;
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
  void capture_physical_snapshot ();
  void confirm_physical_snapshot ();
  void seal_physical_snapshot ();
  template<typename Get, typename Id, typename Query, typename Value>
  void query (Get, Id, Query, Value &);
  template<typename Handle, typename Call>
  Handle list_one (Call);
  void client_scope_failed () noexcept;
  static constexpr std::uint64_t constant_and_scratch = 4096;
  static constexpr std::uint64_t snapshot_scalar_scratch = 1024;
  static constexpr std::uint64_t entry_maintenance_scalar_scratch = 256;
  static constexpr std::uint64_t loaded_maintenance_scalar_scratch = sizeof (loaded_maintenance_scratch);
  static constexpr std::uint64_t finish_step_context_scratch
    = sizeof (std::optional<scoped_restore_current_thread>);
  static constexpr std::uint64_t logical_storage () noexcept;
  owner m_owner;
  physical_snapshot m_snapshot; // Private, same selected-lifetime sizeof reservation.
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
  snapshot_publication m_output; // Fixed1536-byte V2 encoder, same retained ledger.
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
  refresh_kind m_refresh_kind=refresh_kind::none;
  reset_state m_reset_state=reset_state::idle;
  std::uint64_t m_refresh_event=0,m_refresh_generation=0,m_reset_address=0;
  breakpoint *m_reset_object=nullptr;
  bp_location *m_reset_location=nullptr;
  std::size_t m_reset_count=0;
  int m_reset_number=0;
  unsigned m_reset_type=0;
  breakpoint_site m_first_breakpoint_site=breakpoint_site::none;
  unsigned m_first_breakpoint_type=0;
  int m_first_breakpoint_number=0;
  native_result_note m_native_result_note {};
  checkpoint_note m_checkpoint_note {};
  completion_sequence m_completion;
  resume_site m_first_resume_site=resume_site::none;
  enum class path : std::uint8_t { none,entry,callback_step,callback_continue,publication,completion };
  enum class callback_progress : std::uint8_t { none,step_ready,step_inflight,continue_ready };
  enum class entry_epoch : std::uint8_t { unused,entry_commit,armed,retired };
  enum class loaded_epoch : std::uint8_t { unused,prospective,armed,in_flight,amd_returned,retired };
  enum class callback_epoch : std::uint8_t { inactive,armed,in_flight,amd_returned };
  callback_epoch m_callback_epoch=callback_epoch::inactive;
  std::uint64_t m_callback_generation=0;
  bool m_callback_stepping=false;
  callback_epoch m_terminal_epoch=callback_epoch::inactive;
  terminal_origin m_terminal_origin=terminal_origin::none;
  terminal_stage m_terminal_stage=terminal_stage::idle;
  std::uint64_t m_terminal_generation=0;
  enum class unloaded_epoch : std::uint8_t { unused,armed,in_flight,retired };
  unloaded_epoch m_unloaded_epoch=unloaded_epoch::unused;
  target_ops_ref m_loaded_process_ref,m_loaded_base_ref,m_loaded_top_ref;
  loaded_epoch m_loaded_epoch=loaded_epoch::unused;
  target_ops_ref m_entry_process_ref,m_entry_top_ref;
  entry_epoch m_entry_epoch=entry_epoch::unused;
  bool m_entry_maintenance_inflight=false;
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
     guards, with the sole128-byte library callback allocation separate.
     The new snapshot adds its own1024-byte fixed scalar/temporary envelope;
     retained snapshot arrays are already included in sizeof(native_adapter).
     Another256 bytes prepay simultaneous borrowed output pointers/closures,
     without silently reusing any of those existing reservations.
     The finish-step event-context guard has a separate sizeof reservation;
     it only borrows existing owners and is constructed from no-thread state. */
  return sizeof (native_adapter) + constant_and_scratch + snapshot_scalar_scratch
    + entry_maintenance_scalar_scratch + loaded_maintenance_scalar_scratch
    + finish_step_context_scratch + sizeof (callback_maintenance_scratch) + sizeof (terminal_maintenance_scratch) + sizeof (unloaded_maintenance_scratch)
    + sizeof (breakpoint_refresh_scratch)
    + sizeof (native_result_diagnostic_scratch)
    + sizeof (checkpoint_diagnostic_scratch) + sizeof (completion_resume_scratch)
    + limits::client_output_bytes + publication_output::scratch_bytes;
}
static_assert (sizeof (native_adapter) + 4096 + 1024 + 256 + sizeof (loaded_maintenance_scratch) + limits::client_output_bytes + 256
               + sizeof (std::optional<scoped_restore_current_thread>)
               + sizeof (callback_maintenance_scratch) + sizeof (terminal_maintenance_scratch) + sizeof (unloaded_maintenance_scratch)
               + sizeof (breakpoint_refresh_scratch)
               + sizeof (native_result_diagnostic_scratch)
               + sizeof (checkpoint_diagnostic_scratch) + sizeof (completion_resume_scratch)
               <= limits::logical_bytes, "all new fixed retained data is prepaid");
static_assert (limits::file_requested_bytes == target_file_read_cap
               && limits::file_rounds == target_file_round_cap,
               "measured executable and file ledger must remain joined");
}
#endif
