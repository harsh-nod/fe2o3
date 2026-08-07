# General Typed Dispatch V1

Status: interface contract for the next CUDA-Oxide parity vertical slice.

This milestone connects the existing general scalar/slice ABI model and
packing plan to a second typed kernel selected from one authenticated Worker V2
executable. It does not make arbitrary artifact metadata authoritative and it
does not replace the existing exact vecadd profile.

Implementation checkpoint `ceb0e4675173866a50fb737108e6a9b04827691d` has the
V3 lexical registration, rustc-semantic reconstruction, variable descriptor
generation, lifetime-branded host packing, and backend-witness consumer
contract. It does not have the rustc backend witness host-object emitter,
generated alpha/zeta wrappers, a production two-entry container, or alpha/zeta
load, dispatch, and hardware execution. The exit gate therefore remains open.

## Scope

V1 accepts compiler-generated kernel entries whose logical arguments are:

- by-value scalar integers and floating-point values already represented by
  the artifact ABI;
- shared slices in global memory; and
- exclusive `DisjointSlice` values with read-only, write-only, or read-write
  effects.

The first executable fixture contains two kernels with different non-empty
signatures and one shared internal helper. Both kernels reside in one `gfx942`
code object and are selected, packed, resolved, and dispatched independently.

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
identities. At `ceb0e46`, that semantic reconstruction and the witness consumer
contract are implemented, but rustc does not yet emit the witness accessors.
Consequently a general V3 application that attempts to use semantic authority
fails at final link rather than falling back to lexical trust. The exact vecadd
V2 profile remains byte-compatible and does not use this unfinished V3 path.

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
checks element type, layout, access, and address space. Packed bytes remain
inert, retain the selected kernel ID, and carry allocation-borrow lifetimes;
safe code cannot reuse or free a borrowed allocation while an input or packed
value remains live. Safe launch code must additionally retain allocation
provenance, alias admission, and borrowed resources through completion.

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
