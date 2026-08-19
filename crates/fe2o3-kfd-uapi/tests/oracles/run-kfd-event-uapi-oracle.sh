#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
kernel=/usr/src/amdgpu-6.16.13-2341068.24.04
rocr=/home/harsh/work/rocm-systems-7.2.4-issue137-r4-readonly
oracle="$repo_root/crates/fe2o3-kfd-uapi/tests/oracles/kfd_event_uapi_1_18.c"
binary=$(mktemp)
observed=$(mktemp)
expected=$(mktemp)
trap 'rm -f "$binary" "$observed" "$expected"' EXIT

check_hash() {
  local want=$1
  local path=$2
  local got
  got=$(sha256sum "$path" | cut -d' ' -f1)
  test "$got" = "$want" || {
    printf 'source drift: %s\nexpected %s\nobserved %s\n' "$path" "$want" "$got" >&2
    exit 1
  }
}

check_hash b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d "$kernel/include/uapi/linux/kfd_ioctl.h"
check_hash 295114e5bacb3be94cdc17b6760e893198ee51d1c77d5837cfab999c3823485a "$kernel/amd/amdkfd/kfd_events.c"
check_hash de275617babe153c015f22de23d4f3ed013759c0a63da96e061454114f0dd119 "$kernel/amd/amdkfd/kfd_events.h"
check_hash d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409 "$kernel/amd/amdkfd/kfd_process.c"
check_hash f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d "$kernel/amd/amdkfd/kfd_debug.c"
check_hash f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba "$kernel/amd/amdkfd/kfd_chardev.c"
check_hash f991330031c14725b2be0636ec1896ab530dc3d07d530ebd4f47efff97a82a99 "$kernel/amd/amdkfd/kfd_priv.h"
check_hash a76b99eeee2aee1c282659a1e43217817b83260ef52f532c6db8a9dfd1d993d9 "$rocr/projects/rocr-runtime/libhsakmt/src/events.c"
check_hash b7ead541340ac996c2305b2e9660cb3176edcd61ee509d4880f02659fbb6f32b "$rocr/projects/rocr-runtime/libhsakmt/src/queues.c"
check_hash 291f2521e2a4758e852ed20c578aca79e379d1effe4dfd83c62e11347eef2b14 "$rocr/projects/rocr-runtime/runtime/hsa-runtime/core/runtime/amd_aql_queue.cpp"
check_hash fd9e3e9a0874614e70e518ee420aacd2d171452c2755d05b2cf54b55144ec78e "$rocr/projects/rocr-runtime/libhsakmt/include/hsakmt/hsakmttypes.h"
test "$(git -C "$rocr" rev-parse HEAD)" = 97f5574fe2fdc7bef44fb01545347912ee9f1779

cc -std=c11 -Wall -Wextra -Werror -I"$kernel/include/uapi" "$oracle" -o "$binary"
"$binary" >"$observed"

cat >"$expected" <<'EOF'
version=1.18
create_event=0xc0204b08
destroy_event=0x40084b09
set_event=0x40084b0a
reset_event=0x40084b0b
wait_events=0xc0184b0c
event_types=0,1,2,3,4,5,6,7,8
wait_results=0,1,2
signal_limit=4096
queue_exception_mask=0x00000000607f803f
kfd_ioctl_create_event_args size=32 align=8
kfd_ioctl_create_event_args.event_page_offset=0
kfd_ioctl_create_event_args.event_trigger_data=8
kfd_ioctl_create_event_args.event_type=12
kfd_ioctl_create_event_args.auto_reset=16
kfd_ioctl_create_event_args.node_id=20
kfd_ioctl_create_event_args.event_id=24
kfd_ioctl_create_event_args.event_slot_index=28
kfd_ioctl_destroy_event_args size=8 align=4
kfd_ioctl_set_event_args size=8 align=4
kfd_ioctl_reset_event_args size=8 align=4
kfd_memory_exception_failure size=16 align=4
kfd_hsa_memory_exception_data size=32 align=8
kfd_hsa_memory_exception_data.failure=0
kfd_hsa_memory_exception_data.va=16
kfd_hsa_memory_exception_data.gpu_id=24
kfd_hsa_memory_exception_data.ErrorType=28
kfd_hsa_hw_exception_data size=16 align=4
kfd_hsa_signal_event_data size=8 align=8
kfd_event_data size=48 align=8
kfd_event_data.kfd_event_data_ext=32
kfd_event_data.event_id=40
kfd_event_data.pad=44
kfd_ioctl_wait_events_args size=24 align=8
kfd_ioctl_wait_events_args.events_ptr=0
kfd_ioctl_wait_events_args.num_events=8
kfd_ioctl_wait_events_args.wait_for_all=12
kfd_ioctl_wait_events_args.timeout=16
kfd_ioctl_wait_events_args.wait_result=20
kfd_context_save_area_header size=40 align=8
kfd_context_save_area_header.debug_offset=16
kfd_context_save_area_header.debug_size=20
kfd_context_save_area_header.err_payload_addr=24
kfd_context_save_area_header.err_event_id=32
kfd_context_save_area_header.reserved1=36
EOF

diff -u "$expected" "$observed"
printf 'KFD 1.18 event UAPI oracle and source closure: PASS\n'
