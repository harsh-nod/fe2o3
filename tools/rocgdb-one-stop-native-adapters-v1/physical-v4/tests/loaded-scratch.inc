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
