// SPDX-License-Identifier: GPL-3.0-or-later
// Pure owner controls; no GDB, target, runtime API or external input.
#define FE2O3_ONE_STOP_PURE_TEST
#define amd_owned_one_stop_v1 old_owner_namespace
#include "../../src/amd-dbgapi-owned-one-stop-v1.h"
#undef amd_owned_one_stop_v1
#undef GDB_AMD_DBGAPI_OWNED_ONE_STOP_V1_H
#include "amd-dbgapi-owned-one-stop-v1.h"
#include <cstdio>
#include <cstdlib>
namespace amd_owned_one_stop_v1 {
struct core_test {
 static void check(bool b){if(!b)std::abort();}
 static identity id(){return{1,2,3,4,5,6,7,8};}
 static owner& selected(owner&o){check(o.select(id(),limits::logical_bytes));return o;}
 static void run(){
  {owner o;o.poison(failure::commit,commit_site::entry_environment);
   check(!o.selected()&&o.why()==failure::none&&o.first_commit_site()==commit_site::none);}
  {owner o;selected(o);check(o.require(true,failure::commit,commit_site::entry_environment));
   check(!o.invalid()&&o.first_commit_site()==commit_site::none);
   check(!o.require(false,failure::commit,commit_site::entry_environment));
   check(o.why()==failure::commit&&o.first_commit_site()==commit_site::entry_environment);
   o.poison(failure::commit,commit_site::after_amd);o.poison(failure::output);
   check(o.why()==failure::commit&&o.first_commit_site()==commit_site::entry_environment);}
  {owner o;selected(o);o.poison(failure::owner_changed,commit_site::entry_environment);
   o.poison(failure::commit,commit_site::after_amd);
   check(o.why()==failure::owner_changed&&o.first_commit_site()==commit_site::none);}
  {owner o;selected(o);check(!o.commit_started(id()));
   check(o.why()==failure::commit&&o.first_commit_site()==commit_site::owner_commit_start);
   o.poison(failure::commit,commit_site::owner_start_bridge);
   check(o.first_commit_site()==commit_site::owner_commit_start);}
  {owner o;selected(o);check(!o.commit_returned());
   check(o.why()==failure::commit&&o.first_commit_site()==commit_site::owner_commit_return);}
  {owner o;selected(o);o.poison(failure::commit);
   o.poison(failure::commit,commit_site::entry_environment);
   check(o.why()==failure::commit&&o.first_commit_site()==commit_site::none);}
  check(static_cast<unsigned>(failure::commit)==15);
  check(sizeof(owner)<=sizeof(old_owner_namespace::owner)+alignof(owner));
  check(owner::fixed_storage()==sizeof(owner)+sizeof(workspace)+limits::client_output_bytes);
  check(owner::fixed_storage()<=limits::logical_bytes);
  check(limits::api==192&&limits::work==131072&&limits::read_calls==256);
 }
};
}
int main(int argc,char**){
 if(argc!=1)return 2;
 amd_owned_one_stop_v1::core_test::run();
 std::printf("CPU-only first-poison commit-site controls: 6 groups; old-owner-bytes=%zu new-owner-bytes=%zu; no native authority\n",
 sizeof(old_owner_namespace::owner),sizeof(amd_owned_one_stop_v1::owner));
 return 0;
}
