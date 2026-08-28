# General Typed Dispatch V1

Status: living interface contract for the sole Worker V3 production route. The
compiler-to-HSACO slice and generic host lifecycle are implemented. The Worker
V3 verifier API is sealed and decision construction is private in production;
the concrete protected verification authority and end-to-end hardware replay
remain open.

This milestone connects the existing general scalar/slice ABI model and
packing plan to typed kernels selected from one authenticated Worker V3
executable. It does not make arbitrary artifact metadata authoritative.

At current head, exact vecadd is no longer a production macro or host-API
exception: ordinary `#[kernel(typed)]` emits its Worker V3 expectation and
`Arguments` surface. Historical Worker V2 host admission and workload-specific
launch APIs have been deleted, including from qualification builds.

Implementation through `dc9738e367c392f7716eacb8459ca73fa32abbbb` completes
the bounded alpha/zeta `gfx942` compiler-to-HSACO slice. It includes V3 lexical
registration, rustc-semantic reconstruction, authenticated logical/export role
selection, exact named ABI identity, guarded typed lowering, COV6
explicit/implicit ABI canonicalization, checked device-buffer views,
lifetime-branded packing, backend-witness emission and validation, and
the generic generated `Arguments`, `prepare`, and linear `dispatch` contract.
One genuine compiler build publishes both kernels in one inspected
COV6 HSACO. The exact exported payload then executes both kernels on MI300X for
lengths `1`, `255`, `256`, `257`, and `1023`, with independent CPU oracles and
canaries.

Two retired Worker V2 execution tests recorded raw and generated-safe hardware
observations for that digest. They are historical evidence, not selectable
routes or current production coverage.

The remaining composition requires the crate-owned concrete Worker V3
verifier. Default builds cannot implement the sealed trait or construct a
decision, and the synthetic hook exists only under the explicit integration
test feature. Cargo now
durably publishes the bounded
Worker V3 load-readiness envelope, recovers it from exact retained inputs, and
transfers the canonical envelope and artifact-directory descriptors to the application
while retaining a fresh current-publication lease. The accepted host consumer
revalidates that descriptor handoff and currentness before reaching exact
prerequisite admission. No production authenticator binds the bounded
Verus-facing proof-record and physical machine-effect evidence foundations to
compiler origin and the exact payload. The records grant no proof or launch
authority and establish no source-to-machine or Verus-to-machine refinement.
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
helper. Production completion still requires a production prerequisite
authenticator and the remaining proof-to-executable authority joins.

V1 does not accept standalone raw pointers, references not represented as an
approved slice profile, aggregates, enums, closures, return values, dynamic
LDS, asynchronous launch, or caller-constructed ABI descriptions.

## Authority Transitions

```text
macro-generated Rust expectation
        + independently admitted compiler descriptor
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

At `dc9738e`, the deterministic two-entry `ArtifactContainerV1` candidate and
retired raw and generated-safe HSA execution harnesses recorded a bounded
composition result. Those workload-specific adapters are no longer present.
The generic Worker V3 adapter validates compiler and host identities, checked
packing, alias registration, geometry, physical kernarg, and synchronous completion. The
Cargo artifact-container adapter remains inert and exposes no authority
accessor. Cargo's canonical envelope crosses the application boundary through
pinned descriptors while Cargo retains and revalidates its recovered lease.
The accepted host consumer composes that handoff with recovered admission and
fails at the missing genuine verification authority. The production API is
sealed against caller authority, but no concrete Worker V3 verifier currently
satisfies that authentication contract. Retired Worker V2
test authority is not an alternate route.

Cargo validates and pins the exact initial ELF64 x86-64 static image. One valid
PT_TLS owned by a writable, non-executable load is accepted; malformed,
executable-load-backed, and outside-load TLS is rejected. The launch profile
prevents fork/clone and re-exec after the controlled initial `execve`. This is
not containment of arbitrary same-process behavior: allowed `openat`, `mmap`,
`mprotect`, and `pwrite64` operations mean in-process loading and
self-modification are not prevented. Dynamic HIP runtime closure remains out
of scope.

## Frozen Interfaces

### Compiler-generated expectation

The macro supplies an independent bounded lexical expectation for the kernel's
logical ABI, physical components, effects, launch contract, kernel binding, and
generated host-contract identity. Lexical matching is not semantic authority:
production Worker V3 compares the marker binding to the independently admitted
compiler descriptor before verification, and compares the complete generated
argument layout to that descriptor before dispatch. The reviewed verifier must
bind the generated host-contract identity to the exact compiler handoff and
final executable lineage.

Rustc independently validates aliases and monomorphized layouts against
semantic primitive types and genuine trusted `DisjointSlice<T, Index1D>`
identities. The exact alpha/zeta role selector receives the already validated
logical and export names from collection; it does not infer identity from the
macro-generated host wrapper symbol. It assigns source field names only when
both names and the complete semantic signature match. Renamed roots,
logical/export disagreement, reordered arguments, or type lookalikes retain
positional `argN` names and produce a different host contract identity.

The compiler derives one bounded witness payload from each accepted V3
descriptor root. Production Worker V3 rejects marker/descriptor binding
substitution before verification and validates every generated argument field
against the admitted descriptor before dispatch. The alpha/zeta compiler
integration validates both linked witnesses and requires exactly both COV6
entries. The finalizer reconciles the descriptor's complete sizes (`296` for
alpha and `312` for zeta) with metadata's explicit prefixes (`40` and `56`),
while the LLVM worker
canonicalizes the optimized kernels to the complete 256-byte implicit block.
The witness authenticates the compiler expectation only. It does not grant
artifact currency, load, or launch authority.

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
inert. The generic Worker V3 contract emits the unsafe host-SPI implementation
and a safe `prepare` wrapper from the same ABI model used to derive the generated
host contract identity. Preparation packs
the exact `40`- or `56`-byte explicit prefix, registers allocation-relative
regions for alias and in-flight admission, derives 256-thread geometry, and
retains the values in a non-`Clone` prepared invocation through synchronous
completion. Existing packed bytes retain the selected kernel ID and carry
allocation-borrow lifetimes;
safe code cannot reuse or free a borrowed allocation while an input, view, or
packed value remains live. Safe launch code must additionally retain allocation
provenance, alias admission, and borrowed resources through completion.

The checked-view API now provides safe two-way and guarded three-way mutable
splits. The resulting views retain parent-allocation identity and exact,
non-overlapping allocation-relative intervals, with compile-fail coverage for
parent reuse and lifetime escape. A mechanical Verus proof of the split
implementation and general same-allocation hardware coverage remain open;
runtime interval admission continues to reject overlap.

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

Retired `hardware-test-hooks` harnesses executed on MI300X with the
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
results are historical exact-digest `gfx942` observations, not current
production proof-authenticated safe dispatch or parity evidence. The harnesses
and exact alpha/zeta host adapters have been deleted.

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

The replacement milestone passes only when:

1. one external Cargo command compiles two real Rust kernels with different
   scalar/slice signatures and one shared helper into one deterministic
   `gfx942` artifact;
2. the production Worker V3 verifier authenticates compiler, Verus/proof,
   Rust-layout, and machine-effect evidence for the exact payload;
3. generic generated contracts select both entries and derive packing plans
   from their own compiler-generated expectations;
4. both symbols resolve through one Worker V3 executable and cannot cross ABI,
   proof, or executable identities;
5. both kernels execute on MI300X with independent CPU oracles and canary
   checks through the production route; and
6. all rejection, crash-recovery, compile-fail, compiler, runtime, and hardware
   gates pass at one recorded commit.

Passing this gate advances rows 12, 33, 35-39, 48-49, and 78-81, but does not
by itself complete general Rust lowering, asynchronous execution, Verus
refinement, or repository-wide CUDA-Oxide parity.

## Ordered Completion Plan

1. Implemented: durable published-claim reacquisition validates the complete
   plan, receipt, exact files, current generation, and lock before issuing a new
   non-clone lease.
2. Implemented: publication-intent derivation is sealed behind the finalizer API
   with raw and finalized snapshots; Cargo's duplicate derivation is removed.
3. Implemented: Cargo durably publishes and reconstructs the canonical bounded
   Worker V3 load-readiness envelope containing the container, bundle/proof
   index, descriptor lineage, raw/finalized identities, and published claim.
4. Implemented production foundation: the V3 application handoff consumes that
   envelope with a freshly reacquired lease, transfers read-only pinned
   descriptors to an identity-pinned sealed application, and binds them to a
   fresh occurrence. The separate recovered Worker V2 host admission and launch
   bridge are deleted. The production verifier boundary is sealed and its
   decision constructor is private; a concrete crate-owned implementation and a
   generated-safe MI300X replay without the external HSACO test handoff remain
   open.
5. Implemented inert foundation: bounded physical machine-effect evidence and
   executable-evidence records bind reviewed mechanics and identities. Direct
   extraction of each final alpha/zeta entry's complete machine effects and
   production admission of that evidence remain open.
6. Implemented inert foundation: bounded alpha/zeta proof records bind declared
   proof inputs, tools, identities, and freshness. They grant no proof authority,
   are not compiler or machine-code refinement, and are not production-bound to
   the final payload.
7. Implement the concrete crate-owned `WorkerV3VerifierV1` behind the sealed
   boundary only after the compiler, Verus, proof-to-executable, Rust-layout,
   and machine-effect inputs are reviewed and immutable; reject mutation and
   stale replay.
8. Implemented API foundation: safe mutable splits retain exact disjoint
   allocation-relative regions with unit and compile-fail coverage. Mechanical
   Verus correspondence and general same-allocation MI300X execution remain.
9. Only then broaden signatures, control flow, AMD features, async behavior,
   and architecture coverage beyond the bounded `gfx942` profile.
