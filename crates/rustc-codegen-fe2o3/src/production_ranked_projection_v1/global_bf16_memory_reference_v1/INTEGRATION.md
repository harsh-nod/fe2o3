# BF16 Memory Reference Consumer

## Checkpoint

Originally child-only work during compiler23/source diagnostics. Integration49
registers the module under cfg(test) so its five original formula tests and four
new geometry tests are centrally executable. The formulas remain absent from
non-test production compilation; no admission or memory-proof consumer is wired.
The existing ranked rejection remains. No new MIR tag: BF16 reads already use V22/tag80;
Bern's matrix Bind/narrowing contracts must use their own coordinated tags.

`source_read_schedule_v1` builds independent CPU read-event requests using
existing `ProductionSemanticExpressionV2` types. It never constructs a Load,
reserved symbol, live read binding, reference receipt or refinement witness.
Inputs must later be resolved independently from the retained source SSA;
the current input record is deliberately inert and is not sufficient for
production admission.

Five standalone tests PASS against the existing built MIR/pliron libraries,
using rustc directly in `/tmp/fe2o3-bf16-source-reference-DwQD2O`. No Cargo or
dependency build. These check all lanes/roles, tails/overflow, distinct events
at coincident addresses, source-type range bounds, and invalid inputs/budgets.
They do not claim production source import, memory equivalence or launch proof.

## Source Semantics

The reviewed A/B terminals in `fe2o3-device/src/tensor.rs` compute four
component indices with checked row/reduction additions, checked row-stride
multiplication, two checked physical-offset additions and logical/physical
bounds. Every inactive component produces zero U16 bits. A matching current
Wave64 lane is a separate precondition, not an arbitrary extra read guard.

The request records preserve component order 0..3. Global::load is volatile;
equal addresses do not make two reads one event or one interchangeable value.
BF16 is checked at the raw U16 boundary plus exact U16-to-BF16 bitcast/fragment
contract. Do not mislabel BF16 as Float16 (the existing scalar expression
language supports only Float32/64) or introduce a rounding relaxation.

Constant strides retain an explicit multiplication-overflow guard. Dynamic
strides are supported when exact unsigned source casts establish that
`(max(rows) - 1) * max(stride)` fits U64; for example U32-to-U64 rows/stride.
The same row < rows predicate is retained. Unbounded dynamic products still
reject, since the current expression language has no general overflow-result
leaf and its static domain validator cannot accept guarded dynamic division.
No caller-supplied range record can assert that bound.

## Common Proof API Required From Pauli

Reuse one shared live read-site/SSA and memory-version graph. Do not duplicate
it here or convert the current structural-only load binding into authority.

1. Supply actual source input bindings for lane, four captured view fields,
   two base operands and length of the same live readonly Global allocation.
   Retain original root identity, whole expansion identity, expanded-root
   identity, actual call instance, block/site, and replay-checked borrow/Result
   custody. No fresh symbol or raw-pointer/slice origin may supply a field.
2. Associate each CPU request with one actual final typed U16 read SSA result,
   its exact view/allocation, element offset, byte width/alignment, address
   space, guard, zero fallback, unordered ordering and volatile status.
   Reconcile indices under the active guard; the final safe-index Select may
   differ on inactive lanes but must not execute an inactive read.
3. Establish the memory version/noninterference relation from the complete
   effect graph, including unknown/aliased writes and external effects. For
   volatility additionally prove event bijection, guards, multiplicity and
   relative order. Noninterference alone cannot authorize CSE, hoisting or
   swapping volatile reads. Existing `LivePlironSemanticLoadBindingsV1`
   explicitly rejects volatile reads and is not yet this proof API.
4. Bind and revalidate context, function, source/final identities, mutation
   epoch and canonical seal. Every bound result must be consumed; neither an
   unused side record nor a free symbol can grant expression equivalence.
5. Retain output ownership/effect/frame contracts separately. This request
   builder cannot satisfy missing write contracts or create an independent CPU
   reference from the final GPU expression.

## Parent Hooks After API Review

The test-only module declaration is in `production_ranked_projection_v1.rs`.
Its eventual production declaration must accompany the consumed proof relation.
The actual call belongs after common proof source-input resolution, before
the final read/reference requests are sealed and sent to the common scheduler.

Do not yet replace the `GlobalBf16MatrixLoad` rejection in
`apply_capability_terminator_v1`: its existing state only has aggregate
allocation effects, not the four exact live read producers. The replacement
must consume Pauli's checked source/read relation, then register both the
ordinary tensor operand/layout provenance and the four exact read events.
An Access-only recipe or the legacy `project_tensor_load_origin_v1` slice
origin is insufficient. Exact production call-site diff depends on Pauli's
owner-checked constructor; no provisional bypass/acceptance arm is supplied.
