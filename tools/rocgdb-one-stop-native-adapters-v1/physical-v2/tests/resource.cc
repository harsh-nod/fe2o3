/* SPDX-License-Identifier: GPL-3.0-or-later
   Exact actual ledger arithmetic; inert owners are not native admission. */
#define FE2O3_ONE_STOP_PURE_TEST
#define FE2O3_ONE_STOP_OUTPUT_PURE_TEST
#include "amd-dbgapi-owned-one-stop-v1.h"
#include "amd-dbgapi-one-stop-output-v2.h"
#include <cassert>
#include <initializer_list>
namespace amd_owned_one_stop_v1 {
struct core_test {
  static bool reserve(owner&o,std::uint64_t n,std::uint64_t cap=limits::logical_bytes) {
    return o.reserve_selection(n,cap);
  }
  static bool charge(owner&o,std::uint64_t n){return o.debit(counter::work,n);}
  static std::uint64_t minimum(){return owner::fixed_storage();}
};
struct output_test {
  static constexpr std::uint64_t bytes(){return publication_output::scratch_bytes;}
  static constexpr std::uint64_t work(){return publication_output::work_per_row;}
  static bool after_payment(owner&o,FILE*file,unsigned&checks,unsigned&effects){
    if(!core_test::charge(o,work()))return false;
    return publication_output::submit_once(file,[&](){++checks;return true;},
      [&](){++effects;},[&](){++effects;});
  }
};
void prepayment_boundaries(){
  const auto base=core_test::minimum();
  {owner o;assert(core_test::reserve(o,base+output_test::bytes()));
    assert(o.consumed().logical_bytes==base+256);}
  {owner o;assert(!core_test::reserve(o,base+256,base+255));assert(o.invalid());}
  {owner o;assert(core_test::reserve(o,limits::logical_bytes));
    assert(o.consumed().logical_bytes==limits::logical_bytes);}
  {owner o;assert(!core_test::reserve(o,limits::logical_bytes+1));assert(o.invalid());}
}
void composed_work(){
  owner o;assert(core_test::reserve(o,core_test::minimum()+256));
  // Exact prior source census, not a new/reset meter or native execution result.
  constexpr std::uint64_t inherited=87168+13568+8192+1528+48+256+640+2080
    +340+192+128+508+4096+15*256+3*1536;
  static_assert(inherited==127192&&limits::rows==16&&output_test::work()==128);
  assert(core_test::charge(o,inherited));
  for(std::uint64_t row=0;row<limits::rows;++row)assert(core_test::charge(o,output_test::work()));
  assert(o.consumed().work==129240&&limits::work-o.consumed().work==1832);
  assert(core_test::charge(o,1832));assert(o.consumed().work==limits::work);
  assert(!core_test::charge(o,1)&&o.invalid());
  assert(o.consumed().work==limits::work);
  assert(!core_test::charge(o,0)); // First denial is sticky.
}
void debit_before_floor(){
  for(bool physical:{false,true}){owner o;
    assert(core_test::reserve(o,core_test::minimum()+256));
    assert(core_test::charge(o,output_test::work()));
    const auto formatter_floor=o.consumed();
    const auto formatting=physical?4608U:256U;
    assert(core_test::charge(o,formatting));
    assert(o.consumed().work-formatter_floor.work==formatting);
    assert(formatter_floor.work==128);
  }
}
void denied_before_new_checks(){
  FILE*file=std::tmpfile();assert(file!=nullptr);
  {owner baseline;assert(core_test::reserve(baseline,core_test::minimum()+256));
    unsigned checks=0,effects=0;
    assert(output_test::after_payment(baseline,file,checks,effects));
    assert(checks==2&&effects==2&&baseline.consumed().work==128);}
  owner o;assert(core_test::reserve(o,core_test::minimum()+256));
  assert(core_test::charge(o,limits::work-127));const auto before=o.consumed();
  unsigned checks=0,effects=0;assert(!output_test::after_payment(o,file,checks,effects));
  assert(checks==0&&effects==0&&o.invalid()&&o.consumed().work==before.work);
  assert(o.consumed().logical_bytes==before.logical_bytes);
  assert(std::fclose(file)==0);
}
}
int main(){using namespace amd_owned_one_stop_v1;
  prepayment_boundaries();composed_work();debit_before_floor();denied_before_new_checks();
}
