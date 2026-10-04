/* SPDX-License-Identifier: GPL-3.0-or-later
   Compile-candidate safety gate. No environment, command argument, JSON,
   token, receipt or caller pointer can change this source-level refusal.
   A separately reviewed source revision is required even to attempt arming. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_ACTIVATION_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_ACTIVATION_V1_H
namespace amd_owned_one_stop_v1 {
constexpr bool selection_available () noexcept { return false; }
static_assert (!selection_available (), "first adapter is compile-only");
}
#endif
