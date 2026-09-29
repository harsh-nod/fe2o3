# Aggregate Batch Release Pin Budgets

Production/CPU source: `05d858326b36349959de08d9451813a0ea6d954f`.
Final proof source: `c677f1915d1e5efcfdd38e5d60732712e20321c0`.
The second commit adds one proof assertion; executable and CPU-test code match.

This packet closes an aggregate-underflow gap in three completion release paths.
It does not close A1/A2, issue #182, or HIP/HSA parity.

## Runtime Change

Event-batch, reader-batch, and paired reader/event-batch release now validate the
whole roster's pin budget before changing any owner ledger or counter. Two
genuine events sharing a slot must fit that slot's aggregate budget. Paired
release checks both counters through their minimum. Previously, individual
positive-count checks alone could permit a partial decrement when an already
inconsistent owner undercounted an aliasing roster.

Existing duplicate, identity and live-slot validation still runs first, with
unchanged error precedence. Scratch-allocation failure also occurs before owner
mutation and preserves the original roster. The helper's empty/single-item path
does not allocate; multi-item checking uses expected O(N) time and O(N) scratch,
without scanning all 8192 completion slots. No timing improvement is claimed.

Three new CPU groups exercise four independent pin-underflow cases, genuine
shared-slot success, six legacy-error-precedence cases and eleven raw budget
sequences. Refusal checks exact token fields/order, vector allocation/capacity,
and the complete logical owner snapshot. Success checks surviving readers/events,
neighboring slots, identity counters and genuine final cleanup.

## Shared-Body Proof

Production and Verus include the same single-next and reserved-loop executable
bodies. For a lawful iterator with an available nonprophetic decreasing metric
and an initially empty scratch HashMap, the reserved helper succeeds exactly
when each key's occurrence count fits its first supplied budget. Failure returns
the exact original generic non-Copy error value. The single-next theorem checks
only its first item; whole-input coverage depends on the outer selector.

The proof uses actual standard iterator, HashMap entry/or_insert and checked
subtraction operations. Two constructed Vec-iterator witnesses demonstrate
acceptance and rejection, including inconsistent later budgets: `(3,2),(3,0)`
passes; `(3,1),(3,9)` fails. Production projections read the same immutable slot
budget for each repeated key; those projections remain outside this theorem.

## Qualification

- Final signed campaign: **17/17 stages pass**; opening, relocated and closing
  positives each report **8 verified obligations, zero errors**.
- **Nine executable negative controls** are strictly classified logical failures:
  accepting zero, rejecting one, missing/double decrement, omitted write,
  aliased keys, zero initialization and inverted exhaustion/refusal results.
- Signed source binding, exact two-file relocation and complete source continuity
  pass. Pinned tool closure matches before and after: 190 files, 129019839 bytes.
- Completion/dependency-event tests: **19 passed**.
- Broader KFD regressions: **1409 passed, zero failures**, explicitly excluding
  **320 construction-primary tests**. This is not a full KFD-suite rerun.
- Full runtime suite: **1828 passed, zero failures, 30 hardware ignores**.
- **124 doctests**, strict all-feature/all-target Clippy, no-default-feature
  production checks, workspace/included-source formatting and diff checks pass.

The inherited controller and classifier remain SHA-authenticated and unchanged.
The four-group synthetic calibration runs before the inherited source bracket
and is separately recorded; it is not an eighteenth owned stage.

The first signed campaign is retained as failed evidence: the no-decrement
mutant generated both a logical failure and a cast-range recommendation, correctly
rejected by the classifier. An explicit positive-capacity proof assertion fixes
that diagnostic ambiguity without changing production or weakening the classifier.
Development proof failures are also retained, not counted as negative controls.

## Boundaries And Next Work

Trusted boundaries include Rust/compiler behavior, pinned Verus/vstd/Z3 and the
standard-library contracts. The proof does not establish HashMap internals,
allocator behavior, arbitrary iterator callback termination, machine-code
correspondence, or performance.

Still unproved here: `ExactSizeIterator::len` selection, fallible reservation,
the production mapped-iterator laws and slot/min projections, original roster
validation, batch commit/custody, global ledger-to-pin cardinality and outer
rollback composition. CPU tests cover selected compositions, not universal proof.

Next is the actual event-release then bound-cancellation chain, preserving exact
terminal failure prefixes, followed by dispatch cancellation and native callback
ledger frames. Full record-to-rollback conservation also needs recording freshness
and pin-increment refinement. Native target scheduling, physical/semantic
settlement, lane-local storage, protected Worker execution, overlap and matched
HIP/HSA performance remain open. Accepted lane checkpoints are unchanged.

No MI300X work or new GPU/performance result was produced. Three old, fully
archived local raw directories were removed after a read-only content/ownership
audit, reclaiming about 511 MiB; tracked archives and unrelated files remain.

## Replay

`raw.tar.gz` contains logs, tool/source/test-ELF hashes, development and failed
campaign evidence, the complete final signed campaign, exact commands and a
fresh open #182 snapshot. ELFs are not archived; no hermetic-build claim is made.
`SHA256SUMS` seals this README and archive; the archive has its own file manifest.
A fresh restore matched the raw tree exactly and passed all 188 file hashes.

From a clean signed source checkout with the pinned tool closure installed:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-completion-release-pin-budget.py \
  --output /absolute/new/owned/output \
  --verus /absolute/pinned/verus-x86-linux/verus
```
