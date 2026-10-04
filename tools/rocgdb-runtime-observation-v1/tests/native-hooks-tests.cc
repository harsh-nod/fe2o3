// SPDX-License-Identifier: GPL-3.0-or-later
// CPU test doubles around the ACTUAL production hook include.
// No ROCdbgapi linkage, debugger process, target, queue or launch authority.
#include "../src/amd-dbgapi-runtime-observation-v1.h"
#include <cassert>
#include <cstdarg>
#include <cstdint>
#include <cstring>
#include <stdexcept>
#include <vector>
using amd_dbgapi_status_t = int;
using amd_dbgapi_event_kind_t = int;
using amd_dbgapi_event_info_t = int;
constexpr int AMD_DBGAPI_STATUS_SUCCESS = 0;
constexpr int AMD_DBGAPI_EVENT_INFO_PROCESS = 1;
constexpr int AMD_DBGAPI_EVENT_INFO_KIND = 2;
struct amd_dbgapi_event_id_t { std::uint64_t handle; };
struct amd_dbgapi_process_id_t { std::uint64_t handle; };
constexpr amd_dbgapi_event_id_t AMD_DBGAPI_EVENT_NONE {0};
static bool operator== (amd_dbgapi_event_id_t a, amd_dbgapi_event_id_t b)
{ return a.handle == b.handle; }
static bool operator!= (amd_dbgapi_process_id_t a, amd_dbgapi_process_id_t b)
{ return a.handle != b.handle; }
struct amd_dbgapi_inferior_info { amd_dbgapi_process_id_t process_id {200}; };
struct program_space {};
struct fixture_error {};
static int acknowledgements, warnings, query_status, fail_query, ack_status;
static std::uint64_t event_process;
static int event_kind;
static bool fail_output;
static std::vector<amd_runtime_observation_v1::record> emitted;
#define _(s) s
#define PRIu64 "lu"
[[noreturn]] static void error (const char *, ...) { throw fixture_error {}; }
static void warning (const char *, ...) { ++warnings; }
static const char *get_status_string (int) { return "fixture"; }
static int amd_dbgapi_event_processed (amd_dbgapi_event_id_t event) noexcept
{ assert (event.handle == 10); ++acknowledgements; return ack_status; }
static int amd_dbgapi_event_get_info
  (amd_dbgapi_event_id_t event, int query, std::size_t size, void *result)
{
  assert (event.handle == 10);
  if (query == fail_query) return query_status;
  if (query == AMD_DBGAPI_EVENT_INFO_PROCESS)
    { assert (size == sizeof (amd_dbgapi_process_id_t));
      amd_dbgapi_process_id_t value {event_process};
      std::memcpy (result, &value, size); }
  else
    { assert (query == AMD_DBGAPI_EVENT_INFO_KIND && size == sizeof (event_kind));
      std::memcpy (result, &event_kind, size); }
  return 0;
}
static void interps_notify_amd_runtime_observation_v1
  (const amd_runtime_observation_v1::record &row)
{ if (fail_output) throw fixture_error {}; emitted.push_back (row); }

#include "../src/amd-dbgapi-runtime-observation-hooks-v1.inc"

static void reset (amd_dbgapi_inferior_info &info)
{
  runtime_observation = runtime_obs::ledger {};
  runtime_observation_pspace = nullptr;
  acknowledgements = warnings = query_status = fail_query = ack_status = 0;
  event_process = 200; event_kind = 5; fail_output = false; emitted.clear ();
  runtime_observation.bind (reinterpret_cast<std::uintptr_t> (&info), 1, 100, 200);
  runtime_observation.attached ();
}
template<typename F> static void refuses (F f)
{
  bool caught = false;
  try { f (); } catch (fixture_error &) { caught = true; }
  assert (caught);
}
int main ()
{
  amd_dbgapi_inferior_info info;
  // 1: actual wrapper's successful completion is acknowledged exactly once.
  reset (info);
  { observed_native_event_v1 event (info, {10}, 5);
    event.complete (runtime_obs::kind::runtime_complete, 1); }
  assert (acknowledgements == 1 && !runtime_observation.invalid ());
  assert (runtime_observation.at (1).event == 10);
  // 2: query failure still has exactly one fallback ack, never completion.
  reset (info); fail_query = 1; query_status = -3;
  refuses ([&] { observed_native_event_v1 event (info, {10}, 5); });
  assert (acknowledgements == 1 && runtime_observation.invalid ());
  assert (runtime_observation.at (1).invalidation == runtime_obs::reason::query_failed);
  // 3: kind query failure is distinct from successfully queried kind.
  reset (info); fail_query = 2; query_status = -4;
  refuses ([&] { observed_native_event_v1 event (info, {10}, 5); });
  assert (acknowledgements == 1 && runtime_observation.at (1).status == -4);
  // 4: foreign event process is refused by actual wrapper.
  reset (info); event_process = 201;
  refuses ([&] { observed_native_event_v1 event (info, {10}, 5); });
  assert (acknowledgements == 1 && runtime_observation.invalid ());
  // 5: returned kind must equal retrieved kind.
  reset (info); event_kind = 3;
  refuses ([&] { observed_native_event_v1 event (info, {10}, 5); });
  assert (acknowledgements == 1 && runtime_observation.invalid ());
  // 6: foreign native owner never produces a successful row.
  reset (info); amd_dbgapi_inferior_info foreign;
  { observed_native_event_v1 event (foreign, {10}, 5);
    event.complete (runtime_obs::kind::runtime_complete, 1); }
  assert (acknowledgements == 1 && runtime_observation.invalid ());
  // 7: actual failed ack warns once, is not retried, and invalidates.
  reset (info); ack_status = -8;
  { observed_native_event_v1 event (info, {10}, 5);
    event.complete (runtime_obs::kind::runtime_complete, 1); }
  assert (acknowledgements == 1 && warnings == 1 && runtime_observation.invalid ());
  // 8: handler throws after successful query: fallback refuses even if ack succeeds.
  reset (info);
  refuses ([&] { observed_native_event_v1 event (info, {10}, 5);
                  throw fixture_error {}; });
  assert (acknowledgements == 1 && warnings == 0 && runtime_observation.invalid ());
  // 9: ordinary explicit finish is also the once-only callback ack path.
  reset (info);
  { observed_native_event_v1 event (info, {10}, 5);
    assert (event.finish () == 0 && event.finish () == 0); }
  assert (acknowledgements == 1 && !runtime_observation.invalid ());
  // 10: destructor-only scope refuses; done scope does not.
  reset (info);
  { observed_native_scope_v1 scope (runtime_obs::reason::setup_failure); }
  assert (runtime_observation.invalid ());
  reset (info);
  { observed_native_scope_v1 scope (runtime_obs::reason::setup_failure); scope.done (); }
  assert (!runtime_observation.invalid ());
  // 11: output failure invalidates, and consumed row is not replayed.
  reset (info); fail_output = true;
  refuses ([] { flush_runtime_observation (); });
  assert (runtime_observation.invalid ());
  fail_output = false; flush_runtime_observation ();
  assert (emitted.size () == 1 && emitted[0].type == runtime_obs::kind::invalidated);
  // 12: lifecycle invalidation helper is sticky and flush is non-authoritative.
  reset (info); invalidate_runtime_observation (runtime_obs::reason::inferior_exec);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  flush_runtime_observation ();
  assert (emitted.size () == 2 && emitted[1].invalidation == runtime_obs::reason::inferior_exec);
  // 13: public MI-safe wrapper is a no-op before any owner is attached.
  runtime_observation = runtime_obs::ledger {};
  emitted.clear ();
  runtime_obs::flush_safe_point ();
  assert (emitted.empty () && runtime_observation.count () == 0);
  // 14: the exit invalidation remains buffered until the explicit safe point.
  reset (info);
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 1);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  assert (emitted.size () == 1 && runtime_observation.count () == 2);
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 2 && emitted[1].sequence == 2
          && emitted[1].type == runtime_obs::kind::invalidated
          && emitted[1].invalidation == runtime_obs::reason::inferior_exit);
  // 15: later MI prompt and stop safe points cannot replay any consumed row.
  runtime_obs::flush_safe_point ();
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 2 && runtime_observation.count () == 2);
  // 16: output failure through the exported wrapper is sticky and propagates.
  reset (info); fail_output = true;
  refuses ([] { runtime_obs::flush_safe_point (); });
  fail_output = false;
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 1
          && emitted[0].invalidation == runtime_obs::reason::output_failure);
  // 17: an earlier invalidation is never replaced by inferior_exit.
  reset (info);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exec);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 2
          && emitted[1].invalidation == runtime_obs::reason::inferior_exec);

  // 18: mourn invalidates before internal teardown callbacks/reset.  A later
  // inferior_exit observer keeps the same terminal row; this is not MI exit proof.
  reset (info);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  runtime_observation.completed (runtime_obs::kind::runtime_complete, 91, 5, 1, 0);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 2 && emitted[1].sequence == 2
          && emitted[1].invalidation == runtime_obs::reason::inferior_exit);
  // 19: explicit pre-detach precedes target detach and its later exit observer.
  reset (info);
  invalidate_runtime_observation (runtime_obs::reason::detach);
  invalidate_runtime_observation (runtime_obs::reason::detach);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 2
          && emitted[1].invalidation == runtime_obs::reason::detach);
  // 20: direct target detach remains a detach even without a pre-detach call.
  reset (info);
  invalidate_runtime_observation (runtime_obs::reason::detach);
  runtime_observation.completed (runtime_obs::kind::code_objects_complete, 92, 3, 0, 0);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 2
          && emitted[1].invalidation == runtime_obs::reason::detach);
  // 21: an exec lifecycle cause is never replaced by mourn/exit cleanup.
  reset (info);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exec);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 2
          && emitted[1].invalidation == runtime_obs::reason::inferior_exec);
  // 22: teardown cannot erase an earlier failed acknowledgement.
  reset (info);
  runtime_observation.invalidate (runtime_obs::reason::acknowledgement, -7);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  runtime_obs::flush_safe_point ();
  assert (emitted.size () == 2 && emitted[1].status == -7
          && emitted[1].invalidation == runtime_obs::reason::acknowledgement);
  // 23: lifecycle notifications before any observation owner remain inert.
  runtime_observation = runtime_obs::ledger {};
  emitted.clear ();
  invalidate_runtime_observation (runtime_obs::reason::detach);
  invalidate_runtime_observation (runtime_obs::reason::inferior_exit);
  runtime_obs::flush_safe_point ();
  assert (emitted.empty () && runtime_observation.count () == 0);

}
