# Shared Context Entry Integration

The source authority is Ram's `ContextEntrySourceV1` HIR initializer/use proof,
replayed against rustc MIR and carried in private frontend Context custody as
`ProductionKernelContextEntryTransferV1`. Do not rewrite source MIR or grant an
origin from a ZeroSized constant/type. Pauli's earlier normalization experiment
is unmounted. Its authenticated logical-helper identity remains shared custody.

## Mounted Shared API

`ProductionKernelContextEntryTransferV1::checked_ssa_relation(owner, root,
max_work)` exposes Ram's existing `KernelContextEntryPlanV1::new` validation
through `ProductionKernelContextEntrySsaRelationV1`. There is no second graph
resolver. The relation retains the borrowed execution body, exact transfer
block/statement, destination, Context type, issuer local/SSA value, and parameter
SSA value. Construction verifies replay and the source/expanded-call relation.
The public inert transfer constructor itself grants no frontend authority.

Private `AuthenticatedProductionKernelContextsV1::checked_ranked_entry` checks
the custody digest, selected root, kernel binding, and launch identity before
constructing that relation. Ram owns the original HIR initializer/use checker.
The source constant remains unchanged, including the macro's inline issuer
initializer; no source normalization grants authority.

## Mounted Consumers

`project_and_verify_ranked_semantic_mir_with_contexts_v1` is mounted; the existing
no-context entry remains fail-closed for component callers. The production
pipeline and AMD Math callback supply borrowed frontend Context custody before
it is consumed into lowerer inputs. The checked relation is built once per root.

The relation is threaded through `project_intrinsic_contracts`,
`project_authenticated_capabilities_v1`, both dataflow passes, pipeline payload
replay, and `transfer_capability_statements_v1`. At only the exact replayed
ParameterTransfer statement, `context_entry_v1` requires the retained issuer
local's current Known owned Context origin and exact type, transfers it to the
checked destination, and consumes the old local origin. It never seeds
block-entry states or arbitrary same-typed arguments. Missing/Invalid retain
identical no-origin behavior. The following original Shared Borrow establishes
the receiver origin via the existing generic transfer logic.

The Math custody resolver follows the same checked parameter SSA value to its
issuer SSA value, then applies its original terminal issuer/root checks. This
is an alias relation, not an additional Math issuer.

Mounted files:
- Lowerer `kernel_context_entry_relation_01.rs` and parent include.
- Lowerer `numerical_policy_math_01/custody.rs` checked Context alias.
- Importer `kernel_context_ranked_v1.rs` and parent module.
- Ranked `context_entry_v1.rs` and parent optional-relation plumbing.
- `production_pipeline.rs` and Math `import_tests/lowering.rs` custody calls.

## Checkpoint 28

Parent reports exporter28c compiled after its public-getter documentation fix.
Three relation tests passed in a standalone harness linked to cached lowerer
`d85370d7b03a0666`, MIR `2728fc5bcda4deaf`, and Pliron `f3217416c402f6b2`.
They cover original constant/SSA identity, changed source coordinates and empty
evidence, and the work ceiling. These are explicitly inert component inputs,
not authenticated frontend qualification.

Central filters: `kernel_context_entry_relation_tests` (3),
`context_entry_v1::tests` (2), and existing Math SSA/borrowed Context tests.
The latest Math custody alias and ranked-origin tests still need central tests.
Full AMD `policy_math_all13_source_kir_gfx942` / `gfx950` callbacks remain the
qualification boundary. Positive and mutation assertions are unchanged; the
added source assertion requires erased MIR plus authenticated entry custody.
