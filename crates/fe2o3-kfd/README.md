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
exact correlated render descriptor plus a prospective KFD whole-GPU reset-event
descriptor, owns a process-global fe2o3 admission lease, requires KFD 1.18 and
AMDGPU DRM 3.64.0, compares the DRM identity prefix with topology/sysfs,
establishes a disabled-XNACK no-queue barrier, checks the complete bounded
process-aperture inventory, and repeats process, descriptor, topology, XNACK,
aperture, and reset-event observations before committing the token. The
`DEVICE_ADMISSION_PROFILE_MANIFEST_V1` digest binds the exact checked profile
and claim boundary. Retired model history is retained across admissions in the
same process and observation domain; a poisoned history fails closed. Each
successful admission also retains a solver-neutral `DeviceProjectionRecordV1`
covering platform, module-filesystem, and process provenance, both descriptors
and UAPI schemas, the selected topology/DRM profile fields, the explicit
bounded full-GPU identity inventory, firmware and selected capacity
observations, the complete process aperture inventory, and the final
reobservation fence. Projection history is
updated atomically with identity history and links each admission generation to
its exact predecessor. R1 deliberately retains, rather than compacts, at most
`MAX_MODEL_DEVICE_ADMISSIONS_V1` admissions per process/domain. The next bind
fails with `ProjectionHistoryExhausted`; restarting the process creates a new
history domain. This reviewed availability bound avoids silently discarding
substitution evidence. The
`kfd-device-identity` example performs this no-queue admission.

`check_observable_currentness(&mut self)` sandwiches a complete reobservation
between checks of the retained reset-event descriptor. Any event or error
permanently poisons later checks. It also compares the wrapping DRM
`VRAM_LOST_COUNTER`, but never treats that counter or KFD topology generation as
an all-reset generation. Under the pinned driver contract this detects
subscribed whole-GPU resets, VRAM-loss resets, and all changes visible through
the admitted identity, process, descriptor, XNACK, aperture, and topology
queries.

This crate checks userspace schema admission and encapsulates descriptor
ownership. Verus proves the pure canonical-record projection and abstract
generation/history relations. The executable validator checks the same record,
but there is not yet a Verus proof of the Rust implementation or a syscall-to-record
refinement proof, nor a
`ProductionDeviceAuthorityV1` implementation. No R1 API grants VM, allocation,
mapping, queue, event, code, or dispatch authority. It does not enumerate
cache, memory-bank, or link subtrees or prove their reported counts. The
process-global lease excludes other fe2o3 R1 admissions, not arbitrary raw KFD
users in the process. Ancestor traversal, mount-namespace integrity, sysfs
truth, cross-file snapshot semantics, KFD/DRM ioctl behavior, firmware meaning,
and absence of an ABA reset remain named external contracts. KFD does not expose
a sequence snapshot for the prospective subscription, does not report every
engine/per-queue reset through that stream, and creates its anonymous event fd
with an empty mask and without an atomic `CLOEXEC` option. A reset can therefore
occur between descriptor creation and mask enablement. The adapter sandwiches
that enablement between DRM identity/VRAM-counter observations, sets `CLOEXEC`
immediately, and never drains an observed event, but a VRAM-preserving reset in
the enablement gap can remain unobservable. It also cannot close the concurrent
fork/exec inheritance window or exclude interference from arbitrary raw KFD
users in the process. A retained-device, nonwrapping counter incremented for
every reset class plus an atomic create/mask/CLOEXEC operation, or an atomic
generation-snapshot/event handshake, is required for an all-reset currentness
proof. Successful kernel responses and node metadata are checked or Contracted
observations, not proof of the kernel or hardware implementation.

The separate `scripts/runtime-identity-oracle.sh` hardware lane compares the
`kfd-device-identity --all` evidence with bounded output from an isolated
`/opt/rocm/bin/rocminfo` subprocess. A match is recorded only as `Measured` with
`authority=none`; oracle output is never passed to this crate and cannot create
device, VM, memory, queue, dispatch, or proof authority. The exact comparison,
evidence schema, CI separation, and limitations are documented in
`docs/runtime-identity-oracle-v1.md`.
