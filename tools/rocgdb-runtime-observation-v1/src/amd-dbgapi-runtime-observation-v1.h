/* Bounded native ROCdbgapi observation facts, not acceptance authority.
   SPDX-License-Identifier: GPL-3.0-or-later
   This file is part of GDB; GPL version 3 or (at your option) any later version. */
#ifndef GDB_AMD_DBGAPI_RUNTIME_OBSERVATION_V1_H
#define GDB_AMD_DBGAPI_RUNTIME_OBSERVATION_V1_H

#include <array>
#include <cstddef>
#include <cstdint>

namespace amd_runtime_observation_v1
{
/* Drain the existing bounded numeric ledger at an ordinary interpreter safe
   point. No lifecycle mutation, replay, target query or CLI observer dispatch.
   Output failures retain the original sticky invalidation and propagate. */
void flush_safe_point ();

enum class kind : std::uint32_t
{
  attach_complete = 1,
  runtime_complete = 2,
  code_objects_complete = 3,
  breakpoint_complete = 4,
  invalidated = 5
};
enum class reason : std::uint32_t
{
  none = 0, foreign_owner = 1, capacity = 2, query_failed = 3,
  event_identity = 4, handler_unwind = 5, acknowledgement = 6,
  preloaded_runtime = 7, runtime_unloaded = 8, runtime_restriction = 9,
  unsupported_event = 10, callback_identity = 11, callback_nested = 12,
  detach = 13, inferior_exit = 14, inferior_exec = 15, inferior_fork = 16,
  object_removed = 17, output_failure = 18, setup_failure = 19
};

/* No strings, pointers, physical registers, queue data or readiness bool.
   Native identifiers describe this one debugger lifetime only. */
struct record
{
  std::uint64_t sequence = 0;
  std::uint64_t generation = 1;
  std::uint64_t inferior = 0;
  std::uint64_t pid = 0;
  std::uint64_t process = 0;
  std::uint64_t event = 0;
  std::uint64_t event_kind = 0;
  std::uint64_t runtime_state = 0;
  std::uint64_t callback = 0;
  std::uint64_t breakpoint = 0;
  std::uint64_t thread = 0;
  std::uint64_t action = 0;
  std::int64_t status = 0;
  kind type = kind::invalidated;
  reason invalidation = reason::none;
};

/* One owner for the entire producer lifetime.  A second attach, even to a
   recycled inferior address or PID, cannot restart a successful transcript.
   Slot 32 is reserved for a terminal overflow refusal.  No destructor calls
   output, allocates, throws, or dereferences the remembered owner address. */
class ledger
{
public:
  bool bind (std::uintptr_t owner, std::uint64_t inferior, std::uint64_t pid,
             std::uint64_t process) noexcept
  {
    if (m_bound || owner == 0 || inferior == 0 || pid == 0 || process == 0)
      {
        invalidate (reason::foreign_owner);
        return false;
      }
    m_bound = true;
    m_owner = owner;
    m_base.inferior = inferior;
    m_base.pid = pid;
    m_base.process = process;
    return !m_invalid;
  }

  bool owns (std::uintptr_t owner, std::uint64_t process) const noexcept
  { return m_bound && m_owner == owner && m_base.process == process; }

  bool bound () const noexcept { return m_bound; }
  bool invalid () const noexcept { return m_invalid; }
  bool attaching () const noexcept { return m_attaching; }
  std::uint64_t callback () const noexcept { return m_callback; }

  void attached () noexcept
  {
    if (!m_bound || !m_attaching)
      { invalidate (reason::foreign_owner); return; }
    m_attaching = false;
    append (kind::attach_complete);
  }

  void completed (kind type, std::uint64_t event, std::uint64_t event_kind,
                  std::uint64_t runtime_state, std::int64_t status) noexcept
  {
    if (status != 0)
      { invalidate (reason::acknowledgement, status); return; }
    if (type == kind::runtime_complete && m_attaching)
      { invalidate (reason::preloaded_runtime); return; }
    record r;
    r.type = type;
    r.event = event;
    r.event_kind = event_kind;
    r.runtime_state = runtime_state;
    r.status = status;
    r.callback = m_callback;
    push (r);
  }

  void begin_callback (std::uint64_t breakpoint, std::uint64_t thread) noexcept
  {
    if (m_callback != 0 || breakpoint == 0 || thread == 0)
      { invalidate (reason::callback_nested); return; }
    /* At most 31 ordinary records can exist.  Monotonic callback ordinals
       nevertheless have their own finite bound, independent of flushing. */
    if (++m_callbacks > 32)
      { invalidate (reason::capacity); return; }
    m_callback = m_callbacks;
    m_breakpoint = breakpoint;
    m_thread = thread;
  }

  void end_callback (std::uint64_t event, std::uint64_t event_kind,
                     std::uint64_t action, std::int64_t status) noexcept
  {
    if (m_callback == 0)
      { invalidate (reason::callback_identity); return; }
    if (status != 0)
      invalidate (reason::acknowledgement, status);
    else
      {
        record r;
        r.type = kind::breakpoint_complete;
        r.event = event;
        r.event_kind = event_kind;
        r.callback = m_callback;
        r.breakpoint = m_breakpoint;
        r.thread = m_thread;
        r.action = action;
        push (r);
      }
    m_callback = 0;
    m_breakpoint = 0;
    m_thread = 0;
  }

  void invalidate (reason why, std::int64_t status = 0) noexcept
  {
    if (m_invalid)
      return;
    m_invalid = true;
    record r;
    r.type = kind::invalidated;
    r.invalidation = why;
    r.status = status;
    push_terminal (r);
  }

  /* The caller outputs only at an ordinary safe point.  Marking a row sent
     before invoking an output callback prevents replay after output throws.
     Output exceptions must call output_failed and propagate; a truncated
     stream is never accepted by the independent consumer. */
  const record *next () noexcept
  {
    return m_sent < m_count ? &m_rows[m_sent++] : nullptr;
  }
  void output_failed () noexcept { invalidate (reason::output_failure); }
  std::size_t count () const noexcept { return m_count; }
  const record &at (std::size_t i) const noexcept { return m_rows[i]; }

private:
  void append (kind type) noexcept
  { record r; r.type = type; push (r); }
  void decorate (record &r) noexcept
  {
    r.sequence = m_count + 1;
    r.generation = m_base.generation;
    r.inferior = m_base.inferior;
    r.pid = m_base.pid;
    r.process = m_base.process;
  }
  void push (record r) noexcept
  {
    if (m_invalid)
      return;
    if (!m_bound)
      { invalidate (reason::foreign_owner); return; }
    if (r.event != 0)
      for (std::size_t i = 0; i < m_count; ++i)
        if (m_rows[i].event == r.event)
          { invalidate (reason::event_identity); return; }
    if (m_count >= m_rows.size () - 1)
      { invalidate (reason::capacity); return; }
    decorate (r);
    m_rows[m_count++] = r;
  }
  void push_terminal (record r) noexcept
  {
    if (m_count < m_rows.size ())
      { decorate (r); m_rows[m_count++] = r; }
  }
  std::array<record, 32> m_rows {};
  record m_base {};
  std::uintptr_t m_owner = 0;
  std::size_t m_count = 0;
  std::size_t m_sent = 0;
  std::uint64_t m_callback = 0;
  std::uint64_t m_callbacks = 0;
  std::uint64_t m_breakpoint = 0;
  std::uint64_t m_thread = 0;
  bool m_bound = false;
  bool m_attaching = true;
  bool m_invalid = false;
};

static_assert (sizeof (record) <= 128, "fixed numeric record bound");
static_assert (sizeof (ledger) <= 4096, "fixed lifetime record storage bound");

/* The callable must perform exactly the existing native acknowledgment and
   return its actual status; zero means AMD_DBGAPI_STATUS_SUCCESS.  finish is
   never retried, including after failure.  Unwind fallback is diagnostic
   refusal, even when its actual acknowledgment succeeds. */
template<typename Acknowledge> class event_ack
{
public:
  event_ack (ledger &events, Acknowledge acknowledge) noexcept
    : m_events (events), m_acknowledge (acknowledge) {}
  event_ack (const event_ack &) = delete;
  event_ack &operator= (const event_ack &) = delete;
  ~event_ack () noexcept
  {
    if (!m_attempted)
      {
        m_attempted = true;
        m_status = m_acknowledge ();
        m_events.invalidate (reason::handler_unwind, m_status);
      }
  }
  std::int64_t finish () noexcept
  {
    if (!m_attempted)
      {
        m_attempted = true;
        m_status = m_acknowledge ();
      }
    return m_status;
  }
private:
  ledger &m_events;
  Acknowledge m_acknowledge;
  std::int64_t m_status = 0;
  bool m_attempted = false;
};
}
#endif
