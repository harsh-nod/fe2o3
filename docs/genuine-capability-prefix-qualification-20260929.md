# Genuine retained capability-prefix qualification — 2026-09-29

A test-only checkpoint now runs the actual Rust-derived constant analysis,
root-entry preparation, Option/enum/scalar/provenance/allocation analyses, both
capability FIFO passes, final source-index replay and read binding under the
original compiler owner, physical resource ledger and accepted-credit counter.

The nominal calls are authenticated against the actual retained source and
pipeline inputs. The independent oracle recomputes complete DATA through the
unchanged original analyses and original capability propagation/driver. It
compares block/local states, reached-empty versus unreached states, duplicate
FIFO visits, effects, bound reads and semantic work; it is not seeded by
candidate output. Pending owners survive the checked/facts postflights and
are dropped before accepted storage is released.

The closed fixture profile has 31 constant slots (all unresolved), 19 blocks,
zero caller layout effects, two bound reads and 21 visits in each FIFO pass.
Original semantic capability work is 1,237; actual query visits are [1, 1, 1].
An empty resolved-constant or caller-layout set is legitimate for this source;
it does not mean the corresponding analysis was skipped.

## Executed evidence

Full qualification passed 227 serial authority/capability tests, nine policy
integration tests, 368 model tests, 3,360 backend tests (197 ignored),
backend/extractor builds and all five genuine Rust-source sessions.

The identity and swap01 sources completed 36 positive numerical CPU runs.
Outer callback error/panic cases completed one numerical run each. Four
eligible source sessions each exercised three complete prefix modes
(Compare, CallbackError, CallbackPanic) and two controlled actual-query failure
modes (QueryError, QueryPanic); wrong-launch never entered this checkpoint.
There were 12 complete-prefix observations and eight controlled query failures.
The query failures are explicitly incomplete, not successful analyses.

Root compared every recorded observation with the preceding source checkpoint:
only the exact added work counts differ (1,653,029 for identity-derived cases,
1,653,729 for swap01). Numerical payloads, canaries, negative outcomes, source
and canonical identities, storage observations and ordinary-route refusals
remain equal. Earlier analysis markers are identical; source addresses in
earlier Fixed markers differ only between sessions and join within each
session. Null unexecuted numerical slots are not counted as completed runs.

Full regression receipt:
`7d7ab3668e6e175ba681dd8e8613c6568de7a74f609006a5cc6d00bda1c83a67`.
Complete comparison readback:
`5eca5a3c637a99f19639a93259c4c217f73abad54b1877b7d02e4fb8fc148c43`.
Independent source review:
`86a89619944f525686efdefe1e139302d0ef3dd05d447c856a7a119a22111d00`.

Two earlier failed qualification attempts remain retained: a missing checked-add
propagation operator, then a source-order control matching its own literal.
The corrected control requires unique real-method signatures and preserves
all ordering checks, including the complete original driver call.

## Limits and remaining integration

This is a closed, test-only ActualCapabilityPrefix, not the ordinary production
route, full F2, or whole-root BeforeArgumentWriters. Its current actual reference
ABI is empty after authentic scanning. The retained return-style oracle keeps
completed results; it does not claim every legacy temporary or panic payload
survives its original error boundary. Logical source-level accounting is not
measured native stack, allocator overhead or RSS.

Earlier shared-read/singleton/scalar-borrow owners, the canonical induction join,
projected views, lazy scopes, slice reservations, later argument writers, joint
bounds and ordinary production admission still need their complete original-order
integration. The existing BF16 semantic-Assert limitation is unchanged; this
does not establish a genuine nonempty Fixed proof or singleton contract.
No GPU execution, inferior/attach operation or physical debugger capture ran.
Broad accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Fresh qualification after incoming main reconciliation

The unchanged prefix was requalified on incoming main
`67def3828a63189030010dfafd5d4328c144a25f`, together with the inert retained
singleton component. The full gate passed 270 serial authority/capability tests
(four ignored), 20 policy/runtime-manifest controls, 370 model tests and
3,374 backend tests (197 ignored), builds and all five genuine source sessions.
Receipt: `e4c1626866af5ccaaf17fb44a8b21dd752ef2970df08c2d22a140c8cd1ff1849`.

All complete observations equal the earlier successful prefix checkpoint.
Only the explicitly reconciled fixture lockfile pin differs in source and
invocation inputs; registry package versions/checksums remain unchanged.
Readback: `74fbba04dc95b57700998a9ac514abb51639cdfebb4577b8bf6e4e53ec59ae17`.
See [singleton qualification](retained-scalar-singleton-qualification-20260929.md)
for the retained initial lockfile failure, component controls and unchanged limits.
