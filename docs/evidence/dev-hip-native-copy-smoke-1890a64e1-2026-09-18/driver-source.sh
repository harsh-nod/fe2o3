#!/usr/bin/env bash
set -euo pipefail
date -u +%FT%T.%NZ
uname -a
dpkg-query -W amdgpu-dkms rocm-smi-lib rocm-core
cat /sys/module/amdgpu/version /sys/module/amdgpu/srcversion
modinfo -F filename amdgpu
modinfo -F srcversion amdgpu
base=/usr/src/amdgpu-6.16.13-2341068.24.04
printf 'selected_device_identity_and_topology\n'
cat /sys/bus/pci/devices/0000:85:00.0/unique_id /sys/bus/pci/devices/0000:85:00.0/numa_node /sys/bus/pci/devices/0000:85:00.0/local_cpulist
printf 'selected_MP1_major_minor_revision\n'
cat /sys/bus/pci/devices/0000:85:00.0/ip_discovery/die/0/MP1/0/major /sys/bus/pci/devices/0000:85:00.0/ip_discovery/die/0/MP1/0/minor /sys/bus/pci/devices/0000:85:00.0/ip_discovery/die/0/MP1/0/revision
sha256sum "$base/amd/pm/amdgpu_pm.c" "$base/amd/pm/swsmu/amdgpu_smu.c" "$base/amd/pm/swsmu/smu13/smu_v13_0_6_ppt.c" "$base/amd/pm/swsmu/smu13/smu_v13_0_6_ppt.h" /lib/modules/6.8.0-124-generic/updates/dkms/amdgpu.ko.zst
nl -ba "$base/amd/pm/amdgpu_pm.c" | sed -n '1443,1467p'
nl -ba "$base/amd/pm/swsmu/amdgpu_smu.c" | sed -n '771,786p'
nl -ba "$base/amd/pm/swsmu/smu13/smu_v13_0_6_ppt.c" | sed -n '908,938p;1445,1459p;1490,1501p;1901,1926p;1955,1976p;3133,3146p;3201,3210p'
rg '^CONFIG_HZ' /boot/config-6.8.0-124-generic
printf 'gpu_metrics_header_structure_size_little_endian_format_content\n'
od -An -N4 -tu1 /sys/bus/pci/devices/0000:85:00.0/gpu_metrics
date -u +%FT%T.%NZ
