# Runtime model verification

This directory contains the initial issue #137 Verus specifications. The
authenticated runner proves six obligations over bounded abstract models.

`runtime_lifecycle_v1.rs` proves:

1. a retaining dispatch is bound to the exact VM, physical-device identity,
   and device generation carried by its referenced mapping; and
2. releasing a mapping preserves the runtime invariant when no prepared,
   published, or ambiguous dispatch retains that mapping.

`device_identity_generation_v1.rs` proves:

1. registering a fresh device generation preserves unique active generations;
2. registering a VM preserves its exact active device-generation binding;
3. an active VM cannot be substituted onto another generation of the same
   physical device; and
4. an active or stale generation cannot be reused as a fresh admission.

Run both proofs and all expected-negative mutations with the exact Verus
release whose executable, complete release closure, version, proof sources,
source checker, transcript, and mutations are pinned under `verus/pins`:

```sh
VERUS=/absolute/path/to/verus \
  crates/fe2o3-runtime-model/verus/verify-verus.sh
```

The mutations must fail at their named postconditions: release while retained,
VM generation substitution, stale generation reuse, and topology/render PCI
substitution. The launcher rejects source substitution, lexically audits all
proof files for trusted constructs, clears the environment, bounds execution
time, pins Z3 through the authenticated Verus release closure, and rechecks
the authenticated inputs after verification.

These are proofs of abstract transition relations. They are not refinement
proofs of `src/model.rs` or `src/device_identity.rs`, and the model-only
correlation receipt is not production device authority. A later sealed adapter
must authenticate the KFD topology, DRM render, partition, schema, and process
XNACK observations, bind the dynamically allocated KFD device node to the
opened file descriptor and sysfs device, and prove that concrete ioctl/sysfs
results refine this model. `DeviceGenerationV1` is a software admission
incarnation for stale-token rejection; topology correlation does not detect or
attest a GPU reset. Firmware execution, hardware completion, progress, liveness,
coherency, performance, and absence of kernel/firmware defects remain outside
this proof boundary.
