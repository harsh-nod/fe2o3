// SPDX-License-Identifier: GPL-3.0-or-later
// CPU-only exact diagnostic core/helper with explicitly mocked owner/refusal.
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <stdexcept>
#include "checkpoint-core.inc"
enum class failure { none, checkpoint_changed, native_result, owner_changed };
struct mock_owner { bool chosen=true; failure why=failure::none;
  bool selected () const { return chosen; } bool invalid () const { return why!=failure::none; }
  void poison (failure next) { if (!invalid ()) why=next; }
};
struct checkpoint_diagnostic_scratch { std::array<const void *,32> predicate_reference_storage; };
struct fixture {
  mock_owner m_owner; checkpoint_note m_checkpoint_note; unsigned require_calls=0;
  void require (bool yes,failure why) { ++require_calls; if(!yes || m_owner.invalid ()) { m_owner.poison (why); throw m_owner.why; } }
#include "checkpoint-helper.inc"
};
static unsigned checks=0;
static void check(bool yes) { ++checks; if(!yes)throw std::runtime_error("diagnostic control failed"); }
template<class Body> static failure refusal(Body body) { try { body(); } catch(failure why) { return why; } throw std::runtime_error("missing refusal"); }
int main () {
  check(sizeof(checkpoint_note)==2 && alignof(checkpoint_note)==2);
  { fixture f; int calls=0; f.checkpoint_require(checkpoint_site::hash_finish,[&]{++calls;return true;}); check(calls==1&&f.require_calls==1&&f.m_checkpoint_note.site==checkpoint_site::none&&!f.m_owner.invalid()); }
  { fixture f; int calls=0; const auto why=refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_codec,[&]{++calls;return false;});}); check(calls==1&&why==failure::checkpoint_changed&&f.m_checkpoint_note.site==checkpoint_site::checkpoint_codec); }
  { fixture f; const auto why=refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_root_bind,[&]{f.m_owner.poison(failure::checkpoint_changed);return false;});}); check(why==failure::checkpoint_changed&&f.m_checkpoint_note.site==checkpoint_site::checkpoint_root_bind); }
  { fixture f; f.m_owner.poison(failure::checkpoint_changed); int calls=0; refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_codec,[&]{++calls;return false;});}); check(calls==1&&f.m_checkpoint_note.site==checkpoint_site::none); }
  { fixture f; f.m_owner.chosen=false; refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_codec,[]{return false;});}); check(f.m_checkpoint_note.site==checkpoint_site::none); }
  { fixture f; refusal([&]{f.checkpoint_require(checkpoint_site::transport_ring,[]{return false;});}); refusal([&]{f.checkpoint_require(checkpoint_site::sole_queue_fields,[]{return false;});}); check(f.m_checkpoint_note.site==checkpoint_site::transport_ring&&f.require_calls==2); }
  { fixture f; refusal([&]{f.checkpoint_require(checkpoint_site::none,[]{return false;});}); check(f.m_checkpoint_note.site==checkpoint_site::none); }
  { fixture f; int sentinel=17; bool same=false; try { f.checkpoint_require(checkpoint_site::checkpoint_codec,[&]() -> bool { throw &sentinel; }); } catch(int *p) { same=p==&sentinel; } check(same&&f.require_calls==0&&f.m_checkpoint_note.site==checkpoint_site::none&&!f.m_owner.invalid()); }
  { fixture f; unsigned left=0,right=0; refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_codec,[&]{return (++left,false) && (++right,true);});}); check(left==1&&right==0); }
  { fixture f; unsigned left=0,right=0; f.checkpoint_require(checkpoint_site::checkpoint_codec,[&]{return (++left,true) && (++right,true);}); check(left==1&&right==1&&f.m_checkpoint_note.site==checkpoint_site::none); }
  { fixture f; refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_codec,[&]{f.checkpoint_require(checkpoint_site::transport_read,[]{return false;});return true;});}); check(f.m_checkpoint_note.site==checkpoint_site::transport_read&&f.require_calls==1); }
  { fixture f; const auto why=refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_codec,[&]() -> bool {f.m_owner.poison(failure::native_result);throw f.m_owner.why;});}); check(why==failure::native_result&&f.m_checkpoint_note.site==checkpoint_site::none&&f.require_calls==0); }
  { fixture f; const auto why=refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_codec,[&]{f.m_owner.poison(failure::owner_changed);return true;});}); check(why==failure::owner_changed&&f.m_checkpoint_note.site==checkpoint_site::none&&f.require_calls==1); }
  { fixture f; int argument=0; f.checkpoint_require(checkpoint_site::checkpoint_pc_argument,[&]{argument=42;return argument==42;}); check(argument==42&&f.require_calls==1); }
  { fixture f; f.m_owner.poison(failure::native_result); const auto why=refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_codec,[]{return false;});}); check(why==failure::native_result&&f.m_checkpoint_note.site==checkpoint_site::none); }
  { checkpoint_note note; note.site=checkpoint_site::executable_before; unsigned calls=0; const bool yes=note.evaluate(true,checkpoint_site::hash_finish,[&]{++calls;return false;}); check(!yes&&calls==1&&note.site==checkpoint_site::executable_before); }
  { fixture f; const auto why=refusal([&]{f.checkpoint_require(checkpoint_site::checkpoint_codec,[&]{f.m_owner.poison(failure::owner_changed);return false;});}); check(why==failure::owner_changed&&f.m_checkpoint_note.site==checkpoint_site::checkpoint_codec); }
  for(unsigned n=1;n<=38;++n) { fixture f; const auto site=static_cast<checkpoint_site>(n); unsigned calls=0; const auto why=refusal([&]{f.checkpoint_require(site,[&]{++calls;return false;});}); check(why==failure::checkpoint_changed&&f.m_checkpoint_note.site==site&&calls==1&&f.require_calls==1); }
  if(checks!=56) return 2;
  std::printf("checkpoint diagnostic controls passed: %u\n",checks);
}
