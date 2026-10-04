// SPDX-License-Identifier: GPL-3.0-or-later
// CPU-only exact extracted predicate and thread method; GDB object/arch/refusal scaffolding is mocked.
#include <array>
#include <cstdint>
#include <cstdio>
#include <stdexcept>
using simd_lanes_mask_t=std::uint64_t; // CPU scaffolding, not an ABI/layout claim.
struct gdbarch { bool hook=false; simd_lanes_mask_t mask=0; bool throws=false; };
static gdbarch arch;
static unsigned hook_calls=0;
static void gdb_assert(bool yes) { if(!yes)throw std::runtime_error("mock gdb_assert"); }
struct scoped_restore_current_thread { scoped_restore_current_thread() {} ~scoped_restore_current_thread() {} };
static void switch_to_inferior_no_thread(void *) {}
static gdbarch *target_thread_architecture(int) { return &arch; }
static bool gdbarch_active_lanes_mask_p(gdbarch *a) { return a->hook; }
static simd_lanes_mask_t gdbarch_active_lanes_mask(gdbarch *a,void *) {
  ++hook_calls; if(a->throws)throw &arch; return a->mask;
}
struct thread_info { void *inf=this; int ptid=1; simd_lanes_mask_t active_simd_lanes_mask(); };
#include "thread-method.inc"
enum class phase { provisional_checkpoint, selected };
enum breakpoint_type { bp_breakpoint, bp_other };
enum enabled_state { bp_enabled, bp_disabled };
struct mock_owner { phase value=phase::provisional_checkpoint; phase state() const { return value; } };
struct mock_thread { bool exec=false,resume=false; bool executing()const{return exec;} bool resumed()const{return resume;} };
struct fixture;
struct mock_location_ref { const int *value=nullptr; const int *get() const { return value; } };
struct bpstat { const fixture *breakpoint_at=nullptr; mock_location_ref bp_location_at; bool stop=true; simd_lanes_mask_t simd_lane_mask=1; };
struct fixture {
  mock_owner m_owner; mock_thread host,other; mock_thread *m_host=&host,*selected=&host;
  int location=0,other_location=0; bpstat storage; const bpstat *status=&storage;
  breakpoint_type type=bp_breakpoint; int number=-1; enabled_state enable_state=bp_enabled;
  bool silent=false; int ignore_count=0; const void *commands=nullptr;
  const char *cond_string=nullptr,*extra_string=nullptr;
  fixture() { storage.breakpoint_at=this;storage.bp_location_at.value=&location; }
  mock_thread *inferior_thread() const { return selected; }
  const int &first_loc()const{return location;}
  bool presentation() const { const auto &adapter=*this; return
#include "presentation-condition.inc"
  ; }
  bool legacy() const { const auto &adapter=*this; return
#include "legacy-condition.inc"
  ; }
};
static unsigned checks=0;
static void check(bool yes){++checks;if(!yes)throw std::runtime_error("scalar presentation control failed");}
int main() {
  {fixture f;check(f.presentation()&&!f.legacy());}
  {fixture f;f.storage.simd_lane_mask=0;check(!f.presentation()&&f.legacy());}
  for(std::uint64_t mask=0;mask<256;++mask){fixture f;f.storage.simd_lane_mask=mask;check(f.presentation()==(mask==1)&&f.legacy()==(mask==0));}
  const std::array<void(*)(fixture&),17> mutations={{
    [](fixture&f){f.m_owner.value=phase::selected;},
    [](fixture&f){f.selected=&f.other;},
    [](fixture&f){f.host.exec=true;},
    [](fixture&f){f.host.resume=true;},
    [](fixture&f){f.status=nullptr;},
    [](fixture&f){f.storage.breakpoint_at=nullptr;},
    [](fixture&f){f.storage.bp_location_at.value=&f.other_location;},
    [](fixture&f){f.storage.stop=false;},
    [](fixture&f){f.type=bp_other;},
    [](fixture&f){f.number=0;},
    [](fixture&f){f.number=1;},
    [](fixture&f){f.enable_state=bp_disabled;},
    [](fixture&f){f.silent=true;},
    [](fixture&f){f.ignore_count=1;},
    [](fixture&f){f.commands=&f;},
    [](fixture&f){f.cond_string="x";},
    [](fixture&f){f.extra_string="x";}
  }};
  for(auto mutate:mutations){fixture f;mutate(f);check(!f.presentation());f.storage.simd_lane_mask=0;check(!f.legacy());}
  {thread_info t;arch={false,0,false};hook_calls=0;check(t.active_simd_lanes_mask()==1&&hook_calls==0);}
  for(const auto mask:std::array<simd_lanes_mask_t,4>{{0,1,3,UINT64_C(1)<<63}}){
    thread_info t;arch={true,mask,false};hook_calls=0;check(t.active_simd_lanes_mask()==mask&&hook_calls==1);
  }
  {thread_info t;arch={true,0,true};hook_calls=0;bool same=false;try{t.active_simd_lanes_mask();}catch(gdbarch*p){same=p==&arch;}check(same&&hook_calls==1);}
  {thread_info t;t.inf=nullptr;arch={false,0,false};bool refused=false;try{t.active_simd_lanes_mask();}catch(const std::runtime_error&){refused=true;}check(refused);}
  // Faithful assignment order, not a full bpstat_check_breakpoint_conditions execution.
  {fixture f;thread_info t;arch={false,0,false};f.storage.simd_lane_mask=0;
   const bool constructor_only=f.legacy()&&!f.presentation();
   f.storage.simd_lane_mask=t.active_simd_lanes_mask();
   check(constructor_only&&f.presentation()&&!f.legacy());}
  if(checks!=300)return 2;
  std::printf("scalar checkpoint presentation controls passed: %u\n",checks);
}
