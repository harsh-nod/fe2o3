/* SPDX-License-Identifier: MIT
   Private positive-only queue-health profile. No ERROR-state emulation.
   Facts enter only from the producer's live driver and lifecycle call sites. */
#ifndef AMD_DBGAPI_QUEUE_HEALTH_V1_H
#define AMD_DBGAPI_QUEUE_HEALTH_V1_H
#include <cstddef>
#include <cstdint>
namespace amd::dbgapi::detail
{
class queue_health_v1 final
{
  bool m_baseline_started = false;
  bool m_baseline_finished = false;
  bool m_empty = false;
  bool m_tainted = false;
  std::uint8_t m_queries = 0;
public:
  static constexpr std::uint8_t query_limit = 10;
  void taint () noexcept { m_tainted = true; }
  bool begin_baseline () noexcept
  {
    if (m_baseline_started || m_tainted) { taint (); return false; }
    m_baseline_started = true;
    return true;
  }
  void finish_baseline (bool observed_empty) noexcept
  {
    if (!m_baseline_started || m_baseline_finished)
      { taint (); return; }
    m_baseline_finished = true;
    m_empty = observed_empty;
    if (!observed_empty) taint ();
  }
  bool candidate () const noexcept
  {
    return m_baseline_finished && m_empty && !m_tainted;
  }
  bool take_query () noexcept
  {
    if (!candidate () || m_queries == query_limit)
      { taint (); return false; }
    ++m_queries; // Consume before the sole driver call, regardless of result.
    return true;
  }
  std::uint8_t queries () const noexcept { return m_queries; }
};
static_assert (sizeof (queue_health_v1) == 5, "fixed private health ledger");
// A fresh snapshot is not itself an API queue state. This predicate is used
// only after the retained process lifecycle qualifies take_query().
template <typename Snapshot, typename Mask>
bool same_healthy_queue_v1 (const Snapshot &birth, const Snapshot &now,
                            Mask new_queue, Mask no_exception) noexcept
{
  return birth.exception_status_complete && now.exception_status_complete
    && birth.state == decltype (birth.state) {}
    && now.state == decltype (now.state) {}
    && birth.exception_status == new_queue
    && now.exception_status == no_exception
    && birth.queue_id == now.queue_id && birth.gpu_id == now.gpu_id
    && birth.queue_type == now.queue_type
    && birth.ring_base_address == now.ring_base_address
    && birth.ring_size == now.ring_size
    && birth.write_pointer_address == now.write_pointer_address
    && birth.read_pointer_address == now.read_pointer_address
    && birth.ctx_save_restore_address == now.ctx_save_restore_address
    && birth.ctx_save_restore_area_size == now.ctx_save_restore_area_size;
}
}
#endif
