# Math Defined Contracts

## Canonical-Index Fix Checkpoint

MIR21 integration is mounted. The actual source sweep confirmed that the old
raw ordinal assumptions rejected getter entry 1 and argument-before-return
local orderings. `defined_math_v1.rs` now resolves each unique local by its
exact role and type, uses `body.entry()` and the call's exact CallReturn target,
and requires the entry and return blocks to be distinct. Two-local/two-block
getter and bridge recipes and the three-local/one-block Bind recipe stay closed.
Bind fields still copy their exact Argument(0)/Argument(1) reference locals.
All ABI, body, projection, marker, root and digest checks remain in place.
No imported bodies are reordered and no canonical payload bytes are changed.

Eight new tests use the filter `defined_math_permutation`. The six tests under
`defined_math_v1/tests/permutations.rs` cover all 16 getter/bridge local/block
orders, all six Bind local orders, malformed edges/roles/references and exact
body commitments. Two whole-document tests are mounted through the existing
`defined_math_document_tests.rs` include: 36 identity-sorted V21 roundtrips and
replayed occurrence checks, with and without epoch records, plus stale metadata
rejection. Rustfmt checked; central compilation, test execution and real source
full-import success remain pending. No Cargo invocation by this worker.

Pauli owns Math SSA; Ram owns KIR operation 25. Neither area is changed here.

Central MIR16 passed all six recipe-level permutation tests. The shared document
fixture's composed root block identities have since been corrected without any
index or CFG change. MIR17 passed 212 tests; its remaining three failures were
original admission of the epoch-mixed fixture, not decoding. That fixture appended
epoch identities 24..32 after Math identity 190. Composition now assigns synthetic
type/layout identities 191..199 while preserving every TypeId, record order and
layout. No production codec or validation logic changed for either fixture fix.

Two additional tests in `defined_math_document_codec_tests.rs` check exact type
and function record byte boundaries and complete raw request equality under
V19/V20/V21, including None/epoch/Math function tails. The negative test deliberately
restores the unsorted epoch identity and distinguishes the original admission
error from the decoder's wrapped Validation error. Neither decoder repairs nor
fixture reordering are used. Central rerun and real Math full-import remain pending.

MIR18 passed 214 tests and identified the remaining epoch-mixed fixture failure
as original admission `TypeOutsideRootClosure` for its imported subgroup-only
u32. Composition now imports exactly the six Workgroup/epoch types (original
fixture indices 5..10), excluding its unrelated u32/subgroup types 4/11/12.
A partial checked type map remaps all selected fields, ABI/body references,
projection metadata and root call operands; an out-of-subset reference fails the
test fixture rather than acquiring a default mapping. Current added identities
are 191..196 and added TypeIds are 12..17. The new
`defined_math_v21_epoch_composition_keeps_exact_root_type_closure` regression
also requires reintroduced ordered-but-unused u32 data to fail the original
closure gate. No production root-closure visitor, schema or codec change.

## Original Detached Checkpoint (Historical)

Checkpoint: payloads, original-body validators, bounded codec helpers and 14
tests are staged in this directory and `../defined_math_v1.rs`. Rustfmt passes.
They are NOT mounted or centrally compiled yet. No existing MIR parent, enum,
wire version, partition payload, lowerer, analysis, backend or simulator was
changed for this checkpoint. MathCurrent source guard/tests are separately
mounted and frozen for parent compiler10d.

## Minimal Test Mount

These two module declarations are sufficient to compile all 14 tests without
adding enum cases or accepting any new document bytes:

```rust
// semantic_mir_v1.rs
mod defined_math_v1;

// semantic_mir_v1/canonical_decode.rs
#[path = "defined_math_v1/canonical_decode.rs"]
mod defined_math_v1;
```

Filter: `defined_math_`. The record child has 11 tests; the decoder child has 3.
The rooted fixture exercises record validation only, not whole-document ABI
admission, provider authentication, expansion custody, or executable lowering.

## Payloads And Identities

`SemanticKernelMathDeriveV1` follows the actual retained getter call to an
actual Defined bridge, then its actual call to the unbranded Current intrinsic.
It checks both original fixed bodies and all three source ABIs. The getter
receiver is preserved as argument zero even though optimized Rust MIR does not
read it. The bridge has no independent issuer role.

Payload order, with little-endian u32 IDs and raw 32-byte identities:

| Offset | Derive Field |
| --- | --- |
| 0 | Getter function, source identity, ABI identity, body digest (100 bytes) |
| 100 | Bridge function, source identity, ABI identity, body digest (100 bytes) |
| 200 | Current callable, source identity, ABI identity (68 bytes) |
| 268 | Context reference, context, branded Math, unbranded Math type IDs |
| 284 | Existing seven-field kernel provenance encoding (196 bytes) |
| 480 | Kernel brand identity |

Total: 512 payload bytes. No enum discriminant is emitted by `encode_payload`.

`SemanticPolicyMathBindV1` validates the actual retained constructor aggregate:
the unique Return local; copy argument zero's Math reference into field zero; copy
argument one's policy reference into field one; exact ZST marker into field
two. It rejects Move in this closed original-body recipe because the observed
source uses Copy. Caller moves and wrapper moves remain separate SSA obligations.
Its source ABI retains two Direct shared-reference arguments and Pair return.

| Offset | Bind Field |
| --- | --- |
| 0 | Constructor function, source identity, ABI identity, body digest |
| 100 | Math reference, Math, policy reference, capability, bound type IDs |
| 120 | Existing kernel provenance encoding |
| 316 | Strict policy identity |
| 348 | Kernel brand identity |

Total: 380 payload bytes, excluding the shared enum discriminant.

Both records include explicit ABI identities in addition to their full body
commitments. Each digest uses the existing V19 canonical function fragment,
with ONLY the generalized defined-contract field cleared, capped at 16 KiB.
Derive observes two separate fragments. Global validators reserve and charge
serialization plus hashing against ValidationWork. Generic typed metadata does
not authenticate a source provider or establish dominating issuance.

## Agreed Parent Hooks Still Required

1. Re-export the five record/types names from the new module. Add closed enum
   cases `KernelMathDerive` and `PolicyMathBind` in `defined_capability_v1.rs`.
   Forward common function/source/body/provenance getters. Do not add a second
   metadata field to FunctionDecl or manufacture a bridge-only issuer.
2. Reserve discriminants centrally (1/2 proposed; epoch0 remains byte-identical).
   Have the enum writer emit the Math discriminant then `encode_payload`.
   Decide the enclosing minimum wire version centrally; this child deliberately
   does not claim a V20 or V21 compatibility extension.
3. Route FunctionDecl's attachment builder to
   `validate_math_derive_attachment` / `validate_math_bind_attachment`.
   In the main function-validation pass call `validate_math_derive` /
   `validate_math_bind`. Local attachment alone is insufficient: only the full
   validators resolve the bridge/Current roster, type edges and selected root.
4. The decoder child methods consume payloads only. Preserve the existing
   epoch decoder's tag0 wrapper (its direct tests use that entry point), split
   out its payload reader if the shared dispatcher consumes the discriminant,
   then dispatch to `kernel_math_derive_payload` / `policy_math_bind_payload`.
   Require full admission and canonical re-encoding for a complete document.
5. `workgroup_borrow_v1::workgroup_epoch_projection` needs explicit Math cases
   returning None. The epoch-specific expansion adapter in
   `semantic_direct_call_expansion_v1/workgroup_epoch_v1.rs` must explicitly
   filter the Math cases; do not feed them to the epoch projection constructor.
   The common replay-checked `defined_capability_bindings` already preserves
   arbitrary closed contracts and needs no parallel Math occurrence table.
6. Include Bind's `(capability, policy, provenance, kernel_brand)` in existing
   numerical capability claims, so constructor/issuance/consumer claims cannot
   disagree even before SSA lowering. This is consistency checking, not proof
   that any issued value reaches a use.
7. The importer must obtain the records from original retained functions after
   canonical function/callable/type rosters exist. Authenticate original getter
   and bridge through the exact reviewed external-helper API; Bind through its
   existing reviewed marker plus the original-body validator. Match source
   canonical function identities and source ABI/type IDs to those rosters.
   The source adapter remains separate work; no consumer seeds a constructor.

## Custody Boundary

MathDerive types omit consumer-specific FP element, operation and bound-reference
types. Bind omits FP operation/element/bound-reference too. Join those only from
actual MIR19 consumer records and replay-checked original call occurrences.
The planned lowerer must track owner generations, Copy/Move, borrow/reborrow,
parameter/return transfers and frame death before emitting Derive/Bind/F32 SSA.
Derive consumes the actual original KernelContext owner; Bind consumes Math and
policy from that same dominating issuer. Strict numerical and target obligations
remain mandatory. Nothing in these detached records legalizes bare MathF32 or
removes the current downstream fail-closed policy consumer hooks.

## Test Names

```text
defined_math_getter_binds_original_getter_bridge_and_current
defined_math_bind_retains_both_reference_edges
defined_math_rejects_substituted_or_erased_callees
defined_math_rejects_moves_swapped_fields_and_wrong_marker
defined_math_rejects_abi_and_reference_substitution
defined_math_commitments_detect_same_shape_body_and_abi_changes
defined_math_rejects_nominal_alias_and_missing_identity
defined_math_body_hashing_is_bounded_and_deterministic
defined_math_payloads_preserve_exact_fixed_fields
defined_math_roster_validation_rechecks_root_and_bridge
defined_math_roster_validation_charges_hash_work
defined_math_payload_decoder_round_trips_exactly
defined_math_payload_decoder_rejects_every_truncation_and_suffix
defined_math_payload_decoder_rejects_zero_and_aliased_identities
```
