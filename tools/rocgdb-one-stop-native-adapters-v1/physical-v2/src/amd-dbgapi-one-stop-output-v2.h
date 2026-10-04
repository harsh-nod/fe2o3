/* SPDX-License-Identifier: GPL-3.0-or-later
   Closed borrowed stdio submission check. No owner, unwrap, retry or delivery
   certificate. Actual stream selection remains in the sole native adapter. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_OUTPUT_V2_H
#define GDB_AMD_DBGAPI_ONE_STOP_OUTPUT_V2_H
#include <cstdio>
#include <cstddef>
namespace amd_owned_one_stop_v1 {
#ifdef FE2O3_ONE_STOP_OUTPUT_PURE_TEST
struct output_test;
#endif
class publication_output final {
  friend class native_adapter;
#ifdef FE2O3_ONE_STOP_OUTPUT_PURE_TEST
  friend struct output_test;
#endif
  publication_output ()=delete;
  static constexpr std::size_t scratch_bytes=256;
  static constexpr std::size_t work_per_row=128;
  template<typename Current,typename Put,typename Flush>
  static bool submit_once (FILE *file,Current &&current,Put &&put,Flush &&flush) {
    // Complete coexistence envelope: all three closures, caller UI/interpreter/
    // raw/pager/FILE locals, callback reference arguments, and this FILE argument.
    // Logical payload bound, not compiler stack-frame/RSS accounting.
    static_assert (sizeof (Current)+sizeof (Put)+sizeof (Flush)
      +12*sizeof (void *)+2*sizeof (FILE *)<=scratch_bytes,
      "one-stop output guard scratch must be prepaid before selection");
    if (file==nullptr || !current () || std::ferror (file)!=0) return false;
    put ();
    flush ();
    // Do not dereference a stale FILE after a changed borrowed stream identity.
    return current () && std::ferror (file)==0;
  }
};
} // namespace amd_owned_one_stop_v1
#endif
