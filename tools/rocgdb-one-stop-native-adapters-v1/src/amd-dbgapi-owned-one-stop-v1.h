/* SPDX-License-Identifier: GPL-3.0-or-later
   This file is part of GDB; GPL version 3 or (at your option) any later version.
   Fixed same-client one-stop owner. No serialized authority/importer.
   Native mutation is private to the in-process adapter; test access is compiled
   only by the separate standalone pure-control translation unit. */
#ifndef GDB_AMD_DBGAPI_OWNED_ONE_STOP_V1_H
#define GDB_AMD_DBGAPI_OWNED_ONE_STOP_V1_H
#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <type_traits>

namespace amd_owned_one_stop_v1 {
class native_adapter;
#ifdef FE2O3_ONE_STOP_PURE_TEST
struct core_test;
#endif

enum class phase : std::uint8_t {
  disabled, armed_entry, host_running, provisional_checkpoint, retired_checkpoint,
  publication_consumed, awaiting_gpu_stop, provisional_gpu_stop, current_gpu_stop,
  completion_consumed, awaiting_target_completion, lifecycle_disappeared, invalid
};
enum class failure : std::uint8_t {
  none, profile_unavailable, already_selected, owner_changed, order, budget,
  acknowledgement, event_identity, callback_identity, breakpoint_changed,
  retirement_incomplete, checkpoint_changed, stop_changed, resume_scope,
  native_result, commit, output, command, lifecycle, allocation
};
enum class resume_kind : std::uint8_t { publication, completion };
enum class counter : std::uint8_t {
  api, read_bytes, read_calls, work, diagnostic_rows,
  file_requested_bytes, file_observed_bytes, file_probe_bytes, file_rounds, proc_requested_bytes
};
struct usage {
  std::uint64_t logical_bytes = 0, api = 0, read_bytes = 0, read_calls = 0, work = 0, rows = 0;
  std::uint64_t file_requested_bytes = 0, file_observed_bytes = 0, file_rounds = 0;
  std::uint64_t file_probe_bytes = 0;
  std::uint64_t proc_requested_bytes = 0;
};
/* A closed policy, not user configuration. Logical memory is prepaid once.
   Read volume counts every repeated inferior read, separately from storage.
   Existing GDB/lib internals are not asserted to fit this logical envelope. */
struct limits {
  static constexpr std::uint64_t logical_bytes = 64 * 1024;
  static constexpr std::uint64_t api = 192;
  static constexpr std::uint64_t read_bytes = 64 * 1024;
  static constexpr std::uint64_t read_calls = 256;
  static constexpr std::uint64_t work = 131072;
  /* Separate fixed two-pass executable-file budget. Requested EOF probes
     consume capacity but contribute zero observed bytes on exact EOF. */
  static constexpr std::uint64_t file_requested_bytes = 8324130;
  static constexpr std::uint64_t file_observed_bytes = 8324128;
  static constexpr std::uint64_t file_probe_bytes = 2;
  static constexpr std::uint64_t file_rounds = 8324224;
  static constexpr std::uint64_t proc_requested_bytes = 16384;
  static constexpr std::uint64_t rows = 16;
  static constexpr std::size_t events = 64;
  static constexpr std::size_t callbacks = 8;
  static constexpr std::uint64_t client_output_bytes = 128;
};

/* Fixed adapter scratch; all members are included in the selected lifetime's
   logical prepayment. Do not add an unaccounted parallel stack buffer. */
struct workspace {
  std::array<std::uint8_t, 776> checkpoint_read {};
  std::array<std::uint8_t, 264> kernarg_read {};
  std::array<std::uint8_t, 256> image_chunk {};
  std::array<std::uint8_t, 256> digest_state {};
  std::array<std::uint8_t, 128> locator {};
  std::array<std::uint64_t, 16> native_scalars {};
  std::array<std::uintptr_t, 16> thread_scan {};
};

/* These are internal observations, not independently accepted proofs. Only
   native_adapter can submit them to an owner. No public mutating entry exists. */
struct identity {
  std::uintptr_t inferior = 0, program_space = 0, process_owner = 0;
  std::uint64_t inferior_number = 0, pid = 0, pid_start = 0, process = 0, host_thread = 0;
};
inline bool same (const identity &a, const identity &b) noexcept {
  return a.inferior == b.inferior && a.program_space == b.program_space
    && a.process_owner == b.process_owner && a.inferior_number == b.inferior_number
    && a.pid == b.pid && a.pid_start == b.pid_start && a.process == b.process
    && a.host_thread == b.host_thread;
}
inline bool complete (const identity &a) noexcept {
  return a.inferior && a.program_space && a.process_owner && a.inferior_number
    && a.pid && a.pid_start && a.process && a.host_thread;
}
struct checkpoint {
  std::array<std::uint8_t, 776> bytes {};
  std::uintptr_t breakpoint_object = 0;
  std::uint64_t breakpoint_number = 0, host_pc = 0, record_address = 0;
  std::uint64_t queue = 0, entry = 0, signal_base = 0, code_object = 0;
};
inline bool same (const checkpoint &a, const checkpoint &b) noexcept {
  return a.bytes == b.bytes && a.breakpoint_object == b.breakpoint_object
    && a.breakpoint_number == b.breakpoint_number && a.host_pc == b.host_pc
    && a.record_address == b.record_address && a.queue == b.queue
    && a.entry == b.entry && a.signal_base == b.signal_base
    && a.code_object == b.code_object;
}
struct gpu_stop {
  std::uint64_t thread = 0, wave = 0, workgroup = 0, dispatch = 0, queue = 0;
  std::uint64_t agent = 0, architecture = 0, pc = 0, completion_base = 0;
};
inline bool same (const gpu_stop &a, const gpu_stop &b) noexcept {
  return a.thread == b.thread && a.wave == b.wave && a.workgroup == b.workgroup
    && a.dispatch == b.dispatch && a.queue == b.queue && a.agent == b.agent
    && a.architecture == b.architecture && a.pc == b.pc
    && a.completion_base == b.completion_base;
}
/* Exact retained target ABI layout, not a public permission constructor.
   The native adapter reads all40 bytes on the selected little-endian target. */
struct runtime_root {
  std::uint32_t version = 11, reserved0 = 0;
  std::uint64_t map = 0, breakpoint = 0;
  std::uint32_t state = 0, reserved1 = 0;
  std::uint64_t loader_base = 0;
};
static_assert (sizeof (runtime_root) == 40 && alignof (runtime_root) == 8,
               "fixed x86_64 runtime root");
static_assert (offsetof (runtime_root, map) == 8
               && offsetof (runtime_root, breakpoint) == 16
               && offsetof (runtime_root, state) == 24
               && offsetof (runtime_root, loader_base) == 32,
               "source-owned root field offsets");
inline bool root_header (const runtime_root &r) noexcept {
  return r.version == 11 && r.reserved0 == 0 && r.reserved1 == 0
    && r.loader_base == 0 && r.breakpoint != 0;
}
inline bool root_constants (const runtime_root &a, const runtime_root &b) noexcept {
  return a.version == b.version && a.reserved0 == b.reserved0
    && a.breakpoint == b.breakpoint && a.reserved1 == b.reserved1
    && a.loader_base == b.loader_base;
}
struct diagnostic {
  std::uint64_t sequence = 0;
  phase state = phase::disabled;
  failure why = failure::none;
};

/* Not movable/copyable; its address, the actual GDB owners and fixed scratch
   remain retained for this debugger lifetime. No reset/rebind or permit escape. */
class owner final {
public:
  owner () noexcept = default;
  owner (const owner &) = delete;
  owner &operator= (const owner &) = delete;
  owner (owner &&) = delete;
  owner &operator= (owner &&) = delete;
  bool selected () const noexcept { return m_selected; }
  phase state () const noexcept { return m_phase; }
  failure why () const noexcept { return m_failure; }
  usage consumed () const noexcept { return m_usage; }
  bool invalid () const noexcept { return m_phase == phase::invalid; }
  static constexpr std::uint64_t fixed_storage () noexcept;

private:
  friend class native_adapter;
#ifdef FE2O3_ONE_STOP_PURE_TEST
  friend struct core_test;
#endif
  void poison (failure why) noexcept {
    if (!m_selected || invalid ()) return;
    m_failure = why; m_phase = phase::invalid;
    /* Failure retains all charged storage and work. No late allocation. */
  }
  bool fail (failure why) noexcept { poison (why); return false; }
  bool require (bool condition, failure why) noexcept {
    return condition && !invalid () ? true : fail (why);
  }
  bool debit (counter kind, std::uint64_t amount) noexcept {
    if (!m_selected || invalid ()) return false;
    if (m_phase == phase::disabled
        && (kind == counter::api || kind == counter::read_bytes
            || kind == counter::read_calls || kind == counter::diagnostic_rows))
      return fail (failure::order); // Bootstrap cannot query a native target.
    std::uint64_t *used = nullptr, cap = 0;
    switch (kind) {
      case counter::api: used = &m_usage.api; cap = limits::api; break;
      case counter::read_bytes: used = &m_usage.read_bytes; cap = limits::read_bytes; break;
      case counter::read_calls: used = &m_usage.read_calls; cap = limits::read_calls; break;
      case counter::file_requested_bytes: used = &m_usage.file_requested_bytes; cap = limits::file_requested_bytes; break;
      case counter::file_observed_bytes: used = &m_usage.file_observed_bytes; cap = limits::file_observed_bytes; break;
      case counter::file_probe_bytes: used = &m_usage.file_probe_bytes; cap = limits::file_probe_bytes; break;
      case counter::file_rounds: used = &m_usage.file_rounds; cap = limits::file_rounds; break;
      case counter::proc_requested_bytes: used = &m_usage.proc_requested_bytes; cap = limits::proc_requested_bytes; break;
      case counter::work: used = &m_usage.work; cap = limits::work; break;
      case counter::diagnostic_rows: used = &m_usage.rows; cap = limits::rows; break;
    }
    if (used == nullptr || *used > cap || amount > cap - *used) return fail (failure::budget);
    *used += amount; return true;
  }
  bool note () noexcept {
    if (!debit (counter::diagnostic_rows, 1)) return false;
    const auto n = static_cast<std::size_t> (m_usage.rows - 1);
    m_rows[n] = {m_usage.rows, m_phase, failure::none};
    return true;
  }
  /* Reserve BEFORE metered file/PID inspection. Disabled is not armed: every
     transition still requires its actual phase. A failed bootstrap cannot be
     reset or rebound. The adapter supplies sizeof its complete retained state
     plus explicit constant/scratch/client-output envelopes, not a user cap. */
  bool reserve_selection (std::uint64_t retained_bytes,
                          std::uint64_t prepaid_capacity) noexcept {
    if (m_selected) return fail (failure::already_selected);
    m_selected = true;
    if (retained_bytes < fixed_storage () || retained_bytes > limits::logical_bytes
        || prepaid_capacity < retained_bytes) return fail (failure::budget);
    m_usage.logical_bytes = retained_bytes;
    return true;
  }
  bool bind_selection (const identity &actual) noexcept {
    if (!require (m_selected && m_phase == phase::disabled && !m_identity.inferior,
                  failure::order)
        || !require (complete (actual), failure::owner_changed)) return false;
    m_identity = actual; m_phase = phase::armed_entry; return note ();
  }
  bool select (const identity &actual, std::uint64_t prepaid_capacity) noexcept {
    return reserve_selection (fixed_storage (), prepaid_capacity)
      && bind_selection (actual);
  }
  /* Used when the fixed measured executable/profile is absent. No native
     adapter may turn this failure into armed_entry from a CLI/JSON claim. */
  void select_unavailable () noexcept {
    if (m_selected) { poison (failure::already_selected); return; }
    m_selected = true; m_usage.logical_bytes = fixed_storage ();
    poison (failure::profile_unavailable);
  }
  bool check_owner (const identity &actual) noexcept {
    return require (complete (m_identity) && complete (actual)
                    && same (m_identity, actual), failure::owner_changed);
  }
  bool entry_continuation_consumed (const identity &actual) noexcept {
    if (!check_owner (actual) || !require (m_phase == phase::armed_entry, failure::order))
      return false;
    /* The adapter consumes BEFORE calling the actual entry resume. Failure
       during that call/commit must poison; state is not a retry capability. */
    m_phase = phase::host_running; return note ();
  }
  bool event (std::uint64_t id, std::int64_t status) noexcept {
    if (!require (status == 0, failure::acknowledgement)
        || !require (id != 0 && m_events_used < m_events.size (), failure::event_identity))
      return false;
    if (!debit (counter::work, m_events_used + 1)) return false;
    for (std::size_t n = 0; n < m_events_used; ++n)
      if (m_events[n] == id) return fail (failure::event_identity);
    m_events[m_events_used++] = id; return true;
  }
  bool runtime_loaded (const identity &actual, std::uint64_t event_id,
                       std::int64_t ack_status) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::host_running && !m_runtime, failure::order)
        || !event (event_id, ack_status)) return false;
    m_runtime = true; return note ();
  }
  bool callback_begin (const identity &actual, std::uintptr_t bp_object,
                       std::uint64_t native_breakpoint, std::uint64_t thread) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::host_running && m_runtime && !m_callback,
                     failure::callback_identity)
        || !require (bp_object && native_breakpoint && thread == m_identity.host_thread
                     && m_callbacks < limits::callbacks, failure::callback_identity))
      return false;
    if (!require (!m_origin_callback_object
                  || (bp_object == m_origin_callback_object
                      && native_breakpoint == m_origin_callback_id),
                  failure::callback_identity)) return false;
    if (!m_origin_callback_object) {
      m_origin_callback_object = bp_object; m_origin_callback_id = native_breakpoint;
    }
    ++m_callbacks; m_callback = true;
    m_callback_object = bp_object; m_callback_id = native_breakpoint; return true;
  }
  bool code_object_completed (const identity &actual, std::uint64_t event_id,
                              std::uint64_t native_object, std::int64_t ack_status) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::host_running && m_callback && !m_object
                     && native_object != 0, failure::order)
        || !event (event_id, ack_status)) return false;
    m_object = native_object; return note ();
  }
  bool callback_complete (const identity &actual, std::uintptr_t bp_object,
                           std::uint64_t native_breakpoint, std::uint64_t thread,
                           std::uint64_t resume_event, std::uint32_t action,
                           std::int64_t ack_status) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::host_running && m_callback
                     && bp_object == m_callback_object && native_breakpoint == m_callback_id
                     && thread == m_identity.host_thread, failure::callback_identity))
      return false;
    if (action == 1) {
      if (!require (resume_event == 0 && ack_status == 0, failure::callback_identity))
        return false;
    } else if (action == 2) {
      if (!event (resume_event, ack_status)) return false;
    } else return fail (failure::callback_identity);
    m_callback = false; m_callback_object = 0; m_callback_id = 0;
    ++m_completed_callbacks; return note ();
  }
  bool bind_checkpoint_root (const identity &actual, std::uint64_t root_address,
                              const runtime_root &observed) noexcept {
    if (!check_owner (actual) || !debit (counter::work, 160)
        || !require (m_phase == phase::host_running && m_object && !m_callback
                     && m_origin_callback_object && !m_root_address
                     && root_address && root_header (observed)
                     && observed.state == 0 && observed.map != 0, failure::checkpoint_changed))
      return false;
    m_root_address = root_address; m_root = observed; return true;
  }
  bool recheck_checkpoint_root (const identity &actual, std::uint64_t root_address,
                                 const runtime_root &observed) noexcept {
    if (!check_owner (actual) || !debit (counter::work, 160)
      || !require (m_phase == phase::retired_checkpoint && !m_root_rechecked
                  && root_address == m_root_address
                  && root_constants (observed, m_root) && observed.map == m_root.map
                  && observed.state == m_root.state, failure::checkpoint_changed)) return false;
    m_root_rechecked = true; return true;
  }
  bool checkpoint_hit (const identity &actual, const checkpoint &observed) noexcept {
    std::uint64_t observed_root = 0;
    for (std::size_t n = 0; n != 8; ++n)
      observed_root |= std::uint64_t (observed.bytes[216 + n]) << (8 * n);
    if (!check_owner (actual)
        || !require (m_phase == phase::host_running && m_runtime && m_object
                     && m_completed_callbacks != 0 && !m_callback
                     && m_root_address && observed_root == m_root_address, failure::order)
        || !require (observed.breakpoint_object && observed.breakpoint_number
                     && observed.host_pc && observed.record_address
                     && observed.queue && observed.entry && observed.signal_base
                     && observed.code_object == m_object, failure::checkpoint_changed))
      return false;
    m_checkpoint = observed; m_phase = phase::provisional_checkpoint;
    return note ();
  }
  bool host_stop_output_completed (const identity &actual, std::uint64_t actual_pc) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::provisional_checkpoint
                     && actual_pc == m_checkpoint.host_pc && !m_host_stop_notified, failure::order))
      return false;
    m_host_stop_notified = true; return true;
  }
  bool deleting_breakpoint (std::uintptr_t bp_object, std::uint64_t number) noexcept {
    if (!require (m_phase == phase::provisional_checkpoint && m_host_stop_notified
                  && !m_delete_seen && bp_object == m_checkpoint.breakpoint_object
                  && number == m_checkpoint.breakpoint_number, failure::breakpoint_changed))
      return false;
    m_delete_seen = true; return true; // NOT retired yet.
  }
  bool retirement_finished (const identity &actual, std::uint64_t actual_pc,
                             std::size_t remaining_owned_objects,
                             std::size_t remaining_breakpoints_at_pc) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::provisional_checkpoint && m_host_stop_notified
                     && m_delete_seen && actual_pc == m_checkpoint.host_pc
                     && remaining_owned_objects == 0 && remaining_breakpoints_at_pc == 0,
                     failure::retirement_incomplete)) return false;
    m_phase = phase::retired_checkpoint; return note ();
  }
  bool early_proceed (const identity &actual) noexcept {
    if (!check_owner (actual)) return false;
    /* Initial/internal host continuation is deliberately NOT admitted here.
       It requires its separately source-pinned native adapter route. */
    return require (m_phase == phase::retired_checkpoint
                    || m_phase == phase::current_gpu_stop, failure::resume_scope);
  }
  bool consume_publication (const identity &actual, const checkpoint &requeried,
                            std::size_t actual_host_threads, std::size_t actual_gpu_threads,
                            int actual_step, std::int64_t actual_signal) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::retired_checkpoint
                     && m_root_rechecked && same (m_checkpoint, requeried), failure::checkpoint_changed)
        || !require (actual_host_threads == 1 && actual_gpu_threads == 0
                     && actual_step == 0 && actual_signal == 0, failure::resume_scope))
      return false;
    m_resume = resume_kind::publication; m_phase = phase::publication_consumed;
    return note ();
  }
  bool stopped_after_ack (const identity &actual, std::uint64_t event_id,
                          const gpu_stop &observed, std::int64_t ack_status) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::awaiting_gpu_stop, failure::order)
        || !event (event_id, ack_status)
        || !require (observed.thread && observed.wave && observed.workgroup
                     && observed.dispatch && observed.agent && observed.architecture
                     && observed.queue == m_checkpoint.queue
                     && m_checkpoint.entry <= std::numeric_limits<std::uint64_t>::max () - 80
                     && observed.pc == m_checkpoint.entry + 80
                     && observed.completion_base == m_checkpoint.signal_base,
                     failure::stop_changed)) return false;
    m_stop = observed; m_phase = phase::provisional_gpu_stop; return note ();
  }
  bool gpu_stop_output_completed (const identity &actual, const gpu_stop &requeried) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::provisional_gpu_stop && same (m_stop, requeried),
                     failure::stop_changed)) return false;
    m_phase = phase::current_gpu_stop; return note ();
  }
  bool consume_completion (const identity &actual, const gpu_stop &requeried,
                           std::size_t actual_host_threads, std::size_t actual_gpu_threads,
                           int actual_step, std::int64_t actual_signal) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::current_gpu_stop && same (m_stop, requeried),
                     failure::stop_changed)
        || !require (actual_host_threads == 1 && actual_gpu_threads == 1
                     && actual_step == 0 && actual_signal == 0, failure::resume_scope))
      return false;
    m_resume = resume_kind::completion; m_phase = phase::completion_consumed;
    return note ();
  }
  bool during_resume () const noexcept {
    return m_phase == phase::publication_consumed || m_phase == phase::completion_consumed;
  }
  bool cpu_resume_returned () noexcept {
    if (!require (during_resume () && !m_cpu_returned && !m_body_returned, failure::order))
      return false;
    m_cpu_returned = true; return true;
  }
  bool wave_resume_returned (std::uint64_t wave, std::int64_t actual_status) noexcept {
    if (!require (m_phase == phase::completion_consumed && m_cpu_returned
                  && !m_wave_returned && !m_body_returned && wave == m_stop.wave, failure::order)
        || !require (actual_status == 0, failure::native_result)) return false;
    m_wave_returned = true; return true;
  }
  bool resume_body_returned () noexcept {
    if (!require (during_resume () && m_cpu_returned && !m_body_returned
                  && (m_resume == resume_kind::publication ? !m_wave_returned : m_wave_returned),
                  failure::order)) return false;
    m_body_returned = true; return true;
  }
  bool commit_started (const identity &actual) noexcept {
    if (!check_owner (actual)
        || !require (during_resume () && m_body_returned && !m_commit_started, failure::commit))
      return false;
    m_commit_started = true; return true;
  }
  bool commit_returned () noexcept {
    if (!require (during_resume () && m_commit_started, failure::commit)) return false;
    m_phase = m_resume == resume_kind::publication
      ? phase::awaiting_gpu_stop : phase::awaiting_target_completion;
    m_cpu_returned = m_wave_returned = m_body_returned = m_commit_started = false;
    return note ();
  }
  enum class terminal_stage : std::uint8_t {
    idle, deleting, delete_complete, consistent, objects_acknowledged,
    withdrawal_complete, unloaded
  };
  bool terminal_callback_begin (const identity &actual, std::uintptr_t bp_object,
                                std::uint64_t native_breakpoint, std::uint64_t thread,
                                std::uint64_t root_address,
                                const runtime_root &observed) noexcept {
    if (!check_owner (actual) || !debit (counter::work, 160)
        || !require (m_phase == phase::awaiting_target_completion && m_runtime
                     && !m_callback && m_root_address && root_address == m_root_address
                     && bp_object == m_origin_callback_object
                     && native_breakpoint == m_origin_callback_id
                     && thread == m_identity.host_thread && m_callbacks < limits::callbacks
                     && root_constants (observed, m_root), failure::callback_identity))
      return false;
    if (m_terminal == terminal_stage::idle) {
      if (!require (observed.state == 2 && observed.map == m_root.map,
                    failure::callback_identity)) return false;
      m_terminal = terminal_stage::deleting;
    } else if (m_terminal == terminal_stage::delete_complete) {
      if (!require (observed.state == 0 && observed.map == 0,
                    failure::callback_identity)) return false;
      m_terminal = terminal_stage::consistent;
    } else return fail (failure::order);
    ++m_callbacks; m_callback = true;
    m_callback_object = bp_object; m_callback_id = native_breakpoint; return true;
  }
  bool terminal_object_completed (const identity &actual, std::uint64_t event_id,
                                   std::int64_t ack_status) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::awaiting_target_completion && m_callback
                     && m_terminal == terminal_stage::consistent && m_empty_objects,
                     failure::order)
        || !event (event_id, ack_status)) return false;
    m_terminal = terminal_stage::objects_acknowledged; return true;
  }
  bool terminal_callback_complete (const identity &actual, std::uintptr_t bp_object,
                                    std::uint64_t native_breakpoint, std::uint64_t thread,
                                    std::uint64_t resume_event, std::uint32_t action,
                                    std::int64_t ack_status) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::awaiting_target_completion && m_callback
                     && bp_object == m_callback_object && native_breakpoint == m_callback_id
                     && thread == m_identity.host_thread, failure::callback_identity))
      return false;
    if (m_terminal == terminal_stage::deleting) {
      if (!require (action == 1 && resume_event == 0 && ack_status == 0
                    && !m_empty_objects, failure::callback_identity)) return false;
      m_terminal = terminal_stage::delete_complete;
    } else if (m_terminal == terminal_stage::objects_acknowledged) {
      if (!require (action == 2, failure::callback_identity)
          || !event (resume_event, ack_status)) return false;
      m_terminal = terminal_stage::withdrawal_complete;
    } else return fail (failure::order);
    m_callback = false; m_callback_object = 0; m_callback_id = 0;
    ++m_completed_callbacks; return true; // No extra diagnostic rows.
  }
  bool runtime_unloaded (const identity &actual, std::uint64_t event_id,
                         std::int64_t ack_status) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::awaiting_target_completion && m_runtime
                     && !m_callback && !m_allocation_active
                     && m_terminal == terminal_stage::withdrawal_complete, failure::lifecycle)
        || !event (event_id, ack_status)) return false;
    m_runtime = false; m_terminal = terminal_stage::unloaded; return note ();
  }
  bool lifecycle_disappeared (const identity &actual) noexcept {
    if (!check_owner (actual)
        || !require (m_phase == phase::awaiting_target_completion
                     && m_terminal == terminal_stage::unloaded && !m_runtime, failure::lifecycle))
      return false;
    m_phase = phase::lifecycle_disappeared; return note ();
    // No normal-exit, completion, queue destruction or reclamation assertion.
  }
  bool begin_client_output (std::size_t expected_bytes) noexcept {
    if (!require (m_selected && complete (m_identity) && !m_allocation_active && expected_bytes != 0
                  && expected_bytes <= limits::client_output_bytes, failure::allocation))
      return false;
    m_allocation_active = true; m_allocation_requested = false;
    m_allocation_bytes = expected_bytes; m_allocation_bounded = false;
    m_allocation_empty = false; return true;
  }
  /* URI_NAME is the only bounded-length output. Handles remain exact-size.
     The actual callback request selects the one retained length BEFORE malloc;
     the later returned pointer/length must match it exactly. */
  bool begin_client_locator () noexcept {
    if (!begin_client_output (limits::client_output_bytes)) return false;
    m_allocation_bounded = true; return true;
  }
  bool begin_client_empty_objects () noexcept {
    if (!require (m_selected && complete (m_identity) && !m_allocation_active
                  && m_phase == phase::awaiting_target_completion && m_callback
                  && m_terminal == terminal_stage::consistent && !m_empty_objects,
                  failure::allocation)) return false;
    m_allocation_active = true; m_allocation_requested = false;
    m_allocation_empty = true; m_allocation_bounded = false;
    m_allocation_bytes = 0; return true;
  }
  bool client_empty_objects_returned (std::uintptr_t actual_pointer,
                                      std::size_t actual_count,
                                      std::int64_t actual_status) noexcept {
    if (!require (m_allocation_active && m_allocation_empty && m_allocation_requested
                  && !m_allocation_pointer && actual_pointer == 0 && actual_count == 0
                  && actual_status == 0, failure::allocation)) return false;
    m_allocation_active = false; m_allocation_requested = false;
    m_allocation_empty = false; m_empty_objects = true; return true;
  }
  bool request_client_output (std::size_t actual_bytes) noexcept {
    if (!require (m_allocation_active && !m_allocation_requested
                  && (m_allocation_empty ? actual_bytes == 0
                       : (actual_bytes != 0
                          && (m_allocation_bounded ? actual_bytes <= m_allocation_bytes
                              : actual_bytes == m_allocation_bytes))),
                  failure::allocation)) return false;
    m_allocation_bytes = actual_bytes;
    m_allocation_requested = true; return true; // BEFORE malloc, or direct null for empty.
    // The empty scope never calls malloc(0); null is not a native handle identity.
  }
  bool client_output_allocated (std::uintptr_t actual_pointer) noexcept {
    if (!require (m_allocation_active && !m_allocation_empty
                  && m_allocation_requested && !m_allocation_pointer
                  && actual_pointer != 0, failure::allocation)) return false;
    m_allocation_pointer = actual_pointer; return true;
  }
  bool client_output_returned (std::uintptr_t actual_pointer, std::size_t actual_bytes,
                               std::int64_t actual_status) noexcept {
    return require (m_allocation_active && m_allocation_pointer != 0
                    && actual_pointer == m_allocation_pointer && actual_bytes == m_allocation_bytes
                    && actual_status == 0, failure::allocation);
  }
  bool releasing_client_output (std::uintptr_t actual_pointer) noexcept {
    // Cleanup of the one known allocation remains allowed AFTER poisoning.
    // The adapter still calls the real matching free exactly once.
    if (!m_allocation_active || !m_allocation_pointer || actual_pointer != m_allocation_pointer)
      return fail (failure::allocation);
    m_allocation_active = false; m_allocation_requested = false;
    m_allocation_pointer = 0; m_allocation_bytes = 0; m_allocation_bounded = false; return true;
  }
  const diagnostic *next_diagnostic () noexcept {
    return m_sent < m_usage.rows ? &m_rows[m_sent++] : nullptr;
    // Caller must poison(output) on any thrown/partial/unknown write.
  }

  identity m_identity {};
  runtime_root m_root {};
  std::uint64_t m_root_address = 0, m_origin_callback_id = 0;
  std::uintptr_t m_origin_callback_object = 0;
  terminal_stage m_terminal = terminal_stage::idle;
  bool m_empty_objects = false, m_allocation_empty = false, m_root_rechecked = false;
  checkpoint m_checkpoint {};
  gpu_stop m_stop {};
  std::array<std::uint64_t, limits::events> m_events {};
  std::array<diagnostic, limits::rows> m_rows {};
  usage m_usage {};
  std::size_t m_events_used = 0, m_callbacks = 0, m_completed_callbacks = 0, m_sent = 0;
  std::uintptr_t m_callback_object = 0;
  std::uint64_t m_callback_id = 0, m_object = 0;
  phase m_phase = phase::disabled;
  failure m_failure = failure::none;
  resume_kind m_resume = resume_kind::publication;
  bool m_selected = false, m_runtime = false, m_callback = false;
  bool m_host_stop_notified = false, m_delete_seen = false;
  bool m_cpu_returned = false, m_wave_returned = false, m_body_returned = false;
  bool m_commit_started = false;
  bool m_allocation_active = false, m_allocation_requested = false;
  bool m_allocation_bounded = false;
  std::size_t m_allocation_bytes = 0;
  std::uintptr_t m_allocation_pointer = 0;
};
constexpr std::uint64_t owner::fixed_storage () noexcept {
  return sizeof (owner) + sizeof (workspace) + limits::client_output_bytes;
}
static_assert (owner::fixed_storage () <= limits::logical_bytes, "complete fixed logical envelope");
static_assert (!std::is_copy_constructible<owner>::value, "owner cannot be copied");
static_assert (!std::is_move_constructible<owner>::value, "address-bound owner cannot be moved");
} // namespace amd_owned_one_stop_v1
#endif
