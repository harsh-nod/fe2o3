# Runtime model verification

This directory contains the initial issue #137 Verus specifications. The
authenticated runner proves ten obligations over finite abstract traces. The
sequence lengths are not bounded by these proofs.

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
4. while a current generation is active, that generation or an older one
   cannot be reused as a fresh admission.

`device_projection_refinement_v1.rs` proves the pure boundary introduced for
the executable adapter:

1. the model projection retains every field represented in the formal
   canonical observation, including the literal V1 profile and UAPI-schema
   identities, initial wrapping VRAM-loss counter, and contracted reset-fence
   facts;
2. a canonical record satisfying the explicitly modeled V1 predicates projects
   to a model value satisfying the same exact profile/schema identities and
   contracted currentness facts;
3. the projection preserves the explicit 1-through-16-entry topology
   inventory, its pairwise physical/KFD/render/PCI identity uniqueness, and the
   unique selected-device match without replacing the inventory with an opaque
   hash;
4. appending a later generation preserves its exact predecessor link and the
   single-physical-device history invariant.

Run both proofs and all expected-negative mutations with the exact Verus
release whose executable, complete release closure, version, proof sources,
source checker, transcript, and mutations are pinned under `verus/pins`:

```sh
VERUS=/absolute/path/to/verus \
  crates/fe2o3-runtime-model/verus/verify-verus.sh
```

`scripts/ci-local.sh verus` invokes the same authenticated runner. The
`runtime-model-verus.yml` pull-request workflow downloads the named release and
then relies on this runner's executable and complete-closure pins before any
proof result is accepted.

The mutations must fail at their named postconditions: release while retained,
VM generation substitution, stale generation reuse, topology/render PCI
substitution, dropped DRM schema identity, lost history predecessor, mixed
cross-source identity, and a dropped final reset-fence observation. The launcher
rejects source substitution, lexically audits all proof files for trusted
constructs, clears the environment, bounds execution time, pins Z3 through the
authenticated Verus release closure, and rechecks the authenticated inputs after
verification.

The projection proof establishes the mathematical relation implemented by the
pure canonical-record mapping; it is not a proof that the executable Rust
implements that relation, nor that the adapter observed truthful kernel data.
The other files prove abstract transition relations, not refinement of
`src/model.rs` or `src/device_identity.rs`. All receipts remain model-only and
are not production device authority. A later sealed adapter refinement must
authenticate the KFD topology, DRM render, partition, schema, and process XNACK
observations, bind the dynamically allocated KFD device node to the opened file
descriptor and sysfs device, and connect concrete ioctl/sysfs results to the
canonical record. `DeviceGenerationV1` is a software admission
incarnation for stale-token rejection; topology correlation does not detect or
attest a GPU reset. The reset booleans and wrapping VRAM-loss value are retained
contracted observations only; these proofs do not establish an all-reset
generation, ABA freedom, or correctness of the KFD event stream. Firmware
execution, hardware completion, progress, liveness, coherency, performance, and
absence of kernel/firmware defects remain outside this proof boundary.
