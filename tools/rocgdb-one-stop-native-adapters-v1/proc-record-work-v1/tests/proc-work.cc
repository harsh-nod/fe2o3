/* SPDX-License-Identifier: GPL-3.0-or-later
   Pure IO mocks, exact production fragments, real owner ledger. */
#define FE2O3_ONE_STOP_PURE_TEST 1
#include "amd-dbgapi-owned-one-stop-v1.h"
#include <cassert>
#include <cstdio>
#include <cstring>
#include <string>
#include <limits>
using namespace amd_owned_one_stop_v1;
static std::string record;
static long returned=0;
static unsigned opens=0,reads=0,closes=0,touches=0;
static bool admitted=false,io_paid=false,open_ok=true;
static int mocked_open(const char*p,int flags){assert(io_paid&&std::strcmp(p,"/proc/123/stat")==0&&flags==7);++opens;return open_ok?4:-1;}
static long mocked_read(int fd,char*out,std::size_t n){assert(io_paid&&fd==4&&n==1024);++reads;std::memcpy(out,record.data(),record.size()<n?record.size():n);return returned;}
constexpr int O_RDONLY=1,O_CLOEXEC=2,O_NOFOLLOW=4;
struct scoped_fd{int value;explicit scoped_fd(int f):value(f){}int get()const{return value;}~scoped_fd(){if(value>=0)++closes;}};
struct tracked_record{std::array<char,1024>storage{};constexpr std::size_t size()const{return 1024;}char*data(){return storage.data();}char&operator[](std::size_t n){assert(admitted&&n<size());++touches;return storage[n];}};
namespace amd_owned_one_stop_v1 {
struct rejected{failure why;};
class native_adapter {
public:
 struct subject{int pid=123;} subject_value;
 subject*m_inferior=&subject_value;std::array<char,128>m_proc_path{};tracked_record m_proc;owner ledger;
 void require(bool yes,failure why){if(!yes){ledger.poison(why);throw rejected{ledger.why()};}}
 void debit(counter k,std::uint64_t n){if(!ledger.debit(k,n))throw rejected{ledger.why()};if(k==counter::proc_requested_bytes){assert(n==1024);io_paid=true;}if(k==counter::work)admitted=true;}
 std::uint64_t read_start_ticks();std::uint64_t legacy_read_start_ticks();
};
#define open mocked_open
#define read mocked_read
#include "read-start-ticks.inc"
#include "legacy-read-start-ticks.inc"
#undef read
#undef open
struct core_test {
 static void reset(native_adapter&v,const std::string&r,long n){record=r;returned=n;opens=reads=closes=touches=0;admitted=io_paid=false;open_ok=true;assert(v.ledger.select({1,2,3,4,5,6,7,8},owner::fixed_storage()));}
 static std::string valid(std::size_t n=0){std::string r="123 (a)b) R ";for(unsigned i=4;i<22;++i)r+="1 ";r+="987 ";if(n){assert(n>=r.size()+2);r+=std::string(n-r.size()-1,'0');}else r+="0";return r+"\n";}
 static bool call(native_adapter&v,bool old,std::uint64_t&t){try{t=old?v.legacy_read_start_ticks():v.read_start_ticks();return true;}catch(const rejected&){return false;}}
 static bool same(const budget_failure_note&a,const budget_failure_note&b){return a.kind==b.kind&&a.state==b.state&&a.present==b.present&&a.used==b.used&&a.requested==b.requested&&a.cap==b.cap;}
 static void run(){
  static_assert(limits::work==131072&&limits::proc_requested_bytes==16384,"unchanged caps");
  for(std::size_t n=valid().size();n<1024;++n){auto r=valid(n);native_adapter v;reset(v,r,static_cast<long>(r.size()));assert(v.read_start_ticks()==987&&v.ledger.consumed().work==r.size());assert(v.ledger.consumed().proc_requested_bytes==1024&&opens==1&&reads==1&&closes==1);assert(touches<=4*r.size()+64);native_adapter old;reset(old,r,static_cast<long>(r.size()));assert(old.legacy_read_start_ticks()==987&&old.ledger.consumed().work==1024);}
  const auto good=valid();
  for(std::size_t at=0;at<good.size();++at)for(char c:std::string(" )(\n0x")){auto r=good;r[at]=c;std::uint64_t x=0,y=0;native_adapter old;reset(old,r,static_cast<long>(r.size()));bool b=call(old,true,x);native_adapter now;reset(now,r,static_cast<long>(r.size()));bool n=call(now,false,y);assert(b==n&&(!n||x==y));}
  for(long n:{-1L,0L,1024L,1025L,std::numeric_limits<long>::max()}){native_adapter v;reset(v,good,n);std::uint64_t t=0;assert(!call(v,false,t)&&touches==0&&!admitted&&reads==1&&closes==1);assert(v.ledger.consumed().work==0&&v.ledger.consumed().proc_requested_bytes==1024&&!v.ledger.m_budget_failure.present);}
  {native_adapter v;reset(v,good,1);std::uint64_t t=0;assert(!call(v,false,t)&&v.ledger.consumed().work==1&&touches>0);}
  {native_adapter v;reset(v,good,static_cast<long>(good.size()));open_ok=false;std::uint64_t t=0;assert(!call(v,false,t)&&reads==0&&touches==0&&closes==0);}
  {native_adapter v;reset(v,good,static_cast<long>(good.size()));v.ledger.m_usage.work=limits::work-good.size();assert(v.read_start_ticks()==987&&v.ledger.consumed().work==limits::work);}
  {native_adapter v;reset(v,good,static_cast<long>(good.size()));v.ledger.m_usage.work=limits::work-good.size()+1;std::uint64_t t=0;assert(!call(v,false,t)&&touches==0&&!admitted&&reads==1&&closes==1);auto n=v.ledger.m_budget_failure;assert(n.present&&n.kind==counter::work&&n.used==limits::work-good.size()+1&&n.requested==good.size()&&n.cap==limits::work);assert(!call(v,false,t)&&same(n,v.ledger.m_budget_failure)&&reads==1&&touches==0);}
  {native_adapter v;reset(v,good,static_cast<long>(good.size()));v.ledger.m_usage.proc_requested_bytes=limits::proc_requested_bytes-1023;std::uint64_t t=0;assert(!call(v,false,t)&&opens==0&&reads==0&&touches==0&&v.ledger.m_budget_failure.kind==counter::proc_requested_bytes);}
  {native_adapter v;reset(v,good,static_cast<long>(good.size()));v.ledger.m_phase=phase::awaiting_target_completion;v.ledger.m_usage.work=131029;assert(!v.ledger.debit(counter::work,128));auto n=v.ledger.m_budget_failure;assert(n.state==phase::awaiting_target_completion&&n.used==131029&&n.requested==128&&n.cap==131072&&v.ledger.why()==failure::budget);assert(!v.ledger.debit(counter::work,1)&&same(n,v.ledger.m_budget_failure));v.ledger.poison(failure::native_result);assert(same(n,v.ledger.m_budget_failure));}
 }
};
}
int main(){amd_owned_one_stop_v1::core_test::run();}
