// SPDX-License-Identifier: GPL-3.0-or-later
// CPU fixtures only. Never links ROCdbgapi or starts a process/debugger.
#include "../src/amd-dbgapi-runtime-observation-v1.h"
#include <cassert>
#include <stdexcept>
using namespace amd_runtime_observation_v1;

struct fake_ack
{
  int *calls;
  std::int64_t result;
  std::int64_t operator() () const noexcept { ++*calls; return result; }
};
static ledger attached ()
{
  ledger x;
  assert (x.bind (11, 1, 100, 200));
  x.attached ();
  assert (!x.invalid () && x.count () == 1);
  return x;
}
int main ()
{
  // 1: cold owner binds once and records completed attachment.
  { auto x = attached (); assert (x.at (0).type == kind::attach_complete);
    assert (x.at (0).pid == 100 && x.at (0).process == 200); }
  // 2: restart/recycled numerical owner never starts another generation.
  { auto x = attached (); assert (!x.bind (11, 1, 100, 200));
    assert (x.invalid () && x.at (1).invalidation == reason::foreign_owner); }
  // 3: foreign identity is not owned.
  { auto x = attached (); assert (x.owns (11, 200));
    assert (!x.owns (12, 200) && !x.owns (11, 201)); }
  // 4: preloaded runtime has no cold-launch completion.
  { ledger x; x.bind (11, 1, 100, 200);
    x.completed (kind::runtime_complete, 10, 5, 1, 0);
    assert (x.invalid () && x.at (0).invalidation == reason::preloaded_runtime); }
  // 5: setup failure stays invalid even after later attached call.
  { ledger x; x.bind (11, 1, 100, 200); x.invalidate (reason::setup_failure);
    x.attached (); assert (x.invalid () && x.count () == 1); }
  // 6: completed real event fields are retained only after zero status.
  { auto x = attached (); x.completed (kind::runtime_complete, 10, 5, 1, 0);
    assert (x.at (1).event == 10 && x.at (1).runtime_state == 1); }
  // 7: ack failure is not an ordinary success row.
  { auto x = attached (); x.completed (kind::runtime_complete, 10, 5, 1, -3);
    assert (x.invalid () && x.at (1).type == kind::invalidated);
    assert (x.at (1).status == -3); }
  // 8: explicit success acknowledges once, even if finish called twice.
  { auto x = attached (); int calls = 0;
    { event_ack<fake_ack> a (x, {&calls, 0});
      assert (a.finish () == 0 && a.finish () == 0); }
    assert (calls == 1 && !x.invalid ()); }
  // 9: explicit failure is not retried by destructor.
  { auto x = attached (); int calls = 0;
    { event_ack<fake_ack> a (x, {&calls, -9}); assert (a.finish () == -9); }
    assert (calls == 1); }
  // 10: ordinary unwind/fallback is invalid even after native ack success.
  { auto x = attached (); int calls = 0;
    { event_ack<fake_ack> a (x, {&calls, 0}); }
    assert (calls == 1 && x.invalid ());
    assert (x.at (1).invalidation == reason::handler_unwind); }
  // 11: exception fallback cannot fabricate completion.
  { auto x = attached (); int calls = 0;
    try { event_ack<fake_ack> a (x, {&calls, -4}); throw std::runtime_error ("fixture"); }
    catch (const std::runtime_error &) {}
    assert (calls == 1 && x.invalid () && x.at (1).status == -4); }
  // 12: real code-object handling joins one active callback.
  { auto x = attached (); x.begin_callback (70, 3);
    x.completed (kind::code_objects_complete, 11, 3, 0, 0);
    x.end_callback (12, 4, 2, 0);
    assert (!x.invalid () && x.at (1).callback == 1);
    assert (x.at (2).breakpoint == 70 && x.at (2).thread == 3);
    assert (x.at (2).event == 12 && x.at (2).action == 2); }
  // 13: immediate resume deliberately has no resume event.
  { auto x = attached (); x.begin_callback (70, 3); x.end_callback (0, 0, 1, 0);
    assert (!x.invalid () && x.at (1).event == 0 && x.at (1).action == 1); }
  // 14: nested callback invalidates.
  { auto x = attached (); x.begin_callback (70, 3); x.begin_callback (71, 3);
    assert (x.invalid () && x.at (1).invalidation == reason::callback_nested); }
  // 15: wrong callback completion cannot synthesize a hit.
  { auto x = attached (); x.end_callback (12, 4, 2, 0);
    assert (x.invalid ()); }
  // 16: callback ack failure invalidates.
  { auto x = attached (); x.begin_callback (70, 3); x.end_callback (12, 4, 2, -4);
    assert (x.invalid () && x.at (1).status == -4); }
  // 17: duplicate native event IDs are rejected.
  { auto x = attached (); x.completed (kind::runtime_complete, 10, 5, 1, 0);
    x.completed (kind::code_objects_complete, 10, 3, 0, 0);
    assert (x.invalid () && x.at (2).invalidation == reason::event_identity); }
  // 18: flushing does not reset storage/work allowance.
  { auto x = attached ();
    for (unsigned i = 0; i < 30; ++i)
      { x.completed (kind::code_objects_complete, 10 + i, 3, 0, 0);
        while (x.next () != nullptr) {} }
    assert (x.count () == 31 && !x.invalid ());
    x.completed (kind::code_objects_complete, 99, 3, 0, 0);
    assert (x.count () == 32 && x.invalid ());
    assert (x.at (31).sequence == 32 && x.at (31).invalidation == reason::capacity); }
  // 19: output failure is terminal, never replayed as a new stream.
  { auto x = attached (); assert (x.next ()->sequence == 1);
    x.output_failed (); assert (x.next ()->type == kind::invalidated);
    assert (x.next () == nullptr && x.invalid ()); }
  // 20: all invalidation causes are sticky, without reviving on later data.
  for (reason why : {reason::runtime_unloaded, reason::runtime_restriction,
                     reason::inferior_exec, reason::inferior_fork,
                     reason::inferior_exit, reason::detach, reason::object_removed})
    { auto x = attached (); x.invalidate (why);
      x.completed (kind::runtime_complete, 10, 5, 1, 0);
      x.invalidate (reason::setup_failure);
      assert (x.count () == 2 && x.at (1).invalidation == why); }
}
