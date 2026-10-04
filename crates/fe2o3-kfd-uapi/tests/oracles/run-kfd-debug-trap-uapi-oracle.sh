#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
kernel=/usr/src/amdgpu-6.16.13-2341068.24.04
oracle="$repo_root/crates/fe2o3-kfd-uapi/tests/oracles/kfd_debug_trap_uapi_1_18.c"
binary=$(mktemp)
observed=$(mktemp)
expected=$(mktemp)
trap 'rm -f "$binary" "$observed" "$expected"' EXIT

test "$(sha256sum "$kernel/include/uapi/linux/kfd_ioctl.h" | cut -d' ' -f1)" = b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d
test "$(sha256sum "$kernel/amd/amdkfd/kfd_debug.c" | cut -d' ' -f1)" = f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d
test "$(sha256sum "$kernel/amd/amdkfd/kfd_chardev.c" | cut -d' ' -f1)" = f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba

cc -std=c11 -Wall -Wextra -Werror -I"$kernel/include/uapi" "$oracle" -o "$binary"
"$binary" >"$observed"

cat >"$expected" <<'EOF'
dbg_trap=0xc0204b26
runtime_enable=0xc0104b25 modes=1,2
ops=0,1,2,3,4,5,6,7,8,9,10,11,12,13,14
kfd_runtime_info size=16 align=8
kfd_ioctl_runtime_enable_args size=16 align=8
kfd_ioctl_runtime_enable_args.r_debug=0
kfd_ioctl_runtime_enable_args.mode_mask=8
kfd_ioctl_runtime_enable_args.capabilities_mask=12
kfd_queue_snapshot_entry size=64 align=8
kfd_queue_snapshot_entry.queue_id=40
kfd_queue_snapshot_entry.reserved=60
kfd_dbg_device_info_entry size=120 align=8
kfd_dbg_device_info_entry.gpu_id=56
kfd_dbg_device_info_entry.gfx_target_version=88
kfd_dbg_device_info_entry.debug_prop=116
kfd_context_save_area_header size=40 align=8
kfd_context_save_area_header.err_payload_addr=24
kfd_ioctl_dbg_trap_enable_args size=24 align=8
kfd_ioctl_dbg_trap_send_runtime_event_args size=16 align=8
kfd_ioctl_dbg_trap_set_exceptions_enabled_args size=8 align=8
kfd_ioctl_dbg_trap_set_wave_launch_override_args size=16 align=4
kfd_ioctl_dbg_trap_set_wave_launch_mode_args size=8 align=4
kfd_ioctl_dbg_trap_suspend_queues_args size=24 align=8
kfd_ioctl_dbg_trap_resume_queues_args size=16 align=8
kfd_ioctl_dbg_trap_set_node_address_watch_args size=24 align=8
kfd_ioctl_dbg_trap_clear_node_address_watch_args size=8 align=4
kfd_ioctl_dbg_trap_set_flags_args size=8 align=4
kfd_ioctl_dbg_trap_query_debug_event_args size=16 align=8
kfd_ioctl_dbg_trap_query_exception_info_args size=24 align=8
kfd_ioctl_dbg_trap_queue_snapshot_args size=24 align=8
kfd_ioctl_dbg_trap_device_snapshot_args size=24 align=8
kfd_ioctl_dbg_trap_args size=32 align=8
kfd_ioctl_dbg_trap_args.pid=0
kfd_ioctl_dbg_trap_args.op=4
kfd_ioctl_dbg_trap_args.enable=8
EOF

diff -u "$expected" "$observed"
printf 'KFD 1.18 debug-trap UAPI oracle: PASS\n'
