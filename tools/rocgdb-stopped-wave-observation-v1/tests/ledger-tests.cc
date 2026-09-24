/* SPDX-License-Identifier: GPL-3.0-or-later
   Pure synthetic ledger controls; NOT native-source or current-stop evidence. */
#include "../src/amd-dbgapi-stopped-wave-observation-v1.h"
#include <cstdio>
#include <cstdlib>
#include <utility>
namespace obs = amd_stopped_wave_observation_v1;
static unsigned checks;
static void require (bool value) { if (!value) std::abort (); }
template<class F> static void test (const char *name, F run)
{ run (); ++checks; std::printf ("PASS %s\n", name); }
static obs::stop_identity identity ()
{
  obs::stop_identity s;
  s.wave=1; s.workgroup=2; s.dispatch=3; s.queue=4; s.agent=5;
  s.architecture=6; s.os_agent=7;
  s.state=3; s.stop_reason=1u<<10; s.lanes=64;
  s.grid={64,1,1}; s.workgroup_size={64,1,1};
  s.queue_type=1; s.queue_state=1; s.elf_machine=0x4f;
  return s;
}
static obs::ledger ready ()
{
  obs::ledger l;
  require (l.bind (1,2,3,4)); l.attached ();
  l.completed (obs::kind::runtime_complete, 10,5,1,0);
  require (!l.invalid ()); return l;
}
static obs::reason last_reason (const obs::ledger &l)
{ require (l.invalid ()); return l.at (l.count ()-1).invalidation; }
int main ()
{
  test ("profile-zero-queue-packet-valid", [] { require(obs::closed_profile(identity())); });
  test ("stage-is-not-publication", [] { auto l=ready(); l.stage(11,1,22,identity(),0);
    require(l.pending() && l.count()==2); });
  test ("actual-safe-point-shaped-publication", [] { auto l=ready(); l.stage(11,1,22,identity(),0);
    l.publish_normal_stop(22,identity()); require(l.count()==3 && !l.pending() && !l.invalid());
    const auto &r=l.at(2); require(r.type==obs::kind::wave_stop_complete && r.stop_generation==1
      && r.event==11 && r.thread==22 && r.generation==1 && r.sequence==3); });
  test ("attach-cannot-reset", [] { auto l=ready(); require(!l.bind(1,2,3,4));
    require(last_reason(l)==obs::reason::foreign_owner); });
  test ("invalid-before-bind-sticky", [] { obs::ledger l; l.invalidate(obs::reason::setup_failure);
    require(!l.bind(1,2,3,4) && l.invalid()); });
  test ("preloaded-runtime-refused", [] { obs::ledger l; l.bind(1,2,3,4);
    l.completed(obs::kind::runtime_complete,10,5,1,0);
    require(last_reason(l)==obs::reason::preloaded_runtime); });
  test ("runtime-unloaded-refused", [] { auto l=ready(); l.completed(obs::kind::runtime_complete,11,5,2,0);
    require(last_reason(l)==obs::reason::runtime_unloaded); });
  test ("runtime-restriction-refused", [] { obs::ledger l; l.bind(1,2,3,4);l.attached();
    l.completed(obs::kind::runtime_complete,10,5,3,0);
    require(last_reason(l)==obs::reason::runtime_restriction); });
  test ("event-reuse-refused", [] { auto l=ready();l.stage(10,1,22,identity(),0);
    require(last_reason(l)==obs::reason::event_identity); });
  test ("wrong-event-kind-refused", [] { auto l=ready();l.stage(11,2,22,identity(),0);
    require(last_reason(l)==obs::reason::event_identity); });
  test ("zero-thread-refused", [] { auto l=ready();l.stage(11,1,0,identity(),0);
    require(last_reason(l)==obs::reason::event_identity); });
  test ("failed-ack-refused", [] { auto l=ready();l.stage(11,1,22,identity(),-7);
    require(last_reason(l)==obs::reason::acknowledgement && l.at(l.count()-1).status==-7); });
  test ("unjoined-safe-point-refused", [] { auto l=ready();l.publish_normal_stop(22,identity());
    require(last_reason(l)==obs::reason::stop_changed); });
  test ("wrong-safe-point-thread-refused", [] { auto l=ready();l.stage(11,1,22,identity(),0);
    l.publish_normal_stop(23,identity());require(last_reason(l)==obs::reason::stop_changed); });
  test ("second-stop-refused", [] { auto l=ready();l.stage(11,1,22,identity(),0);
    l.stage(12,1,22,identity(),0);require(last_reason(l)==obs::reason::stop_changed); });
  test ("republish-refused", [] { auto l=ready();l.stage(11,1,22,identity(),0);
    l.publish_normal_stop(22,identity());l.publish_normal_stop(22,identity());
    require(last_reason(l)==obs::reason::stop_changed); });
  test ("pre-stop-command-does-not-invalidate", [] { auto l=ready();l.change(obs::reason::stop_changed);
    require(!l.invalid()); });
  test ("staged-command-invalidates", [] { auto l=ready();l.stage(11,1,22,identity(),0);
    l.change(obs::reason::stop_changed);l.publish_normal_stop(22,identity());
    require(last_reason(l)==obs::reason::stop_changed && l.count()==3); });
  test ("published-command-invalidates", [] { auto l=ready();l.stage(11,1,22,identity(),0);
    l.publish_normal_stop(22,identity());l.change(obs::reason::stop_changed);
    require(last_reason(l)==obs::reason::stop_changed && l.count()==4); });
  test ("callback-resume-joins", [] { auto l=ready();l.begin_callback(40,41);l.end_callback(0,0,1,0);
    require(!l.invalid() && l.at(2).callback==1 && l.at(2).breakpoint==40); });
  test ("callback-halt-joins", [] { auto l=ready();l.begin_callback(40,41);
    l.completed(obs::kind::code_objects_complete,11,3,0,0);l.end_callback(12,4,2,0);
    require(!l.invalid() && l.at(2).callback==1 && l.at(3).event==12); });
  test ("nested-callback-refused", [] { auto l=ready();l.begin_callback(40,41);l.begin_callback(40,41);
    require(last_reason(l)==obs::reason::callback_nested); });
  test ("active-callback-wave-refused", [] { auto l=ready();l.begin_callback(40,41);
    l.stage(11,1,22,identity(),0);require(last_reason(l)==obs::reason::event_identity); });
  test ("callback-wrong-action-refused", [] { auto l=ready();l.begin_callback(40,41);l.end_callback(0,0,2,0);
    require(last_reason(l)==obs::reason::callback_identity); });
  test ("capacity-terminal-slot", [] { auto l=ready();
    for (unsigned i=0;i<70;++i) l.completed(obs::kind::code_objects_complete,20+i,3,0,0);
    require(l.count()==64 && last_reason(l)==obs::reason::capacity); });
  test ("output-consumed-once-no-replay", [] { auto l=ready(); require(l.next()->sequence==1);
    l.invalidate(obs::reason::output_failure);require(l.next()->sequence==2);
    require(l.next()->invalidation==obs::reason::output_failure && l.next()==nullptr); });
  test ("terminal-first-reason-sticky", [] { auto l=ready();l.invalidate(obs::reason::detach);
    l.invalidate(obs::reason::output_failure);require(l.count()==3 && last_reason(l)==obs::reason::detach); });
  using M = void(*)(obs::stop_identity &);
  const std::pair<const char *, M> mutations[] = {
    {"wave",+[](obs::stop_identity &s){++s.wave;}}, {"workgroup",+[](obs::stop_identity &s){++s.workgroup;}},
    {"dispatch",+[](obs::stop_identity &s){++s.dispatch;}}, {"queue",+[](obs::stop_identity &s){++s.queue;}},
    {"agent",+[](obs::stop_identity &s){++s.agent;}}, {"architecture",+[](obs::stop_identity &s){++s.architecture;}},
    {"os-queue",+[](obs::stop_identity &s){++s.os_queue;}}, {"packet",+[](obs::stop_identity &s){++s.packet;}},
    {"os-agent",+[](obs::stop_identity &s){++s.os_agent;}}, {"private",+[](obs::stop_identity &s){++s.private_bytes;}},
    {"group",+[](obs::stop_identity &s){++s.group_bytes;}}, {"state",+[](obs::stop_identity &s){++s.state;}},
    {"reason",+[](obs::stop_identity &s){++s.stop_reason;}}, {"lanes",+[](obs::stop_identity &s){++s.lanes;}},
    {"wave-index",+[](obs::stop_identity &s){++s.wave_index;}},
    {"coord-x",+[](obs::stop_identity &s){++s.coordinate[0];}}, {"coord-y",+[](obs::stop_identity &s){++s.coordinate[1];}},
    {"coord-z",+[](obs::stop_identity &s){++s.coordinate[2];}},
    {"grid-x",+[](obs::stop_identity &s){++s.grid[0];}}, {"grid-y",+[](obs::stop_identity &s){++s.grid[1];}},
    {"grid-z",+[](obs::stop_identity &s){++s.grid[2];}},
    {"size-x",+[](obs::stop_identity &s){++s.workgroup_size[0];}}, {"size-y",+[](obs::stop_identity &s){++s.workgroup_size[1];}},
    {"size-z",+[](obs::stop_identity &s){++s.workgroup_size[2];}},
    {"queue-type",+[](obs::stop_identity &s){++s.queue_type;}}, {"queue-state",+[](obs::stop_identity &s){++s.queue_state;}},
    {"elf",+[](obs::stop_identity &s){++s.elf_machine;}}
  };
  for (const auto &m : mutations) test(m.first,[&]{
    auto l=ready();l.stage(11,1,22,identity(),0);auto changed=identity();m.second(changed);
    l.publish_normal_stop(22,changed);require(last_reason(l)==obs::reason::stop_changed);
  });
  for (const auto &m : mutations) {
    auto changed=identity();m.second(changed);
    if (!obs::closed_profile(changed)) test(m.first,[&]{
      auto l=ready();l.stage(11,1,22,changed,0);
      require(last_reason(l)==obs::reason::unsupported_profile);
    });
  }
  std::uint64_t obs::stop_identity::* const nonzero[] = {
    &obs::stop_identity::wave,&obs::stop_identity::workgroup,&obs::stop_identity::dispatch,
    &obs::stop_identity::queue,&obs::stop_identity::agent,&obs::stop_identity::architecture,
    &obs::stop_identity::os_agent
  };
  for (auto member:nonzero) test("zero-native-identity-refused",[&]{
    auto changed=identity();changed.*member=0;auto l=ready();l.stage(11,1,22,changed,0);
    require(last_reason(l)==obs::reason::unsupported_profile);
  });
  std::printf("ledger controls=%u row=%zu ledger=%zu\n",checks,sizeof(obs::record),sizeof(obs::ledger));
}
