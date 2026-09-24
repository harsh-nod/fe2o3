/* Same-client stopped-wave observation, never execution/read authority.
   SPDX-License-Identifier: GPL-3.0-or-later
   This file is part of GDB; GPL version 3 or (at your option) any later version.
   Derived from the bounded ROCdbgapi observation design in
   amd-dbgapi-runtime-observation-v1.h. The noqueue ledger remains unchanged. */
#ifndef GDB_AMD_DBGAPI_STOPPED_WAVE_OBSERVATION_V1_H
#define GDB_AMD_DBGAPI_STOPPED_WAVE_OBSERVATION_V1_H

#include <array>
#include <cstddef>
#include <cstdint>

struct thread_info;
struct amd_dbgapi_inferior_info;

namespace amd_stopped_wave_observation_v1
{
enum class kind : std::uint32_t {
  attach_complete = 1, runtime_complete = 2, code_objects_complete = 3,
  breakpoint_complete = 4, wave_stop_complete = 5, invalidated = 6
};
enum class reason : std::uint32_t {
  none = 0, foreign_owner = 1, capacity = 2, query_failed = 3,
  event_identity = 4, handler_unwind = 5, acknowledgement = 6,
  preloaded_runtime = 7, runtime_unloaded = 8, runtime_restriction = 9,
  unsupported_event = 10, callback_identity = 11, callback_nested = 12,
  detach = 13, inferior_exit = 14, inferior_exec = 15, inferior_fork = 16,
  object_removed = 17, output_failure = 18, setup_failure = 19,
  resume_attempt = 20, thread_changed = 21, wave_identity = 22,
  unsupported_profile = 23, stop_changed = 24, required_identity_unavailable = 25
};

/* Numeric observation only. OS queue and packet IDs may legitimately be zero.
   No PC, device address, register value, memory bytes or authorization bit. */
struct stop_identity {
  std::uint64_t wave = 0, workgroup = 0, dispatch = 0, queue = 0;
  std::uint64_t agent = 0, architecture = 0, os_queue = 0, packet = 0, os_agent = 0;
  std::uint64_t private_bytes = 0, group_bytes = 0;
  std::uint32_t state = 0, stop_reason = 0, lanes = 0, wave_index = 0;
  std::array<std::uint32_t, 3> coordinate {};
  std::array<std::uint32_t, 3> grid {};
  std::array<std::uint32_t, 3> workgroup_size {};
  std::uint32_t queue_type = 0, queue_state = 0, elf_machine = 0;
};
inline bool same_identity (const stop_identity &a,
                           const stop_identity &b) noexcept
{
  return a.wave == b.wave && a.workgroup == b.workgroup
    && a.dispatch == b.dispatch && a.queue == b.queue && a.agent == b.agent
    && a.architecture == b.architecture && a.os_queue == b.os_queue
    && a.packet == b.packet && a.os_agent == b.os_agent && a.private_bytes == b.private_bytes
    && a.group_bytes == b.group_bytes && a.state == b.state
    && a.stop_reason == b.stop_reason && a.lanes == b.lanes
    && a.wave_index == b.wave_index && a.coordinate == b.coordinate
    && a.grid == b.grid && a.workgroup_size == b.workgroup_size
    && a.queue_type == b.queue_type && a.queue_state == b.queue_state
    && a.elf_machine == b.elf_machine;
}
inline bool closed_profile (const stop_identity &s) noexcept
{
  return s.wave != 0 && s.workgroup != 0 && s.dispatch != 0 && s.queue != 0
    && s.agent != 0 && s.architecture != 0 && s.os_agent != 0 && s.private_bytes == 0
    && s.group_bytes == 0 && s.state == 3 && s.stop_reason == (1u << 10)
    && s.lanes == 64 && s.wave_index == 0
    && s.coordinate == std::array<std::uint32_t, 3> {0, 0, 0}
    && s.grid == std::array<std::uint32_t, 3> {64, 1, 1}
    && s.workgroup_size == std::array<std::uint32_t, 3> {64, 1, 1}
    && s.queue_type == 1 && s.queue_state == 1 && s.elf_machine == 0x4f;
}
struct record {
  static constexpr std::uint64_t generation = 1;
  std::uint64_t sequence = 0, inferior = 0, pid = 0;
  std::uint64_t process = 0, event = 0, callback = 0, breakpoint = 0;
  std::uint64_t thread = 0, action = 0;
  std::int64_t status = 0;
  stop_identity stop {};
  std::uint32_t event_kind = 0, runtime_state = 0, stop_generation = 0;
  kind type = kind::invalidated;
  reason invalidation = reason::none;
};

/* A single same-client lifetime. These methods accept inert numeric facts for
   pure tests; production calls are exclusively the native hooks below. The
   ledger has NO read capability, queue owner, serialization importer or token
   constructor. A row is historical observation even before invalidation. */
class ledger {
public:
  bool bind (std::uintptr_t owner, std::uint64_t inferior, std::uint64_t pid,
             std::uint64_t process) noexcept
  {
    if (m_bound || owner == 0 || inferior == 0 || pid == 0 || process == 0)
      { invalidate (reason::foreign_owner); return false; }
    m_bound = true; m_owner = owner;
    m_base.inferior = inferior; m_base.pid = pid; m_base.process = process;
    return !m_invalid;
  }
  bool owns (std::uintptr_t owner, std::uint64_t process) const noexcept
  { return m_bound && m_owner == owner && m_base.process == process; }
  bool bound () const noexcept { return m_bound; }
  bool invalid () const noexcept { return m_invalid; }
  bool stop_started () const noexcept { return m_candidate || m_published; }
  bool pending () const noexcept { return m_candidate && !m_invalid; }
  const record &candidate () const noexcept { return m_pending; }
  std::uint64_t process () const noexcept { return m_base.process; }
  std::uint64_t pid () const noexcept { return m_base.pid; }
  void attached () noexcept
  {
    if (!m_bound || !m_attaching) { invalidate (reason::foreign_owner); return; }
    m_attaching = false;
    record r; r.type = kind::attach_complete; push (r);
  }
  void completed (kind type, std::uint64_t event, std::uint32_t event_kind,
                  std::uint32_t runtime, std::int64_t status) noexcept
  {
    if (m_invalid) return;
    if (stop_started ()) { invalidate (reason::stop_changed); return; }
    if (status != 0) { invalidate (reason::acknowledgement, status); return; }
    if (event == 0 || (type == kind::runtime_complete && event_kind != 5)
        || (type == kind::code_objects_complete && event_kind != 3)) {
      invalidate (reason::event_identity); return;
    }
    if (type == kind::runtime_complete) {
      if (m_attaching) { invalidate (reason::preloaded_runtime); return; }
      if (runtime != 1) {
        invalidate (runtime == 2 ? reason::runtime_unloaded : reason::runtime_restriction);
        return;
      }
      if (m_runtime) { invalidate (reason::event_identity); return; }
      m_runtime = true;
    } else if (type != kind::code_objects_complete || !m_runtime) {
      invalidate (reason::event_identity); return;
    }
    record r; r.type = type; r.event = event; r.event_kind = event_kind;
    r.runtime_state = runtime; r.status = status; r.callback = m_callback;
    push (r);
  }
  void begin_callback (std::uint64_t breakpoint, std::uint64_t thread) noexcept
  {
    if (m_invalid) return;
    if (stop_started ()) { invalidate (reason::stop_changed); return; }
    if (!m_runtime || m_callback != 0 || breakpoint == 0 || thread == 0)
      { invalidate (reason::callback_nested); return; }
    if (++m_callbacks > 64) { invalidate (reason::capacity); return; }
    m_callback = m_callbacks; m_breakpoint = breakpoint; m_thread = thread;
  }
  void end_callback (std::uint64_t event, std::uint32_t event_kind,
                     std::uint64_t action, std::int64_t status) noexcept
  {
    if (m_invalid) return;
    if (m_callback == 0) { invalidate (reason::callback_identity); return; }
    if (status != 0) { invalidate (reason::acknowledgement, status); return; }
    if (!((event == 0 && event_kind == 0 && action == 1)
          || (event != 0 && event_kind == 4 && action == 2))) {
      invalidate (reason::callback_identity); return;
    }
    record r; r.type = kind::breakpoint_complete;
    r.event = event; r.event_kind = event_kind; r.action = action;
    r.callback = m_callback; r.breakpoint = m_breakpoint; r.thread = m_thread;
    push (r); m_callback = 0; m_breakpoint = 0; m_thread = 0;
  }
  void stage (std::uint64_t event, std::uint32_t event_kind,
              std::uint64_t thread, const stop_identity &stop,
              std::int64_t acknowledged_status) noexcept
  {
    if (m_invalid) return;
    if (stop_started ()) { invalidate (reason::stop_changed); return; }
    if (!m_bound || m_attaching || !m_runtime || m_callback != 0
        || event == 0 || thread == 0 || event_kind != 1) {
      invalidate (reason::event_identity); return;
    }
    if (acknowledged_status != 0) { invalidate (reason::acknowledgement, acknowledged_status); return; }
    if (!closed_profile (stop)) { invalidate (reason::unsupported_profile); return; }
    if (!fresh_event (event)) { invalidate (reason::event_identity); return; }
    m_pending = {};
    m_pending.type = kind::wave_stop_complete; m_pending.event = event;
    m_pending.event_kind = event_kind; m_pending.thread = thread;
    m_pending.stop = stop; m_pending.stop_generation = 1;
    m_candidate = true;
  }
  /* Called ONLY after complete MI *stopped output and a second actual native
     identity query for the exact current stopped GDB GPU thread. */
  void publish_normal_stop (std::uint64_t thread,
                            const stop_identity &requeried) noexcept
  {
    if (m_invalid) return;
    if (!m_candidate || m_published || thread == 0 || thread != m_pending.thread
        || !same_identity (m_pending.stop, requeried)) {
      invalidate (reason::stop_changed); return;
    }
    push (m_pending);
    if (!m_invalid) { m_candidate = false; m_published = true; }
  }
  void change (reason why) noexcept
  { if (stop_started ()) invalidate (why); }
  void invalidate (reason why, std::int64_t status = 0) noexcept
  {
    if (m_invalid) return;
    m_invalid = true;
    record r; r.type = kind::invalidated; r.invalidation = why; r.status = status;
    if (m_count < m_rows.size ()) { decorate (r); m_rows[m_count++] = r; }
  }
  const record *next () noexcept
  { return m_sent < m_count ? &m_rows[m_sent++] : nullptr; }
  std::size_t count () const noexcept { return m_count; }
  const record &at (std::size_t i) const noexcept { return m_rows[i]; }
private:
  bool fresh_event (std::uint64_t event) const noexcept
  {
    for (std::size_t i = 0; i < m_count; ++i)
      if (event != 0 && m_rows[i].event == event) return false;
    return true;
  }
  void decorate (record &r) noexcept
  {
    r.sequence = m_count + 1;
    r.inferior = m_base.inferior; r.pid = m_base.pid; r.process = m_base.process;
  }
  void push (record r) noexcept
  {
    if (m_invalid) return;
    if (!m_bound) { invalidate (reason::foreign_owner); return; }
    if (!fresh_event (r.event)) { invalidate (reason::event_identity); return; }
    if (m_count >= m_rows.size () - 1) { invalidate (reason::capacity); return; }
    decorate (r); m_rows[m_count++] = r;
  }
  std::array<record, 64> m_rows {};
  record m_base {}, m_pending {};
  std::uintptr_t m_owner = 0;
  std::size_t m_count = 0, m_sent = 0;
  std::uint64_t m_callback = 0, m_callbacks = 0, m_breakpoint = 0, m_thread = 0;
  bool m_bound = false, m_attaching = true, m_runtime = false, m_invalid = false;
  bool m_candidate = false, m_published = false;
};
static_assert (sizeof (record) <= 256, "fixed numeric observation row");
static_assert (sizeof (ledger) <= 24 * 1024, "fixed complete producer lifetime");

/* Native hook declarations, not commands. No function returns a live token. */
void flush_safe_point ();
void after_normal_stop (thread_info *);
void invalidate (reason, std::int64_t status = 0) noexcept;
void check_owner (amd_dbgapi_inferior_info &);
void bind (amd_dbgapi_inferior_info &);
void attached ();
void completed (kind, std::uint64_t, std::uint32_t, std::uint32_t, std::int64_t);
void begin_callback (std::uint64_t, std::uint64_t);
void end_callback (std::uint64_t, std::uint32_t, std::uint64_t, std::int64_t);
void change (reason) noexcept;

/* Independent observation-unwind refusal. The existing R6 event_ack remains
   the SOLE owner that calls amd_dbgapi_event_processed, exactly once. */
class scope_guard {
public:
  explicit scope_guard (reason failure) noexcept : m_failure (failure) {}
  scope_guard (const scope_guard &) = delete;
  scope_guard &operator= (const scope_guard &) = delete;
  ~scope_guard () noexcept { if (!m_done) invalidate (m_failure); }
  void done () noexcept { m_done = true; }
private:
  reason m_failure;
  bool m_done = false;
};
}
#endif
