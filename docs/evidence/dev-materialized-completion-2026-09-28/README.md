# Ordinary Indexed Completion

Development implementation and CPU qualification only. A1/A2, accepted lane
checkpoints, protected Worker application and HIP/HSA parity remain incomplete.

## Source

- Implementation commit: `10b516b9ba16b01f8497bf21e493a0330e973e2c`.
- Final qualified source: `ab81310394f01fbc32f7d08efcef4c21f908d976`.
- Qualified source tree: `5077efb80d9bc231b49270c5b3581f2d9bf117fe`.

Both commits have verified SSH signatures and sign-offs. All `qualified-*`
checks used the final unchanged runtime/protocol sources. The second commit
migrates an older constructed peer-gate fixture to explicit scripted retirement;
it does not relax production receipt requirements. The unrelated untracked
owner-inspection packet was preserved.

## Implementation

Ordinary frontier and pipeline completion now share an indexed transition.
Active remains in its owning slot through consuming poll and recycle calls.
An explicit consuming marker precedes each lower call; returned pending,
completed or retired state is stored before the outer lane loan closes. Recycle
retry requires an actually returned completed receipt, not an error category.
Terminal failure and unwind retain the exact attempted or returned state and
prevent further progress. The recycled observation is packet-count metadata,
not an independently generation-bound authority receipt.

Logical commit preflights custody, descriptor/writeback projection, result
capacity, distinct missing dirty extents and exact pipeline promotion identity.
It uses existing prepublication reservations and accepts matching dirty extents
even when their vector has no spare capacity. Settlement releases exact
allocation/module/deferred-dependency retains, installs the result and promotes
the next owner before profiling. Reporting failure preserves the committed prefix
and next indexed owner, rather than restoring a partially settled predecessor.
Pipeline retirement may be observed out of order; logical results remain ordered.

Publish-to-completion time is recorded at first Ready and preserved across
recycle retries. Recycle timing includes the outer lane operation and accumulates
attempts. Successful retirement resets the unused readback/detach fields.

Completion profiling previously allocated a temporary JSON buffer. Its closed
payload now uses a bounded 1024-byte stack buffer with unchanged canonical
encoding, event identity and byte accounting. Differential tests cover maximum
timing widths and sequence digit boundaries. Generic event-vector growth and
other event payloads are outside the allocation-free claim. This is not a native
latency or HIP/HSA speedup measurement.

## Qualification

Cargo checks ran sequentially with `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`.
Commands, UTC timestamps, full logs and exit statuses are archived.

| Final-source check | Result |
| --- | --- |
| Exact expanded peer-gate control | 1 passed |
| All-feature materialized selection, serial | 30 passed |
| Full all-feature runtime library, serial | 1785 passed, 3 failed, 28 ignored |
| KFD backend subset of that full runtime run | 888 passed, 28 ignored |
| Profiler protocol library and CLI | 32 + 6 passed; 0 doctests |
| Runtime + KFD + profiler protocol strict all-feature/all-target Clippy | Passed |
| Runtime no-default-features check | Passed |
| Workspace format check and source continuity | Passed |

Selections overlap and cannot be summed. The backend row is parsed from the
completed full-runtime log, not a separate successful standalone backend run.
Full KFD tests, runtime-model tests and runtime doctests were not rerun. No new
formal verification campaign or source-to-model correspondence proof was run.

The three unwaived failures occur at `authorized_execution.rs:1317` with
`InspectSocket(PermissionDenied)` / `Operation not permitted`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`.
No admission guard was bypassed and no failing test was skipped. The serial
qualification runner returns nonzero because the full-runtime gate failed.

Preliminary receipts retain the initial three-allocation failure, corrected
Clippy style error, and a standalone backend abort at the peer-gate control.
That old fixture used `PhysicallyRetired/None`, so the stronger production guard
correctly stopped its intended downstream attempt. The corrected fixture opts
into an explicit test-only retired marker. Missing-marker and absent-opt-in
controls still refuse; the valid scripted control reaches the exact missing-lane
error without creating any native receipt. A short-name `--exact` diagnostic
selected zero tests; it is not coverage. The subsequent fully qualified name
reproduced the failure before repair. Preliminary passes are not substituted for
the final-source results above.

## Evidence Scope

Eight new completion test groups comprise one container-invariant group and
seven runtime workflow groups. Workflows cover 48 frontier/pipeline fault cases
on primary/AUX lanes, all 48 unrepaired-Drop subprocesses, four read/write retry
controls, sixteen out-of-order/profile-prefix/existing-dirty controls, four
Published/Completed promotion controls, four pipeline corruption controls and
eighteen frontier/precommit corruption controls. These are case counts, not
independent registered tests or GPU launches.

Snapshots check exact recipe/backing identities, descriptor/writeback storage,
retains, reservations, stream maps, dirty metadata and event prefixes. Pipeline
successors originate from public Pending admissions but are installed as
constructed completion-only owners. They do not qualify public successor
publication, real native receipts or GPU cleanup. Scripted disposal is not
healthy native teardown evidence.

Selected public frontier recycle retries and successful profiled completion
count zero allocations; the initial successful-completion control counted three.
This applies to those scripted CPU workflows and the pre-reserved recorder,
not every completion, error, drain or generic profiling path. Pipeline commit
preflight scans configured slots each time, so this packet does not establish
linear high-depth draining or a native performance improvement.

Read-only swarm review found and corrected custody/preflight, timing-envelope,
script-consumption and coverage issues. Review is not formal refinement. The
existing abstract pipeline model does not prove these native-consuming adapter
transitions or their complete Context composition.

| Acceptance dimension | This packet |
| --- | --- |
| CPU/source | Qualified within the scope above; three full-suite failures remain |
| Formal correspondence | Not established |
| Live Linux/KFD | Not run; MI300X name resolution failed |
| Matched HIP/HSA performance | Not measured |

SSH could not resolve `sharkmi300x-1`; no remote files or jobs were created.
GitHub issue #182 could not be refreshed through the API. Source pushes to both
`origin` (harsh-nod) and `upstream` (powderluv) failed resolving `github.com`.
These failures are recorded without requesting permissions or escalation.

## Next Work

Ordered-successor submission still precedes pipeline-owner installation. Reserve
an indexed staged owner before native submission while retaining the predecessor;
root returned outcomes inside the lane callback and withdraw only a confirmed
retry, without creating logical-epoch holes. Failed publication retaining an
indexed successor must settle only the Pending FIFO/explicit-dependency handoff,
not duplicate Pending or release deferred ordering/resource custody.

Native coupling and fault qualification, Context composition, protected Worker,
formal correspondence, resource closure and matched HIP/HSA workloads remain
open. Accepted Native R125, Admission R118B and Resources R116/V3 checkpoints
and A0-A7 milestone statuses are unchanged.

`receipts.tar.xz` contains the frozen campaign and review/design notes. Verify it
with `sha256sum -c receipts.tar.xz.sha256` in this directory.
