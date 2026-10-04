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
      stopped_obs::flush_safe_point ();
    }
  catch (...)
    {
      runtime_observation.output_failed ();
      stopped_obs::invalidate (stopped_obs::reason::output_failure);
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
invalidate_runtime_observation (runtime_obs::reason why,
                                std::int64_t status = 0) noexcept
{
  if (runtime_observation.bound ())
    runtime_observation.invalidate (why, status);
  // The two schemas intentionally share the closed reason values 0..19.
  stopped_obs::invalidate (static_cast<stopped_obs::reason> (why), status);
}

static_assert (static_cast<unsigned> (runtime_obs::reason::none)
               == static_cast<unsigned> (stopped_obs::reason::none),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::foreign_owner)
               == static_cast<unsigned> (stopped_obs::reason::foreign_owner),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::capacity)
               == static_cast<unsigned> (stopped_obs::reason::capacity),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::query_failed)
               == static_cast<unsigned> (stopped_obs::reason::query_failed),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::event_identity)
               == static_cast<unsigned> (stopped_obs::reason::event_identity),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::handler_unwind)
               == static_cast<unsigned> (stopped_obs::reason::handler_unwind),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::acknowledgement)
               == static_cast<unsigned> (stopped_obs::reason::acknowledgement),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::preloaded_runtime)
               == static_cast<unsigned> (stopped_obs::reason::preloaded_runtime),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::runtime_unloaded)
               == static_cast<unsigned> (stopped_obs::reason::runtime_unloaded),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::runtime_restriction)
               == static_cast<unsigned> (stopped_obs::reason::runtime_restriction),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::unsupported_event)
               == static_cast<unsigned> (stopped_obs::reason::unsupported_event),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::callback_identity)
               == static_cast<unsigned> (stopped_obs::reason::callback_identity),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::callback_nested)
               == static_cast<unsigned> (stopped_obs::reason::callback_nested),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::detach)
               == static_cast<unsigned> (stopped_obs::reason::detach),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::inferior_exit)
               == static_cast<unsigned> (stopped_obs::reason::inferior_exit),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::inferior_exec)
               == static_cast<unsigned> (stopped_obs::reason::inferior_exec),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::inferior_fork)
               == static_cast<unsigned> (stopped_obs::reason::inferior_fork),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::object_removed)
               == static_cast<unsigned> (stopped_obs::reason::object_removed),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::output_failure)
               == static_cast<unsigned> (stopped_obs::reason::output_failure),
               "closed shared reason mapping");
static_assert (static_cast<unsigned> (runtime_obs::reason::setup_failure)
               == static_cast<unsigned> (stopped_obs::reason::setup_failure),
               "closed shared reason mapping");

/* Preserve pre-bind semantics of the original direct ledger calls too. */
static void
invalidate_observation_event_v1 (runtime_obs::reason why,
                                 std::int64_t status = 0) noexcept
{
  runtime_observation.invalidate (why, status);
  stopped_obs::invalidate (static_cast<stopped_obs::reason> (why), status);
}

static void
check_runtime_observation_owner (amd_dbgapi_inferior_info &info)
{
  stopped_obs::check_owner (info);
  if (!runtime_observation.owns
        (reinterpret_cast<std::uintptr_t> (&info), info.process_id.handle))
    invalidate_observation_event_v1 (runtime_obs::reason::foreign_owner);
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
        invalidate_observation_event_v1 (runtime_obs::reason::query_failed,
                                        status);
        error (_("runtime observation event query failed (%s)"),
               get_status_string (status));
      }
    if (process != info.process_id || actual_kind != kind
        || event == AMD_DBGAPI_EVENT_NONE)
      {
        invalidate_observation_event_v1 (runtime_obs::reason::event_identity);
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
    stopped_obs::completed
      (type == runtime_obs::kind::runtime_complete
         ? stopped_obs::kind::runtime_complete : stopped_obs::kind::code_objects_complete,
       m_event.handle, static_cast<std::uint32_t> (m_kind), runtime, status);
  }
  std::int64_t finish ()
  {
    auto status = m_ack.finish ();
    if (status != AMD_DBGAPI_STATUS_SUCCESS)
      stopped_obs::invalidate (stopped_obs::reason::acknowledgement, status);
    m_stop_scope.done ();
    if (status != AMD_DBGAPI_STATUS_SUCCESS)
      {
        invalidate_observation_event_v1
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
  stopped_obs::scope_guard m_stop_scope {stopped_obs::reason::handler_unwind};
};

class observed_native_scope_v1
{
public:
  explicit observed_native_scope_v1 (runtime_obs::reason failure)
    : m_failure (failure) {}
  ~observed_native_scope_v1 () noexcept
  {
    if (!m_done)
      {
        invalidate_observation_event_v1 (m_failure);
      }
  }
  void done () noexcept { m_done = true; }
private:
  runtime_obs::reason m_failure;
  bool m_done = false;
};
