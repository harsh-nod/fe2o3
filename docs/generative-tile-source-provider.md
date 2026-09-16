# Generative Tile Source Provider

## Status

This is a staged source contract for #275 and #272, not executable tile support.
Production rejects the four nominal authority types before ordinary structural
type lowering and rejects the staged callable terminals. Logical context entry
generation and collector authentication are separate prerequisites; they do not
admit executable context or tile operations. No semantic-MIR version, intrinsic
tag, schedule or runtime authority is introduced. Existing SIMT APIs retain their
behavior.

The provider extracts the context/workgroup nucleus reviewed in historical
capability snapshot `a9559337915285ceed881bb2bef59437c5112eda`. That snapshot's
conflicting wire formats are not imported. The older unpublished tile draft's
unbranded `WorkgroupCollectives` receiver is replaced, not treated as equivalent.

## Source Contract

`KernelContext<'kernel, Kernel, Target, Launch>` is opaque, zero-sized and
invariant in each identity. Target and launch markers are sealed. There is no
public context constructor. The reserved safe, out-of-line root issuance terminal
always traps unless a future authenticated lowering replaces it.

`KernelContext::with_workgroup` accepts a higher-ranked `FnOnce` callback owning
`WorkgroupCapability<'workgroup, KernelCapabilityBrand<...>, InitialEpoch>`.
The private issuer cannot be called directly by downstream source. The capability
contains two u64 coordinates plus private zero-sized epoch/identity markers; it
must not be imported as an ignored ZST. No public epoch token or transition is
provided yet.

The tile carriers are:

```rust
MaskedTile1D<'workgroup, T, const L: usize, const E: usize, Brand, Epoch = InitialEpoch>
LaneFragment<'workgroup, T, const L: usize, const E: usize, Brand, Epoch = InitialEpoch>
```

`Epoch` must implement sealed `SynchronizationEpoch`. The initial terminals
support u32, 1 <= L <= 256 and 1 <= E <= 125. L need not be a power of two.
Two arrays and two zero-field markers retain the existing carrier structural
budget. Kernel brand, workgroup lifetime and epoch remain invariant.

`load_masked` takes a shared borrow of the exact workgroup capability, an
ordinary shared input slice and a base. Its receiver-borrow lifetime is distinct
from the generative lifetime: borrowing the callback-local owned token must not
require borrowing that local for the whole generated scope. Tile-to-fragment and
fragment-to-parts consume their input. Parts are ordinary arrays and confer no
authority. No public reconstruction, copying or cross-thread transfer is provided.

Type-checked composition and scope/brand/epoch refusal cases are in
`crates/fe2o3-device/tests/ui/`. They do not execute a tile load. All staged
terminal bodies stop execution; none synthesizes values as a host fallback.

## Admission Boundary

A first, unique source argument spelled `KernelContext<'_>` is logical rather
than a caller-supplied kernarg. The macro preserves the original body in a
non-inlined helper, brands its context with the nominal kernel marker, and
generates a physical root that issues and moves the context into that helper.
Typed host arguments, registration function pointers and worker adapters contain
only the remaining physical arguments. A trusted `KernelResult` retains the
existing unit-returning typed entry convention.

A higher-ranked invariant type check authenticates the original source type
before branding it. Genuine default-context aliases named `KernelContext` work;
same-named lookalikes, never-type aliases, hidden custom brands and aliases
fixing `'static` do not. The check does not accept value coercions.
Opaque aliases with other names are not silently rewritten. Explicit brands,
borrowed contexts, nonfirst contexts, argument subpatterns and unsupported
attribute/control-flow/assembly combinations receive source diagnostics.

The bounded FE2O3KC V1 sidecar is an inert declaration. The collector binds its
exact physical root, sibling helper and nominal enum, authenticates the provider
and brands, and compares the source and adjusted rustc ABIs. The nominal enum
must have the declared kernel's generated marker name and cannot be shared by
distinct physical roots. Rehashing a sidecar does not establish marker ownership.
The collector then checks the
generated root's actual pre-optimization MIR value flow: one issuer, a moved
issuer result at helper argument zero, unchanged physical argument ordering,
and a closed return path without branching, cycles or root re-entry. Both
production entry points capture bounded, session-owned evidence before rustc
monomorphization can erase zero-sized value transport. The optimized wrapper
must correspond to that evidence; zero-sized constants are accepted only in
the exact authenticated helper argument slots. Issuance in reachable helpers
or undeclared roots rejects. Unsupported wrapper transport rejects rather than
being inferred from matching zero-sized types.

Only this generated wrapper protocol is inspected; user kernel bodies continue
through the existing general importer. Bound declarations and authenticated
producer evidence remain distinct. Even a valid entry still stops at the
unimplemented `KernelContextIssue` terminal in semantic preflight without a
simulation bundle or kernel artifact. Nominal caller-supplied capabilities
remain rejected at the separate capability-owner boundary.

The trusted device registry binds each reserved diagnostic item to the exact
reviewed source closure and compiler definition path. Nominal authority refusal
runs before struct-to-aggregate conversion, including retained aliases, nested
fields, reference pointees and zero-length array elements. Otherwise an unused
zero-sized context argument could bypass callable-terminal rejection.

Ordinary structs and zero-sized values remain supported. A phantom type argument
alone carries no authority value and does not expand the retained executable
type graph. Forged reserved diagnostic providers are rejected rather than treated
as ordinary structural values.

The existing production source lane explicitly tests named refusal and absence
of requested bundle/LLVM output. Compile-time dependency rlibs are not kernel
artifacts and are not required to disappear from Cargo's target directory.
The same lane compiles canonical and forged context declarations, including
zero-sized physical arguments, equal-type context substitution, argument
reordering, discarded issuance, loops, re-entry, incompatible ABIs, foreign
helpers/markers/roots, duplicate/orphan declarations and malformed/oversized
payloads. Fixture source and canonical sidecars live only in owned scratch
directories; they do not modify the compiler checkout during validation.
The full borrowed tile callback currently stops at the earlier closure-layout
guard; it is not counted as nominal-type importer coverage. The forged-provider
control passes type and FnAbi construction, then stops because its legacy V1
registration has no typed kernel binding. Ordinary and phantom-only ZST controls
separately require successful inert bundle export.

The direct phantom controls cover both `PhantomData<KernelContext<'static>>`
and capability-independent `PhantomData<&'static u32>`. Descriptor construction
uses the importer's fallible monomorphic signature normalization before deriving
exact argument identities, including erasure of nested lifetimes. Physical
layout checks and exact semantic identity comparison remain unchanged.

## Required Integration

The [unified SIMT/tile integration ADR](unified-simt-tile-integration.md) records
the proposed shared graph, schedule, scratch and acceptance boundaries. It is
not an approved schema allocation or evidence that the staged operations execute.

The unified capability owner must connect the authenticated frontend producer
to the canonical semantic schema, preserving registration, nominal kernel,
logical helper, target/launch brands and physical-root-only issuance. Frontend
authentication alone is not semantic capability transport. No unsafe issuer
or unused unsafe sealing trait is required for this entry protocol.

Workgroup issuance must carry checked producer/root/call occurrence identity,
not only a matching type or launch geometry. That identity and epoch must survive
the tile load, fragment and parts transitions. Callback transport must support
the actual capability argument and borrowed input/output captures; ordinary
owned-closure support does not establish those contracts.

The tile load's read stays at its original effect position. Checked offset
overflow or an out-of-bounds mask yields zero without memory access; uniform
arrival/input/base and a valid selected distribution remain compiler obligations.
Schedule selection is not part of the source algorithm type.

A per-invocation fragment result can depend on the selected distribution.
The same-result M2 example additionally needs the branded reduction/LDS/epoch
contract and group output. A layout mapping test alone cannot establish equivalence
of an arbitrary body that observes lane identity.

Only after canonical import, producer checks, occurrence transport, schedule-aware
materialization and downstream verification agree may the explicit refusals be
replaced for the admitted operation family. UI success does not close M1 or M2.
