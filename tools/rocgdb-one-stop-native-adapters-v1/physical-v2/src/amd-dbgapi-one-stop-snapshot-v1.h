/* SPDX-License-Identifier: GPL-3.0-or-later
   Closed physical read-set state. Empty construction grants nothing.
   Only the actual same-client native adapter can fill or seal these bytes.
   No serialized importer, public value accessor or independent resource meter. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_SNAPSHOT_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_SNAPSHOT_V1_H
#include "amd-dbgapi-owned-one-stop-v1.h"
namespace amd_owned_one_stop_v1 {
#ifdef FE2O3_ONE_STOP_SNAPSHOT_PURE_TEST
struct snapshot_test;
#endif
/* A separately reviewed source change is required before this read set can
   execute. Existing selection availability does not enable physical capture. */
inline constexpr bool snapshot_capture_available () noexcept { return false; }
enum class snapshot_issue : std::uint8_t {
  none, not_started, register_unavailable, memory_unavailable, short_memory,
  native_error, register_shape, owner_changed, stop_changed, order, resumed
};
enum class snapshot_read_result : std::uint8_t { success, unavailable, error };
class physical_snapshot final {
public:
  physical_snapshot () noexcept = default;
  physical_snapshot (const physical_snapshot &) = delete;
  physical_snapshot &operator= (const physical_snapshot &) = delete;
  physical_snapshot (physical_snapshot &&) = delete;
  physical_snapshot &operator= (physical_snapshot &&) = delete;
private:
  friend class native_adapter;
  friend class snapshot_publication;
#ifdef FE2O3_ONE_STOP_SNAPSHOT_PURE_TEST
  friend struct snapshot_test;
#endif
  static constexpr std::uint64_t dwarf_s12 = 44;
  static constexpr std::size_t register_bytes = 4, output_bytes = 272;
  static constexpr std::uint64_t prepaid_work = 4096;
  enum class stage : std::uint8_t {
    idle, register_pending, memory_pending, provisional, unavailable_pending,
    confirmed, unavailable_confirmed, current, unavailable_current, revoked
  };
  bool live_ledger (const owner &o) const noexcept {
    const auto u=o.consumed ();
    return m_budget_owner==&o && o.selected () && !o.invalid ()
      && u.logical_bytes==m_floor.logical_bytes
      && u.api>=m_floor.api && u.read_bytes>=m_floor.read_bytes
      && u.read_calls>=m_floor.read_calls && u.work>=m_floor.work
      && u.rows>=m_floor.rows && u.file_requested_bytes>=m_floor.file_requested_bytes
      && u.file_observed_bytes>=m_floor.file_observed_bytes
      && u.file_probe_bytes>=m_floor.file_probe_bytes && u.file_rounds>=m_floor.file_rounds
      && u.proc_requested_bytes>=m_floor.proc_requested_bytes;
  }
  void clear_values () noexcept {
    m_register.fill (0); m_output.fill (0);
  }
  bool refuse (snapshot_issue why) noexcept {
    if (m_stage!=stage::revoked) {
      clear_values (); m_issue=why; m_stage=stage::revoked;
    }
    return false;
  }
  bool begin (const owner &o, const identity &id, const gpu_stop &stop,
              std::uint64_t address) noexcept {
    if (m_stage!=stage::idle) return refuse (snapshot_issue::order);
    const auto u=o.consumed ();
    if (!o.selected () || o.invalid () || o.state ()!=phase::provisional_gpu_stop
        || !complete (id) || !stop.thread || !stop.wave || !stop.workgroup
        || !stop.dispatch || !stop.queue || !stop.agent || !stop.architecture
        || !stop.pc || !stop.completion_base
        || address==0 || address>UINT64_MAX-output_bytes
        || u.logical_bytes<sizeof (physical_snapshot)
        || u.logical_bytes>limits::logical_bytes || u.work<prepaid_work)
      return refuse (snapshot_issue::owner_changed);
    m_budget_owner=&o; m_floor=u; m_identity=id; m_stop=stop;
    m_address=address; m_stage=stage::register_pending; m_issue=snapshot_issue::none;
    return true;
  }
  bool register_complete (std::uint64_t actual_register, std::uint64_t size,
                          snapshot_read_result result) noexcept {
    if (m_stage!=stage::register_pending) return refuse (snapshot_issue::order);
    if (!actual_register || size!=register_bytes)
      return refuse (snapshot_issue::register_shape);
    m_register_id=actual_register;
    if (result==snapshot_read_result::error) return refuse (snapshot_issue::native_error);
    if (result==snapshot_read_result::unavailable) {
      clear_values (); m_issue=snapshot_issue::register_unavailable;
      m_stage=stage::unavailable_pending; return true;
    }
    m_stage=stage::memory_pending; return true;
  }
  bool memory_complete (snapshot_read_result result, std::uint64_t actual) noexcept {
    if (m_stage!=stage::memory_pending) return refuse (snapshot_issue::order);
    if (actual>output_bytes || result==snapshot_read_result::error
        || (result==snapshot_read_result::unavailable && actual!=0))
      return refuse (snapshot_issue::native_error);
    if (result==snapshot_read_result::unavailable || actual!=output_bytes) {
      clear_values ();
      m_issue=result==snapshot_read_result::unavailable
        ? snapshot_issue::memory_unavailable : snapshot_issue::short_memory;
      m_stage=stage::unavailable_pending; return true;
    }
    m_stage=stage::provisional; return true;
  }
  bool confirm (const owner &o, const identity &id, const gpu_stop &stop) noexcept {
    if (m_stage!=stage::provisional && m_stage!=stage::unavailable_pending)
      return refuse (snapshot_issue::order);
    if (!live_ledger (o) || o.state ()!=phase::current_gpu_stop || !same (id,m_identity))
      return refuse (snapshot_issue::owner_changed);
    if (!same (stop,m_stop)) return refuse (snapshot_issue::stop_changed);
    m_stage=m_stage==stage::provisional ? stage::confirmed : stage::unavailable_confirmed;
    return true;
  }
  bool seal (const owner &o, const identity &id, const gpu_stop &stop,
             bool actual_retirement_completed) noexcept {
    if (m_stage!=stage::confirmed && m_stage!=stage::unavailable_confirmed)
      return refuse (snapshot_issue::order);
    if (!actual_retirement_completed || !live_ledger (o)
        || o.state ()!=phase::current_gpu_stop || !same (id,m_identity))
      return refuse (snapshot_issue::owner_changed);
    if (!same (stop,m_stop)) return refuse (snapshot_issue::stop_changed);
    m_stage=m_stage==stage::confirmed ? stage::current : stage::unavailable_current;
    return true;
  }
  bool readable (const owner &o, const identity &id, const gpu_stop &stop) const noexcept {
    return m_stage==stage::current && live_ledger (o)
      && o.state ()==phase::current_gpu_stop && same (id,m_identity) && same (stop,m_stop);
  }
  void revoke (snapshot_issue why) noexcept {
    if (m_stage!=stage::idle && m_stage!=stage::revoked) (void) refuse (why);
  }
  /* API destinations are private and never returned to a caller. They remain
     unreadable until both independent full native stop observations agree and
     the actual post-auto-delete safe point seals this same retained owner. */
  std::array<std::uint8_t,register_bytes> m_register {};
  std::array<std::uint8_t,output_bytes> m_output {};
  const owner *m_budget_owner=nullptr;
  usage m_floor {};
  identity m_identity {};
  gpu_stop m_stop {};
  std::uint64_t m_address=0, m_register_id=0;
  stage m_stage=stage::idle;
  snapshot_issue m_issue=snapshot_issue::not_started;
};
static_assert (187 + 4 <= limits::api, "existing full query census plus fixed snapshot API calls");
static_assert (sizeof (physical_snapshot)<=1024, "bounded retained physical read set");
static_assert (!std::is_copy_constructible<physical_snapshot>::value
               && !std::is_move_constructible<physical_snapshot>::value,
               "physical read set cannot escape by value");
} // namespace amd_owned_one_stop_v1
#endif
