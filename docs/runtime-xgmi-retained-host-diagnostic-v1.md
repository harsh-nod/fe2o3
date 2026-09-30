# Retained XGMI Host Diagnostic V1

This opt-in diagnostic attributes **host-observed** costs of the ordinary
retained-pair native API. It is not a device timestamp, completion authority,
formal refinement, runtime-facade benchmark, or performance acceptance result.
The accepted native benchmark producers and their parsers remain unchanged.

The candidate starts at `1c589082e8c76410ab3b5b2683468ca6a1932519`.
No previously built executable or solver result qualifies the changed library.

## Observation Boundary

The `hardware-diagnostic` methods preserve the retained environment assumption,
opening and closing paired operational checks, packet bytes, one doorbell per
batch, ticket validation, relative timeout resolution, observe-before-deadline
behavior, pending custody, terminal quarantine, and adaptive wait policy.
Ordinary calls instantiate compile-time-disabled observers. Diagnostic results
are returned only after the same successful custody decision; an error, terminal
result, or unwind publishes no diagnostic result.

Submission reports opening checks, preparation, native publication, closing
checks, and total call time. Wait reports opening checks, roster validation,
scan, retirement, closing checks, and total call time. The scan records rounds,
**all completion-slot observations including pending values**, spin/yield/sleep
counts, and requested rather than actual sleep duration. First/all completion
offsets are the host's observations relative to scan entry, not when the GPU
completed work. Ready slots are still skipped on later scans.

Best-effort thread CPU time and voluntary/involuntary context-switch deltas use
the existing Linux helper. Unavailable/invalid values are explicit, never zero.
Counter overflow invalidates the complete counter group without granting
completion or causing an operational error.

Profiling adds clock reads, CPU observations and counters. These perturb host
timing, readiness observations, scheduling, and possibly the chosen adaptive
wait stage. Device progress overlaps host checks and sleeps. In particular,
wall time minus CPU time minus requested sleep is not avoidable copy latency.
Instrumented numbers must not replace ordinary benchmark numbers.

## Producer And Parser

The separate example is `kfd-xgmi-retained-host-diagnostic`, requiring
`hardware-diagnostic`. Its closed controls are one MiB per copy, depths 1/16/32,
one prime, two warmups and ten samples per direction. The two unique IDs and
the explicit reviewed-environment flag are mandatory. Source/destination
allocation flags, guarded extents, setup order, per-slot patterns, direction
order and final source/destination readback match the ordinary retained series.
Scope entry and finish remain outside samples. Output is delayed until both
directions pass readback/canaries and explicit teardown.

The output schema is `fe2o3.xgmi-retained-host-diagnostic.v1`: one summary and
twenty ordered sample rows. `xgmi_retained_host_diagnostic.py` validates exact
keys, independently supplied endpoint/GPU/engine identity, closed controls,
statuses, counter bounds and nested host-time ranges. It has no performance
acceptance field. The ordinary benchmark parser rejects this stream.

## Required Qualification

Source/light controls are not Rust compilation or native qualification. Before
any remote job, freeze and review the exact candidate, then run bounded fresh
qualification with original owned process handles and fresh source/tool/raw
closure:

1. Pinned formatter checks on changed blocks and new Rust files, preserving
   original benchmark, policy, packet and shared proof-body bytes.
2. Fresh `fe2o3-kfd` no-default-feature and all-feature scoped builds, including
   ordinary and diagnostic examples with their required features. Build output
   goes into a new owned target directory, never an accepted evidence tree.
3. Fresh CPU tests for ordinary/profiled ready, timeout, malformed completion,
   duplicate tickets, unwind and terminal custody; disabled observer/pause
   controls; counter overflow; strict parser negatives. Run the full affected
   KFD suite with bounded concurrency and deadline when practical. A timeout or
   missing test remains incomplete, not accepted from an old ELF.
4. Review the changed whole-source proof closures before any separately
   authorized guard refresh/solver run. No guard is silently re-pinned here.
5. Only after review, build on MI300X under the existing shared build lock,
   using the existing private resource limits, independent host/API admission,
   fixed query settling, exactly-zero idle observations and owned cleanup.

The smallest proposed matched diagnostic campaign has 24 invocations:
for each depth 1, 16, 32 run ordinary KFD, profiled KFD, HSA, HIP, HIP, HSA,
profiled KFD, ordinary KFD. This preserves the existing 18 ordinary invocations
as a subsequence. Keep the same independently admitted physical pair and all
existing thresholds; no retry-until-idle or engine-matching claim. Bind and
archive each diagnostic record separately; require its own exact parser and
command/source/binary joins. No runner execution is authorized by this document.

## Proof Boundaries

Shared retained post-catch operation macros and policy bytes are unchanged.
The new observer wrappers and executable wait body still need fresh native
tests; this change does not extend a Verus theorem to diagnostic timing.
The whole-KFD closures in `check-retained-pair-post-catch.py` and
`check-retained-pair-owned-storage.py`, the runtime/accounting/KFD closure in
`check-retained-credit-dispatch.py`, and the build-file closure in
`check-request-charge.py` cover changed paths. Their existing guard constants
are preserved and must not be presented as accepting this candidate.
Campaign-wide source inventories also change, including the portable retained
credit campaign and native-series transport. Historical receipts remain bound
to their original source commits.
