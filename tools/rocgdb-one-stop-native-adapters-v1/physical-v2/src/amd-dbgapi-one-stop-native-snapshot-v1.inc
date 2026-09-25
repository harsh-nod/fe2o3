/* SPDX-License-Identifier: GPL-3.0-or-later
   Fixed, source-disabled same-client physical reads. Not a new dbgapi client,
   GDB regcache projection, CLI command, snapshot importer or resume permission. */
namespace amd_owned_one_stop_v1 {
void native_adapter::capture_physical_snapshot () {
  if (!snapshot_capture_available ()) return;
  try {
    require (m_owner.state ()==phase::provisional_gpu_stop && m_path==path::none
             && m_gpu_thread!=nullptr && m_gpu_thread_ref.get ()==m_gpu_thread
             && m_gpu_thread->inf==m_inferior && m_callback_progress==callback_progress::none
             && same (m_owner.m_stop,m_candidate_stop),failure::stop_changed);
    const checkpoint_view cp (m_owner.m_checkpoint.bytes);
    require (cp.u64 (168)==physical_snapshot::output_bytes && cp.u64 (176)==4096
             && cp.u64 (160)!=0 && cp.u64 (160)<=UINT64_MAX-physical_snapshot::output_bytes,
             failure::checkpoint_changed);
    /* One fixed prepaid envelope covers byte initialization/clearing, bounded
       metadata/state comparisons and both seal/revoke paths. Storage is already
       included in sizeof(native_adapter) before selection. Never a second meter. */
    debit (counter::work,physical_snapshot::prepaid_work);
    require (m_snapshot.begin (m_owner,current_identity (),m_candidate_stop,cp.u64 (160)),
             failure::stop_changed);
    auto &actual=info ();
    const amd_dbgapi_wave_id_t wave {m_candidate_stop.wave};
    const amd_dbgapi_architecture_id_t architecture {m_candidate_stop.architecture};
    amd_dbgapi_register_id_t reg {};
    api ();
    const auto mapped=amd_dbgapi_dwarf_register_to_register
      (architecture,physical_snapshot::dwarf_s12,&reg);
    require (mapped==AMD_DBGAPI_STATUS_SUCCESS && reg.handle!=0,failure::native_result);
    amd_dbgapi_size_t size=0;
    query (amd_dbgapi_register_get_info,reg,AMD_DBGAPI_REGISTER_INFO_SIZE,size);
    require (size==physical_snapshot::register_bytes,failure::native_result);
    /* No wave/architecture register list or string allocation. The conversion
       API returns the register of this exact queried architecture. Its read
       independently validates wave/register compatibility and allocation. */
    api (); debit (counter::read_calls,1);
    debit (counter::read_bytes,physical_snapshot::register_bytes);
    const auto reg_status=amd_dbgapi_read_register
      (wave,reg,0,physical_snapshot::register_bytes,m_snapshot.m_register.data ());
    const auto reg_result=reg_status==AMD_DBGAPI_STATUS_SUCCESS
      ? snapshot_read_result::success
      : (reg_status==AMD_DBGAPI_STATUS_ERROR_REGISTER_NOT_AVAILABLE
         ? snapshot_read_result::unavailable : snapshot_read_result::error);
    require (m_snapshot.register_complete (reg.handle,size,reg_result),failure::native_result);
    if (reg_result==snapshot_read_result::unavailable) return; // Typed empty result, not zero bits.

    amd_dbgapi_size_t bytes=physical_snapshot::output_bytes;
    api (); debit (counter::read_calls,1);
    debit (counter::read_bytes,physical_snapshot::output_bytes);
    const auto memory_status=amd_dbgapi_read_memory
      (actual.process_id,wave,AMD_DBGAPI_LANE_NONE,AMD_DBGAPI_ADDRESS_SPACE_GLOBAL,
       cp.u64 (160),&bytes,m_snapshot.m_output.data ());
    const auto memory_result=memory_status==AMD_DBGAPI_STATUS_SUCCESS
      ? snapshot_read_result::success
      : (memory_status==AMD_DBGAPI_STATUS_ERROR_MEMORY_ACCESS
         ? snapshot_read_result::unavailable : snapshot_read_result::error);
    require (m_snapshot.memory_complete (memory_result,bytes),failure::native_result);
    /* SUCCESS may be short: the core clears BOTH value arrays and retains only
       an unavailable reason. No suffix, zero initialization or partial byte
       prefix is misrepresented as a complete physical observation. */
  } catch (...) {
    m_snapshot.revoke (snapshot_issue::native_error);
    m_owner.poison (failure::native_result);
    throw;
  }
}
void native_adapter::confirm_physical_snapshot () {
  if (!snapshot_capture_available ()) return;
  require (m_snapshot.confirm (m_owner,current_identity (),m_candidate_stop),
           failure::stop_changed);
}
void native_adapter::seal_physical_snapshot () {
  if (!snapshot_capture_available ()) return;
  require (m_gpu_thread!=nullptr && inferior_thread ()==m_gpu_thread
           && m_gpu_thread->state==THREAD_STOPPED && !m_gpu_thread->executing ()
           && !m_gpu_thread->resumed () && m_path==path::none
           && m_snapshot.seal (m_owner,current_identity (),m_candidate_stop,m_stop_retired),
           failure::stop_changed);
}
} // namespace amd_owned_one_stop_v1
