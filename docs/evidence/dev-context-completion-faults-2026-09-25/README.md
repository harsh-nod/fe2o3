# Context Completion Fault Development

## Scope

Source implementation: signed `dbd5dfb4c73c3965e419725137e85e394fd2b958`.
Five new runtime tests exercise 40 scenarios through the actual Context adapters:

- 36 injected scenarios: stable-reader release, producer-reader release and
  writer settlement, each with a pre-effect error, pre-effect panic or
  post-effect panic, across Success, confirmed cancellation/NoEffect, backend
  Failed/Unknown and backend Quiescent/Unknown.
- Three early-validation corruptions: stable reference, producer reference and
  writer marker. All reject before the first journal boundary or backend poll.
- One late writer-marker corruption after real input release. This deliberately
  calls the two adapters separately, not a concurrently mutated Context call.

The matrix checks actual Context roots and record markers, exact reader counts
per allocation, writer state and allocation lineage, terminal/quiescent state,
producer dependency retains, callback retention and the original quiescent
backend diagnostic. It checks the whole credit-usage vector: six allocations
remain charged, the four exact bound allocations are quarantined and the two
unrelated allocations retain their credits. A valid quarantine does not poison
the accounting invariant. Poll/cancel retry and cleanup cannot dispose resources
or invoke callbacks after the context is sealed.

Post-effect panics are distinguished from pre-effect failure. The committed
journal transition is not rolled back; the corresponding Context root/marker
remains for fail-stop diagnosis and quarantine. In particular, a writer may be
settled and freed in the journal while its Context root remains. The tests do
not relabel that stale root as an Unknown journal writer.

All hook declarations, storage and six call sites are `cfg(test)`. Production
prechecks, release/settlement bodies, dependency release and status publication
are unchanged. No new unsafe code or public API is introduced.

## Evidence Boundary

This is CPU development evidence, not new Verus, native or performance
qualification. The errors are explicitly injected at the Context adapter
boundary; they do not prove the journal's internal error atomicity. After-effect
error injection is forbidden because it would misrepresent the journal's error
contract. Panic injection brackets complete journal effects, not every possible
intermediate instruction inside a journal operation.

The receipt runner checks named tests and exact suite summaries, records
commands/environment/stdout/stderr, brackets 5,947 tracked workspace source
inputs and checks each command's process-group absence. It does not authenticate
the full compiler/dependency closure, observe native GPU quiescence or establish
global process absence. Four runner tests cover nonzero commands, missing tests,
managed signal cleanup and refusal of optimized Python. Build-cache reuse is
allowed; these are correctness checks, not timing measurements.

Context map/journal executable correspondence, producer-first reconciliation
proofs, generated execution, high-depth native graphs, aggregate residency,
physical overlap and matched performance remain open. A1/A2, Native R125,
Admission R118B C1/C2/C3 and Resources R116/V3 acceptance are unchanged.

## Development Corrections

Manual development checks first passed the 36-scenario matrix. Adding the
identity and accounting assertions exposed test-code mistakes: writer references
use `slot/key`, not an `incarnation` field; the public credit query returns a
`Result<Option<RuntimeResourceCreditUsageV1>>`; and ordinary quarantine preserves
the account's unpoisoned invariant. The corrected five-test focused run passed.
Those manual attempts are not the retained source-bound CPU campaign.

Review corrected the receipt runner's split doctest summaries (4 and 42),
optimized-Python refusal and interruption cleanup. The original `cpu/` run uses
the runner from `dbd5dfb4`; the successor runner uses passive signal flags so a
second managed signal cannot unwind its owned-group cleanup.

## Reproduction

From the repository root, with a clean committed source tree and an existing,
task-owned Cargo target directory:

```sh
python3 -I -B docs/evidence/dev-context-completion-faults-2026-09-25/test_run.py
python3 -I -B docs/evidence/dev-context-completion-faults-2026-09-25/run.py \
  --output /path/to/new-receipts --target /path/to/owned-target
```

The runner does not delete the supplied target directory. Only its owner should
remove it after all checks have terminated and the receipts have been retained.
