# fe2o3-compiler-lineage

This crate owns canonical, bounded **inert content formats**. The V3 base records one
caller-selected Rust compilation invocation beside fifteen caller-supplied
semantic compilation transcripts through a compact final compiler-module
commitment. The capsule does not duplicate exact final LLVM bytes; the nested
`CompilerModuleHandoffV2` in the surrounding compiler-FFI handoff retains them.
That association is data, not proof that the inputs share a producer or
derivation.

`InertProductionSemanticCapsuleV3` is deliberately inert. Its name is an API
boundary: it must never be accepted where an authenticated producer-owned
capsule is required. Its hashes detect accidental corruption and byte
substitution relative to the bytes in one capsule. Public construction does
not authenticate who produced those bytes,
prove that a receipt is truthful, establish that one stage derived the next,
or grant compiler, artifact, publication, load, or launch authority. A later
producer-owned integration boundary must construct the receipts from retained
typed stage owners, authenticate that construction, bind this inert capsule to
the exact outer module handoff, and expose a distinct move-only admitted type.
The outer handoff and producer authentication are deliberately absent here.

The V3 decoder is strict: it accepts only version 3, zero flags and reserved bits,
one exact total length, canonical V3 rustc invocation bytes, a canonical AMD
target spelling matching that invocation, nonempty bounded receipt preimages,
matching per-receipt inert content identities, and a matching terminal inert
capsule identity. It
never falls back to another schema. Receipt payloads other than the rustc
invocation and target remain opaque to this dependency-light crate; their
stage-specific producers are responsible for supplying canonical transcripts.
In particular, the producer-owned semantic-to-LLVM association codec must bind
the exact final LLVM module identity from the nested V2 handoff and the compact
final compiler-module commitment receipt identity as two distinct axes.

Resource limits are part of the wire contract:

- semantic MIR: at most 128 MiB;
- every other stage transcript, including the compact final compiler-module
  commitment: at most 4 MiB each;
- complete capsule: at most 160 MiB;
- rustc invocation: the bound exported by `fe2o3-rustc-invocation`;
- target spelling: at most 128 bytes.

Lengths are checked before inert receipt allocation. Shared decoding retains
stage preimages as ranges in one canonical backing, with separately decoded
invocation metadata. The caller pays the entire backing capacity, including any
bytes outside its selected range. These bounds limit, but do not
eliminate CPU and memory denial-of-service risk when decoding untrusted input.

## Native Capsule V4

`InertProductionSemanticCapsuleV4` wraps one unchanged V3 base plus a mandatory
`F2NRF1` carrier containing unchanged F2RFO1 output and F2NSRC1 source packet.
The complete V4 capsule is still at most **160 MiB**, including both members and
framing. The carrier's independent 64 MiB output and 4 MiB source limits do not
increase that ceiling or the legacy receipt limits.

The fixed 48-byte header is `F2O3ISV4`, version4 (`u16`), policy1 (`u16`), header
length48 (`u32`), total/base/carrier lengths (`u64` each), and eight zero reserved
bytes. All integers are little-endian. Exact base and carrier bytes follow, then
`SHA256("FE2O3/INERT-PRODUCTION-SEMANTIC-CAPSULE/V4\0" || LE64(preimage length) ||
preimage)`. Layout/seal/read helpers reuse the carrier's bounded framing engine,
permit direct encoding in one allocation, prepay byte visits, and never retry
another version. Seal callback refusal or panic, including captured-value
destruction, precedes mutation.

Owned/shared-range decoding retains the base, its receipts and carrier in one
allocation. Native base MIR cannot exceed the complete source packet's length;
this is a necessary size check, **not** proof that their MIR or semantics agree.
Stage contents remain inert and may be unrelated. These content decoders are
unmetered like V3; production must prepay their work/working set and independently
recover source/F evidence with all cross-field joins. Typed production custody,
protected publication and launch integration remain outstanding.

## Native Neutral Graph Framing

`InertNativeNeutralSubjectV1` commits to two distinct constituents: a canonical
V12 mixed-SSA graph and its import-contract catalog. Its fixed 96-byte frame
contains both digests and lengths; the frame has its own domain-separated
identity. It is not interchangeable with the graph identity or any legacy KIR
identity.

`NativeNeutralModuleRefV1` borrows an envelope containing that subject and the
exact graph/catalog byte slices. Decoding checks framing and lengths, not the
truth of the constituent digests. The envelope shares the 4 MiB non-semantic
receipt limit. Constructing or decoding either format grants no authority.

Consumers must admit the graph through
`VerifiedCanonicalKernelIrModuleV12::from_canonical_bytes_with_verification_budget_v12`,
decode the catalog through `InertCanonicalKernelIrContractCatalogV1`, and compare
both actual identities and lengths with the subject. Direct graph admission
retains the single freshly decoded and semantically checked executable; it does
not clone or decode a second graph. Each successful admission transfers a
retained-storage receipt that callers must reserve while that owner remains live.
Logical payload accounting is not an allocator or process-RSS bound.

`CanonicalKirInventoryV1` indexes that exact graph.
`check_kernel_ir_contract_catalog_v1` uses the inventory to resolve catalog
bindings to allocation occurrences and check all six pipeline-marker kinds,
their storage, contract keys, and epoch types. Catalog bindings identify direct
physical allocations. Marker operands may carry those allocations through block
arguments or pointer selects when the owner-bound `CanonicalKirMustAliasV1`
analysis resolves every incoming origin to the same allocation. Conflicting or
ungrounded cycles, external pointer inputs, calls, casts, and offsets remain
unknown. This check does not authenticate source declarations or prove
pipeline-event ordering. Source replay, checked optimization-origin transport,
target binding, and protected host admission are separate requirements. These
framing APIs do not activate a new production route or establish compiler-wide
formal verification.

## Native Root Rosters

`MultiRootProofRosterTranscriptV3` and `MultiRootTargetBindingTranscriptV3`
provide bounded, association-only framing for the full native graph/catalog
subject. Their distinct V3 headers accept singleton or multiple-root rosters;
legacy V2 bytes and the multiple-root minimum remain unchanged. Proof rows
retain semantic-root order and an explicit descriptor-canonical permutation.
Target workgroup rows retain the supplied semantic-root order.

Neither codec validates payload derivation or joins those rows to an actual
source, graph, or launch configuration. Those checks belong to admitted
consumers. Exporting these codecs does not activate native production
compilation or grant proof, artifact, load, or launch authority.
