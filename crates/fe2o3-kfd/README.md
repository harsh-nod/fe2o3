# fe2o3-kfd

Owned syscall adapters for the direct-KFD fe2o3 runtime. The initial slice is
deliberately limited to opening `/dev/kfd`, querying its UAPI version, and
producing checked admission evidence for the exact reviewed schema in
`fe2o3-kfd-uapi`.

The public safe API does not expose file descriptors, raw ioctl arguments, or
an unchecked way to construct a schema-admitted descriptor. Schema admission
is not device admission: this slice does not yet authenticate sysfs topology,
the matching render node, physical device/partition/target identity, kernel
boot/module identity, or their generations. Later KFD operations must require
a separate device-bound typestate and modeled resource ownership.

This crate checks userspace schema admission and encapsulates descriptor
ownership. Successful kernel responses and node metadata remain contracted
observations rather than proofs of the kernel implementation.
