# Gathered Frame Compute Consumers

Implementation baseline: `0bbef399bdb8f219627d656c7ca1a66b15cfeba2`.

## Scope

An ordinary producer-aware compute launch can consume an initialized allocation
assembled by ordered partial peer writes, naming only the exact latest gather
event. Each original input alias must be a checked Read range. The preserved-frame
contract is separate from the last peer's actual written window; the existing
copied-range predicate and queued-output restrictions are unchanged.

Context uses its existing atomic mixed stable/active/queued read acquisition and
per-node completion reconciliation. No future epoch or lineage is guessed.
Backend admission retains an immutable destination-frame snapshot, authenticates
the exact bounded predecessor chain and rejects unrelated indexed owners. Native
handoff waits for genuine success and restoration of all original owners. A
settled predecessor may release its old source without invalidating the retained
destination frame. Released public events do not release these dependencies.

Admission while an older gather ancestor owns the native children is also
supported as a metadata-only wait. It requires the exact authenticated ancestor,
both matching child reservations and both original InFlight storage slots. It
does not permit a dependent launch to acquire child custody early or drive an
unrelated native blocker.

Explicit consumer progress restores that already-started ancestor before
following the tail's source-compute dependency. Otherwise the source compute
can wait on native child reservations whose owner lies later in the dependency
cursor, forming a progress cycle. Restoration uses the existing paired failure
envelope and returns before any consumer child handoff.

The deferred-chain witness adds `--gather-compute` and
`--gather-compute-overlap`. Each device first completes the unchanged R57 V2
A+B->C setup gate. Pending source C+B->D launches feed checked windows into the
sink's original C, then its unchanged C+B->D consumer feeds a return peer and
full return-allocation D2H. All pipeline submissions precede explicit progress;
only the final readback stream is registered. The consumer names only the latest
gather event, mixing queued C with stable B and a full Write D.

The source inputs and kernel authority are unchanged. Source windows have
different aligned offsets and lengths, with gaps or ordered overlap in the
0.25-initialized sink C. The independent oracle overlays A+2B source bytes,
computes the sink's C+B elementwise, and checks every source D plus complete
sink C/D, returned E and host bytes. The return peer copies D[20:262100] to
E[131:262211]. E is 262,913 A5-initialized bytes; all E bytes are read back at
offset 97 into a 263,297-byte 5A-initialized host allocation. The host guards
are therefore 97 and 287 bytes. No expected computed output is uploaded.

## CPU Qualification

Selected build, runtime and checks: `attempt-02`. The complete current runtime
suite passed 2,198 tests with the same 32 prelisted ignores and zero filters.
The three witness suites passed 11 directed, 10 deferred and 7 live-sharded
tests, giving 2,226 fresh test passes. The exact baseline roster adds 17 runtime
tests and two example tests without deleting or newly ignoring older tests.
Strict Clippy, no-default and hardware-feature checks, formatting and whitespace
checks passed. The source control selection `proposal-02` / `attempt-02-after`
passed all 32 controls without executing a solver.

The first focused run exposed two bounded-progress failures in late gather
consumer paths. Both are retained in the raw packet. They led to the authenticated
ancestor-progress fix, not fixture completion or a larger progress limit. The
second focused run passed all 17 tests; this supplemental run is not counted
again in the full-suite total. Initial compiler diagnostics and superseded
attempt-01/source-control receipts are also retained, not selected as final
source qualification.

The unchanged KFD ELF and complete transitive inputs authenticate historical
reuse of 1,934 KFD passes and the retained arithmetic proof/mutation receipts
from the native-peer-subranges packet. Those are not fresh test or solver runs.

## Native Qualification

The eight new cases passed on MI300X with three and four devices, both device
orders, and disjoint or overlapping gather windows. Each uses two or three
source computes and one sink consumer, with only the latest gather event as the
consumer's peer dependency. The full byte oracle, completion roster, native-copy
counter and explicitly completed owner shutdown are checked independently.
The 17-test focused diagnostic is supplemental to the complete CPU run, not
additional native coverage.

All 20 existing controls also passed on the same source and rebuilt witnesses:
eight ordered gather/readback cases and 12 checked-window, late-compute,
subrange-readback, deferred-chain and live-batch cases. The campaign therefore
passed 28 of 28 cases with no failures. Selected devices were MI300X cards 4-7;
each case rechecked UID/BDF, idle use, VRAM, process attachments and host memory.
These checks were shared-host point observations, not exclusive reservations.

The three uploaded executable hashes matched before and after execution. The
final owned-process census was empty. Only the three owned binaries and the
owned `/tmp/fe2o3-gather-compute-20261002-leQolv` directory were removed, and its
absence plus the final host availability checks passed. No unrelated process,
file or GPU reset was used. The outer campaign receipt confirms source stability.

## Evidence

`raw.tar.xz` retains the bounded command receipts, full stdout/stderr, exact test
rosters, source snapshots, current-source control checks, failed diagnostics,
independent native byte-oracle controller and cleanup receipts. Its per-file
manifest is `raw-manifest.json`; `SHA256SUMS` binds this README and both files.
The candidate implementation patch is `raw/candidate.patch`, SHA-256
`27b604161b8a16647da245e1ea82529929af1fb7c784d57e878d1b03dce09fbf`.
This patch contains the staged crate changes, not the evidence archive itself.
The independent receipt audit `raw/audit-01.json` passed against the selected
source, executable, test, control, hardware and cleanup identities.

## Verification Boundary

Shared journal, fold, observer and reconciliation proof bodies are unchanged.
Their success/lease contracts do not establish the new runtime frame-to-launch
admission premise. The source refresh permits only the reviewed
`prepare_pending_input_v1` method body change when reconstructing the protected
owner file; all other signatures, bodies and forwarders must match the baseline.
No new solver execution, whole-adapter refinement, compiler-to-machine theorem
or DMA-engine proof is claimed.

The backend's scripted compute tests exercise real retained ownership and
handoff, not compute arithmetic or physical DMA. Native compute execution uses
finite R57 qualification authority, not a production verifier/provider bridge.

## Limits

This increment retains Read-only input aliases, exact latest-writer events,
bounded ordered chains and whole-allocation serialization. It does not introduce
concurrent slice ownership or arbitrary dependency graphs. The native gather
consumer witness uses one pipeline batch per process; it does not reset or widen
the two-request R57 authority. Compute may publish during admission, so
all-before-explicit-progress is not physical concurrency evidence. Native fault
injection, physical overlap, performance acceptance and full HIP/HSA parity are
not claimed. Shared-host availability observations are not an exclusive GPU
reservation, and logical guards do not cover hidden physical pool padding.

The new hardware modes cover prequeued gathered consumers. Late admission while
a gathered ancestor is already published is covered by CPU regressions only;
the retained native late-compute controls exercise direct peer consumers, not
this new gathered-frame case. The new witness does not qualify repeated expired
deadline resumption of the same command future.
