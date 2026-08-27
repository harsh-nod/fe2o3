# Source-to-simulator bundle V1

`fe2o3-export-sim` turns an ordinary admitted `#[kernel]` crate into one
bounded, content-addressed CPU-simulation input without requiring a GPU. It is
an explicit extraction command. It does not run the kernel and never falls
back from a failed hardware launch.

```text
fe2o3-export-sim \
  --crate my_kernel_crate \
  --output "$PWD/my-kernel.fe2sim" \
  --target gfx942 \
  -- --package my-kernel-package --lib
```

The command requires the repository's pinned nightly toolchain, its `rust-src`
component, and the AMDGPU Rust target. `--crate` is rustc's crate name, which
normally replaces package-name hyphens with underscores. Arguments after `--`
are Cargo package, feature, or target-kind selection; `--target` and
`--target-dir` are owned by the exporter and rejected there.

The command invokes Cargo `check` with `fe2o3-rustc-extract` as the workspace
wrapper. Dependencies pass through. The selected primary crate enters
extraction-only custody and shares the production source/MIR/KIR stages:

```text
admitted rustc collection
  -> semantic MIR owner
  -> ranked generic checks
  -> target-neutral KIR lowering
  -> exact verified KIR V7 projection
  -> simulation bundle
```

There is no second MIR importer or KIR lowerer. Extraction consumes the
transaction immediately after target-neutral KIR verification, before formal
memory admission, target LLVM lowering, protected compiler-module publication,
linking, loading, or launch. It is not a protected publication occurrence. The
output is created new with mode `0600`; an existing path, including a symlink,
is rejected.

## Simulate and debug the bundle

Prepare the same strict request document used by the raw KIR simulator, then
consume the bundle directly:

```text
fe2o3-kir-sim \
  --bundle "$PWD/my-kernel.fe2sim" \
  --request "$PWD/request.json"

fe2o3-debug sim \
  --bundle "$PWD/my-kernel.fe2sim" \
  --request "$PWD/request.json" \
  --protocol jsonl
```

Both commands use the existing hardened regular-file boundary, reject
oversize/change/substitution, strictly decode and revalidate the complete
bundle, map its exact target, and admit only its embedded verified KIR V7. They
do not recompile, re-lower, load, launch, or fall back to a different route.
`--bundle` is mutually exclusive with raw `--kir-v7`.

When a future compiler bundle contains its bounded debug map, `fe2o3-debug`
passes the exact payload, verified simulation-bundle subject, and committed map
identity through the compiler-bundle admission API. Source locations are then
labeled `compiler_bundle_bound`, which means exact content association, not
protected compiler-execution authentication. The map wire contains only its
compile-time bundle subject and KIR identity. Request and logical wave width
remain runtime inputs; their configuration identity is derived internally and
rechecked before the map is bound. Bundle session identities also commit the
verified bundle subject and therefore its exact target. The current exporter
still emits no map, so source inspection is typed unavailable for its bundles.

## Exact identities and bounds

Bundle V1 is a strict binary wire with a hard maximum of the 16 MiB KIR bound,
the 4 MiB debug-map bound, an optional exact 690-byte inert compiler-execution
subject association, and a small fixed header/target allowance. Its simulation
bundle subject identity commits:

- an explicit compiler-execution binding state;
- the exact domain-separated V3 inert receipt identities and preimage lengths
  for rustc's compiler-owned identity-inventory and preflight-plan transcripts;
- the production canonical KIR V8 identity and length;
- the exact verified canonical KIR V7 identity and length;
- the `gfx942:xnack-` or `gfx950:xnack-` target admitted by the live rustc
  session;
- a deterministic projection of every kernel name, entry, launch domain,
  workgroup size, parameter type, and result type.

The extraction command always emits
`SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly`. The other
wire variant retains a claimed subject identity and all 690 claimed canonical
bytes. It is intentionally an unverified association: bundle decoding commits
those bytes but does not validate the nested subject. A higher owner must
strictly decode `InertCompilerExecutionSubjectV1`, independently reconstruct
the subject from an already-retained or consumed exact strict V3 handoff, and
cross-check the source receipts, KIR, target, and transaction before promotion.
That same move-only production join is not implemented by this extraction
path.

The complete bundle identity additionally commits the optional debug-map
bytes. Decoding re-verifies canonical V7, independently re-encodes V8 from the
same semantics, rederives the kernel ABI and both identities, and rejects
truncation, trailing bytes, unknown flags, zero identities, and substitution.
`VerifiedSimulationBundleV1::canonical_kir_v7()` exposes the exact bytes used
by direct `--bundle` admission in `fe2o3-kir-sim` and `fe2o3-debug`.

A valid bundle proves exact local bundle content. It does not authenticate
compiler execution, source authorship, or provenance and grants no proof,
artifact, compiler, load, hardware, or launch authority. The canonical subject
and current signature protocol alone authenticate nothing; a signature-verified
receipt under that protocol authenticates only the policy-pinned signing key,
not protected compiler execution. That property remains false until the
protected issuer and durable Worker V3 consumer join are implemented.

## Version and debug-map boundary

Production currently retains canonical KIR V8 for ordinary kernels and V9 for
the gfx950 attention surface. Simulator bundle V1 projects the already-lowered
V8 module through the frozen V7 encoder. If any semantic field cannot round
trip through V7, export fails. V9 is rejected with a typed diagnostic; it is
never silently downgraded.

The bundle reserves a separately bounded `fe2o3-debug-source-map-v1` payload
and exposes a non-circular simulation bundle subject identity for that map to
bind. This command emits `debug map none`: compiler-owned source-span
projection is not yet wired into the transaction. A caller-supplied map is an
unverified association and cannot acquire compiler provenance by merely naming
a KIR digest or bundle identity.

## Current UX boundary

This first command is the production extraction driver packaged as a direct
binary. It is not a `cargo fe2o3 simulate` route: that deleted route selected a
qualification oracle and performed a second MIR-to-KIR lowering. A future
`cargo fe2o3 export-sim` spelling may wrap this command, but must preserve the
same selector-free transaction and authority boundary.
