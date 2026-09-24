/* SPDX-License-Identifier: GPL-3.0-or-later
   Included once by amd-dbgapi-target.c after its actual inferior owner.
   Private engineering producer V1.  No attach, poller, queue or command here. */
namespace runtime_obs = amd_runtime_observation_v1;
static runtime_obs::ledger runtime_observation;
static program_space *runtime_observation_pspace = nullptr;

static void
flush_runtime_observation ()
{
  try
    {
      while (const auto *row = runtime_observation.next ())
        interps_notify_amd_runtime_observation_v1 (*row);
    }
  catch (...)
    {
      runtime_observation.output_failed ();
      throw;
    }
}

/* MI calls this explicitly after a complete stop record or before its prompt.
   Reuse the same once-only ledger and existing output/error path. */
void
amd_runtime_observation_v1::flush_safe_point ()
{
  flush_runtime_observation ();
}

static void
invalidate_runtime_observation (runtime_obs::reason why) noexcept
{
  if (runtime_observation.bound ())
    runtime_observation.invalidate (why);
}

static void
check_runtime_observation_owner (amd_dbgapi_inferior_info &info)
{
  if (!runtime_observation.owns
        (reinterpret_cast<std::uintptr_t> (&info), info.process_id.handle))
    runtime_observation.invalidate (runtime_obs::reason::foreign_owner);
}

struct native_event_ack_v1
{
  amd_dbgapi_event_id_t event;
  std::int64_t operator() () const noexcept
  { return static_cast<std::int64_t> (amd_dbgapi_event_processed (event)); }
};

/* Construct the ack owner before querying the event, so query failure still
   acknowledges exactly once and invalidates during unwinding. */
class observed_native_event_v1
{
public:
  observed_native_event_v1 (amd_dbgapi_inferior_info &info,
                           amd_dbgapi_event_id_t event,
                           amd_dbgapi_event_kind_t kind)
    : m_event (event), m_kind (kind),
      m_ack (runtime_observation, native_event_ack_v1 {event})
  {
    check_runtime_observation_owner (info);
    amd_dbgapi_process_id_t process {};
    amd_dbgapi_event_kind_t actual_kind {};
    auto status = amd_dbgapi_event_get_info
      (event, AMD_DBGAPI_EVENT_INFO_PROCESS, sizeof (process), &process);
    if (status == AMD_DBGAPI_STATUS_SUCCESS)
      status = amd_dbgapi_event_get_info
        (event, AMD_DBGAPI_EVENT_INFO_KIND, sizeof (actual_kind), &actual_kind);
    if (status != AMD_DBGAPI_STATUS_SUCCESS)
      {
        runtime_observation.invalidate (runtime_obs::reason::query_failed,
                                        status);
        error (_("runtime observation event query failed (%s)"),
               get_status_string (status));
      }
    if (process != info.process_id || actual_kind != kind
        || event == AMD_DBGAPI_EVENT_NONE)
      {
        runtime_observation.invalidate (runtime_obs::reason::event_identity);
        error (_("runtime observation event owner/kind differs"));
      }
  }
  observed_native_event_v1 (const observed_native_event_v1 &) = delete;
  observed_native_event_v1 &operator= (const observed_native_event_v1 &) = delete;

  void complete (runtime_obs::kind type, std::uint64_t runtime = 0)
  {
    auto status = finish ();
    runtime_observation.completed (type, m_event.handle,
                                  static_cast<std::uint64_t> (m_kind),
                                  runtime, status);
  }
  std::int64_t finish ()
  {
    auto status = m_ack.finish ();
    if (status != AMD_DBGAPI_STATUS_SUCCESS)
      {
        runtime_observation.invalidate
          (runtime_obs::reason::acknowledgement, status);
        warning (_("Failed to acknowledge amd-dbgapi event %" PRIu64),
                 m_event.handle);
      }
    return status;
  }
private:
  amd_dbgapi_event_id_t m_event;
  amd_dbgapi_event_kind_t m_kind;
  runtime_obs::event_ack<native_event_ack_v1> m_ack;
};

class observed_native_scope_v1
{
public:
  explicit observed_native_scope_v1 (runtime_obs::reason failure)
    : m_failure (failure) {}
  ~observed_native_scope_v1 () noexcept
  {
    if (!m_done)
      runtime_observation.invalidate (m_failure);
  }
  void done () noexcept { m_done = true; }
private:
  runtime_obs::reason m_failure;
  bool m_done = false;
};
