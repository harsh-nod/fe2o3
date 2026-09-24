/* SPDX-License-Identifier: GPL-3.0-or-later
   This file is part of GDB; GPL version 3 or (at your option) any later version.
   Included once after the real amd_dbgapi_inferior_info definition. */
namespace stopped_obs = amd_stopped_wave_observation_v1;
static stopped_obs::ledger stopped_wave_observation;

static_assert (AMD_DBGAPI_EVENT_KIND_WAVE_STOP == 1, "pinned native event");
static_assert (AMD_DBGAPI_EVENT_KIND_CODE_OBJECT_LIST_UPDATED == 3, "pinned native event");
static_assert (AMD_DBGAPI_EVENT_KIND_BREAKPOINT_RESUME == 4, "pinned native event");
static_assert (AMD_DBGAPI_EVENT_KIND_RUNTIME == 5, "pinned native event");
static_assert (AMD_DBGAPI_RUNTIME_STATE_LOADED_SUCCESS == 1, "pinned runtime");
static_assert (AMD_DBGAPI_RUNTIME_STATE_UNLOADED == 2, "pinned runtime");
static_assert (AMD_DBGAPI_WAVE_STATE_STOP == 3, "pinned stopped state");
static_assert (AMD_DBGAPI_WAVE_STOP_REASON_DEBUG_TRAP == (1u << 10), "debugtrap only");
static_assert (AMD_DBGAPI_OS_QUEUE_TYPE_HSA_AQL == 1, "AQL only");
static_assert (AMD_DBGAPI_QUEUE_STATE_VALID == 1, "valid queue");
static_assert (AMD_DBGAPI_BREAKPOINT_ACTION_RESUME == 1, "native callback");
static_assert (AMD_DBGAPI_BREAKPOINT_ACTION_HALT == 2, "native callback");

void
stopped_obs::flush_safe_point ()
{
  try {
    while (const auto *row = stopped_wave_observation.next ())
      interps_notify_amd_stopped_wave_observation_v1 (*row);
  } catch (...) {
    stopped_wave_observation.invalidate (reason::output_failure);
    throw;
  }
}
void
stopped_obs::invalidate (reason why, std::int64_t status) noexcept
{
  if (stopped_wave_observation.bound ())
    stopped_wave_observation.invalidate (why, status);
}
void
stopped_obs::change (reason why) noexcept
{ stopped_wave_observation.change (why); }

void
stopped_obs::check_owner (amd_dbgapi_inferior_info &info)
{
  if (!stopped_wave_observation.owns
      (reinterpret_cast<std::uintptr_t> (&info), info.process_id.handle))
    stopped_wave_observation.invalidate (reason::foreign_owner);
}
void
stopped_obs::bind (amd_dbgapi_inferior_info &info)
{
  if (info.inf == nullptr || info.inf->num <= 0 || info.inf->pid <= 0
      || info.inf->pspace->cbfd != nullptr)
    { stopped_wave_observation.invalidate (reason::unsupported_profile); return; }
  stopped_wave_observation.bind (reinterpret_cast<std::uintptr_t> (&info),
                                info.inf->num, info.inf->pid, info.process_id.handle);
}
void
stopped_obs::attached ()
{ stopped_wave_observation.attached (); }
void
stopped_obs::completed (kind type, std::uint64_t event, std::uint32_t native_kind,
                        std::uint32_t runtime, std::int64_t status)
{ stopped_wave_observation.completed (type, event, native_kind, runtime, status); }
void
stopped_obs::begin_callback (std::uint64_t breakpoint, std::uint64_t thread)
{ stopped_wave_observation.begin_callback (breakpoint, thread); }
void
stopped_obs::end_callback (std::uint64_t event, std::uint32_t native_kind,
                          std::uint64_t action, std::int64_t status)
{ stopped_wave_observation.end_callback (event, native_kind, action, status); }
