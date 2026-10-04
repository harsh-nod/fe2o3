/* SPDX-License-Identifier: GPL-3.0-or-later
   Private same-owner physical publication. Bytes are never imported, copied out
   as an owner, filled by an oracle or made readable by a serialized field. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_PUBLICATION_V2_H
#define GDB_AMD_DBGAPI_ONE_STOP_PUBLICATION_V2_H
#include "amd-dbgapi-one-stop-snapshot-v1.h"
#include "amd-dbgapi-one-stop-checkpoint-v1.h"
namespace amd_owned_one_stop_v1 {
#ifdef FE2O3_ONE_STOP_PUBLICATION_PURE_TEST
struct publication_test;
#endif
inline constexpr bool snapshot_publication_available () noexcept { return false; }
class snapshot_publication final {
  friend class native_adapter;
#ifdef FE2O3_ONE_STOP_PUBLICATION_PURE_TEST
  friend struct publication_test;
#endif
  snapshot_publication () noexcept = default;
  snapshot_publication (const snapshot_publication &)=delete;
  snapshot_publication &operator= (const snapshot_publication &)=delete;
  snapshot_publication (snapshot_publication &&)=delete;
  snapshot_publication &operator= (snapshot_publication &&)=delete;
  static constexpr std::size_t capacity=1536;
  static constexpr std::uint64_t work_for (bool physical) noexcept {
    return physical ? 3*capacity : 256;
  }
  bool append_char (char c) noexcept {
    if (m_size>=capacity-1) return false;
    m_bytes[m_size++]=c; m_bytes[m_size]=0; return true;
  }
  template<std::size_t N> bool literal (const char (&text)[N]) noexcept {
    static_assert (N<=256,"only fixed bounded protocol literals");
    if (N-1>capacity-1-m_size) return false;
    for (std::size_t n=0;n<N-1;++n) m_bytes[m_size++]=text[n];
    m_bytes[m_size]=0; return true;
  }
  template<std::size_t N> bool field (const char (&key)[N],std::uint64_t value) noexcept {
    std::array<char,20> reverse {}; std::size_t count=0;
    do { reverse[count++]=static_cast<char> ('0'+value%10); value/=10; } while (value);
    if (!literal (",") || !literal (key) || !literal ("=\"")) return false;
    while (count) if (!append_char (reverse[--count])) return false;
    return literal ("\"");
  }
  template<std::size_t N> bool hex (const std::array<std::uint8_t,N> &bytes) noexcept {
    static_assert (N==4 || N==272,"closed physical payload sizes");
    if (N*2>capacity-1-m_size) return false;
    constexpr char digits[]="0123456789abcdef";
    for (const auto byte:bytes) {
      m_bytes[m_size++]=digits[byte>>4]; m_bytes[m_size++]=digits[byte&15];
    }
    m_bytes[m_size]=0; return true;
  }
  static bool prepaid (const owner &o,const usage &before,bool physical) noexcept {
    const auto now=o.consumed (); const auto cost=work_for (physical);
    return o.selected () && !o.invalid () && before.work<=limits::work
      && cost<=limits::work-before.work && now.work==before.work+cost
      && now.logical_bytes==before.logical_bytes
      && now.api==before.api && now.read_bytes==before.read_bytes
      && now.read_calls==before.read_calls && now.rows==before.rows
      && now.file_requested_bytes==before.file_requested_bytes
      && now.file_observed_bytes==before.file_observed_bytes
      && now.file_probe_bytes==before.file_probe_bytes && now.file_rounds==before.file_rounds
      && now.proc_requested_bytes==before.proc_requested_bytes;
  }
  bool still_current (const owner &o,const identity &id,const physical_snapshot &s,
                      const gpu_stop &stop) const noexcept {
    const bool available=s.m_stage==physical_snapshot::stage::current
      && s.m_issue==snapshot_issue::none;
    const bool unavailable=s.m_stage==physical_snapshot::stage::unavailable_current
      && (s.m_issue==snapshot_issue::register_unavailable
          || s.m_issue==snapshot_issue::memory_unavailable || s.m_issue==snapshot_issue::short_memory);
    return (available || unavailable) && s.live_ledger (o)
      && o.state ()==phase::current_gpu_stop && same (id,s.m_identity) && same (stop,s.m_stop);
  }
  bool render (const owner &o,const usage &before,const identity &id,const diagnostic &row,
               const physical_snapshot &s,const gpu_stop &stop,const checkpoint &checkpoint) noexcept {
    const bool physical=row.state==phase::current_gpu_stop;
    // No protected byte read or buffer mutation before these ownership/debit checks.
    if (!prepaid (o,before,physical) || !complete (id) || row.why!=failure::none
        || row.sequence==0 || row.sequence>limits::rows || row.sequence>before.rows
        || row.state==phase::disabled || row.state>=phase::invalid) return false;
    if (physical) {
      if (m_physical_attempted) return false;
      m_physical_attempted=true; // A refused/partial attempt is never retried.
      if (!still_current (o,id,s,stop)) return false;
      const checkpoint_view cp (checkpoint.bytes);
      if (checkpoint.entry==0 || checkpoint.entry>UINT64_MAX-80
          || stop.pc!=checkpoint.entry+80 || stop.completion_base!=checkpoint.signal_base
          || stop.queue!=checkpoint.queue || checkpoint.code_object==0
          || checkpoint.record_address==0 || cp.u64 (80)!=checkpoint.entry
          || cp.u64 (128)!=checkpoint.signal_base || cp.u64 (160)!=s.m_address
          || (s.m_address&4095)!=0 || cp.u64 (168)!=272 || cp.u64 (176)!=4096
          || cp.u64 (184)!=s.m_address+8 || cp.u64 (192)!=256
          || cp.u64 (56)>UINT64_MAX-6144 || cp.u64 (56)+6144!=checkpoint.entry
          || cp.u64 (56)+1984!=cp.u64 (72) || cp.u64 (144)==0 || s.m_register_id==0)
        return false;
    }
    m_size=0; m_bytes[0]=0;
    if (!literal ("=fe2o3-owned-one-stop-v2") || !field ("s",row.sequence)
        || !field ("p",static_cast<unsigned> (row.state)) || !field ("pid",id.pid)
        || !field ("i",id.inferior_number) || !field ("a",id.process)
        || !field ("t",id.host_thread) || !field ("start",id.pid_start)) return false;
    if (physical) {
      const checkpoint_view cp (checkpoint.bytes);
      if (!field ("g",stop.thread) || !field ("w",stop.wave) || !field ("wg",stop.workgroup)
          || !field ("d",stop.dispatch) || !field ("q",stop.queue) || !field ("ag",stop.agent)
          || !field ("ar",stop.architecture) || !field ("pc",stop.pc)
          || !field ("c",stop.completion_base) || !field ("cp",checkpoint.record_address)
          || !field ("co",checkpoint.code_object) || !field ("entry",checkpoint.entry)
          || !field ("desc",cp.u64 (72)) || !field ("kernarg",cp.u64 (144))
          || !field ("out",s.m_address) || !field ("regid",s.m_register_id)
          || !field ("dwarf",44) || !field ("rbytes",4) || !field ("mbytes",272)
          || !literal (",status=\"")) return false;
      switch (s.m_issue) {
        case snapshot_issue::none: if (!literal ("available")) return false; break;
        case snapshot_issue::register_unavailable:
          if (!literal ("register-unavailable")) return false;
          break;
        case snapshot_issue::memory_unavailable:
          if (!literal ("memory-unavailable")) return false;
          break;
        case snapshot_issue::short_memory: if (!literal ("short-memory")) return false; break;
        default: return false;
      }
      if (!literal ("\",reg=\"")) return false;
      if (s.m_issue==snapshot_issue::none && !hex (s.m_register)) return false;
      if (!literal ("\",mem=\"")) return false;
      if (s.m_issue==snapshot_issue::none && !hex (s.m_output)) return false;
      if (!literal ("\"")) return false;
    }
    return literal ("\n");
  }
  const char *data () const noexcept { return m_bytes.data (); }
  void forget () noexcept { m_size=0; m_bytes[0]=0; }
  std::array<char,capacity> m_bytes {};
  std::size_t m_size=0;
  bool m_physical_attempted=false;
};
static_assert (sizeof (snapshot_publication)<=1560,"fixed complete output state");
} // namespace amd_owned_one_stop_v1
#endif
