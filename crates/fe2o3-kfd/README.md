# fe2o3-kfd

Owned syscall adapters for the direct-KFD fe2o3 runtime. The initial slice is
deliberately limited to opening `/dev/kfd`, querying its UAPI version, and
producing checked admission evidence for the exact reviewed schema in
`fe2o3-kfd-uapi`. The R1 topology slice additionally provides strict,
read-only discovery of the kernel-owned KFD sysfs tree for the initial `gfx942`
profile. It bounds every read, node/property count, and parsed field; rejects
symlinks, non-regular inputs, duplicate identities, malformed values, and
unknown property keys; and records topology generation plus filesystem and
platform provenance. Default-host discovery additionally records a strictly
parsed boot UUID, bounded kernel release, optional `amdgpu` module version
identities, and opaque KFD firmware-version observations, then correlates every
render minor to a kernel sysfs link below `/sys/devices`.
KFD unique ID, PCI domain/location, vendor/device ID, PCI revision, render
device number, and typed compute/memory partition observations are captured or
must agree. The initial admission layer can require the exported `SPX/NPS1` V1
partition constant without losing the observed values. The fixed
kernel-owned render and PCI symlinks are resolved deliberately; symlinks in
the KFD topology tree and regular-file inputs remain prohibited.

The public safe API does not expose file descriptors or raw ioctl arguments.
The R1 composition path consumes an explicitly selected unique ID and returns a
non-cloneable `CheckedGfx942XnackMinusDevice`. It retains `/dev/kfd` and the
exact correlated render descriptor, owns a process-global fe2o3 admission
lease, requires KFD 1.18 and AMDGPU DRM 3.64.0, compares the DRM identity prefix
with topology/sysfs, establishes a disabled-XNACK no-queue barrier, checks the
complete bounded process-aperture inventory, and repeats process, descriptor,
topology, XNACK, and aperture observations before committing the token. The
`DEVICE_ADMISSION_PROFILE_MANIFEST_V1` digest binds the exact checked profile
and claim boundary. Retired model history is retained across admissions in the
same process and observation domain; a poisoned history fails closed. The
`kfd-device-identity` example performs this no-queue admission.

This crate checks userspace schema admission and encapsulates descriptor
ownership. The abstract Verus device-generation theorem is stored only as a
model receipt; there is not yet a concrete adapter refinement proof or a
`ProductionDeviceAuthorityV1` implementation. No R1 API grants VM, allocation,
mapping, queue, event, code, or dispatch authority. It does not enumerate
cache, memory-bank, or link subtrees or prove their reported counts. The
process-global lease excludes other fe2o3 R1 admissions, not arbitrary raw KFD
users in the process. Ancestor traversal, mount-namespace integrity, sysfs
truth, cross-file snapshot semantics, KFD/DRM ioctl behavior, firmware meaning,
and absence of an ABA reset remain named external contracts. Successful kernel
responses and node metadata are checked or contracted observations, not proof
of the kernel or hardware implementation.

The separate `scripts/runtime-identity-oracle.sh` hardware lane compares the
`kfd-device-identity --all` evidence with bounded output from an isolated
`/opt/rocm/bin/rocminfo` subprocess. A match is recorded only as `Measured` with
`authority=none`; oracle output is never passed to this crate and cannot create
device, VM, memory, queue, dispatch, or proof authority. The exact comparison,
evidence schema, CI separation, and limitations are documented in
`docs/runtime-identity-oracle-v1.md`.
