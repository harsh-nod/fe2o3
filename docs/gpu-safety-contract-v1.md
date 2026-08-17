# fe2o3 GPU Safety Contract v1

Status: normative target contract for new safety-sensitive implementation.

This document freezes the interfaces shared by the device API, kernel IR,
static analyses, Verus model, artifact binder, and host runtime. It refines the
[verification model](verification-model.md); that document defines the proof
obligations, while this one defines the representations and authority rules
used to implement them.

Bounded profile-specific code implements portions of this contract, including
generated argument preparation, artifact admission, linear runtime states, and
proof-record schemas. No general path satisfies the complete contract or turns
source proof into machine-code authority. Compatibility APIs remain available
only through explicit unsafe boundaries until the generated path described here
covers them.

## Authority Rule

No single layer may declare a launch safe or a kernel verified.

- Rust types prevent local API misuse and carry launch-scoped capabilities.
- Kernel IR records executable effects and rejects unsupported or unproved
  memory and convergence behavior.
- Verus proves source-level properties in the versioned abstract GPU model.
- Artifact binding checks that source, executable, contracts, proof evidence,
  target, ABI, payload, and generated Rust types identify the same kernel.
- The runtime validates dynamic launch, context, allocation, and lifetime facts.

A missing, unknown, or mismatched fact fails closed. Tests, sanitizers, compiler
attributes, self-reported manifests, and proof records are evidence, but none
of them independently creates safe-launch or `Verified` authority.

## Launch Brand and Scope

Every prepared launch creates an unforgeable type-level kernel brand `K` and a
borrow-scoped launch identity `'launch`. Conceptually, generated code exposes:

```rust,ignore
LaunchContext<'launch, K, Domain>
ThreadIndex<'launch, IndexSpace>
Uniform<'launch, Scope, T>
DisjointSlice<'buffer, 'launch, T, IndexSpace>
```

The exact stored fields are private. Safe constructors are generated or
crate-sealed and require loaded-kernel authority that is already bound to the
artifact, context, and prepared launch. Raw launches may construct equivalent
values only inside an unsafe boundary that states all missing obligations.

`K` prevents mixing kernels. `'launch` prevents retaining a witness for another
launch. `IndexSpace` prevents using a valid index with the wrong mapping.
`Domain` records rank, geometry, and participating execution scopes.

## Thread Witnesses

`ThreadIndex<'launch, IndexSpace>` is opaque and is not `Copy`, `Clone`, `Send`,
or `Sync`. Its executable representation may be an integer, but its semantic
value includes:

- the launch identity and current dynamic thread;
- the source coordinate and logical index;
- the index-space mapping and required geometry assumptions;
- the execution epoch in which unique write authority is valid.

Moving a witness within one invocation is permitted. Duplicating it,
constructing it from an arbitrary integer, transferring it between threads, or
changing its index space is not a safe operation. Arithmetic produces a plain
index or a separately proved mapping witness; it does not preserve unique-write
authority automatically.

## Uniformity and Convergence

Uniformity is a capability, not a marker inferred from a Rust type. The
variation lattice is:

```text
GridUniform < WorkgroupUniform < SubgroupUniform < Varying
```

The join of operand variation determines the variation of a pure result unless
an intrinsic has a stronger reviewed rule. Function parameters are `Varying`
unless their generated ABI contract marks them launch-uniform. Thread and lane
coordinates are varying at the corresponding scope. Constants and validated
launch extents are grid-uniform.

`Uniform<'launch, Scope, T>` can be created safely only from a generated
uniform launch argument, a constant, or a reviewed uniform intrinsic. A
varying value cannot be promoted by assertion or unchecked conversion.

A barrier is valid only when all required participants:

1. reach the same dynamic barrier instance;
2. reach instances in the same order;
3. do not exit while peers can wait;
4. use a scope and memory semantics covering the communicated regions.

IR analysis tracks control dependence and dynamic barrier order. A barrier
under control more varying than its participation scope is rejected unless the
primitive explicitly carries an active-mask contract. A backend `convergent`
attribute preserves an already established fact; it does not prove it.

## Memory Regions and Permissions

Every memory effect names a region:

```text
Region = (allocation, address_space, byte_offset, byte_length)
```

`allocation` is a symbolic allocation identity, not a raw address. Offset and
length are checked affine expressions over launch arguments, thread
coordinates, and constants. When the compiler cannot retain provenance or
bound the range, the region is `Unknown`; an unknown region cannot support a
safe launch without a separately bound unsafe contract.

Executable permissions are:

- shared read;
- exclusive non-atomic write;
- atomic access with operation, ordering, and scope;
- ownership transfer at a valid synchronization boundary.

Fractional permissions may be used as Verus ghost state, but are not part of
the device ABI. Read shares may overlap. Non-atomic write conflicts require
disjoint byte ranges or valid ordering. Atomic conflicts require compatible
atomic types, operations, scopes, and orderings. Atomic/non-atomic overlap is a
conflict unless explicitly ordered by the model.

Host argument admission applies the same byte-range rules across all arguments
and in-flight launches. Distinct Rust wrapper objects are not assumed disjoint
when they refer to the same allocation.

## Effect and Epoch Model

Kernel IR records effects per invocation and synchronization epoch, not merely
per address space. Each effect contains:

```text
kind, region, access width, alignment, invocation mapping, epoch,
atomic ordering/scope when applicable, and source location
```

An invocation mapping relates the dynamic thread to the affected byte range.
The analyzer must prove bounds and pairwise non-conflict for concurrently
unordered invocations. Conservative failure is reported as an unsatisfied
obligation; it must not be silently converted to a safe summary.

A barrier closes an epoch only for memory spaces and participants covered by
its semantics. Initialization and permission transfer into the next epoch are
therefore scoped. Workgroup, subgroup, device, and system ordering are not
interchangeable.

## Disjoint Views

`DisjointSlice<'buffer, 'launch, T, IndexSpace>` carries a live allocation,
byte extent, element layout, address space, index mapping, and exclusive-write
partition. Its executable ABI remains pointer plus length, with any dynamic
layout parameter such as row width bound into the generated view rather than
accepted anew at each access.

Safe mutable access consumes or borrows a matching
`ThreadIndex<'launch, IndexSpace>` and performs a bounds check. Arbitrary
integer indexing is unsafe. Conversion from raw pointers, remapping index
spaces, overlapping mutable subviews, and unchecked access are unsafe unless a
proof-carrying constructor establishes the same contract.

Static layouts use proof-carrying views such as linear chunks and row-major
tiles. A layout type must define checked host construction, logical-to-byte
mapping, bounds, and disjointness conditions once; compiler analysis and Verus
reuse that definition rather than maintaining separate formulas.

## Property-Level Assurance

Safety and correctness are tracked per property. The initial property set is:

- bounds and address-overflow freedom;
- provenance, alignment, and memory safety;
- initialization;
- race freedom;
- convergence and synchronization order;
- launch validity and host lifetime;
- functional correctness;
- layout or index-mapping validity when a nontrivial view is used.

The aggregate labels remain:

- `Verified`: every required property has authenticated, identity-bound proof
  evidence and all compiler/runtime checks pass;
- `Checked`: required static and dynamic checks pass, but complete authenticated
  proof evidence is absent;
- `Unsafe`: at least one obligation is delegated to a documented unsafe caller.

The current proof record is version 1 and lacks independent convergence and
layout property tags. Adding those tags requires a new backward-compatible
record version or an explicitly versioned model extension; existing tags must
not be reinterpreted.

## Same-Source Verus Rule

There is one executable kernel body. Ordinary rustc, the device frontend, and
Verus consume that body under controlled configuration. Proof-only ghost state
and lemmas may be erased; a macro may generate ABI shims and proof harnesses,
but it may not generate or maintain a second algorithmic implementation.

The proof model uses the same launch brand, index-space mapping, region, and
epoch definitions as compiler analysis. Correspondence is bound by semantic
and contract hashes. Until translation validation exists, this establishes
identity and evidence, not a theorem that LLVM machine code refines the source.

## Versioning and Migration

This contract is `gpu-safety-contract-v1`. Implementations serialize its
identity anywhere a cached analysis, proof, or artifact depends on these rules.
Semantic changes require a new identity and invalidate dependent evidence.

Migration proceeds in vertical slices:

1. Add region/effect and uniformity analyses without granting new authority.
2. Make arbitrary mutable indexing explicitly unsafe.
3. Introduce branded witnesses and generated view construction for one vecadd
   kernel while retaining raw compatibility APIs.
4. Bind the same vecadd body to Verus proof evidence and typed host launch.
5. Add workgroup memory, epoch transfer, managed barriers, and tile layouts.
6. Remove a compatibility path only after its replacement passes compile-fail,
   CPU reference, local RDNA, and required CDNA hardware tests.

During migration, a legacy kernel may execute, but any path using unbranded
indices, caller-packed arguments, unknown regions, or unauthenticated proof
records remains `Unsafe`. Compatibility must never be represented as parity.

## Required Test Classes

Every implementation of this contract includes focused positive and negative
tests:

- compile-fail: brand, lifetime, index-space, non-transfer, and alias misuse;
- IR verifier: malformed regions, overflow, overlap, divergent barriers, and
  incompatible epochs/scopes;
- Verus: positive proofs plus invalid bounds, injectivity, convergence, and
  initialization fixtures that must fail;
- host: overlapping allocation ranges, wrong context/brand/layout, and
  in-flight lifetime failures;
- hardware: CPU differential execution on local RDNA and required CDNA targets;
- artifact: model/version/hash mismatch and assurance non-promotion.

Hardware and sanitizer tests detect implementation defects outside the model.
They supplement proofs and static checks and never change assurance by
themselves.
