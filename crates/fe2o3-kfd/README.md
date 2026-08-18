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
parsed boot UUID, bounded kernel release, and optional `amdgpu` module version identities, then
correlates every render minor to a kernel sysfs link below `/sys/devices`.
KFD unique ID, PCI domain/location, vendor/device ID, PCI revision, render
device number, and typed compute/memory partition observations are captured or
must agree. Each GPU observation also retains the bounded, opaque KFD topology
`fw_version` and `sdma_fw_version` integers. The initial admission layer can
require the exported `SPX/NPS1` V1
partition constant without losing the observed values. The fixed
kernel-owned render and PCI symlinks are resolved deliberately; symlinks in
the KFD topology tree and regular-file inputs remain prohibited.

The public safe API does not expose file descriptors, raw ioctl arguments, or
an unchecked way to construct a schema-admitted descriptor. Schema admission
is not device admission: this slice does not yet authenticate sysfs topology,
the matching render node, physical device/partition/target identity, kernel
boot/module identity, or their generations. Later KFD operations must require
a separate device-bound typestate and modeled resource ownership.

This crate checks userspace schema admission and encapsulates descriptor
ownership. Topology discovery does not open DRM render nodes or issue DRM
ioctls, and it grants no VM, allocation, mapping, queue, or ioctl authority. It does not
enumerate cache, memory-bank, or link subtrees, prove their reported counts,
admit XNACK state, or replace render-FD/DRM-info correlation in the later
authority layer. The safe filesystem API checks terminal entries and stable
metadata, while the kernel-owned sysfs mount remains a contract boundary for
ancestor traversal and race behavior. Successful kernel responses and node
metadata remain contracted observations rather than proofs of the kernel
implementation. In particular, the two firmware version integers can be bound
into later R1 evidence, but they do not identify firmware bytes or source,
authenticate loaded firmware, or prove which firmware executes on the device.
