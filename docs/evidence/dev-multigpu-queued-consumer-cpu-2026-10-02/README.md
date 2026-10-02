# Queued Multi-GPU Consumer CPU Checks

## Source and Scope

Base: `91bb347442e58e7883adb01bd526e22e31dcae65`, signed and pushed to both
repositories before this increment. The raw packet records source and guard
changes, exact source identities, commands, environments, test rosters, outputs
and executable hashes. Documentation is outside its source patch.

The existing typed `launch_producer_aware_v1` API can now admit a consumer of
an exact pending ordinary native compute/XGMI copy. The router retains immutable
launch data, producer identities, allocation bindings, module and stream without
taking child allocation custody. Child admission occurs only after the copy's
queue is retired, both original allocations are restored and child reservations
are released. This avoids a consumer holding the allocation its producer needs.

Indexed retains protect ordinary mutation and release paths. Public event
release does not drop producer custody. The delayed child handoff rechecks
capacity and shares the retained launch payload instead of charging a second
payload copy. Quiescent errors settle the consumer only when it never dispatched
or its exact child submission is quiescent; an earlier stream operation's error
cannot release a still-pending consumer. Original diagnostics survive. Terminal
errors and unwinds retain both router and child roots.

The Context completion adapter treats ordinary scalar copies as observation
leaves. It observes their actual result, not their consumer's success, and does
not assign directed ancestry or success-gate the copy's own dependencies.
Consumer-only drain may require repeated calls while bounded journal
reconciliation catches up. A failed never-dispatched consumer does not fabricate
an observation of its parent's logical result.

## Coverage Boundaries

Ten new Context test functions cover admission before/after physical producer
completion, seven completion ingress paths, exact writers and epochs, contained
reads and aliases, independent event release, resource/read retention, failure,
cancellation, and actual descended ordinary-copy poll faults. Mock poll faults
include Quiescent, Rejected, Terminal and panic without a separate peer observe.

Nine public router integration functions contain 28 cases. They compose the real
Context journal, native router and scripted peer transport, including pre/post
publication admission, deadline resumption, restoration order, original owners,
full byte comparisons, cancellation, six-stage error/unwind prefixes, malformed
requests, contained reads, aliases and control-only dependencies. An additional
real unflushed compute dependency keeps the consumer pending after actual child
admission. It does not fabricate GPU consumer completion. The downstream-event
test checks rejection and independent event/result retention. Scripted peer
bytes and stage traces are CPU evidence, not hardware payload movement.

Six lower router tests start at the restored-peer boundary and exercise indexed
retains, actual child pending admission/cancellation, capacity rechecking,
terminal/unwind custody, overflow, exact-target quiescence and diagnostic
retention. Fault tests call shared attribution/error helpers around real pending
child state; they do not inject faults through the entire native child flush.
These filters overlap with the public integration filter and are not additive
unique suite counts.

## Native Witness and Access

`gfx942-runtime-compute-xgmi-smoke --queued-consumer SOURCE_ID DESTINATION_ID` enables the journal,
queues the exact destination consumer before copy progress, releases its public
copy event, checks observers remain Pending and expired drain is rejected, then
drives only the consumer. The default explicit-copy-drain mode remains unchanged.
Both require four exact launches, sentinel replacement, unchanged source, one
native completion, 13 full-buffer readbacks and explicit cleanup. Three CLI
tests pass and the runnable witness builds; neither mode ran on GPUs here.

Read-only SSH attempts at `2026-10-02T02:26:39Z` and `02:51:48Z` exit 255 at
hostname resolution for `sharkmi300x-1`, before remote execution. The local
Tailscale map attempt also fails. No configuration/service changes, remote
scratch, workload or GPU-pair admission occurred. Both admission receipts are
preserved in the raw packet.

## Final Unit and Tooling Results

The accepted build, runtime and tooling packets are all `attempt-04`. Cargo's
structured compiler-artifact output selects the actual test executable. Its
SHA-256 is checked before and after the focused and unfiltered runs; each focused
test list must be nonempty. Runs use the pinned nightly, locked offline
dependencies, one build job and test thread, optimized tests with debug and
overflow assertions, and the recorded clean environment.

| Check | Result |
| --- | --- |
| Runtime `deferred_compute` | 7 passed |
| Runtime `queued_consumer` | 10 passed |
| Runtime `producer_launch` | 94 passed, 2 existing ignores |
| Runtime `compute_xgmi` | 32 passed |
| Runtime `multi_group_drain_tests` | 7 passed |
| Full runtime | 1,980 passed, 0 failed, 32 existing ignores, zero filtered |
| Strict combined all-feature Clippy | Passed |
| Runtime no-default library | Passed, existing `Route::Native` dead-code warning |
| Witness CLI tests / runnable build | 3 passed / built |
| Touched Rust formatting / whitespace | Passed |

Filters overlap. The full runtime exits zero in 90.99 seconds; all 25 new test
functions pass. The prior checkpoint's three socket-inspection permission
failures do not recur in this environment. No telemetry source was changed and
no new tests were ignored. KFD implementation is unchanged; a broad KFD suite
was not rerun or newly qualified by this runtime campaign.

Final runtime ELF SHA-256:

```text
2b0ffa35e90e21a189644eb3f059c9182faf047dc2e21d8ee4fb8dd60b1cdc96
```

## Retained Earlier Attempts

The first strict precheck passed. The first optimized unit build also passed,
but the controller selected the preceding cached executable instead of Cargo's
new artifact path. Its empty new-test filters were caught; none of that runtime
attempt is accepted. Editing that controller while its discarded run was active
also caused Bash to reread changed trailing lines. The controller now selects
the executable from Cargo JSON and runs an immutable per-attempt script copy.
A subsequent corrected-artifact attempt stopped before tests because `rg` was
absent from its clean PATH; the nonempty-list check now uses `grep`.

The first actual new-binary runtime run passed 1,979 tests and failed one fixture
cleanup assertion: it cancelled an unrelated child gate while the pending copy
still owned that child. The fixture now drains the copy before cancelling the
gate. No production change or assertion weakening was needed. Its source
preimage, failed outputs and all earlier controller attempts remain in the raw
packet. The final rebuild took 37.39 seconds; final results above supersede
these earlier attempts without deleting them.

## Source Controls

All 32 commands in the existing source-control workflow pass in
`source-ci/attempt-02-after`, with owned process groups absent afterward and
unchanged before/after source inventories. Independent review and the applied
verifier authenticate proposal-03: nine guard/test files, 18 SHA literals,
seven source-roster counts, 1,018 source inputs and 76 unchanged executable proof
files. The runtime roster is 340 files, or 345 with the five model schemas.

The initial proposal stopped because the journal-observer guard reconstructs
the whole producer-reader owner, whose admission filter changed. Review checks
that restoring exactly that one removed line reproduces the baseline owner;
all four observer methods and their 38-file proof closure remain unchanged.
Only the derived reconstructed-owner hash is refreshed. The first workflow then
caught the duplicate owner hash in `test-producer-input-composition.py`; the
final proposal updates that one native-owner `PINS` entry as well. All other
`PINS` entries, predicates, contracts, solver counts, mutation rosters and
qualification flags remain unchanged. Earlier failed proposals and workflow
receipts are retained; no failed result is relabeled as passed.

The historical `check-producer-input-preflight.py` pins and the completion
adapter's unchanged-source correspondence are outside this workflow and remain
historical. This increment changes their source boundary. Neither this metadata
maintenance nor unchanged proof bodies formally qualify the new adapter or
native route. No solver was executed by these source-control checks.

## Remaining Gates

There is one deferred head per stream. Ordinary `launch` still rejects pending
copy inputs. A deferred consumer's event is not accepted as an exact downstream
producer-aware input. Native eligibility remains initialized PUBLIC buffers,
equal full logical extents, one packet and the existing R57 qualification
authority. Covered consumer reads do not relax native copy eligibility.

Hardware correctness, outstanding native group shutdown, physical overlap,
persistent peer mappings, multi-packet transfers, arbitrary-kernel authority,
workload partitioning, all-device scaling and matched HIP/HSA performance remain
open. A3 is incomplete. No speedup, parity or new formal verification is claimed.

## Raw Packet

`SHA256SUMS` identifies `raw.tar.gz`. The packet includes all attempts, source
and guard patches, the exact source roster, controllers, toolchain, artifact
identities and access receipts. `final-attempts.txt` selects accepted tests and
`final-elf.sha256` binds the unit executable and runnable witness. Executables
and Cargo caches are not bundled. Replay uses the recorded base plus source patch.
