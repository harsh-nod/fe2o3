# Borrowed Workgroup MIR V20

MIR20 borrowed-Workgroup checkpoint is hooked and ready for parent compilation.
Only WorkgroupEpochProjection and SubgroupDeriveBorrowed are implemented. No
Math payloads, importer hooks, lowerer changes, or new terminal IDs are included.
No Cargo or network execution by this worker; no compilation/test-pass claim.

## Checkpoint Files And Checks

MIR parent hooks:

- `src/semantic_mir_v1.rs`: module exports, optional defined contract, V20,
  admission/signature/type checks, minimum version and canonical encoding.
- `src/semantic_mir_v1/canonical_decode.rs`: V20 decoding and test inclusion.
- `src/semantic_direct_call_expansion_v1.rs`: common/epoch mapping exports.

Completed children under `crates/fe2o3-mir-model/`:

- `src/semantic_mir_v1/defined_capability_v1.rs`
- `src/semantic_mir_v1/workgroup_borrow_v1.rs`
- `src/semantic_mir_v1/canonical_decode/workgroup_borrow_v1.rs`
- `src/semantic_mir_v1/workgroup_borrow_decode_tests.rs`
- `src/semantic_direct_call_expansion_v1/defined_capability_v1.rs`
- `src/semantic_direct_call_expansion_v1/workgroup_epoch_v1.rs`
- This integration note.

Eight new mounted tests: three `workgroup_borrow_` and five `workgroup_epoch_`.
They cover canonical roundtrip, MIR19/legacy preservation, substituted type
edges and ZST rejection, unchanged defined body, malformed/truncated metadata,
roster/byte/work limits, independent recipe rejection, and nested same/other
owner occurrence mapping with stale-source rejection. Parent should also rerun
the eleven existing policy Math tests. Children pass rustfmt checking; all
three parent files pass rustfmt parser checking without whole-file formatting.

The new execution variant requires downstream exhaustive matches. Parent owns
those changes, including the MIR-to-KIR operation conversion in
`semantic_execution_capability_01.rs`. Source must still attach authenticated
facts; lowerer must still resolve actual owned values and retain refinement
obligations. The previous all47 borrowed-subgroup failures are not claimed fixed.

## Shared Defined Contract

### Stable API For Parent Relay

The signatures below are now mounted and exported from `semantic_mir_v1`.
This is the ready signal for Ram's source hooks and Bern/Turing's scoped
consumers, subject to the parent's compilation and regression results.

```rust
impl SemanticFunctionDeclV1 {
    pub fn with_defined_capability_contract(
        self,
        contract: SemanticDefinedCapabilityContractV1,
    ) -> Result<Self, SemanticMirErrorV1>;

    pub const fn defined_capability_contract(
        &self,
    ) -> Option<&SemanticDefinedCapabilityContractV1>;

    pub fn with_workgroup_epoch_projection(
        self,
        record: SemanticWorkgroupEpochProjectionV1,
    ) -> Result<Self, SemanticMirErrorV1>;

    pub const fn workgroup_epoch_projection(
        &self,
    ) -> Option<&SemanticWorkgroupEpochProjectionV1>;
}
```

The implemented closed enum is:

```rust
pub enum SemanticDefinedCapabilityContractV1 {
    WorkgroupEpochProjection(SemanticWorkgroupEpochProjectionV1), // tag 0
}
```

Only `WorkgroupEpochProjection` exists in this checkpoint. KernelMathDerive
and PolicyMathBind remain future coordination targets, not callable APIs or
accepted wire tags. No unsupported body contract is replaced by a trust flag.

The shared enum exposes `function() -> SemanticFunctionIdV1`,
`source_identity() -> SemanticFunctionIdentityV1`,
`body_identity() -> &[u8; 32]`, and
`provenance() -> SemanticKernelCapabilityProvenanceV1`. Attachment consumes and
returns the original declaration without replacing its ABI, locals, blocks,
statements, terminators, or call edges. A conflicting second attachment rejects.
Its source identity and bounded body commitment must agree with that declaration.

Payload constraints for the two pending Math variants:

- `KernelMathDerive`: retain distinct context-reference/context/math type IDs,
  actual receiver argument zero, kernel brand and root provenance, and the exact
  defined getter body. Its defined `DeviceMath::current_branded` call must retain
  the original callable/function identity and separately checked helper body.
  The argument-free legacy terminal is not receiver or constructor attribution.
- `PolicyMathBind`: retain the ordered math-reference/math and
  policy-reference/capability edges, bound output, exact policy/kernel brand,
  root provenance, and original constructor aggregate/body. Math/policy inputs
  are source arguments zero/one, not reconstructed values. This constructor
  contract is independent of an FP consumer's function or scalar type.

One record per defined function bounds the roster by the existing function
limit. Epoch's body-fragment budget is 16 KiB. The Math proposal permits at most
one additional retained delegate body, each under the same fixed fragment cap;
it does not introduce an arbitrary recipe/dependency list. Admission must charge
body hashing and validate closed ABI/type/body recipes before accepting a tag.
All original functions remain traversed and subject to normal call-graph limits.

Checked expansion must use one common defined-contract occurrence mapping for
all variants: original function/body, caller/callee instance, source call block,
ordered actual operands, and destination/frame transfers. The common
binding is implemented, not permission to create separate Math
trust flags. Rebuilt expanded roots must not inherit a callee's attachment.
Neither body hashes nor these records authenticate a Rust provider or prove
same-owner SSA equality; those checks remain with source custody and lowering.

One optional `SemanticDefinedCapabilityContractV1` field is implemented on
`SemanticFunctionDeclV1`, with `with_defined_capability_contract(contract)` and
`defined_capability_contract()` APIs. The epoch convenience methods below wrap
its typed `WorkgroupEpochProjection` variant; there is no bespoke epoch field.
Only epoch tag 0 is accepted. Unsupported tags reject; no policy source
attribution is enabled by a generic identity label. The bounded Workgroup
checkpoint does not await implementation of future Math variants.

## Borrowed Subgroup

Append execution-operation tag 25 under compiler-intrinsic tag 73, gated at MIR
V20: `SubgroupDeriveBorrowed { workgroup_reference, workgroup, subgroup, width }`.
The existing `SemanticExecutionCapabilityContractV1` retains source identity,
signature, provenance, workgroup brand and input epoch. Signature is exactly
`[workgroup_reference] -> subgroup`, with SharedBorrow ownership and direct
Rust ABI. Width is exactly 64 for the currently reviewed source provider.
The reference must point to the distinct owned Workgroup type. Workgroup retains
its live size/rank fields; subgroup retains its physical u32 lane and markers.
Requirements include target support, dynamic Workgroup identity and lifetime.
No SSA authority is created by admitting this record.

Legacy `SubgroupDerive`, partition records, and MIR19 policy records retain their
tags, payloads, obligations and old minimum wire versions. There is no epoch
projection terminal and no borrowed-to-owned type substitution.

## Defined Epoch Projection

`SemanticWorkgroupEpochProjectionTypesV1` has public fields `reference`,
`workgroup`, `epoch_reference`, `epoch_type`, plus `new([TypeId; 4])` and `all()`.

`SemanticWorkgroupEpochProjectionV1::for_defined_function(function, body, types,
provenance, brand, epoch)` creates an inert record from the retained defined
`SemanticFunctionDeclV1`. It derives source function identity and a bounded
canonical function-fragment SHA-256 using the frozen V19 fragment encoding.
The exact executable body must remain two locals, one block, one assignment:
`_0 = &((*_1).2); return`, with typed dereference and field projections.
The full ABI and body, including source coordinates, participate in the digest.
This structural check does not authenticate a Rust provider or field name.

Agreed with the source-owner handoff: attach to the unchanged original body with
`SemanticFunctionDeclV1::with_workgroup_epoch_projection(record) -> Result<Self,
SemanticMirErrorV1>`. `function.workgroup_epoch_projection()` returns
`Option<&SemanticWorkgroupEpochProjectionV1>`. No request-roster builder is used.
Getters retain `function`, `source_identity`, `body_identity`, `types`,
`provenance`, `brand`, `epoch`, `receiver_argument` (0), `source_field` (2), and
the exact typed `projection()`. Admission checks the retained DEFINED callable,
exact ABI/type graph, canonical body digest and root provenance. Records do not
add function roots or excuse unreachable bodies. There is at most one record
per function, bounded by the function limit; validation work is charged explicitly.

V20 appends an optional closed defined-capability record after each existing function body.
V2..V19 encode no option tag or extension bytes. Projection presence selects
V20 even without a borrowed subgroup consumer. Record kind 0 has a fixed-size
payload; encoded receiver/field values must remain 0/2. Historical V19 policy
encoding does not depend on this extension.

## Checked Expansion

`SemanticCallExpansionV1::defined_capability_bindings(source)` returns
`Result<Vec<SemanticExpandedDefinedCapabilityV1>, SemanticCallExpansionErrorV1>`.
It exposes `contract()`, expansion/root identities, root, caller/callee instance,
source caller function/block, expanded call/entry blocks, ordered `arguments()`,
remapped `destination()`, ordered `callee_arguments()` and `callee_return()`.
The source argument order and caller place projections are retained exactly.

`SemanticCallExpansionV1::workgroup_epoch_projection_bindings(source)` verifies
source/replay identity and returns bounded, immutable
`SemanticExpandedWorkgroupEpochProjectionV1` bindings. Each retains the source
record, overall/root expansion identities, root, parent/callee call instances,
source caller function/block, expanded caller/projection coordinates, exact
remapped receiver operand and callee receiver/return locals. Mapping uses the
existing MIR operand remapper; source type/function ordinals remain unchanged.
Nested wrappers preserve separate instances and distinct same-typed operands.
The record remains attached to the original getter in `source.functions()`;
never copy it onto a rebuilt expanded root with a different identity/body.
Canonical receiver/return local indices are recovered from their formal roles,
not assumed to equal rustc's original `_1`/`_0` ordinals.

The lowerer must recover the existing owned Workgroup ValueId from that actual
operand through the retained parameter-transfer chain, then compare it with
the subgroup issuer input. Type/brand/epoch equality is insufficient. Missing
binding, wrong root, constant/fabricated receiver, stale source/body or lost
instance evidence rejects. This API is not an SSA equality or refinement proof.

## Source Coordination

Read source facts from Ramanujan's `workgroup_source_v1.rs` and its
`INTEGRATION.md` Exact Next Parent Hook. No native worker-message tool is exposed
in this session. Ramanujan: please confirm that semantic import preserves the
reviewed two-local/one-assignment getter exactly, and that owned Workgroup keeps
four fields with live u64 size/rank and epoch at ordinal 2. The device source
currently declares those fields; no layout-based owner recovery is proposed.
