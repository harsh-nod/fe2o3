/* SPDX-License-Identifier: GPL-3.0-or-later
   Private fixed measured CPU target, NOT a native-run qualification.
   Source replacement/review is required to select another executable. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_PROFILE_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_PROFILE_V1_H
#include "amd-dbgapi-one-stop-checkpoint-v1.h"
namespace amd_owned_one_stop_v1 {
inline constexpr char target_path[] = "/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/target-gfx950-one-stop-observer-r3/debug/fe2o3-private-one-stop-target";
inline constexpr std::uint64_t target_bytes = 4162064;
inline constexpr digest_bytes target_sha {{
  0x1e,0xc7,0x2e,0x9d,0x53,0xdf,0x21,0xa5,0x10,0x08,0x99,0x51,0xa6,0xbb,0xa9,0xa0,
  0x16,0x7d,0x8b,0x7e,0x23,0x23,0xe3,0xdc,0xd1,0xd1,0xec,0x5a,0xdd,0x3f,0x3b,0xdb
}};
/* Exact1116-byte retained trap text followed by2980 zero bytes. Distinct
   from the logical trap digest retained inside the target checkpoint. */
inline constexpr digest_bytes fixed_trap_backing_sha {{
  0x3d,0x50,0x09,0xe2,0x13,0x70,0x8e,0x59,0x79,0xd5,0xfb,0xc2,0xb7,0xf9,0xe4,0x77,0x97,0x12,0xbb,0xcb,0xfa,0x07,0xa0,0xda,0x30,0x1d,0xd3,0xb2,0xaf,0x05,0x47,0x95
}};
inline constexpr char entry_symbol[] = "fe2o3_gfx950_one_stop_process_entry_v1";
inline constexpr char checkpoint_symbol[] = "fe2o3_gfx950_one_stop_prepublication_checkpoint_v1";
inline constexpr std::uint64_t entry_symbol_value = 0xc7800;
inline constexpr std::uint64_t checkpoint_symbol_value = 0x19daf0;
inline constexpr std::uint64_t entry_symbol_size = 16;
inline constexpr std::uint64_t checkpoint_symbol_size = 20;
/* Exact measured file payload plus two independent EOF probes. */
inline constexpr std::uint64_t target_file_read_cap = 2 * target_bytes + 2;
inline constexpr std::uint64_t target_file_round_cap = 2 * ((target_bytes + 9 + 63) / 64) * 64;
static_assert (target_file_read_cap == 8324130, "fixed two-pass file reads/EOF");
static_assert (target_file_round_cap == 8324224, "fixed two-pass SHA rounds");
/* Measured content is not qualification. The actual native hook path must
   establish its same-client/entry/retirement/resume joins independently; no
   Boolean, command argument or JSON field can waive those joins. */
}
#endif
