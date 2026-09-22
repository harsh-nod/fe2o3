# Qualified Enrollment Correspondence

Source: `22f71884a6b5cdb72816423b280a16b62931295d`.

- Verus: two whole-root runs, each 422 verified and zero errors at default solver limits.
- Seventeen scoped controls: six projections, five executable omissions/mutations, four contract sensitivities and two live-fixture sensitivities.
- CPU: 874 unit tests and 27 doctests passed; nine ignored tests in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- No performance benchmarks, native GPU execution or HIP/HSA measurements were run.

The raw relational harness requires only represented pre-state and corresponding input/output
slices. Exact decisions, projected output and represented post-state follow from the two
independently verified executions. No accepted-admission or post-representation premise is added.
Rejections use each execution contract's own unchanged-state guarantee; equal sequence views
are not used to manufacture opaque historical Vec identity.

The issued harness adds the existing logical issued-producer invariant and obtains preservation
of that invariant, reader/reservation storage and all previously valid producer statuses.
These remain properties of the represented logical reader/producer model, not a proof of the
actual reader wrapper or native producer authority.

Concrete witnesses cover permuted-slot success, late alias rollback, dirty-output precedence,
constructor-based issued-state success/replay and a synthetic live Pending reservation while
enrolling a distinct allocation. The live fixture retains a nonzero producer-reservation
count and checks protected allocation/member/writer contents, reservation state and Pending status.

The root includes historical Begin only because existing value views use its declaration.
Inherited obligations overlap prior packets; 422 is not a count of new obligations.
The production runtime source is unchanged. Paired execution and fixture population exist
only in the verification harness and add no runtime execution or allocation.

Scope remains normal execution in the modeled contents. There is no proof here of physical
Vec storage/capacity, allocation failure, unwind, the public forwarding wrapper, universal
existence of a represented issued model, or production constructor/Begin/acquire reachability.
Native pending-consumer admission remains closed and full HIP/HSA parity remains unproved.
