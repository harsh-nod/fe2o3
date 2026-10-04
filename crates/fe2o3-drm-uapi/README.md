# fe2o3-drm-uapi

Reviewed `no_std` raw definitions for the first fe2o3 AMDGPU render-identity
slice. The crate contains no file access, allocation, FFI, pointer dereference,
syscall execution, topology policy, or resource authority.

## Admitted schema

`linux-x86_64-drm-amdgpu-3.64.0-dkms-6.16.13-identity-currentness-v1` admits
only the x86_64 LP64 generic-`_IOC` layouts checked on the MI300X development
host.

Kernel-side sources:

- `linux-headers-6.8.0-124` package `6.8.0-124.124`
- `/usr/src/linux-headers-6.8.0-124/include/uapi/drm/drm.h` SHA-256
  `3ab6ac01bf91067aed96b70d7fa7847a86e7f726d74278151f085143688659cc`
- `amdgpu-dkms` package `1:6.16.13.30300400-2341068.24.04`
- `/usr/src/amdgpu-6.16.13-2341068.24.04/include/uapi/drm/amdgpu_drm.h`
  SHA-256 `9d7ff60a211d2aa73a6c15b2da49e050cebe518fc059ee93e31d61288f7b60dc`
- `/usr/src/amdgpu-6.16.13-2341068.24.04/amd/amdgpu/amdgpu_kms.c`
  SHA-256 `ef2375c3f35ad4a24b560326b55676a907d6d2ba248e469a62e84e877435101c`
- `/usr/src/amdgpu-6.16.13-2341068.24.04/amd/amdgpu/amdgpu_device.c`
  SHA-256 `4d0edc4b714c005e911596e0e2e616be7fdbbb3526069938e4cc078eaba83673`
- active AMDGPU DRM interface version `3.64.0`

Independent userspace comparison:

- `linux-libc-dev` package `6.8.0-137.137`
- `/usr/include/drm/drm.h` SHA-256
  `6b80aff056e2ac2e126e5144a3ce2c750292edb4d080d4689ac487dc17e4dae8`
- `libdrm-dev` package `2.4.125-1ubuntu0.1~24.04.2`
- `/usr/include/libdrm/drm.h` SHA-256
  `e97d535df3d33844a7c66578cb5adb501c57d17fb5ba55395309d1f275432060`
- `/usr/include/libdrm/amdgpu_drm.h` SHA-256
  `2881120496c69fc2154e590d0bc6e615a48adc43df1a658dd8cd8f78ec648557`

C oracles built independently against the active DKMS/exported-core headers and
the installed libdrm headers agree on every admitted size, alignment, offset,
request number, and query number. Production Rust code does not link libdrm.
`DRM_UAPI_SCHEMA_MANIFEST` binds the reviewed facts and provenance; its SHA-256
is recomputed in Rust tests.

The shared oracle source is preserved at
`tests/oracles/drm_amdgpu_identity_v1.c`. On the reviewed host, the two builds
select the active DKMS/exported-core headers and the independent libdrm headers:

```text
cc -std=c11 -Wall -Wextra -Werror \
  -DHAVE_DRM_COLOR_CTM_3X4 \
  -I/usr/src/amdgpu-6.16.13-2341068.24.04/include/uapi \
  tests/oracles/drm_amdgpu_identity_v1.c -o /tmp/drm-active-oracle
cc -std=c11 -Wall -Wextra -Werror -DFE2O3_LIBDRM_ORACLE \
  -I/usr/include/libdrm tests/oracles/drm_amdgpu_identity_v1.c \
  -o /tmp/drm-libdrm-oracle
```

## Admitted operations

- `DRM_IOCTL_VERSION`, used later to require driver name `amdgpu` and exact
  reviewed interface version `3.64.0`
- `DRM_IOCTL_AMDGPU_INFO` with `AMDGPU_INFO_ACCEL_WORKING`
- `DRM_IOCTL_AMDGPU_INFO` with `AMDGPU_INFO_DEV_INFO`, limited to the immutable
  first 20 bytes through `DrmAmdgpuDeviceIdentityV1`
- `DRM_IOCTL_AMDGPU_INFO` with `AMDGPU_INFO_VRAM_LOST_COUNTER`, limited to one
  `u32` destructive-reset observation

The full `drm_amdgpu_info_device` is append-only and currently 448 bytes in both
reviewed headers. V1 requests only its five-field prefix, which the active
driver supports through the UAPI's bounded `return_size` copy.

The VRAM-loss counter is not an all-reset generation. In the reviewed driver it
is incremented only when selected recovery paths determine that VRAM was lost,
and its `u32` value can wrap. It is useful only as one component of a contracted
currentness check.

## Identity boundary

The device ID, revisions, and family identify a GPU model, not one physical GPU.
A later adapter must bind an opened render descriptor's `st_rdev` to the kernel
DRM sysfs object, PCI BDF, KFD topology `drm_render_minor`, and topology/device
generation. Neither a successful ioctl nor a matching model record grants that
identity by itself.

Opaque addresses in `DrmVersion` and `DrmAmdgpuInfo` are data fields only. A
later unsafe syscall adapter owns pointer provenance, allocation lifetimes,
buffer bounds, and kernel-call contracts.

## Not supported

Other generic DRM or AMDGPU queries, fd ownership, sysfs parsing, PCI/KFD
pairing, topology snapshots, VM acquisition, GEM allocation, mappings, queues,
submission, synchronization, all-reset detection, reset recovery, and all
ioctl execution are outside this crate. A different architecture or data model
requires a new named schema.
