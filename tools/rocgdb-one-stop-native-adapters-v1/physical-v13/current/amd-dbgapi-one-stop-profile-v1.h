/* SPDX-License-Identifier: GPL-3.0-or-later
   Private fixed measured CPU target, NOT a native-run qualification.
   Source replacement/review is required to select another executable. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_PROFILE_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_PROFILE_V1_H
#include "amd-dbgapi-one-stop-checkpoint-v1.h"
namespace amd_owned_one_stop_v1 {
inline constexpr char target_path[] = "/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/target-gfx950-one-stop-observer-diagnostic-r75-r2/debug/fe2o3-private-one-stop-target";
inline constexpr std::uint64_t target_bytes = 4222440;
inline constexpr digest_bytes target_sha {{
  0xfd,0xae,0xcf,0xe0,0x54,0xd4,0xc3,0x2f,0x17,0x21,0x74,0x2c,0xe2,0x83,0x95,0xd8,
  0xe0,0x7a,0x7a,0x2c,0xd1,0xb6,0x0d,0x96,0xd3,0x1d,0x07,0xbc,0x81,0xd2,0xfa,0x32
}};
/* Exact1116-byte retained trap text followed by2980 zero bytes. Distinct
   from the logical trap digest retained inside the target checkpoint. */
inline constexpr digest_bytes fixed_trap_backing_sha {{
  0x3d,0x50,0x09,0xe2,0x13,0x70,0x8e,0x59,0x79,0xd5,0xfb,0xc2,0xb7,0xf9,0xe4,0x77,0x97,0x12,0xbb,0xcb,0xfa,0x07,0xa0,0xda,0x30,0x1d,0xd3,0xb2,0xaf,0x05,0x47,0x95
}};
inline constexpr char entry_symbol[] = "fe2o3_gfx950_one_stop_process_entry_v1";
inline constexpr char checkpoint_symbol[] = "fe2o3_gfx950_one_stop_prepublication_checkpoint_v1";
inline constexpr std::uint64_t entry_symbol_value = 0xe09e0;
inline constexpr std::uint64_t checkpoint_symbol_value = 0x163260;
inline constexpr std::uint64_t entry_symbol_size = 16;
inline constexpr std::uint64_t checkpoint_symbol_size = 20;
/* Exact measured file payload plus two independent EOF probes. */
inline constexpr std::uint64_t target_file_read_cap = 2 * target_bytes + 2;
inline constexpr std::uint64_t target_file_round_cap = 2 * ((target_bytes + 9 + 63) / 64) * 64;
static_assert (target_file_read_cap == 8444882, "fixed two-pass file reads/EOF");
static_assert (target_file_round_cap == 8444928, "fixed two-pass SHA rounds");
/* Measured content is not qualification. The actual native hook path must
   establish its same-client/entry/retirement/resume joins independently; no
   Boolean, command argument or JSON field can waive those joins. */
}
#endif
