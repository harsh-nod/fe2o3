# Borrowed Workgroup SSA Adapter

Mounted in `adapter.rs`; `execution.rs::construct_plans` calls
`transparent_borrow_sites_for_execution_v1(semantic, expansion, view, max_work)`.
No Cargo/SSH/network invoked. Tests are mounted but not compiled/run here.

## Boundary

- `direct_sites` accepts only closed shared borrows/reborrows/value forwarding
  to exact execution contracts, preserving distinct reference/pointee TypeIds.
  It covers SubgroupDeriveBorrowed argument0 and partition receiver arguments.
- `execution_sites` obtains common `defined_capability_bindings(source)` through
  full source replay. It checks the exact view object, root/expansion identities,
  original function record, callee instance/locals, entry block and source
  statement before recognizing the defined epoch projection. V21 Math records
  remain retained and are not interpreted as epoch getters.
- Parameter/return aliases are read from the actual replayed body. All uses of
  each reference must stay in the closed graph. Shared forks are supported;
  raw/mutable borrows, escapes, duplicate definitions, unknown calls and other
  projections invalidate the affected component. Unused closed references do
  not make addresses observable either.
- An epoch reference reaching partition derive additionally requires matching
  epoch-reference type, provenance, brand and epoch metadata. This is NOT the
  same-Workgroup ownership proof: the lowerer child must compare actual SSA
  issuers, and KIR must compare the actual subgroup issuer's operand ValueId.
- Only address-observability classification changes. Source bodies, projection
  places, ordinary SSA Use/Define/Kill events and frame lifetime markers remain
  intact. No occurrence list, machine proof, new terminal tag or source-gate
  waiver is introduced.

The deterministic flow scan is capped at 262144 work units. Direct classification
exhaustion retains storage observability; execution classification exhaustion
returns a resource error without a partial accepted set. This does not add
general tuple-contained reference transport or cyclic reference induction.

## Parent Tests

Run the existing central test binaries after the coherent MIR V21 rebuild:

```text
<fe2o3-pliron-test-binary> borrowed_workgroup_v1::tests --nocapture
<fe2o3-lower-mir-kernel-test-binary> borrowed_workgroup_01::tests --nocapture
```

The PLIRON fixture is inert canonical MIR admitted through the V20 API and
expanded through the real replay API. It retains a nested defined getter and
both parameter/return transfers, rejects a missing annotation and foreign
source/view replay, and checks that the source storage kill survives planning.
Its manually constructed aggregate is plain source data for address-observability
testing, NOT issued Workgroup or machine authority. Exact borrowed terminal,
binding/type/arity mutations, escaping forks and work-limit tests are separate
fragment-level classifier tests. Lowerer lifetime tests use actual planned SSA.

Compiler13's actual gfx950 full-import test passed independently. Real combined
SSA -> borrowed KIR -> partition positive/negative coverage is still required
after Lagrange's source carrier and Bernoulli's tag25 contract are integrated.
