/* SPDX-License-Identifier: GPL-3.0-or-later
   Native object identities originate only in this actual same-client list and
   scalar query sequence. Its output remains borrowed in-process solib data. */
namespace amd_owned_one_stop_v1 {
bool native_adapter::selected_object (amd_dbgapi_inferior_info &actual,
                                     amd_dbgapi_code_object_id_t &object,
                                     std::ptrdiff_t &bias,const char *&locator) {
  require (&actual==&info () && m_owner.check_owner (current_identity ()),failure::owner_changed);
  if (m_owner.state ()==phase::awaiting_target_completion) {
    require (m_owner.m_callback && !m_query_output
             && m_owner.begin_client_empty_objects (),failure::allocation);
    m_query_output=true;
    try {
      std::size_t count=0; amd_dbgapi_code_object_id_t *rows=nullptr;
      api ();
      const auto status=amd_dbgapi_process_code_object_list
        (actual.process_id,&count,&rows,nullptr);
      require (m_owner.client_empty_objects_returned
               (reinterpret_cast<std::uintptr_t> (rows),count,status),failure::allocation);
      m_query_output=false;
      return false; // No object/URI/permission is fabricated for the empty list.
    } catch (...) { m_owner.poison (failure::native_result); client_scope_failed (); throw; }
  }
  require (m_owner.state ()==phase::host_running && m_owner.m_callback
           && !m_origin_pending && !m_origin_complete,failure::order);
  object=list_one<amd_dbgapi_code_object_id_t>
    ([&actual] (std::size_t *count,amd_dbgapi_code_object_id_t **rows) {
      return amd_dbgapi_process_code_object_list (actual.process_id,count,rows,nullptr);
    }); // The one real list allocation is freed BEFORE the URI query.
  query (amd_dbgapi_code_object_get_info,object,AMD_DBGAPI_CODE_OBJECT_INFO_LOAD_ADDRESS,bias);
  require (!m_query_output && m_owner.begin_client_locator (),failure::allocation);
  m_query_output=true;
  try {
    char *uri=nullptr; api ();
    const auto status=amd_dbgapi_code_object_get_info
      (object,AMD_DBGAPI_CODE_OBJECT_INFO_URI_NAME,sizeof (uri),&uri);
    const auto bytes=m_owner.m_allocation_bytes;
    require (uri!=nullptr && m_owner.client_output_returned
             (reinterpret_cast<std::uintptr_t> (uri),bytes,status)
             && bytes>=1 && bytes<=m_work.locator.size (),failure::allocation);
    debit (counter::work,bytes*2);
    for (std::size_t n=0;n<bytes;++n) m_work.locator[n]=static_cast<std::uint8_t> (uri[n]);
    require (m_owner.releasing_client_output (reinterpret_cast<std::uintptr_t> (uri)),
             failure::allocation);
    ::free (uri); m_query_output=false;
    std::uint64_t address=0;
    require (fixed_memory_locator (reinterpret_cast<const char *> (m_work.locator.data ()),
                                   bytes,m_owner.m_identity.pid,address),
             failure::checkpoint_changed);
    m_object=object; m_object_bias=bias; m_original_address=address; m_origin_pending=true;
    locator=reinterpret_cast<const char *> (m_work.locator.data ());
    return true;
  } catch (...) { m_owner.poison (failure::native_result); client_scope_failed (); throw; }
}
void native_adapter::read_runtime_root (std::uint64_t address) {
  require (address!=0 && (address&7)==0,failure::checkpoint_changed);
  static_assert (sizeof (runtime_root)==40 && std::is_standard_layout<runtime_root>::value,
                 "exact retained target root ABI");
  read (address,reinterpret_cast<std::uint8_t *> (&m_root_read),sizeof (m_root_read));
}
void native_adapter::inspect_empty_transport (const checkpoint_view &cp) {
  std::uint64_t word=0;
  read_u64 (cp.u64 (112),word); require (word==0,failure::checkpoint_changed);
  read_u64 (cp.u64 (120),word); require (word==0,failure::checkpoint_changed);
  read_u64 (cp.u64 (136),word); require (word==1,failure::checkpoint_changed);
  read (cp.u64 (88),m_work.image_chunk.data (),64);
  for (std::size_t n=0;n<64;++n)
    require (m_work.image_chunk[n]==(n==0 ? 1 : 0),failure::checkpoint_changed);
  read (cp.u64 (144),m_work.kernarg_read.data (),264);
  for (std::size_t n=0;n<264;++n)
    require (m_work.kernarg_read[n]==m_candidate.bytes[480+n],failure::checkpoint_changed);
  constexpr std::uint64_t first=UINT64_C (0x0123456789abcdef);
  constexpr std::uint64_t last=UINT64_C (0xfedcba9876543210);
  for (std::size_t at=0;at<272;) {
    const auto count=std::min<std::size_t> (256,272-at);
    std::uint64_t address=0; require (checked_sum (cp.u64 (160),at,address),failure::checkpoint_changed);
    read (address,m_work.image_chunk.data (),count);
    for (std::size_t n=0;n<count;++n) {
      const auto index=at+n;
      const std::uint8_t expected=index<8 ? static_cast<std::uint8_t> (first>>(index*8))
        : index<264 ? 0xa5 : static_cast<std::uint8_t> (last>>((index-264)*8));
      require (m_work.image_chunk[n]==expected,failure::checkpoint_changed);
    }
    at+=count;
  }
  // Deliberately not the whole8MiB ring or unused3824 output bytes.
}
void native_adapter::inspect_checkpoint (bool recheck) {
  require (m_owner.check_owner (current_identity ()) && m_origin_complete
           && m_file_passes>=1 && m_checkpoint_passes==(recheck ? 1u : 0u)
           && m_host==inferior_thread () && m_host->state!=THREAD_EXITED
           && (!recheck || (m_infrun_origin && !m_infrun_step && !m_infrun_user_step))
           && !m_host->executing () && !m_host->resumed (),
           failure::checkpoint_changed);
  ++m_checkpoint_passes; // Consume the fixed pass before any possible read.
  require (read_start_ticks ()==m_start_ticks,failure::owner_changed);
  current_executable ();
  if (recheck) hash_executable ();
  debit (counter::work,2176+4096+512);
  auto *cache=get_thread_regcache (m_host);
  ULONGEST argument=0;
  require (regcache_read_pc (cache)==m_checkpoint_pc
           && regcache_cooked_read_unsigned (cache,AMD64_RDI_REGNUM,&argument)==REG_VALID
           && argument!=0 && (argument&7)==0,failure::checkpoint_changed);
  if (recheck) require (argument==m_owner.m_checkpoint.record_address,failure::checkpoint_changed);
  m_candidate.record_address=argument; m_candidate.host_pc=m_checkpoint_pc;
  read (argument,m_work.checkpoint_read.data (),m_work.checkpoint_read.size ());
  m_candidate.bytes=m_work.checkpoint_read;
  const checkpoint_view cp (m_candidate.bytes);
  require (cp.valid (m_owner.m_identity.pid),failure::checkpoint_changed);
  std::uint64_t base=0,descriptor=0,entry=0;
  require (apply_bias (0,m_object_bias,base) && apply_bias (1984,m_object_bias,descriptor)
           && apply_bias (6144,m_object_bias,entry) && base==cp.u64 (56)
           && descriptor==cp.u64 (72) && entry==cp.u64 (80),failure::checkpoint_changed);
  m_candidate.entry=entry; m_candidate.signal_base=cp.u64 (128);
  m_candidate.code_object=m_object.handle;
  sole_queue (cp,recheck);
  inspect_empty_transport (cp);
  hash_inferior (m_original_address,5312,fixed_object_sha);
  hash_inferior (base,12288,fixed_image_sha);
  hash_inferior (cp.u64 (200),4096,fixed_trap_backing_sha);
  read (argument,m_work.checkpoint_read.data (),m_work.checkpoint_read.size ());
  require (m_work.checkpoint_read==m_candidate.bytes
           && regcache_read_pc (cache)==m_checkpoint_pc
           && read_start_ticks ()==m_start_ticks
           && m_owner.check_owner (current_identity ()),failure::checkpoint_changed);
  read_runtime_root (cp.u64 (216));
  require (m_callback_pc!=0 && m_root_read.breakpoint==m_callback_pc
           && m_callback_object==m_owner.m_origin_callback_object,
           failure::checkpoint_changed);
  if (recheck) {
    require (same (m_owner.m_checkpoint,m_candidate)
             && m_owner.recheck_checkpoint_root (current_identity (),cp.u64 (216),m_root_read),
             failure::checkpoint_changed);
  } else {
    require (m_owner.bind_checkpoint_root (current_identity (),cp.u64 (216),m_root_read),
             failure::checkpoint_changed);
  }
}
}
