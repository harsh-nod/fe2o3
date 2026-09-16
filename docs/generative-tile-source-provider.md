# Generative Tile Source Provider

## Status

This is a staged source contract for #275 and #272, not executable tile support.
Production rejects the four nominal authority types before ordinary structural
type lowering and rejects the staged callable terminals. No semantic-MIR version,
intrinsic tag, macro entry shim, root issuer, schedule or runtime authority is
introduced. The existing SIMT APIs retain their behavior.

The provider extracts the context/workgroup nucleus reviewed in historical
capability snapshot `a9559337915285ceed881bb2bef59437c5112eda`. That snapshot's
conflicting wire formats are not imported. The older unpublished tile draft's
unbranded `WorkgroupCollectives` receiver is replaced, not treated as equivalent.

## Source Contract

`KernelContext<'kernel, Kernel, Target, Launch>` is opaque, zero-sized and
invariant in each identity. Target and launch markers are sealed. There is no
public context constructor or root issuance function in this stage.

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

## Required Integration

The unified capability owner must supply the canonical schema and complete
authenticated root issuance chain: registration, nominal kernel marker, logical
helper, target/launch brands, physical-root-only issuance and ignored logical ABI.
No unsafe issuer or unused unsafe sealing trait is copied merely to stage this API.

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
