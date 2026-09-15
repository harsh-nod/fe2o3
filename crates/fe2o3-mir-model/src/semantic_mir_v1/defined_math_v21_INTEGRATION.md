# MIR V21 Integration Checkpoint

Production hooks are mounted. Central compile/test confirmation is pending;
the 14 detached tests passed centrally before production integration. No Cargo
was run by this worker. The new `defined_math_v21_` filter contains 7 whole-
document tests; `defined_math_` includes these plus the original 14 tests.

## Stable API

All five types from `defined_math_v1/INTEGRATION.md` are re-exported by
`semantic_mir_v1`. Their constructor signatures are unchanged. Shared enum:

```rust
SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(record) // tag 0, V20
SemanticDefinedCapabilityContractV1::KernelMathDerive(record)         // tag 1, V21
SemanticDefinedCapabilityContractV1::PolicyMathBind(record)           // tag 2, V21
function.with_defined_capability_contract(contract)?
```

`admit_current_production` selects V21 only when a Math defined contract is
present. `admit_exact_v21` and `decode_exact_v21_canonical` are available. V20
rejects tags 1/2. Epoch tag0 remains byte-identical; MIR19 PolicyMathF32/tag79
and its historical writer path are unchanged. V21 retains the existing optional
metadata slot after the full function body, not a terminal replacement.

Construct records only AFTER the canonical function/callable/type rosters and
original calls have their final indices. Defined callable IDs equal function
indices in the full Defined prefix. Getter, bridge and Current IDs are observed
from that roster and committed with their original bodies and ABIs. Moving a
function or inserting a Defined callable requires rebuilding the records from
the remapped original bodies; copying stale metadata is rejected.

Full admission dispatches to both original-body/ABI/type/root validators and
charges their bounded digest work. Bind shares policy/provenance/brand claims
with issuance and consumers; getter, Bind and consumers also agree on the
nominal Math type's provenance and brand. This checks consistency, not issuance
dominance, source-provider authentication or numerical/refinement proof.

## Common Occurrences

`SemanticCallExpansionV1::defined_capability_bindings` is unchanged. All closed
variants retain original contracts, caller/callee instances, source/expanded
blocks, ordered Copy/Move arguments, formal argument locals and destination.
The epoch convenience adapter explicitly filters both Math variants; it still
replay-checks the complete source. No parallel Math occurrence table is needed.

The seven new tests cover full documents with and without epoch metadata,
canonical prefix index shifts, historical version selection, payload tag gates,
full-document mutations/truncation/suffix rejection, stale bodies/ABI/type/root,
conflicting policy/brand claims, bounded encoding/work and mixed common/epoch
occurrence replay. An arbitrary whole-roster permutation regression is not yet
included. Existing detached tests retain the exact original source recipes.
