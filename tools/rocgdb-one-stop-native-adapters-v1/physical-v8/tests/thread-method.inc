simd_lanes_mask_t
thread_info::active_simd_lanes_mask ()
{
  gdb_assert (this->inf != nullptr);

  scoped_restore_current_thread restore_thread;
  switch_to_inferior_no_thread (this->inf);
  gdbarch *arch = target_thread_architecture (this->ptid);

  if (gdbarch_active_lanes_mask_p (arch))
    return gdbarch_active_lanes_mask (arch, this);

  /* Default: only one lane is active, lane 0.  */
  return 1;
}
