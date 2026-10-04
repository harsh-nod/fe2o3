/* SPDX-License-Identifier: GPL-3.0-or-later
   Concrete same-client target/file I/O and bounded allocator adapters.
   Included after the existing real target/inferior definitions. */
namespace amd_owned_one_stop_v1 {
native_adapter &native_adapter::instance () noexcept {
  static native_adapter retained;
  return retained;
}
[[noreturn]] void native_adapter::reject (failure why) {
  m_snapshot.revoke (snapshot_issue::owner_changed);
  m_owner.poison (why);
  error (_("fixed owned one-stop native relation refused (%u)"),
         static_cast<unsigned> (m_owner.why ()));
}
void native_adapter::require (bool yes, failure why) {
  if (!yes || m_owner.invalid ()) reject (why);
}
void native_adapter::debit (counter kind, std::uint64_t amount) {
  if (!m_owner.debit (kind, amount)) reject (failure::budget);
}
void native_adapter::api () { debit (counter::api,1); debit (counter::work,8); }
void native_adapter::read (std::uint64_t address, std::uint8_t *out, std::size_t bytes) {
  require (m_inferior != nullptr && current_inferior () == m_inferior
           && bytes != 0 && out != nullptr && address != 0
           && bytes <= 776 && address <= UINT64_MAX-bytes, failure::owner_changed);
  debit (counter::read_calls,1); debit (counter::read_bytes,bytes);
  if (target_read_memory (address,out,bytes) != 0) reject (failure::native_result);
}
void native_adapter::hash_input (const std::uint8_t *bytes, std::size_t count, bool file) {
  std::uint64_t blocks=0;
  require (count<=256 && m_hash.input_blocks (count,blocks),failure::budget);
  if (file) debit (counter::file_rounds,blocks*64);
  else { debit (counter::work,count); debit (counter::work,blocks*64); }
  require (m_hash.update (bytes,count),failure::budget);
}
void native_adapter::hash_finish (const digest_bytes &expected, bool file) {
  debit (file ? counter::file_rounds : counter::work,m_hash.finish_blocks ()*64);
  require (m_hash.finish (m_digest) && m_digest==expected,failure::checkpoint_changed);
}
void native_adapter::hash_inferior (std::uint64_t address, std::uint64_t bytes,
                                   const digest_bytes &expected) {
  require (bytes==5312 || bytes==12288 || bytes==4096,failure::checkpoint_changed);
  m_hash=sha256 {}; // Fresh digest only; the owning resource ledger never resets.
  for (std::uint64_t at=0;at<bytes;) {
    const auto n=static_cast<std::size_t> (std::min<std::uint64_t> (256,bytes-at));
    std::uint64_t current=0;
    require (checked_sum (address,at,current),failure::checkpoint_changed);
    read (current,m_work.image_chunk.data (),n);
    hash_input (m_work.image_chunk.data (),n,false); at+=n;
  }
  hash_finish (expected,false);
}
void native_adapter::read_u64 (std::uint64_t address, std::uint64_t &value) {
  read (address,m_work.image_chunk.data (),8); debit (counter::work,8); value=0;
  for (unsigned n=0;n!=8;++n) value|=std::uint64_t (m_work.image_chunk[n])<<(8*n);
}
void native_adapter::hash_executable () {
  require (m_exec_fd>=0 && m_file_passes<2,failure::checkpoint_changed);
  ++m_file_passes; // Consume before lseek/read: any outcome cannot retry.
  struct stat before {},after {};
  require (::fstat (m_exec_fd,&before)==0 && S_ISREG (before.st_mode)
           && before.st_size==static_cast<off_t> (target_bytes)
           && before.st_dev==m_exec_identity.st_dev && before.st_ino==m_exec_identity.st_ino
           && before.st_size==m_exec_identity.st_size
           && before.st_mtim.tv_sec==m_exec_identity.st_mtim.tv_sec
           && before.st_mtim.tv_nsec==m_exec_identity.st_mtim.tv_nsec
           && before.st_ctim.tv_sec==m_exec_identity.st_ctim.tv_sec
           && before.st_ctim.tv_nsec==m_exec_identity.st_ctim.tv_nsec,
           failure::checkpoint_changed);
  require (::lseek (m_exec_fd,0,SEEK_SET)==0,failure::native_result);
  m_hash=sha256 {};
  for (std::uint64_t at=0;at<target_bytes;) {
    const auto n=static_cast<std::size_t> (std::min<std::uint64_t> (256,target_bytes-at));
    debit (counter::file_requested_bytes,n);
    const auto got=::read (m_exec_fd,m_work.image_chunk.data (),n);
    if (got>0) debit (counter::file_observed_bytes,static_cast<std::uint64_t> (got));
    require (got==static_cast<ssize_t> (n),failure::native_result); // No EINTR/short-read retry.
    hash_input (m_work.image_chunk.data (),n,true); at+=n;
  }
  debit (counter::file_requested_bytes,1);
  const auto eof=::read (m_exec_fd,m_work.image_chunk.data (),1);
  if (eof>0) debit (counter::file_probe_bytes,static_cast<std::uint64_t> (eof));
  require (eof==0,failure::checkpoint_changed);
  hash_finish (target_sha,true);
  require (::fstat (m_exec_fd,&after)==0 && after.st_dev==before.st_dev
           && after.st_ino==before.st_ino && after.st_size==before.st_size
           && after.st_mtim.tv_sec==before.st_mtim.tv_sec
           && after.st_mtim.tv_nsec==before.st_mtim.tv_nsec
           && after.st_ctim.tv_sec==before.st_ctim.tv_sec
           && after.st_ctim.tv_nsec==before.st_ctim.tv_nsec,
           failure::checkpoint_changed);
}
void *native_adapter::allocate_client (std::size_t bytes) {
  if (!m_query_output) {
    if (m_owner.selected () && (m_owner.invalid () || m_owner.state ()==phase::disabled)) {
      m_owner.poison (failure::allocation); return nullptr;
    }
    return ::malloc (bytes); // Unselected/ordinary baseline calls unchanged.
  }
  if (m_in_allocate || !m_owner.request_client_output (bytes)) {
    m_owner.poison (failure::allocation); return nullptr;
  }
  if (m_owner.m_allocation_empty) return nullptr; // Exact size0 callback; never malloc(0).
  m_in_allocate=true;
  void *const result=::malloc (bytes);
  m_in_allocate=false;
  if (!m_owner.client_output_allocated (reinterpret_cast<std::uintptr_t> (result))) {
    if (result) ::free (result);
    return nullptr;
  }
  return result;
}
void native_adapter::deallocate_client (void *pointer) {
  if (!m_query_output) { ::free (pointer); return; }
  if (pointer==nullptr) return; // No-op, never an object identity or release witness.
  if (m_owner.releasing_client_output (reinterpret_cast<std::uintptr_t> (pointer)))
    ::free (pointer);
  else m_owner.poison (failure::allocation);
}
void native_adapter::client_scope_failed () noexcept {
  // Only this exact allocation may be freed after a failed/throwing call.
  if (m_owner.m_allocation_pointer) {
    auto *p=reinterpret_cast<void *> (m_owner.m_allocation_pointer);
    if (m_owner.releasing_client_output (m_owner.m_allocation_pointer)) ::free (p);
  }
  m_query_output=false; m_in_allocate=false;
}
template<typename Handle,typename Call>
Handle native_adapter::list_one (Call call) {
  static_assert (sizeof (Handle)==8,"fixed installed native handle width");
  require (!m_query_output && m_owner.begin_client_output (sizeof (Handle)),failure::allocation);
  m_query_output=true;
  try {
    std::size_t count=0; Handle *rows=nullptr;
    api ();
    const auto status=call (&count,&rows); // Actual API allocates only through guarded callback.
    require (count==1 && rows!=nullptr
             && m_owner.client_output_returned (reinterpret_cast<std::uintptr_t> (rows),
                                                sizeof (Handle),status),
             failure::allocation);
    const Handle only=*rows;
    require (only.handle!=0,failure::owner_changed);
    require (m_owner.releasing_client_output (reinterpret_cast<std::uintptr_t> (rows)),
             failure::allocation);
    ::free (rows); m_query_output=false; return only;
  } catch (...) { m_owner.poison (failure::native_result); client_scope_failed (); throw; }
}
template<typename Get,typename Id,typename Query,typename Value>
void native_adapter::query (Get get,Id id,Query query,Value &value) {
  static_assert (std::is_trivially_copyable<Value>::value,"fixed scalar query output");
  static_assert (sizeof (Value)<=24,"closed fixed scalar query roster");
  api ();
  const auto status=get (id,query,sizeof (value),&value);
  require (status==AMD_DBGAPI_STATUS_SUCCESS,failure::native_result);
}
}
