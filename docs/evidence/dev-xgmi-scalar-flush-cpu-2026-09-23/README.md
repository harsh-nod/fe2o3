# Nonwaiting Scalar XGMI Flush

Development CPU checkpoint. This is not formal refinement, native GPU
qualification, a performance comparison, or A1/A2 acceptance.

## Correction

The scalar XGMI flush implementation previously published a bounded prefix and
synchronously drained it before publishing the next. This contradicted the
`RuntimeFlushBackendV1` nonwaiting contract and could wait without a deadline.

Flush now admits the complete ready directional set only when it fits the
63-ticket publication window. A larger set rejects before entering publication;
a nonempty set also rejects while the window is busy. Empty flush succeeds
without observing completion. An admitted flush invokes one publication call
and leaves its tickets owned for subsequent observation. The prefix-drain state
and its unbounded backoff helper are removed. Native publication error classes
and recovered-prepublication failure handling are preserved.

Three added tests cover oversized/busy/empty admission, all accepted window
sizes, exact publication-call counts, unchanged error classes and the absence
of completion/backoff paths in the scalar facade. Existing diagnostic source
guards track the new gate and following method. These tests exercise the shared
publication gate and source wiring, not every lower-driver outcome through the
full native facade. Ordered-segment execution is unchanged.

## CPU Results

Pinned nightly `2026-04-03`, all features, test optimization level 1, debug
assertions enabled, incremental compilation disabled and two build jobs:

- GNU runtime library: 1,224 passed, zero failed, 20 hardware-only ignores.
- Musl runtime library: 1,224 passed, zero failed, 20 hardware-only ignores.
- Runtime doctests: 46 passed across the two reported groups.
- Formatting and all-target warnings-denied Clippy pass.
- All seven recorded commands finish with status zero and process-group absence.
- All 3,803 captured source/configuration/fixture hashes and the runner hash
  match before and after the run.

The exact commands, tool versions, outcomes and raw logs are in `records`.
`runner.py` is the archived development runner; it retains the original absolute
workspace and output paths, not a portable qualification interface. Receipts
are local observations, not independent execution or hermetic-build attestation.
The committed implementation files match the recorded source hashes.

The first attempt is retained at
`/home/harsh/.codex-tmp/fe2o3-xgmi-flush-20260923-TNYmEfMy/cpu-1`.
It failed an obsolete diagnostic source anchor, reported the unused backoff
helper, and rejected its source bracket because formatting had not completed
before input capture. It is not accepted or substituted into this fresh run.
The corrected run began only after a terminal successful formatting check.

## Remaining Work

Bounded progress through a larger scalar backlog requires a separate operation;
restoring blocking behavior inside flush is not an implementation of that API.
Pending-producer reads, exact dependency ownership, panic-safe retention,
producer-first reconciliation and native chain qualification remain open.
No MI300X operation or HIP/HSA benchmark ran for this checkpoint. The accepted
Native R125, Admission R118B and Resources R116/V3 lane checkpoints are unchanged.
