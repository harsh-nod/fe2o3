/* SPDX-License-Identifier: GPL-3.0-or-later
   Root-run CPU controls. The actual submission helper is shared with the native
   adapter; wrapper shells exercise exact copied predicates, NOT GDB custody. */
#define FE2O3_ONE_STOP_OUTPUT_PURE_TEST
#include "amd-dbgapi-one-stop-output-v2.h"
#include "introspection-fixtures.inc"
#include <cassert>
#include <cstring>
#include <utility>
#include <stdexcept>
namespace amd_owned_one_stop_v1 {
struct output_test {
  template<typename C,typename P,typename F>
  static bool submit (FILE *file,C &&current,P &&put,F &&flush) {
    return publication_output::submit_once (file,std::forward<C>(current),
      std::forward<P>(put),std::forward<F>(flush));
  }
  static constexpr std::size_t scratch () { return publication_output::scratch_bytes; }
  static constexpr std::size_t work () { return publication_output::work_per_row; }
};
}
using amd_owned_one_stop_v1::output_test;
namespace {
struct file_owner {
  FILE *file;
  explicit file_owner (bool full=false):file(full ? std::fopen("/dev/full","w") : std::tmpfile()) {
    assert(file!=nullptr);
  }
  ~file_owner () { if(file!=nullptr) (void)std::fclose(file); }
  file_owner(const file_owner&)=delete;file_owner&operator=(const file_owner&)=delete;
};
struct calls { unsigned current=0,put=0,flush=0; };
bool submit (FILE *file,calls &n,bool permit=true) {
  return output_test::submit(file,[&](){++n.current;return permit;},
    [&](){++n.put;(void)std::fputs("bounded row\n",file);},
    [&](){++n.flush;(void)std::fflush(file);});
}
void healthy () {
  file_owner x;calls n;assert(submit(x.file,n));
  assert(n.current==2&&n.put==1&&n.flush==1&&std::ferror(x.file)==0);
  std::rewind(x.file);char bytes[32]={};
  assert(std::fread(bytes,1,12,x.file)==12);
  assert(std::memcmp(bytes,"bounded row\n",12)==0);
}
void before_effect_refusals () {
  file_owner x;calls null;assert(!submit(nullptr,null));
  assert(null.current==0&&null.put==0&&null.flush==0);
  calls changed;assert(!submit(x.file,changed,false));
  assert(changed.current==1&&changed.put==0&&changed.flush==0);
}
void nonthrowing_unbuffered_error () {
  file_owner x(true);assert(std::setvbuf(x.file,nullptr,_IONBF,0)==0);
  calls n;assert(!submit(x.file,n));
  assert(n.current==2&&n.put==1&&n.flush==1&&std::ferror(x.file)!=0);
  calls retry;assert(!submit(x.file,retry)); // Sticky error; no clearerr/retry effect.
  assert(retry.current==1&&retry.put==0&&retry.flush==0&&std::ferror(x.file)!=0);
}
void nonthrowing_buffered_error () {
  char storage[4096]={};file_owner x(true);
  assert(std::setvbuf(x.file,storage,_IOFBF,sizeof(storage))==0);
  calls n;int put_result=EOF,flush_result=0;
  assert(!output_test::submit(x.file,[&](){++n.current;return true;},
    [&](){++n.put;put_result=std::fputs("bounded row\n",x.file);assert(std::ferror(x.file)==0);},
    [&](){++n.flush;flush_result=std::fflush(x.file);}));
  assert(put_result!=EOF&&flush_result==EOF&&std::ferror(x.file)!=0);
  assert(n.current==2&&n.put==1&&n.flush==1);
}
void preexisting_error () {
  file_owner x(true);assert(std::setvbuf(x.file,nullptr,_IONBF,0)==0);
  assert(std::fputs("preexisting",x.file)==EOF&&std::ferror(x.file)!=0);
  calls n;assert(!submit(x.file,n));
  assert(n.current==1&&n.put==0&&n.flush==0&&std::ferror(x.file)!=0);
}
void changed_identity () {
  file_owner x;bool current=true;calls n;
  assert(!output_test::submit(x.file,[&](){++n.current;return current;},
    [&](){++n.put;(void)std::fputs("row",x.file);},
    [&](){++n.flush;(void)std::fflush(x.file);current=false;}));
  assert(n.current==2&&n.put==1&&n.flush==1);
}
void changed_identity_before_old_file_probe () {
  file_owner x;FILE *borrowed=x.file;bool current=true;calls n;
  assert(!output_test::submit(borrowed,[&](){++n.current;return current;},
    [&](){++n.put;(void)std::fputs("row",borrowed);},
    [&](){++n.flush;assert(std::fclose(borrowed)==0);x.file=nullptr;current=false;}));
  // The second identity refusal short-circuits ferror on the now closed FILE.
  assert(n.current==2&&n.put==1&&n.flush==1);
}
void exceptions_do_not_retry () {
  for(unsigned at=0;at<2;++at){file_owner x;calls n;bool caught=false;
    try{(void)output_test::submit(x.file,[&](){++n.current;return true;},
      [&](){++n.put;if(at==0)throw std::runtime_error("put");},
      [&](){++n.flush;throw std::runtime_error("flush");});}
    catch(const std::runtime_error&){caught=true;}
    assert(caught&&n.current==1&&n.put==1&&n.flush==(at==0?0U:1U));
  }
}
void exact_wrappers () {
  file_owner x,foreign;stdio_file s(x.file);pager_file p(&s);
  assert(p.one_stop_borrowed_output_matches(x.file));
  assert(!p.one_stop_borrowed_output_matches(nullptr));
  assert(!p.one_stop_borrowed_output_matches(foreign.file));
  stdio_file owning(x.file,true);pager_file owns(&owning);
  assert(!owns.one_stop_borrowed_output_matches(x.file));
  struct derived_stdio:stdio_file {using stdio_file::stdio_file;};
  derived_stdio d(x.file);pager_file derived_child(&d);
  assert(!derived_child.one_stop_borrowed_output_matches(x.file));
  struct derived_pager:pager_file {using pager_file::pager_file;};
  derived_pager outer(&s);assert(!outer.one_stop_borrowed_output_matches(x.file));
  ui_file unknown;pager_file wrapper(&unknown);
  assert(!wrapper.one_stop_borrowed_output_matches(x.file));
  pager_file nested(&p);assert(!nested.one_stop_borrowed_output_matches(x.file));
  pager_file absent(nullptr);assert(!absent.one_stop_borrowed_output_matches(x.file));
}
void pending_wrapper_state () {
  for(unsigned kind=0;kind<4;++kind){file_owner x;stdio_file s(x.file);pager_file p(&s);
    assert(p.one_stop_borrowed_output_matches(x.file));
    p.fixture_state(kind==0,kind==1?"pending":"",kind==2?1:0,kind==3?1:0);
    assert(!p.one_stop_borrowed_output_matches(x.file));
  }
}
void wrapper_refusal_prevents_submission () {
  file_owner x;stdio_file s(x.file);pager_file p(&s);calls n;
  p.fixture_state(true,"",0,0);
  assert(!output_test::submit(x.file,[&](){++n.current;return p.one_stop_borrowed_output_matches(x.file);},
    [&](){++n.put;},[&](){++n.flush;}));
  assert(n.current==1&&n.put==0&&n.flush==0);
}
void wrapper_change_after_single_flush () {
  file_owner x;stdio_file s(x.file);pager_file p(&s);calls n;
  assert(!output_test::submit(x.file,[&](){++n.current;return p.one_stop_borrowed_output_matches(x.file);},
    [&](){++n.put;(void)std::fputs("row",x.file);},
    [&](){++n.flush;(void)std::fflush(x.file);p.fixture_state(false,"",1,0);}));
  assert(n.current==2&&n.put==1&&n.flush==1);
}
}
int main () {
  static_assert(output_test::scratch()==256&&output_test::work()==128);
  healthy();before_effect_refusals();nonthrowing_unbuffered_error();
  nonthrowing_buffered_error();preexisting_error();changed_identity();
  changed_identity_before_old_file_probe();exceptions_do_not_retry();exact_wrappers();
  pending_wrapper_state();wrapper_refusal_prevents_submission();wrapper_change_after_single_flush();
}
