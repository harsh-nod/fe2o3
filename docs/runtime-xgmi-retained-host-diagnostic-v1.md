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
The observer wrappers and executable wait body require separate fresh native
tests; the completed campaign below supplies that bounded qualification, not
an extension of a Verus theorem to diagnostic timing.
The whole-KFD closures in `check-retained-pair-post-catch.py` and
`check-retained-pair-owned-storage.py`, the runtime/accounting/KFD closure in
`check-retained-credit-dispatch.py`, and the build-file closure in
`check-request-charge.py` cover changed paths. The original signed candidate
`d6b1906d0` preserves their old constants. Integration at `877451ebf` refreshes
only those four reviewed inventories after checking that all executable proof
inputs and the 14/8/41/3 obligation and 29/21/25/21 mutation rosters are unchanged.
Their source controls pass; no solver was rerun and no theorem is extended to
the timing observer.
Campaign-wide source inventories also change, including the portable retained
credit campaign and native-series transport. Historical receipts remain bound
to their original source commits.

The [CPU qualification packet](evidence/dev-xgmi-retained-host-diagnostic-cpu-2026-09-30/README.md)
binds the signed implementation to a complete 1,830-test all-feature KFD run,
focused no-default tests, strict Clippy and example builds. Its explicit linked
record combines nineteen prior closed stages with seven fresh continuation
stages, not one continuous campaign. The no-default full suite and native GPU
examples were not run. Integration adds the portable helpers at `4ad64047b`;
all seventeen local CI commands pass, including 64 ordinary and 40 diagnostic
harness tests. These local controls do not supply a new formal or native result.

## Portable Native Campaign

The separate `xgmi_retained_host_diagnostic_campaign.py`,
`xgmi_retained_host_diagnostic_native.py`, and
`xgmi_retained_host_diagnostic_transport.py` entrypoints live beside the ordinary
series helpers in `benchmarks/runtime_gfx942`. The pure planner keeps the exact
18 ordinary specifications as a subsequence of 24 trials and requires distinct
receipts across the ordinary and profiled outputs. The native and transport
entrypoints require integration into a signed source closure; a copied external
entrypoint does not authorize execution.

The runner reuses the existing owned recorder, physical observer, API query
collectors, fixed two-second query settling, both postflight observations and
fresh-only census. The separate transport reuses selected-object packaging,
bootstrap receive, raw collection, archive validation, marker-bound cleanup,
the shared build lock and the pidfd/liveness-pipe resource monitor. It requires
both source snapshots to equal the signed selected-object inventory. Uncertain
terminal state or failed readback retains the exact owned remote directory.
A completely read-back rejected run remains rejected and may use the reviewed
cleanup. No existing acceptance threshold is relaxed.

The ordinary and profiled KFD examples are built together with
`--features live-validation,hardware-diagnostic`; Cargo records must contain
exactly `default`, `hardware-diagnostic`, and `live-validation`. HIP/HSA retain
the existing ROCm build flags. All four fresh ELF hashes and independently
collected loaded dependency identities for both KFD executables are joined at
local replay before cleanup. The diagnostic producer has no query-only mode:
the ordinary KFD query observes pair topology and admission, while the strict
diagnostic parser joins reported physical UIDs, GPU IDs and directional engines
to those inputs. This is not a query from the profiled executable.

Threshold-zero physical/API admission observations remain required before each
workload. Both settled and delayed postflights run after malformed successful
output before stopping. GPU availability is a nonexclusive point observation,
not a reservation. Existing 1200-second KFD build, 180-second workload,
5400-second resource monitor and 7200-second SSH bounds are unchanged.

The synthetic controls execute no GPU workloads or external child processes:

```sh
python3 -I -B benchmarks/runtime_gfx942/test_xgmi_retained_host_diagnostic_campaign.py
python3 -I -B benchmarks/runtime_gfx942/test_xgmi_retained_host_diagnostic_native.py
python3 -I -B benchmarks/runtime_gfx942/test_xgmi_retained_host_diagnostic_transport.py
```

Before any remote launch, review the signed source closure, additional ELF and
dependency inventory, exact 24-stage joins, malformed-output stop behavior and
archive/cleanup extension. The helper integration and synthetic controls do not
constitute native qualification, a performance result or a formal refinement.

## Completed Native Campaign

The [October 1 evidence packet](evidence/dev-xgmi-retained-host-diagnostic-native-2026-10-01/README.md)
records the exact 24-invocation plan at signed source `a26dbebb5`, on the
independently admitted MI300X pair with UIDs `ab83d2ffef0d3cdf` and
`d2e26fef80cf5c33` (SMI indices 1 and 2). All fourteen preparation, 327 native
and six transport stages close. Four fresh ELF identities, loader inventories,
physical/API joins, complete-output canaries, postflights and independent replay
pass. Collection precedes removal of the exact owned remote directory; a final
absence check passes. No foreign files or processes were removed.

[Ordinary timings and separate host distributions](evidence/dev-xgmi-retained-host-diagnostic-native-2026-10-01/analysis.md)
show KFD slower in every measured cell: 9.02-26.71% versus HSA and 2.66-31.34%
versus HIP, using the mean of two invocation p50 batch latencies. All 120
profiled samples are complete. Four operational checks total median
23.991-24.702 microseconds per sample. Depth 16 consistently records four
sleeps totaling 375 requested microseconds; depth 32 records five totaling
775 requested microseconds. This motivates a bounded wait-cadence experiment.
It does not establish that those requested sleep times are recoverable latency:
instrumentation perturbs observations, GPU progress overlaps the scan, and
actual sleep duration is unobserved.

The original 1,040-file collection archive, including all four ELFs, is retained
byte-identically once in the public archive. Agent/root readbacks agree; a
separate root publication audit checks all 79 outer members against originals
and the six closed signature/bundle stages. The signed-source bundle requires
the already-public `4ad64047b` parent. Full original-controller replay still
requires the retained source payload/checkout or a byte-exact verified
reconstruction, as the packet explicitly documents. Both failed packaging
preparations remain recorded and do not imply workload retries.

This is nonexclusive shared-host characterization of the native retained API,
not Context-facade qualification, matched comparator engine placement, a device
timeline, source-to-device refinement, an A7 threshold or HIP/HSA parity.
