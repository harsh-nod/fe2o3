# Shared AtomicU32 slice identity

Status: experimental compiler integration for the bounded tensor-channel work
in [Ferric #42](https://github.com/harsh-nod/ferric/issues/42). This document
describes source admission and semantic identity, not protected artifact
authority, hardware correctness, or a cross-workgroup publication protocol.

## Nominal identity and storage

The admitted source spelling is a shared slice of genuine core `AtomicU32`.
The pinned nightly represents this alias as `core::sync::atomic::Atomic<u32>`.
The compiler checks the actual core crate anchored by its language items, the
`Atomic` diagnostic DefId, the exact `u32` generic argument, and rustc's
four-byte, four-aligned, one-field offset-zero layout. A same-named user type,
`UnsafeCell<u32>`, another atomic width, or `!Freeze` alone is not this identity.
Macro spelling recognition is only registration; it does not replace these
compiler checks.

`SemanticRustTypeKindV1::CoreAtomicU32` retains that classification alongside
the original nominal identity, layout identity and aggregate fields. The
pinned storage chain remains `Atomic<u32> -> UnsafeCell<Align4<u32>> ->
Align4<u32> -> u32`. Model validation checks the exact four-byte/alignment-four
storage, offset-zero single fields, unsigned leaf, and ordinary child kinds.
Each layer is charged to the existing validation-work budget.

An inert request can contain a nominal classification claim. Successfully
decoding or admitting that request does not authenticate its producer. The
production source owner, exact source replay and downstream verification must
still establish its correspondence with the compiler-observed program. The
marker does not authorize arbitrary scalar access to atomic storage.

## Canonical encoding

Semantic-MIR V29 adds type-shape tag 14 for the retained `CoreAtomicU32`
aggregate. It follows the unchanged type identity, layout and ABI-properties
prefix and carries the existing aggregate field-list encoding. No other
type-shape tag changes. V29 inherits the published V15 intrinsic grammar and
V28's RustCall local roles; retired V16-V26 and independently reserved V27
remain unavailable.

The marker raises the request's minimum version to V29. Exact older-version
admission rejects it, and older-version decoding rejects tag 14. Ordinary
types retain their old bytes; an unmarked request does not acquire V29 merely
because it contains a structurally similar aggregate. Fresh imports of genuine
AtomicU32 intermediates, including existing singleton atomic code, can select
V29 without changing how an already serialized legacy request is decoded.

## Slice contract

| Property | Required representation |
| --- | --- |
| Source identity | `RustSourceTypeShapeV1::SharedAtomicSliceU32`, distinct source tag 5 |
| Physical ABI | 64-bit pointer followed by 64-bit element count; size 16, alignment 8 |
| Element storage | `u32`, size 4, alignment 4 |
| Rust pointer representation | Const/shared reference, not ordinary readonly memory |
| Device effects | ReadWrite, SharedBorrow, SharedAtomic |

The pointer and count describe one retained slice allocation. A separate
integer next to a singleton `DeviceGlobalMutPtr<u32>` is not equivalent extent
evidence. Checked atomic borrowing must retain that allocation, actual extent,
exact index and dominating guard. Atomic operations retain their explicit Rust
ordering and System scope; the new type marker supplies neither bounds proof
nor allocation-coherence evidence. Ordinary readonly slices and singleton
pointer admission keep their existing contracts.

## Checked indexed borrowing

The first ranked route starts at a direct kernel argument with the exact
shared atomic-slice identity. Its private custody inventory is separate from
ordinary projected shared borrows. It authenticates the observed core path:
a shared indexed borrow, the atomic's offset-zero field address, and the exact
pointer cast to the physical `u32` cell. Single-definition pointer copies may
transport that origin; arbitrary pointer arithmetic, mixed roots and escaped
pointers do not acquire it.

The pinned rustc can also retain a dead sibling raw-pointer cast from the
atomic's `UnsafeCell` field to its offset-zero `Align4` storage. Only an exactly
typed, same-width/address-space cast into a temporary with no semantic uses
may be ignored. A metered exhaustive scan excludes a second definition,
projected/address uses, call arguments, terminator operands and return locals.
The temporary is not added to the pointer or coherence inventory; making it
live restores the normal rejection rather than granting new memory authority.

Each effect retains its semantic block and statement, access kind, address,
ordering and System scope. Projection checks that exact statement before
emitting the effect. The root, actual slice extent and exact index must match
an authenticated Rust bounds guard whose success block dominates the borrow.
A later borrow may reuse that guard only with the same stable source and index.
The generated dynamic guard has a trapping failure edge, never a total-load
fallback. Ordinary reads or writes through the atomic allocation, including
an alias with an ordinary scalar type, are not authorized by this inventory.

Shared atomic slices remain potentially aliasing shared roots. In particular,
a separate ordinary shared input slice is not automatically disjoint from an
atomic slice: an atomic write plus such a read is rejected when their relative
allocation offsets are unknown. A caller can instead supply an existing
`DisjointSlice<u32>` input capability and read its invocation-owned cell through
`get_mut(ThreadIndex)`. That is an explicit exclusive input contract, not a
noalias upgrade for the shared atomic slice.

The current dynamic-stability rule is deliberately conservative. Pointer-chain
definitions and non-argument index definitions must each be unique and occur
in acyclic blocks of the original CFG. This includes call destinations, not
only assignments. A single syntactic definition inside a cycle can execute
repeatedly and therefore is not treated as immutable. A stable pointer captured
before a loop may be used in that loop, subject to the remaining proof gates;
creating the indexed borrow or defining its index inside the loop is not yet
supported. This does not admit loop-created scheduler channels.

Custody construction, membership queries, sorting and cycle checks are charged
to the existing work and storage limits. Exhaustion rejects projection without
publishing partial authority. Kernels without a nominal atomic-slice argument
retain the existing singleton-allocation route. Indexed atomic coherence does
not establish ordinary tensor read-from relationships or cross-workgroup
publication; those require separate proofs and runtime contracts.

## Tests and execution boundary

`atomic_u32_v29_tests` checks nominal round trips, exact older-version rejection,
legacy encoding preservation, and invalid storage/marker mutations.
`atomic_slice_layout_tests` checks distinct source identity and rejects wrong
pointee width and pointer mutability. Macro tests check shared writable atomic
descriptors, ordinary-slice separation, and unsupported source spellings.

The ignored production-extraction driver tests pair a scalar-slice control
with a genuine core atomic slice performing the same independent checked
output write. Separate negatives substitute a local transparent wrapper,
`UnsafeCell<u32>`, or `AtomicU64` under the `AtomicU32` name. These are source
admission tests, not indexed-atomic execution tests. They require the pinned
nightly, rust-src and the production extractor.

The indexed-access regression group has three additional source controls.
Both the two-argument atomic-only control and the three-buffer control with
an exclusive `DisjointSlice<u32>` input pass ordinary source extraction to
gfx950 LLVM. Their assertions check Release store and Acquire load on the
indexed cell, four-byte alignment, and System scope. The original ordinary
shared-input variant is a passing negative: it must reject with
`FE2O3-RACE-002` rather than invent disjoint roots. These controls are separate
from the two-test source-shape admission suite above. They validate compiler
lowering only; the test harness does not retain a native artifact or launch
a GPU, and these results grant no protected runtime authority.

Generated host packing must use a distinct atomic capability and retain an
exclusive host lease through completion; ordinary readonly buffer packing is
not an atomic adapter. Protected preparation remains closed until its exact
runtime allocation/coherence contract is joined. An engineering dispatch and
its numerical checks, when performed, must be reported separately with exact
artifact, input and worker identities. See the [verification model](verification-model.md).
