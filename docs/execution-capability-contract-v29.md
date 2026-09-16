# Execution Capability Integration Contract

Status: the inert semantic MIR V29 types and callable codec are implemented;
executable capability integration is not complete. M0 and M1 remain incomplete. The production
importer constructs authenticated ContextIssue but still rejects workgroup/tile
terminals. Source identity 122 and the context commitment are an unpublished
draft pending the shared #271 allocation. The implementation must
use the one production graph and existing verification and launch gates.

## Independent Version Allocations

| Namespace | Allocation | Base |
| --- | --- | --- |
| Semantic MIR | V29 | Published V28 |
| Canonical KIR | V15 | Published V12 |
| Registered semantic operations | V3, Execution family 6 | Published V2 |

These versions do not activate intermediate historical formats. Semantic MIR
V16-V26, held numerical V27 and historical KIR V13/V14 remain separate. Match
explicit supported schemas, not a numeric range that admits those drafts.

The following tags are allocated together. Only the five callable MIR encodings
and four semantic MIR roles are implemented here; KIR/SO codecs remain pending.

| Operation | MIR intrinsic | KIR operation | Execution opcode |
| --- | --- | --- | --- |
| ContextIssue | 81 | 32 | 1 |
| WorkgroupDerive | 82 | 33 | 2 |
| WorkgroupScopeEnd | 83 | 34 | 3 |
| MaskedTileLoadU32 | 84 | 35 | 4 |
| MaskedTileIntoFragmentU32 | 85 | 36 | 5 |
| LaneFragmentIntoPartsU32 | 86 | 37 | 6 |

KIR type tags 9/10/11/12 denote context/workgroup/masked-u32-tile/u32-fragment.
All four are non-storable. Semantic MIR type tags 14/15/16/17 respectively carry
those roles alongside their unchanged aggregate fields, rustc layout and ABI.
Tile and fragment roles carry L:u16 and E:u16. Tag 13 remains str. Trusted source
terminal identities and checked-transformation receipt versions require their
own closed inventories and allocations. Source CombinedV4 tags through 121
and semantic intrinsic 68 are occupied. ScopeEnd has no public Rust terminal.

MIR callable payloads are exact type IDs: ContextIssue(C), WorkgroupDerive(C,W),
MaskedTileLoadU32(W,T), MaskedTileIntoFragmentU32(T,F), and
LaneFragmentIntoPartsU32(F,P). Geometry comes from the exact role types rather
than a duplicate operation field. Signatures require the actual mutable context
borrow, shared workgroup and u32 slice borrows, usize base, matching geometry,
and ordinary ([u32;E],[bool;E]) result tuple. The callable decoder rejects
69-80 and 83. ScopeEnd is generated in checked KIR callback materialization,
not inserted into the original authenticated source MIR or given a fake ABI.

Role layout and signatures are inert checks, not authentication. A bounded
request-wide containment map rejects constant fabrication through owned nested
carriers; references and function signatures stop ownership propagation.
Ordinary wrappers may transport existing roles, and borrowed closure captures
remain representable. Affine moves, actual borrow provenance, scope closure,
effects and schedule correctness still require canonical graph validation.
Current-production MIR admission/decoding and executable lowering reject V29
until that integration exists. Ordinary aggregate/ZST lowering cannot erase it.

The single rustc importer now preserves these nominal roles in its complete
type inventory, including unused parameters, nested carriers and referenced
types. It authenticates the reviewed device definition, checks generic argument
kinds and the initial u32/geometry profile, and retains rustc's actual fields,
layout, ABI and instantiated type identity. It selects explicit inert V29
construction when that inventory contains a role. Ordinary requests retain
their existing production schema selection.

This is type representation, not capability issuance. In particular, a caller
parameter with a capability type does not gain authority: executable
materialization still rejects before aggregate/ZST erasure or export. The four
remaining source terminals remain rejected. ContextIssue consumes the move-only
original/optimized entry receipt through preflight and actual body construction.
Collection ordinals are not semantic IDs: the receipt joins exact Instances to
the sorted function table and checked local/block mappings. It also retains the
exact optimized body, so stale or substituted MIR cannot reuse call coordinates.

Only the authenticated helper's argument zero can recover an erased context as
a move from the issuer destination. Surviving moves and ordinary ZST operands
are preserved. Both call occurrences, destinations, return edges and unwind
actions must be consumed once, independent of semantic block construction order.
The real rustc signature, FnAbi, type identity, physical root and existing
reference-effect bindings remain intact. No arbitrary role constant is admitted.

The draft V5 body-child extension binds the separately captured original MIR
digest, original and optimized occurrences, sorted function/local/block IDs,
and restored-argument binding. The original body is hashed while borrowed,
without querying optimized MIR. The existing function child binds optimized
MIR; ordinary bodies receive no extra transcript fields. This is source custody
and inert construction, not checked KIR materialization or executable authority.

## Ownership And Effects

One private, session-bound original/optimized issuance receipt authenticates
logical ContextIssue against the physical wrapper. Preserve physical export,
target, launch and argument order. There is no caller-supplied context kernarg.
Type equality and zero-sized layout never authenticate issuance.

WorkgroupDerive exclusively borrows its actual context. Its graph occurrence
identifies the scope; sequential InitialEpoch scopes remain distinct. LoadMasked
borrows that workgroup, so multiple loads are legal. IntoFragment consumes a
tile, and IntoParts consumes a fragment. Their exact producer, scope, epoch and
geometry remain associated with those values.

ScopeEnd consumes the workgroup, explicitly discards the exact remaining live
tiles/fragments of that scope, and releases its context borrow. Reject missing,
duplicate or foreign discard operands and subsequent scoped uses. Source values
are affine: unused tiles may be dropped, not forced through IntoParts.
ScopeEnd is not a GPU barrier or proof of memory visibility.

Issuance, acquire, consume and scope end carry ordered compiler effects even
when physically inert. Masked loads additionally carry physical Reads. Empty
physical effects never justify deleting, duplicating or reordering lifecycle
operations. Rust reference provenance must survive authenticated materialization;
do not turn a zero-sized context borrow into an ordinary addressable pointer.

## CFG And Structured Lowering

The first implementation checks acyclic, same-function capability regions after
generic checked callback materialization. Propagate ownership along actual CFG
edges and require exact state equality at joins. Mutually exclusive branches
may each consume the same incoming value. Every admitted exit closes its scope.
Reject capability phis, residual capability calls, exceptional exits, unreachable
capability islands and cycles until complete rules exist. Trapping operations
must be checked as well as terminators. Ordinary parts can escape; roles cannot.

IntoParts returns E u32 values followed by E bool masks using ordinary aggregate
transport. Keep 1 <= L <= 256, 1 <= E <= 125 and initial [L, 1, 1] geometry.
Checked address overflow or an out-of-bounds mask yields zero, false and no Read.
Blocked/Striped selection is immutable and binds source/SSA, structured input,
root, target, launch and covered operations before scalarization. Per-lane
results need independent distribution-specific oracles. Balanced ownership does
not prove workgroup-uniform arrival, input/base, numerical behavior or coverage.

Checked expansion must replay each generated guard/load/result and discharged
lifecycle operation against the exact input/output graph and schedule. The
existing Retained/ConstantFrom lineage is insufficient. Preserve original
call-path/access lineage and the complete ranked/formal effect census. Do not
relax the helper-purity gate as a substitute for that integration.

Lower/discharge all structured roles and operations on the same graph before
existing simulator or LLVM export. Residual operations reject; neither erased
types nor matching layouts can stand in for a successful lifecycle proof.

## Required Acceptance

Land types, codecs, bounded structural/scope verification, operand/effect
traversal and transformation replay together before admission. Test duplicate
issuance, overlapping/wrong/missing scope ends, repeated legal loads, stale or
double consumption, affine disposal, balanced branches, asymmetric joins,
bypasses, traps, cycles, hidden storage, source/schedule substitution, swapped
parts, exact Read traces, old-decoder refusal and exact/one-short budgets.

The first ordinary-source positive returns parts from with_workgroup and
folds/stores in the logical root, including empty/tail/full/overflow/canary cases.
Manual KIR fixtures alone do not establish production support. General helper
reads, reduction, protected proof and target-matched hardware remain separate
required integration work under the full roadmap, not implied by this contract.
