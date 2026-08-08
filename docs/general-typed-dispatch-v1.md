# General Typed Dispatch V1

Status: interface contract for the next CUDA-Oxide parity vertical slice.

This milestone connects the existing general scalar/slice ABI model and
packing plan to a second typed kernel selected from one authenticated Worker V2
executable. It does not make arbitrary artifact metadata authoritative and it
does not replace the existing exact vecadd profile.

Implementation through `dc9738e367c392f7716eacb8459ca73fa32abbbb` completes
the bounded alpha/zeta `gfx942` compiler-to-HSACO slice. It includes V3 lexical
registration, rustc-semantic reconstruction, authenticated logical/export role
selection, exact named ABI identity, guarded typed lowering, COV6
explicit/implicit ABI canonicalization, checked device-buffer views,
lifetime-branded packing, backend-witness emission and validation, and
signature-specific generated `Arguments`, `prepare`, and linear `dispatch`
adapters. One genuine Worker V2 build publishes both kernels in one inspected
COV6 HSACO. The exact exported payload then executes both kernels on MI300X for
lengths `1`, `255`, `256`, `257`, and `1023`, with independent CPU oracles and
canaries.

Two execution tests now exist. One deliberately calls the reviewed raw unsafe
HSA adapter. The other exercises generated slice capabilities, typed
alpha/zeta selection and preparation, the reviewed executable lifecycle, and
safe `dispatch`, but uses test-only semantic witnesses and an explicitly fake
prerequisite authenticator. Both accept the same externally supplied
SHA-256-pinned HSACO.

The remaining production composition is larger than enabling the Cargo adapter
and implementing `WorkerV2PrerequisiteAuthenticatorV1`. Cargo drops the live
currentness lease, but the canonical published claim can now reacquire a fresh
lease after durable revalidation. A bounded Worker V2 load envelope retains the
container, bundle/proof evidence, descriptor lineage, raw/finalized identities,
and that canonical claim. Cargo does not yet publish the envelope, host
admission accepts only live in-process preparation/publication objects, and the
application runner receives no pinned bundle descriptor. No production authenticator,
Verus proof, or machine-code effect/refinement evidence is bound to the payload.
The production-safe exit gate therefore remains open, this result is not a
CUDA-Oxide parity claim, and Complete remains `0`.

## Scope

V1 accepts compiler-generated kernel entries whose logical arguments are:

- by-value scalar integers and floating-point values already represented by
  the artifact ABI;
- shared slices in global memory; and
- exclusive `DisjointSlice` values with read-only, write-only, or read-write
  effects.

The bounded alpha/zeta executable contains two kernels with different non-empty
signatures in one `gfx942` code object and selects, packs, resolves, and
dispatches them independently through both the raw and generated-safe hardware
paths. The full G3.1 exit fixture additionally requires a shared internal
helper. Production completion also requires Cargo envelope publication,
application handoff, recovered host admission, and a production prerequisite
authenticator.

V1 does not accept standalone raw pointers, references not represented as an
approved slice profile, aggregates, enums, closures, return values, dynamic
LDS, asynchronous launch, or caller-constructed ABI descriptions.

## Authority Transitions

```text
macro-generated Rust expectation
        + backend-issued semantic witness
        + authenticated artifact entry
        + exact finalized executable occurrence
        + observed context/device
        + reviewed compiler/proof prerequisites
                         |
                         v
manifest-validated packing plan
                         |
typed values + retained allocation/alias witnesses
                         |
                         v
kernel-bound explicit kernarg bytes
                         |
exact HSA executable + resolved symbol + physical ABI observation
                         |
prepared geometry + initialized COV6 hidden span
                         |
                         v
synchronous dispatch and completion
```

No descriptive manifest, proof record, HSA observation, or packed byte buffer
grants authority by itself. Every transition consumes or borrows the exact
upstream evidence that it validates.

At `dc9738e`, the deterministic two-entry `ArtifactContainerV1` candidate,
exact generated alpha/zeta adapter, and raw and generated-safe HSA execution
harnesses are landed.
The adapter validates compiler and host identities, checked packing, alias
registration, geometry, physical kernarg, and synchronous completion. The
Cargo artifact-container adapter is deliberately inert, exposes no container or
serialization accessor, and is compiled only for tests. Separately, production
host APIs already admit finalized bundles from durable publication, retain and
revalidate currentness, authenticate load prerequisites, and drive the reviewed
runtime adapter. No production `WorkerV2PrerequisiteAuthenticatorV1` currently
satisfies the unsafe authentication contract. The Cargo candidate also cannot
cross the compiler/application boundary: the separate durable envelope and
lease-reacquisition foundations are not connected to Cargo or recovered host
admission, and the runner passes no pinned descriptor. The raw harness bypasses
these gaps. The generated-safe harness exercises the state machine but
substitutes explicit test authority at the missing authenticator boundary.

## Frozen Interfaces

### Compiler-generated expectation

The macro supplies an independent bounded lexical expectation for the kernel's
logical ABI, physical components, effects, launch contract, kernel binding, and
generated host-contract identity. Lexical matching is not semantic authority:
the expectation must obtain a versioned, identity-bound witness from private
backend-defined pointer/length accessors. Host validation rejects missing,
malformed, substituted, wrong-profile, wrong-binding, wrong-contract, and
trailing witness bytes before artifact admission can use the expectation.

Rustc independently validates aliases and monomorphized layouts against
semantic primitive types and genuine trusted `DisjointSlice<T, Index1D>`
identities. The exact alpha/zeta role selector receives the already validated
logical and export names from collection; it does not infer identity from the
macro-generated host wrapper symbol. It assigns source field names only when
both names and the complete semantic signature match. Renamed roots,
logical/export disagreement, reordered arguments, or type lookalikes retain
positional `argN` names and produce a different host contract identity.

Worker V2 derives one bounded witness payload from each accepted V3 descriptor
root, emits private binding-derived pointer/length accessors in host objects,
and adds those objects to the host link. The alpha/zeta integration validates
both linked witnesses and requires exactly both COV6 entries. The finalizer
reconciles the descriptor's complete sizes (`296` for alpha and `312` for zeta)
with metadata's explicit prefixes (`40` and `56`), while the LLVM worker
canonicalizes the optimized kernels to the complete 256-byte implicit block.
The witness authenticates the compiler expectation only. It does not grant
artifact currency, load, or launch authority. The exact vecadd V2 profile
remains byte-compatible.

### Artifact and bundle binding

One finalized payload occurrence may contain several kernel entries. Each
typed selection binds all of the following:

- bundle and container identity;
- finalized payload digest and target;
- kernel ID, logical name, export symbol, and descriptor symbol;
- ABI, effects, launch contract, and source identity; and
- the proof record admitted for that exact kernel.

Selecting another kernel never reuses the first kernel's ABI, effects, proof,
packing plan, HSA symbol, or launch authority.

### Argument packing

The existing `GeneratedArgumentPackingPlanV1` remains the only V1 packing
engine. Generated adapters bind values by source argument index. The plan
rejects missing, duplicate, reordered, wrong-kind, wrong-width, wrong-access,
wrong-address-space, wrong-pointer-width, and cross-kernel values.

General V3 scalar binding accepts only canonical `i8`/`u8` through
`i64`/`u64`, `f32`, and `f64` identities. Shared and exclusive slice binding
checks element type, layout, access, and address space. Checked shared and
exclusive `DeviceBuffer` views retain allocation identity and selected-region
provenance while rejecting invalid ranges and borrow violations. The macro emits
an opaque signature-specific `Arguments` holder whose fields are typed scalars
and source-index-bound slice capabilities. General lookalike holders remain
inert. For the exact authenticated alpha/zeta roles, the macro also emits the
unsafe host-SPI implementation and a safe `prepare` wrapper from the same ABI
model used to derive the generated host contract identity. Preparation packs
the exact `40`- or `56`-byte explicit prefix, registers allocation-relative
regions for alias and in-flight admission, derives 256-thread geometry, and
retains the values in a non-`Clone` prepared invocation through synchronous
completion. Existing packed bytes retain the selected kernel ID and carry
allocation-borrow lifetimes;
safe code cannot reuse or free a borrowed allocation while an input, view, or
packed value remains live. Safe launch code must additionally retain allocation
provenance, alias admission, and borrowed resources through completion.

The current checked-view API cannot yet form two simultaneous mutable subviews
of one allocation through a safe split operation. That is a remaining API and
proof obligation even though runtime interval admission rejects overlapping
regions supplied through existing trusted paths.

### HSA resolution and dispatch

A selected kernel is resolved against the already loaded executable through
the reviewed adapter. Resolution validates the exact executable object, export
symbol, kernel object identity, kernarg segment size/alignment, and physical
metadata before producing an opaque non-`Clone` token that borrows the loaded
executable.

The reviewed COV6 adapter accepts a bounded variable explicit prefix followed
by the exact 256-byte hidden span. It preserves every explicit byte, initializes
the complete hidden span from the exact geometry and queue, and rejects offset,
size, alignment, kernel, executable, queue, or geometry substitution.

V1 dispatch is synchronous. The resolved kernel, prepared arguments, geometry,
allocation witnesses, and queue resources remain live until exact completion.

The ignored `hardware-test-hooks` harnesses have executed on MI300X with the
Worker V2-produced COV6 HSACO whose SHA-256 is
`3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4`.
Each loaded one executable, resolved both symbols, packed the frozen layouts,
initialized the COV6 hidden span, and ran lengths `1`, `255`, `256`, `257`, and
`1023`; both independent CPU oracles and all canaries passed. The raw harness
calls the reviewed unsafe HSA adapter directly. At commit
`dc9738e367c392f7716eacb8459ca73fa32abbbb`, the generated-safe harness passed
the same matrix using checked generated slice capabilities, typed preparation,
safe dispatch, and one reviewed loaded executable. It still uses test-only
semantic witnesses and an explicitly fake prerequisite authenticator. The
results are exact-digest `gfx942` hardware and runtime-composition evidence,
not production proof-authenticated safe dispatch or parity evidence.

## Required Rejection Tests

- generated expectation versus manifest ABI, effect, launch, or identity
  mismatch;
- kernel name, symbol, descriptor, payload, target, or proof substitution;
- argument omission, duplication, reorder, kind, width, access, address-space,
  pointer-width, and cross-kernel substitution;
- HSA executable, symbol, kernel object, kernarg size/alignment, hidden offset,
  and queue substitution;
- unload while a selected or resolved kernel is live;
- mutable/shared and mutable/mutable overlap across one launch and in-flight
  launches; and
- stale source, proof, bundle, publication, or runtime observation identity.

## Exit Gate

The milestone passes only when:

1. one external Cargo command compiles two real Rust kernels with different
   scalar/slice signatures and one shared helper into one deterministic
   `gfx942` Worker V2 artifact;
2. independent typed generated adapters select both entries and derive packing
   plans from their own compiler-generated expectations;
3. both symbols resolve from one loaded HSA executable and cannot cross ABI,
   proof, or executable identities;
4. both kernels execute on MI300X with independent CPU oracles and canary
   checks;
5. source/unit, compile-fail, malformed-input, native-worker, and hardware
   tests pass at one recorded commit; and
6. the parity dashboard records the evidence without promoting a row beyond
   its CUDA-Oxide acceptance contract.

Passing this gate advances rows 12, 33, 35-39, 48-49, and 78-81, but does not
by itself complete general Rust lowering, asynchronous execution, Verus
refinement, or repository-wide CUDA-Oxide parity.

## Ordered Completion Plan

1. Implemented: durable published-claim reacquisition validates the complete
   plan, receipt, exact files, current generation, and lock before issuing a new
   non-clone lease.
2. Implemented: publication-intent derivation is sealed behind the finalizer API
   with raw and finalized snapshots; Cargo's duplicate derivation is removed.
3. Implemented schema: a canonical bounded Worker V2 load envelope contains the
   container, bundle/proof index, descriptor lineage, raw/finalized identities,
   and canonical published claim. Cargo must now publish it durably.
4. Add recovered host admission from a decoded envelope plus a freshly
   reacquired lease, and hand a read-only pinned descriptor to the application.
   Re-run the generated-safe MI300X matrix without an external-HSACO handoff;
   retain the explicit fake-authenticator label until the next gates pass.
5. Produce bounded machine-code effect evidence for each final alpha/zeta entry
   and its closed call graph. Bind accepted global loads/stores and address
   derivations to the descriptor effects, analyzer/toolchain identity, kernel
   identity, and exact payload digest; reject unsupported or expanded effects.
6. Prove alpha/zeta bounds, overflow freedom, injective writes/race freedom,
   and functional postconditions in Verus. Bind proof/tool identities and
   results to the source contract, launch contract, machine-code evidence, and
   finalized artifact; reject mutations and replay.
7. Implement a production `WorkerV2PrerequisiteAuthenticatorV1` only after the
   compiler, Verus, proof-to-executable, Rust-layout, and machine-effect inputs
   are reviewed and immutable; reject mutation and stale replay.
8. Add safe split mutable views over one allocation with exact disjoint-region
   witnesses, compile-fail lifetime/overlap coverage, runtime alias admission,
   and MI300X execution.
9. Only then broaden signatures, control flow, AMD features, async behavior,
   and architecture coverage beyond the bounded `gfx942` profile.
