#!/bin/sh
set -eu

kernel_source=${1:-/usr/src/amdgpu-6.16.13-2341068.24.04}
rocr_source=${2:-/home/harsh/work/rocm-systems-7.2.4-issue137-r4-readonly}
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
binary=${TMPDIR:-/tmp}/fe2o3-kfd-gfx942-queue-resources-oracle

check() {
    expected=$1
    file=$2
    actual=$(sha256sum -- "$file" | awk '{print $1}')
    test "$actual" = "$expected" || {
        printf '%s: expected %s, observed %s\n' "$file" "$expected" "$actual" >&2
        exit 1
    }
}

check fb4b2a5c9e6981222873bcd7aca7e9c1397cba8f1a6b33634d2a48d4427fe062 "$kernel_source/amd/amdkfd/kfd_queue.c"
check 8526e258824dbe145e4209cf0fed26463729234ba24369f39e3413e7e6e028db "$kernel_source/amd/amdkfd/kfd_process_queue_manager.c"
check de30437ee1ed9ccbdaf855899482c0bebb7f55adc120ac712c96cadef1a0ec6d "$kernel_source/amd/amdkfd/kfd_doorbell.c"
check ccf20227c5cdd5b258758f50f61bbc1008a09ea776c101f035f83963e7d23037 "$kernel_source/amd/amdkfd/kfd_device.c"
check 21166e9dbe2a4c24cbcd6f9ff6193aa093230e91fbafc8b4ac4eee1465cd2c9e "$kernel_source/amd/amdkfd/kfd_mqd_manager_v9.c"
check f991330031c14725b2be0636ec1896ab530dc3d07d530ebd4f47efff97a82a99 "$kernel_source/amd/amdkfd/kfd_priv.h"
check 0fc8804ee63263f3a6f36fe6d7a2907c98610cb9d7db3e33239775a4b315c3de "$kernel_source/amd/amdkfd/kfd_topology.h"
check b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d "$kernel_source/include/uapi/linux/kfd_ioctl.h"

check b7ead541340ac996c2305b2e9660cb3176edcd61ee509d4880f02659fbb6f32b "$rocr_source/projects/rocr-runtime/libhsakmt/src/queues.c"
check 97269f0baf231d490032fc47ea8fe9e1101232477e10f74ff15e616d8e54ad86 "$rocr_source/projects/rocr-runtime/libhsakmt/src/topology.c"
check a2addccabb82e0ca184eaaf722e976e254a898ccfc945d4d956c4e273e196aef "$rocr_source/projects/rocr-runtime/libhsakmt/src/fmm.c"
check fd9e3e9a0874614e70e518ee420aacd2d171452c2755d05b2cf54b55144ec78e "$rocr_source/projects/rocr-runtime/libhsakmt/include/hsakmt/hsakmttypes.h"
check 291f2521e2a4758e852ed20c578aca79e379d1effe4dfd83c62e11347eef2b14 "$rocr_source/projects/rocr-runtime/runtime/hsa-runtime/core/runtime/amd_aql_queue.cpp"
check c39d5f922e855ce57d3c1903beef325e6004431c2ee66ae000aac72a0e5999da "$rocr_source/projects/rocr-runtime/runtime/hsa-runtime/core/runtime/amd_gpu_agent.cpp"
check c6f961251ebc0ceb3da5107964fa34bb5dacf0d3973a0e179fcb06cf5ca98cb3 "$rocr_source/projects/rocr-runtime/runtime/hsa-runtime/core/driver/kfd/amd_kfd_driver.cpp"

cc -std=c11 -Wall -Wextra -Werror \
    -I"$kernel_source/include/uapi" \
    "$here/kfd_gfx942_queue_resources_1_18.c" -o "$binary"
"$binary"
