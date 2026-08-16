# Bounded MoE V1 evidence

Implementation checkpoint: `1281f97487adfd4af32687b7705ba46e5c11152b`.

This checkpoint adds three narrow pieces around the fixed `T8/E4/K2/C4`
top-2 router and expert schedule. They are deliberately separate evidence
classes:

1. a private same-session rustc structural diagnostic for the router source;
2. an inert Verus proof of one exact expert compact-plan arithmetic model; and
3. a host-observed routing-to-expert consistency and upload bridge.

They do not compose into source-to-machine refinement, authenticated router
provenance, artifact authority, dispatch authority, or expert GPU execution.
The CUDA-Oxide parity status remains `0 Complete / 97 Partial / 0 Missing / 12
N/A` in aggregate.

## Private rustc structural record

The `collected-moe-top2-v1` rustc admission retains the source text already
loaded in rustc's `SourceFile`; it does not reopen the source pathname. After
the parent admission checks source identity, compiler semantics, trusted
definitions, the exact root instance, the rustc-derived `FnAbi`, the complete
imported portable-MIR module, and the selected KIR/profile, it constructs an
opaque `ValidatedMoeTop2AuthorityV1`. The private structural producer consumes
that token by value together with an opaque same-session input witness.

The checked `FnAbi` identity commits the Rust calling-convention discriminator,
variadic flag, fixed and actual argument counts, unwind flag, ignored-return
mode discriminator, return size and ABI alignment, and every argument's layout
size/alignment, pair-mode discriminator, and both components' checked regular
attributes, extension, pointee size, and optional pointee alignment. The
readable diagnostic projection is intentionally smaller than that complete
identity: it shows the checked header, result, and eight pair-mode arguments.

The portable-MIR diagnostic is computed over the full imported module. It binds
the portable semantic identity and records counts for functions, roots,
helpers, blocks, statements, terminators, CFG edges, external imports, root
arguments and locals, assignments, calls, indexed places, repeated values, and
observed binary operators. These are whole-module diagnostics, not a proof that
MIR values or effects simulate the KIR routing state machine.

### Canonical KIR/profile table

The private classifier encodes all current `MoeTop2KernelIrV1` and
`MoeTop2ProfileV1` fields, including nested arrays, descriptors, resources, and
policy records, in these 31 ordered aggregate entries. `KIR`, `Profile`, `ABI`,
`Effects`, and `Routing` name the domain-separated projections that include an
entry. An aggregate entry can encode several leaf fields.

| # | Canonical entry | Projections | Canonical content |
|--:|:--|:--|:--|
| 1 | `kir.module-id` | KIR | module identifier |
| 2 | `kir.function-id` | KIR | function identifier |
| 3 | `kir.kernel-id` | KIR, ABI | kernel identifier |
| 4 | `kir.arguments` | KIR, ABI, Effects | ordered role, shape, scalar, offset, size, and alignment records |
| 5 | `kir.shape` | KIR, Routing | tokens, experts, experts per token, capacity, logits, and routes |
| 6 | `kir.layout` | KIR, Routing | layout discriminator |
| 7 | `kir.finite-input` | KIR, Effects, Routing | finite-input policy discriminator |
| 8 | `kir.tie-break` | KIR, Routing | tie-break discriminator |
| 9 | `kir.overflow` | KIR, Routing | overflow policy discriminator |
| 10 | `kir.routing-steps` | KIR, Routing | ordered routing-step discriminators |
| 11 | `kir.packing` | KIR, Routing | all packing predicates and drop sentinel |
| 12 | `kir.ownership` | KIR, Effects | ownership policy, lanes, output lengths, and write predicates |
| 13 | `profile.source-sha256` | Profile | exact source digest |
| 14 | `profile.namespace` | Profile | source namespace |
| 15 | `profile.target` | Profile | target-capability encoding |
| 16 | `profile.code-object-version` | Profile | profile code-object version |
| 17 | `profile.wave-width` | Profile | wave width in lanes |
| 18 | `profile.workgroup-size` | Profile | required workgroup dimensions |
| 19 | `profile.grid` | Profile | grid dimensions |
| 20 | `profile.correspondence` | Profile | correspondence discriminator |
| 21 | `profile.descriptor.logical-name` | Profile, ABI | logical kernel name |
| 22 | `profile.descriptor.export-name` | Profile, ABI | exported kernel name |
| 23 | `profile.descriptor.symbol` | Profile, ABI | descriptor symbol |
| 24 | `profile.descriptor.code-object-version` | Profile, ABI | descriptor code-object version |
| 25 | `profile.descriptor.explicit-kernarg-bytes` | Profile, ABI | explicit kernarg size |
| 26 | `profile.descriptor.complete-kernarg-bytes` | Profile, ABI | complete kernarg size |
| 27 | `profile.descriptor.workgroup-size` | Profile, ABI | descriptor workgroup dimensions |
| 28 | `profile.descriptor.wave-width` | Profile, ABI | descriptor wave width |
| 29 | `profile.descriptor.static-lds-bytes` | Profile | static LDS size |
| 30 | `profile.descriptor.required-dynamic-lds-bytes` | Profile | required dynamic LDS size |
| 31 | `profile.descriptor.maximum-dynamic-lds-bytes` | Profile | maximum dynamic LDS size |

The classifier rejects name, order, removal, duplication, membership, and value
drift. The final record also binds the rustc-loaded source, complete checked
`FnAbi` identity and projection, full imported-MIR diagnostic, compiler
semantics, trusted definitions, root instance, and completed source authority
from the same admission. The record remains private, diagnostic, and inert. It
is not MIR-to-KIR semantic refinement and grants no Worker V2, LLVM, artifact,
load, launch, runtime, GPU, memory-safety, or race-freedom authority.

## Exact compact-plan proof

The compact-plan proof covers only `E4/C4/routes16/width16/tile256`. Given
zero-based monotone expert offsets whose four counts are each at most four, it
proves source ranges remain inside their expert tiles, destination ranges remain
inside the compact tile, nonempty destination ranges are pairwise disjoint and
ordered, their union is exactly the accepted prefix, and every unused tail
element is zero.

The pinned runner requires `19` verified Verus obligations and rejects exactly
seven named negative mutations. A Rust differential test checks every one of
the `5^4 = 625` valid expert-count vectors. The expected proof values are
copyable inert pins. They are not joined to the rustc structural record, host
code, runtime copies, machine addresses, an authenticated proof receipt, or a
GPU artifact.

## Host-observed routing bridge

`MoeRoutingOutputCandidateV1` is caller-supplied data. Conditioned on its top-2
expert IDs, the checker validates the internally consistent fixed relation
across top-2 IDs, requested and admitted counts, exclusive-scan offsets, stable
route slots, accepted permutation, inverse map, sentinels, and the compact plan.
It returns an opaque non-`Clone` witness, but this is not freshness or replay
protection: a caller can construct and check an equivalent candidate again.

The upload function consumes that witness and synchronously uploads its offsets
and inverse together. The returned bridge retains immutable views of both exact
device regions, and expert preparation can no longer splice an unrelated
offset array and inverse view. The opt-in `gfx942` test uploads those
caller-supplied arrays and reads the two destinations back. It does not run the
router or read back router-produced output.

The bridge has no authenticated router-completion provenance and does not
validate top-2 IDs against logits or a tie policy. It does not bind route
weights, packed activations, a router artifact, dispatch, or expert GPU
execution. Its payload digest excludes context, stream, allocation, and region
identities; those are retained as separate observations.

## Manually pinned expert ABI and denial

The expert host adapter retains exactly eight typed regions:

| Role | Element type and count | Access |
|:--|:--|:--|
| activation tiles | `u16[1024]` | shared read-only |
| expert weights | `u16[1024]` | shared read-only |
| expert offsets | `u32[5]` | bridge-retained read-only |
| inverse routing | `u32[16]` | bridge-retained read-only |
| route weights | `f32[16]` | shared read-only |
| expert output tiles | `f32[1024]` | unique read-write |
| compact output | `f32[256]` | unique read-write |
| combined output | `f32[128]` | unique read-write |

The reviewed constants manually pin `gfx942:xnack-`, four GEMM dispatches with
grid/workgroup `[1,1,1]/[64,1,1]`, and one combine dispatch with
`[2,1,1]/[64,1,1]`. GEMM explicit/complete kernarg sizes are `48/304` bytes;
combine sizes are `64/320` bytes; alignment is eight bytes. These facts are not
compiler-derived, and no packed kernarg or device address is exposed. The
expert ABI remains manually pinned, not compiler-derived.

Preparation terminates in `deny_moe_expert_execution_v1`. The denial token
retains all borrows and exposes no artifact, copy, load, dispatch, completion,
or unload operation. The missing authority-bearing path must derive the expert
ABI from authenticated compiler/finalizer output, authenticate router
completion and readback, bind route weights and packed activations, execute the
compact materialization plan, and then establish expert artifact and runtime
authority.

## Typed MoE V2 fail-closed boundary

The implementation through
`10e5f90ece1937aaee77492e8e4e4742863d013b` adds a production-shaped typed
boundary without making the expert path executable. Its exact request/batch
identity commits the routing request, logits source, token activations, caller
route-weight policy, and model expert-weight artifact. Its lifecycle transcript
separately commits dispatch and readback context/stream identity, dispatch,
completion and readback event identities, the completion-before-readback order,
the fixed profile, the complete routing payload, and the shared request/batch.
The typed identity encoding is process-local and pinned to the current Rust
toolchain; it is not a durable cross-toolchain serialization or semantic proof.

The checked-input join consumes completed readback and validates the concrete
route weights, token-activation identity, and exact zero-padded packed activation
layout. The completed upload requires the lifecycle's exact context and stream,
checks all four destination lengths and allocation identities, rejects every
alias pair, and retains typed activation, offsets, inverse, and route-weight
regions together. The generated adapter additionally requires a weight-device-
region binding to the model artifact named by the same request/batch, validates
all eight region lengths, ranges, alignment, contexts, access roles, and alias
pairs, and constructs only the fixed four-GEMM/one-combine ABI records.

Every capability that crosses a lifecycle stage has private fields and is
move-only. The UI suite rejects direct construction, field access, cloning,
reuse after move, synthetic conversion, V1 substitution, raw-weight
substitution, public test-issuer access, and attempts to extract authority.
There is no public or feature-gated production issuer for completion/readback
provenance, and the artifact pipeline cannot issue the required expert-weight
binding. V2 upload and adapter preparation are therefore constructively
unreachable from safe production code.

V2 grants no artifact, copy, load, or dispatch authority and proves no routing
or expert semantics, memory safety, race freedom, numerical correctness, or
source-to-machine refinement. The V1 `gfx942` test described above observed only
caller-supplied offsets/inverse upload and readback through V1. It did not use
V2, and there is no V2 GPU observation or parity promotion.

## Reproduce the bounded checks

Run from the repository root with the pinned Rust toolchain:

```sh
python3 scripts/test-bounded-moe-docs.py
cargo test --locked -p rustc-codegen-fe2o3 --test moe_top2_v1
cargo test --locked -p fe2o3-verifier --test moe_expert_compact_plan_v1
VERUS=/absolute/path/to/pinned/verus \
  ./scripts/test-moe-expert-compact-plan-verus.sh
cargo test --locked -p fe2o3-host --lib moe_routing_expert_bridge_v1::tests
cargo test --locked -p fe2o3-host --test generated_moe_expert_v1_ui
cargo test --locked -p fe2o3-host --lib moe_routing_expert_bridge_v2::tests
cargo test --locked -p fe2o3-host --lib generated_moe_expert_v2::tests
cargo test --locked -p fe2o3-host --test generated_moe_expert_v2_ui
cargo test --locked -p fe2o3-host \
  --test moe_expert_v1_upload_hardware --no-run
cargo test --locked --manifest-path examples/moe_expert_v1/Cargo.toml
```

The opt-in hardware observation requires a `gfx942:xnack-` HIP device:

```sh
cargo test --locked -p fe2o3-host \
  --test moe_expert_v1_upload_hardware \
  gfx942_routing_bridge_upload_readback_and_denial_are_exact \
  -- --ignored --exact --nocapture
```

Passing the last command proves only the V1 caller-supplied offsets/inverse
upload-readback and denial behavior described above. It supplies no V2 hardware
evidence.
