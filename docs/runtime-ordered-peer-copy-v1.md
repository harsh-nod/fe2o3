# Ordered Peer Copy V1

## Contract

`RuntimeContextV1::peer_copy_segments` is an optional local SPI extension, not a
Worker wire operation. It accepts one source allocation, one destination
allocation, and 1 through 4096 ordered descriptors. Each descriptor contains
source-relative offset, destination-relative offset, and nonzero length. The
source and destination bounding envelopes may have different lengths.

The facade validates the complete list before backend entry. The KFD adapter
also validates the complete list against allocation extents, endpoint access,
device/stream binding and the native linear packet limit. No large-segment
splitting is implicit. Order and duplicates are preserved. Destination overlaps
use serial last-writer-wins semantics; untouched bytes must remain unchanged.
This is not an atomic transaction: failure can leave an applied prefix.

4096 is a host metadata policy bound, not the native 63-ticket ring bound. One
three-u64 descriptor vector has at most 96 KiB of payload. The facade and backend
temporarily hold separate snapshots during admission; the backend retains one.
Admission and descriptor execution are linear in descriptor count. Existing
backend custody validation additionally examines logical ownership indexes.

One logical submission retains one source-envelope read lease and one
whole-destination journal writer. Its domain-separated identity includes the
branded stream, both branded envelopes, and the entire ordered descriptor list.
An intermediate native completion does not settle the writer, deliver completion
callbacks, or wake dependent submissions. Cancellation is possible only before
the first publication attempt, never between completed segments.

## Owned Async Calls

`RuntimeAsyncProgressHandleV1::peer_copy_segments` and
`peer_copy_segments_tracked` enqueue the same local SPI through the existing
owner-engine operation registry. They require `RuntimePeerCopySegmentsBackendV1`;
they do not add Worker wire encoding, graph nodes, native routing or authority.
The typed result represents the entire ordered list, not an individual segment.

Both methods consume descriptor and dependency vectors, preserve descriptor
order/duplicates and discard spare vector capacity. One shared-budget permit
charges the combined boxed-slice payload until owner-thread submission or actual
disposal. This excludes record/allocator overhead and Context/backend/GPU custody.
At most 4096 three-u64 descriptors and 256 event IDs can be retained. Invalid or
duplicate dependency lists and excessive descriptor counts fail before enqueue;
empty lists, ranges and live-resource identities remain Context checks on the
owner thread. Successful Context admission takes its own snapshot before the
async payload charge is released.

Dropping a future abandons observation only: accepted owner work still progresses.
Tracked cancellation can stop an unsubmitted operation, with credit returned
when the owner disposes of its request. Timeouts do not cancel or authorize
resource release. Once submitted, existing whole-list settlement and native
custody rules apply. Ordinary owner polling does not imply eligibility for the
single-wait diagnostic capture mode below.

With the version journal enabled, an input allocation with a pending writer
still refuses a new reader with `ContextReserved`, even when the request names
that producer's event. This happens before backend submission. The owner reports
the error without retrying; it does not defer the request until the writer
settles. Explicitly progressing the producer and submitting a fresh consumer
with its completed event is supported. Pending versioned dataflow remains a gap,
not a reason to disable the journal.

Focused CPU regressions cover both methods' snapshot bounds/compaction, combined
credit and rejection refunds, owner-side validation precedence, cancellation,
observer-drop progress, descriptor order/duplicates and background owner-thread
completion. These mock-backed tests are not a new native or formal-refinement
qualification.

## Native Execution

The gfx942 XGMI adapter uses existing native batch-scope APIs. It submits one
segment, waits for that exact singleton ticket, recovers the same mapping pair,
then proceeds to the next descriptor. At most one native ticket is outstanding.
An invocation performs full opening and closing currentness checks; each native
submit and wait retains the existing operational checks inside that scope.
Neither whole-host topology discovery nor endpoint observations are skipped.

The scalar `Gfx942NativeXgmiSdmaBatchV1::wait_until` accepts the original absolute
deadline and returns one completed mapping pair inline. Ordered sequences use
this path instead of wrapping each ticket in the general batch waiter. It removes
four temporary ticket/slot/readiness/completion vectors per segment, while
retaining the same publication-currentness driver and completion-error custody.
This is roster-allocation removal, not an allocation-free currentness path or a
measured latency gain. Existing relative waits retain their resolution point and
one-observation-at-expiry behavior.

`poll` performs at most one segment's publication/completion step. `flush`
observes at most eight eligible descriptors per invocation, advancing only after
the predecessor has completed. An outstanding ticket at entry counts toward
this budget. Every wait receives the same already-expired absolute deadline, so
it observes readiness without waiting for GPU completion. Pending, failure, list
completion, or budget exhaustion stops the loop. Full opening/closing currentness
still brackets each invocation, and operational checks remain on every submit and
wait. This is a bounded ready-prefix scan, not multi-packet publication or a
wall-clock bound: host filesystem observations can block. First-segment
publication is the logical operation's publication point. A failed step is
reported as a quiescent flush error, not success. No background thread is added
to the backend; the existing owner engine can drive this progress.

`wait` uses one absolute deadline, including preparation, for the entire call.
One initial publication/completion scan is allowed at expiry, matching the
existing aggregate progress convention. After a successful segment, expiry
prevents publication of the next descriptor. Mandatory closing checks can finish
after the deadline. Pending returns only after closing, retaining either an exact
ticket or the mapped pair and completed-prefix cursor. Retrying an outstanding
ticket waits for it rather than republishing the segment.

Success requires the complete prefix, successful closing currentness, reusable
mapping restoration, and only then logical settlement. Recoverable submission
failure restores mappings and settles a failed logical result, even when an
earlier prefix exists. Retained publication errors, failed currentness, or
ambiguous completion quarantine custody. Unwinding while native authority is
moved is fail-stop, following the existing aggregate adapter.

The initial implementation reserves its directional logical submission domain:
it requires no active work in that direction, and ordinary copies in that
direction are rejected until it settles. The opposite direction remains
independent subject to existing allocation/dependency ownership checks. Ordinary
aggregate APIs reject sequence handles. Existing diagnostic capture formats are
invalidated for sequence progress rather than populated with misleading records.

## Verification Scope

Production admission and cursor transitions import the pure R74 Rust model.
Tests explore all reachable abstract states for counts 1 through 4, boundary
counts including 63/64/65/4096, every relevant range/overflow rejection, and
scripted publication/wait/close faults at each position in a four-segment list.
Facade tests cover snapshot immutability, unequal envelopes, duplicates, ordered
overlaps, canaries, source immutability, one writer/reader, event settlement,
callbacks, prepublication cancellation, and partial-effect failure.

Ready-prefix tests cover counts through 4096, the eight-observation boundary,
retained-ticket accounting, repeated pending observations, exact deadline/pair
forwarding, and submit/wait/close failures at each prefix position. Source-wiring
checks distinguish poll, flush, and wait call sites. Diagnostic equivalence tests
exercise the same flush sequence, while capture enrollment rejects flush even
when it completes the whole list. Journal regressions require pending-producer
reader refusal before backend entry and successful admission after settlement.

The independent Verus artifact has ten parameterized obligations for arbitrary
accepted counts and five expected-negative controls. It proves abstract cursor
ordering, irreversible publication history, cancellation exclusion, no reopening
or publication after recovery, and successful-close gating under modeled observations. Its ghost
successful-close flag records the observation supplied by the adapter. Invalid
Rust transitions return `None`; the specification stutters on invalid actions.
Rust/Verus correspondence is reviewed, not an executable refinement proof.
The ready-prefix loop uses existing modeled single-ticket transitions; these
obligations do not prove the eight-observation budget or nonwaiting Rust calls.

This does not prove ticket authenticity, actual currentness observations, DMA
effects, Rust/native refinement, hardware liveness, or performance. The native
smoke example checks both directions, 1/65/4096 segments, poll/flush continuation,
cancellation, byte-exact canaries and explicit native shutdown. It must run only
after fresh shared-host identity/activity admission.

## CPU Qualification

The following counts describe the initial ordered-copy qualification. The
[owner/ready-prefix packet](evidence/dev-xgmi-owner-ready-flush-mi300x-2026-09-21/README.md)
records the later signed-source GNU/musl run: 1221 runtime tests passed with 20
hardware-specific tests ignored per target, plus three owner-example tests.

With pinned nightly `2026-04-03`, all features, optimization level 1, debug
assertions and overflow checks enabled, the GNU and musl runtime library suites
each passed 1190 tests, with 20 ignored hardware-specific tests. The musl smoke
example's two CPU tests also passed. The no-default-features context/XGMI subset
passed 195 tests. Warnings-denied all-features Clippy passed for the runtime
library, tests, and smoke example.

The full Rust model library suite passed 799 tests with two ignored, including
the three focused R74 tests. The independent R74 Verus artifact
verified ten obligations, and each of its five expected-negative controls failed
its named obligation. The registered 691-file negative-quality audit passed.
The complete authenticated `crates/fe2o3-runtime-model/verus/verify-verus.sh`
run subsequently passed with pinned Verus `0.2026.08.09.92f466f`, all 691
expected-negative controls, the executable journal/reader mutation checks,
and final source/toolchain integrity checks. Its terminal transcript matched
SHA-256 `cc1a7bea894a1905cd90a318744d1a6c0fc39f8fb14521720993804020ab3afb`.
This establishes the registered model obligations within the scope above,
not executable Rust/native refinement.

## Native Qualification

Signed implementation `a1301779d5536723cbbb5693823ce7f652d91129` passed all
eight smoke cases on freshly admitted MI300X GPUs 1 and 2. Both directions
passed counts 1/65/4096 with whole-operation wait and a four-segment poll/flush
continuation. Explicit shutdown, settled/delayed endpoint checks, and owned
remote cleanup passed. The [source-bound correctness packet](evidence/dev-ordered-peer-copy-mi300x-2026-09-20/README.md)
contains raw receipts, a byte-identical post-trial build and CPU replay, and an
offline verifier. It is not native fault injection or performance evidence.

### Owner-Engine Qualification Fixture

`gfx942-runtime-xgmi-segments-owner-smoke` keeps the version journal enabled.
It checks that a real pending producer-to-consumer dataflow request is refused
without consumer submission or destination changes. It then explicitly progresses
that producer and tests the supported completed-event handoff through the owner
engine: pre-submission cancellation, timeout identity, dropped-observer progress,
both copy directions, ordered overlaps/duplicates, canaries, unchanged sources,
released metadata credit, and complete explicit shutdown. The receipt records
`pending_dataflow=refused_before_submission`, not pending-dataflow support.
`ordered_lists=2` counts successful lists; refused and cancelled requests are
separate attempts.

`benchmarks/runtime_gfx942/xgmi_segments_owner_campaign.py` builds this fixture
from a clean signed checkpoint, performs fresh endpoint admission, records one
correctness trial, collects byte-exact evidence and cleans only its private
remote tree. Settled and delayed endpoint checks remain mandatory. This fixture
is not a performance comparison, an exclusive reservation, fault injection, or
formal refinement. Native qualification is separate from its CPU plan tests.
The runner's `native_execution` flag means the whole campaign qualified; a
false value does not imply the workload never ran before a postflight failure.

Signed source `c44625e12a1567204e1cb186b9f4da867244f686` passed this owner
fixture on freshly admitted GPUs 5 and 6. All six endpoint observations,
byte-exact collection, and owned file/process cleanup passed. The
[source-bound packet](evidence/dev-xgmi-owner-ready-flush-mi300x-2026-09-21/README.md)
includes raw CPU/native receipts, offline replay, and mutation tests. It confirms
the supported completed-event path and the pending-input refusal; it does not
close pending dataflow, performance parity or executable refinement.

## Performance Qualification Plan

The next comparison must use one retained source/destination allocation pair
and one stream for each backend. Descriptor count is independent of existing
benchmark queue depth; keep depth one. Start with useful list payloads of
64 KiB and 2 MiB, each split into 1/65/256/4096 descriptors. Precompute ragged
lengths, permuted disjoint destination ranges, gaps and outer canaries. Give
every prime, warmup and sample its own poisoned destination band within the
same retained allocation. Final whole-allocation validation can then detect
omitted timed lists, unlike repeated overwrites of a primed range.

Primary baselines should enqueue the full list on one HIP stream, or use an
HSA predecessor-signal chain, then observe final completion. fe2o3 submits one
logical sequence and waits for the whole operation, including descriptor
validation, journal work, and full closing currentness. A host-wait-per-segment
HIP/HSA mode may be reported separately, but must not be the sole comparison.

Use one absolute deadline per list, identical plans and device identities,
matched warmup/sample counts, raw samples, and fresh shared-host admission.
Report whole-list p50/p95 latency and useful bytes per second. List time divided
by descriptor count is amortized time per segment, not measured individual
segment latency. HIP/HSA do not provide fe2o3's full-currentness contract; the
comparison must disclose that difference. No host reads or writes of the source
or destination allocation occur between timed lists; initialization, readback,
and completion-record cleanup are outside the
declared timed interval. A separate untimed overlap case checks last-writer
ordering. The existing generic benchmark runner is not evidence that these
workloads are matched.

### Benchmark Implementation

`gfx942-runtime-xgmi-segments-benchmark` implements this separate workload.
The C++ HIP/HSA comparators accept `--ordered-segments <count>` with depth one;
the previous default and `--persistent-hot` modes are unchanged. All three
retain pairs in both directions, execute prime/warmup/sample lists into distinct
poisoned bands, and defer printing until full byte validation and explicit
teardown. The 60-second per-list deadline includes admission/enqueue through
observed completion. HSA signal reset and fe2o3 submission release are outside
timing. A partial enqueue or ambiguous completion never triggers ordinary HSA
resource teardown. Its negative predecessor can be detected only by tail timeout,
not necessarily by early error observation.

The standalone parser requires the exact complete band/direction roster and
caller-supplied workload/device identities. It computes sample-only median p50
and nearest-rank p95 separately by direction. Useful bytes divided by median
nanoseconds is decimal GB/s of useful host-observed throughput, not physical link
bandwidth. The disjoint performance plan cannot prove ordering; the separate
overlap correctness tests remain necessary.

CPU qualification passed three Rust example tests, warnings-denied Clippy,
normal and UBSan C++ plan/custody tests, and Rust/C++ differential checks for all
eight payload/count geometries. Parser adversarial tests reject partial rosters,
incorrect controls, malformed durations and missing completion/teardown claims.
These checks do not establish native performance or formal executable refinement.

`benchmarks/runtime_gfx942/xgmi_peer_segments_campaign.py` is a bounded first
campaign: 64 KiB useful data, 65 descriptors, two warmups and ten samples per
direction, in KFD/HSA/HIP/HIP/HSA/KFD order. It requires a clean signed checkpoint,
records a local musl release build, builds C++ comparators on the remote host,
checks fresh physical endpoint identity/activity before every trial, performs
settled and delayed postflight checks, collects byte-exact raw receipts, and
removes only its private remote tree after successful collection. It is not an
exclusive reservation or general performance acceptance. A collection failure
retains the owned path for recovery instead of deleting its evidence.

### First Native Comparison

The [source-bound MI300X packet](evidence/dev-xgmi-ordered-segments-mi300x-2026-09-20/README.md)
passed all six trials, 120 timed samples, 36 endpoint observations, explicit
teardown and owned cleanup. For 64 KiB through 65 descriptors, the ranges of
per-trial/per-direction medians were 25.279-25.433 ms for KFD, 0.922-0.924 ms
for HSA, and 0.344-0.350 ms for HIP. These are whole-list host latencies, not
confidence intervals or link bandwidth. KFD is not at performance parity on
this workload; the full-currentness and serial publication costs remain in scope.

### Scalar-Wait Qualification

The [scalar-wait source packet](evidence/dev-xgmi-ordered-segments-scalar-wait-mi300x-2026-09-20/README.md)
reran the same six-trial geometry after removing singleton wait rosters. All
120 samples, 36 endpoint observations, byte validation, explicit teardown, and
owned cleanup passed. Per-trial/per-direction medians ranged from 25.240 to
25.348 ms for KFD, 0.923 to 0.925 ms for HSA, and 0.344 to 0.352 ms for HIP.
The KFD range overlaps the prior source's range; this separate historical run
does not establish a speedup or attribute time to vector allocation. KFD remains
far from performance parity. Nine scalar-wait CPU tests and six ordered-runtime
tests qualify deadline forwarding, completion handling, and ownership behavior;
they are not native fault injection or executable formal refinement.

Replay that immutable packet from archive commit
`67227bfb404a3e9518a91e446181b729ec0642b6`. The subsequent retirement
source-wiring test correction changes its selected test-source inventory, not
the runtime implementation or the measured release ELF.

## Ordered Host Attribution

The opt-in `hardware-diagnostic` API now provides bounded, success-only host
phase attribution through `enable_xgmi_segments_diagnostics_v1` and
`finish_xgmi_segments_diagnostics_v1`. Enable it before resource creation and
extract only after successful explicit logical/native shutdown. It excludes
the existing single-copy and aggregate diagnostic modes.

One fresh ordered list must complete within one backend wait (or drain) call,
in increasing backend submission-ID order. Poll/flush, ordinary or aggregate
progress, accepted cancellation, pending/retry, failure, or incomplete teardown
make the capture unavailable without changing execution results. Records are
preallocated and contain the full descriptor count and total useful bytes.

The shared execution loop records admission, preparation, opening currentness,
summed submission, summed wait, closing currentness, and settlement intervals.
Opening and closing include nested native currentness intervals. Nested times
overlap their outer phase and must not be added to it. Enrollment/record append,
facade enqueue/settlement, and submission release are outside backend timing.
The ordinary const-disabled timer reads no clocks and allocates no records.
Enabled timing can consume deadline budget; it does not extend deadlines.

The benchmark flag `--diagnose-ordered-segments` emits a separate strict schema
before the unchanged ordinary receipt, after validating the entire joined
roster and teardown. CPU tests cover disabled clocks, repeated span accounting,
timing overflow/containment, invalidation, extraction, and scripted equivalence
of operation order, custody, retry deadlines, native-error handling and generic
panic payloads. Native authority-crossing unwinds remain abort-only. These are
implementation tests, not executable formal refinement or GPU timing evidence.

The existing campaign's `--host-attribution` switch selects KFD-only
off/on/on/off trials with one feature-enabled release ELF and the fixed
64 KiB/65-descriptor geometry. Its mode-bound parser retains the ordinary
receipt checks and rejects missing/reordered identities, incorrect call counts,
noncanonical/overflowing durations, and inconsistent phase/nested totals. Each
trial retains fresh admission and settled/delayed postflight checks. Idle
observations are not a reservation, and this protocol does not establish
speedup, causal instrumentation overhead, HIP/HSA parity or performance acceptance.
Signed source `a7b8602dce0c5d38ae1425f6544392f03f55341b` subsequently passed
all four diagnostic trials on freshly admitted MI300X GPUs 5 and 6. The
[source-bound attribution packet](evidence/dev-xgmi-ordered-host-attribution-mi300x-2026-09-21/README.md)
contains 52 diagnostic observations, 80 ordinary timed samples, all 24 passing
endpoint observations, explicit cleanup and an offline replay with negative
controls. Across 40 instrumented timed samples, mean backend host time was
25.68 ms: 58.27% in opening/closing currentness and 41.65% in repeated
submission/wait phases. These phases include host checks, not isolated DMA
execution. This KFD-only attribution is neither a matched HIP/HSA comparison nor
a measured gain from the topology allocation change. Native owner-engine wrapper
qualification remains separate from this caller-driven Context workload.

## Remaining Work

- Expand matched useful-segment testing to the remaining seven payload/count
  geometries, and attribute the 65-segment cost before larger optimizations. No
  speedup or parity claim follows from the completed comparisons.
- Graph sequence nodes and explicit negotiated Worker transport support.
- Versioned pending-producer input handoff. A host-deferred owner driver and
  native admission of future readers are distinct designs; neither is supplied
  by the completed-event fixture or ready-prefix flush change.
- More permissive scheduling-domain coexistence and native multi-packet
  publication are separate optimizations. This serial version still publishes
  and waits once per descriptor and does not claim optimal packet throughput.

Pending-input admission requires a distinct producer-bound read reservation,
not weaker stable-reader validation. It must retain the exact writer incarnation,
attempt epoch, event/submission relationship and promised successful lineage;
exclude intervening writes/reuse; and remain releasable through producer failure,
unknown completion and consumer cancellation. Consumer-first observation must
reconcile the retained producer through its own backend result and ordinary
journal settlement, never infer producer success from consumer success. Existing
XGMI backend dependency custody is useful but does not supply these journal
invariants, producer progress registration, or GPU-side dependency packets.
