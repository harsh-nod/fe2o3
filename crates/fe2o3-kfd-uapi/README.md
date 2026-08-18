# fe2o3-kfd-uapi

Reviewed, `no_std`-compatible raw definitions for the first fe2o3 direct-KFD
runtime slice. This crate deliberately does not open devices, discover topology,
issue syscalls, or own resources.

## Admitted schema

`linux-kfd-uapi-1.18-generic-ioc-v1` was transcribed from the active AMDGPU DKMS
driver source installed on the MI300X development host:

- `amdgpu-dkms` package `1:6.16.13.30300400-2341068.24.04`
- `/usr/src/amdgpu-6.16.13-2341068.24.04/include/uapi/linux/kfd_ioctl.h`
  SHA-256 `b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d`
- `/usr/include/asm-generic/ioctl.h` SHA-256
  `76396e5537d75285c3ca20e3b6a79b101eebfdc14d39c104ff7eab778672160e`
- header-declared and running `/dev/kfd` UAPI version `1.18`

The committed slice contains only:

- `kfd_ioctl_get_version_args` and `AMDKFD_IOC_GET_VERSION`
- `kfd_process_device_apertures`,
  `kfd_ioctl_get_process_apertures_new_args`, and
  `AMDKFD_IOC_GET_PROCESS_APERTURES_NEW`
- `kfd_ioctl_acquire_vm_args` and `AMDKFD_IOC_ACQUIRE_VM`
- `kfd_ioctl_set_xnack_mode_args` and `AMDKFD_IOC_SET_XNACK_MODE`
- the generic Linux `_IOC` encoding needed by those requests
- exact-version admission evidence

Compile-time assertions and `tests/kfd_uapi_1_18.rs` pin every struct size,
alignment, field offset, and request number to independent golden values.
`KFD_UAPI_SCHEMA_MANIFEST` canonically binds those ABI facts, source-header and
package provenance, and target encoding; its SHA-256 is recomputed in tests.
This manifest identifies reviewed userspace content. Running kernel, module,
boot, and device identities remain separate contracted observations.

The independent C oracle is preserved at
`tests/oracles/kfd_uapi_1_18.c`. On the reviewed host it is built directly
against the active header with:

```text
cc -std=c11 -Wall -Wextra -Werror \
  -I/usr/src/amdgpu-6.16.13-2341068.24.04/include/uapi \
  tests/oracles/kfd_uapi_1_18.c -o /tmp/kfd-uapi-oracle
```

## Fail-closed boundary

The initial schema accepts exactly UAPI `1.18`. Linux minor UAPI revisions are
normally backwards compatible, but accepting an unreviewed revision would make
the crate's assurance claim broader than its evidence. Supporting another minor
version requires a named schema update, header-oracle comparison, and reviewed
compatibility tests.

The request encoder models Linux's generic `_IOC` bit layout used by the x86_64
MI300X runtime target. An architecture that overrides that layout requires a
separate reviewed schema.

## Not yet supported

Topology parsing, stable device identity, aperture buffer bounds and snapshot
policy, VM ownership, memory allocation and mapping, queue creation,
event/signal handling, code-object loading, XNACK mode policy, and syscall
execution remain outside this crate. In particular, this crate is not a safe
wrapper around `/dev/kfd`; it is the bounded data-only input to that wrapper.
