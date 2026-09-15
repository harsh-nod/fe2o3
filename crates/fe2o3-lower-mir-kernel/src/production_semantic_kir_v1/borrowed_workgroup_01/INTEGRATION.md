# Borrowed Workgroup Lowerer Child

Mounted checkpoint and coordination: see [STAGED_CHECKPOINT.md](STAGED_CHECKPOINT.md).
The KIR25/revision3 schema, verifier, shared-graph matcher, transport, checked
dispatch, backend/simulator adapters and real-import regression are mounted.
Parent Lowerer21 compiled: all nine borrowed graph tests pass; five separate
existing loaded-value/write-contract tests still fail. Parent KIR21b: 136/136
library tests pass. Compiler21 and the actual full-import/lowering run are pending.
The new exhaustive catalog extension is ready for central testing.
No Cargo/SSH/network invocation here. Source replay, issuer equality and all
lifetime gates remain enforced; unsupported paths retain the rejecting fallback.

## Implemented Boundary

`borrowed_workgroup_01.rs` defines a private module with these parent-visible APIs:

```text
BorrowedWorkgroupPlanV1::new(
    owner: &ProductionSemanticSsaOwnerV1,
    context: &RootKernelContextLoweringV1,
    max_work: usize,
) -> Result<BorrowedWorkgroupPlanV1>

plan.subgroup(
    execution_block: SemanticBlockIdV1,
    lowering: &SemanticFunctionLoweringV1,
) -> Result<BorrowedSubgroupLoweringV1>

plan.epoch_matches_subgroup(execution_block, actual_epoch_operand, &subgroup)
    -> Result<()>
```

The constructor replays the production SSA owner and obtains both its execution
view and exact execution SSA plan. It consumes Pauli's replay-checked
`defined_capability_bindings(source)` directly, selecting the original function's
Workgroup epoch records and checking the shared binding's contract and identities.
V21 Math records remain in the unchanged source owner and are not treated as
epoch records. No caller-supplied occurrence list or
type-indexed authority lookup is accepted.

Subgroup resolution retains distinct reference/owned TypeIds and follows actual
`SsaValueV1` definitions, use events and CFG edge arguments. It permits shared
borrows/reborrows, value forwarding and checked tuple/aggregate field forwarding.
All incoming paths must resolve to one original WorkgroupDerive result with the
same root/provenance/brand/epoch. Parameter or constant issuance, mutable/raw
borrows, unannotated epoch fields, conflicting origins and unsupported projections
reject. The final live reference binding and existing owned issuer binding must
have identical logical SSA ValueId/type pairs in the current lowerer.

Epoch projection must belong to the original getter's checked call instance,
callee return/argument locals, exact source body recipe and type/provenance record.
The consumer must be the actual partition derive call, with its original epoch
operand and contract. The resolved owner must equal the subgroup receipt's exact
SSA issuer. This does not replace KIR's check that the ACTUAL subgroup consumer
operand was issued from that same Workgroup ValueId.

Loans retain the owned storage local and definition. Source storage kills/moves,
overwrites, or a possible same-brand/root/epoch transition before the consumer
reject. A returned epoch reference does not extend the lifetime of a callee's
reference slot; only the actual owned referent's storage loan is retained.

The result is deliberately NOT an OperationKind or serializable KIR contract.
It carries the original borrowed source contract, exact Workgroup SSA/ValueId,
nominal result type, full expansion/root identities and original call coordinates.
The checked dispatch converts this receipt to the distinct borrowed KIR contract
only after source-carrier validation; it never emits the legacy owned operation.

## Bounded Limitations

- One source assignment per resolved local/SSA definition in a block. Multiple
  source definitions in one block reject rather than guessing event coordinates.
- All uses of a tracked local in a block must have one resolved SSA identity.
- Traversal depth 128, projection depth 16, explicit work budget. Cycles reject
  through depth/work limits; no cyclic capability induction is claimed.
- Lifetime scanning is conservative at block boundaries. It may reject a kill
  or overwrite earlier in the same block, but never omits the lifetime check.
- Retained-memory reference variables with no promoted SSA uses reject. The new
  PLIRON classifier covers closed shared-reference flows and replay-bound epoch
  transfers. Escapes and unsupported aggregate-reference transport stay in memory.
- The final lowered receiver must already use the same logical ValueId as the
  exact issuer. A separate KIR block parameter for an otherwise equivalent owner
  currently rejects; it needs checked canonical-issuer transport, not removal of
  this equality check or a type-only equivalence claim.
- Source graph unit tests are not authenticated AMD/launch/machine authority.
  Full-import -> SSA -> KIR positive and mutation regressions remain mandatory.

## Exact Parent Hooks

Pauli coordination: the common mapper's mounted signatures are sufficient for
this bounded child. Preserve `contract`, `arguments`, `callee_arguments`,
`callee_return`, `destination`, caller/callee instance, original/expanded block,
and expansion/root identity getters when adding V21 Math variants. Keep full
source replay before filtering consumers. No new MIR origin API is requested
for this bounded implementation. More general repeated definitions will need
the SSA adapter's checked statement/event-origin relation, not a parallel mapper.

1. Mounted next to the other lowerer includes; this exposes the private
   module/tests without changing any dispatch:

   ```rust
   include!("production_semantic_kir_v1/borrowed_workgroup_01.rs");
   ```

   Then parent-run `borrowed_workgroup_01::tests` in the lowerer test binary.
   Tests cover distinct same-typed SSA definitions, multiple versions of one
   local, unreachable/missing uses, storage death, overwrite, exact budget,
   merge edge identities, immutable reference types and legacy signature loss.

2. Implemented in PLIRON `production/semantic_ssa/adapter/borrowed_workgroup_v1.rs`
   with minimal hooks in `adapter.rs` and `execution.rs`. Exact borrowed terminals
   preserve source reference/pointee IDs. Epoch field projection requires Pauli's
   replayed defined-call binding and the exact original source statement. Closed
   shared forks, reborrows and ParameterTransfer/ReturnTransfer value aliases
   are address-transparent; ordinary Use/Define/Kill events remain unchanged.
   See the PLIRON child INTEGRATION.md for tests and bounded exclusions.

3. Lowerer `subgroup_partition_01.rs` needs to classify the new borrowed result
   for SSA transport. Extend role/transport recognition without using that
   type-level classification as owner evidence. Construct this child from the
   existing source SSA owner and authenticated root before lowering the exact
   execution view. Supply its plan alongside `SemanticFunctionLoweringV1`.

4. At the exact borrowed call, after operand bindings exist, invoke `subgroup`.
   Lagrange owns the checked source-occurrence carrier/codec rev5 and lowerer
   source hook. This child does not change that gate. Issuer AND consumer must
   carry replay-checked origins before expanded execution is accepted.

## Required KIR Representation

Legacy `ExecutionCapabilityOperationV1::SubgroupDerive` cannot be reused:
`signature_matches` demands `[workgroup] -> subgroup`. Encoding the actual
`[workgroup_reference] -> subgroup` fails. Rewriting the signature would erase
source custody. Existing Workgroup operands also carry no explicit shared-borrow
role. The new regression asserts this incompatibility rather than making it pass.

Propose a versioned closed `SubgroupDeriveBorrowed` operation carrying
`workgroup_reference`, `workgroup`, `subgroup`, `width`, and a WorkgroupSharedBorrow
operand contract: source signature remains the reference; logical operand is
the existing owned Workgroup SSA value. The source occurrence/loan record must
retain the checked reference definition and exact owner origin. A borrowed
subgroup result role must retain this relationship across consumer transport;
do not simply declare an arbitrary Subgroup token equivalent.

Retain root dominance, source/owned nominal identity, exact issuer ValueId,
brand/epoch and live shared referent obligations in verifier/analysis. Update
partition issuer recognition for this closed borrowed issuer while preserving
`issuer.operands == [epoch_workgroup]`; retain backend/simulator operand checks.
Old KIR encodings remain unchanged. Parent reserved operation tag25 for the
borrowed contract/verifier (Math26); ownership is now this Workgroup worker.

Exact operation fields in the unmounted batch:

```text
SubgroupDeriveBorrowed {
    workgroup_reference: ExecutionTypeIdentityV1,
    workgroup: ExecutionTypeIdentityV1,
    subgroup: ExecutionTypeIdentityV1,
    width: u32,
}
source signature: [workgroup_reference] -> subgroup
logical operands: [the existing owned Workgroup ValueId]
operand contract: WorkgroupSharedBorrow
result role: BorrowedSubgroup { workgroup_reference, workgroup, width }
```

Keep ordinary execution provenance/brand/epoch and required ownership/lifetime
obligations on this operation. The result role's type identities are NOT owner
evidence; the verifier must retain the actual issuer operand equality.

Lagrange root-lowering coordination: the child takes the existing
`RootKernelContextLoweringV1` (`selected_root`, `semantic_type`, `context_type`,
`source`) plus the immutable `ProductionSemanticSsaOwnerV1`. No extra unchecked
root identity fields are requested. Retain the source owner/exact execution view
long enough to construct `BorrowedWorkgroupPlanV1::new`. Invoke `subgroup` with
that view's real block and the current `SemanticFunctionLoweringV1`, whose
`semantic_ssa_bindings` and `locals` must still contain the exact Workgroup issuer
and receiver. The receipt exposes full expansion/root identities, caller instance,
original source function/block and execution block for comparison with the new
source carrier, never as a substitute for its replay. Its private loans and
`owned_ssa` remain available through partition derivation via
`epoch_matches_subgroup`; do not serialize only the nominal result type and drop
this source-side custody before consumers have been checked.

Required integration negatives: other same-typed owner, mismatched binding or
call instance, stale epoch on any path, killed/moved/overwritten referent, raw or
mutable reference, fabricated ZST, ambiguous join, lost parameter/return transfer,
and missing/different KIR issuer. Add real AMD callback lowering coverage before
removing the production rejection or claiming the subgroup errors are fixed.

## Separate Source Fixture Fix

The AMD10 full-import fixture failed during registration, before new coverage.
`canonical_transport_v1/source.rs` no longer has an explicit namespace. Its
harness now sends `derive_crate_binding_id_v1(CRATE_NAME, [METADATA])` through
`CRATE_BINDING_ID_ENV_V1`, using the same constants as the actual rustc argv.
The callback asserts the collector's independently derived session binding.
Parent Compiler13 build and `workgroup_full_import_gfx950_v20` passed. This clears
actual source import/roundtrip, not SSA-to-KIR lowering or launch admission.
