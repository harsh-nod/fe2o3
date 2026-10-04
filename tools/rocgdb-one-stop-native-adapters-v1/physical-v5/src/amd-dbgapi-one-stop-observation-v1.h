/* SPDX-License-Identifier: GPL-3.0-or-later
   In-process hook declarations only. No observation DTO is an owner. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_OBSERVATION_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_OBSERVATION_V1_H
#if HAVE_AMD_DBGAPI
#include "target.h"
#include <amd-dbgapi/amd-dbgapi.h>
#include <cstddef>
#include <cstdint>
#include <exception>
struct breakpoint;
struct program_space;
namespace amd_owned_one_stop_native_v1 {
bool selected () noexcept;
// This guard is constructed only by the stock internal_breakpoint::re_set
// deletion cases. It retains no breakpoint pointer beyond begin/consume.
bool before_internal_breakpoint_reset (breakpoint *,program_space *);
void after_internal_breakpoint_reset ();
void abort_breakpoint_refresh () noexcept;
class internal_breakpoint_reset_guard final {
  bool m_active;
public:
  internal_breakpoint_reset_guard (breakpoint *bp,program_space *pspace)
    : m_active (before_internal_breakpoint_reset (bp,pspace)) {}
  internal_breakpoint_reset_guard (const internal_breakpoint_reset_guard &)=delete;
  internal_breakpoint_reset_guard &operator= (const internal_breakpoint_reset_guard &)=delete;
  ~internal_breakpoint_reset_guard () noexcept {
    if (m_active) abort_breakpoint_refresh ();
  }
  void complete () {
    if (m_active) { after_internal_breakpoint_reset (); m_active=false; }
  }
};
void before_resume (ptid_t,int,gdb_signal);
void after_resume ();
void before_commit ();
void after_commit ();
void unwind () noexcept;
void infrun_begin (thread_info *,ptid_t,bool,gdb_signal,bool,bool,bool,bool);
void infrun_end () noexcept;
void before_finish_step (thread_info *,const target_waitstatus &,bool,bool,std::uint64_t);
void after_auto_delete ();
void after_normal_stop (thread_info *);
void flush_safe_point ();
void mi_input (const char *);
void nested_mi ();
bool selected_object (inferior *,amd_dbgapi_code_object_id_t &,std::ptrdiff_t &,const char *&);
/* Poison on unknown native outcome. This does not swallow GDB errors, retry,
   call a target, ACK an event, free client output or perform native teardown. */
class unwind_guard final {
  int m_depth=std::uncaught_exceptions ();
public:
  unwind_guard () noexcept = default;
  unwind_guard (const unwind_guard &)=delete;
  unwind_guard &operator= (const unwind_guard &)=delete;
  ~unwind_guard () noexcept { if (std::uncaught_exceptions ()>m_depth) unwind (); }
};
}
#endif
#endif
