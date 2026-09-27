# Child Compute Peer Gate

Date: 2026-09-26. Development CPU and shared-scalar proof evidence.
This does not close A1/A2, issue #182, native qualification or HIP/HSA parity.

Parent: `a65cd3149a51e8ce268714c1b3c6bee5e6014eb4`.
Signed implementation: `db3213524d23ff8559979f663ab092b6ed94a588`.

## Implementation

A private gate is installed on the genuine pending child compute record before
opportunistic publication. Completed cooperative peer producers use this path
immediately. Pending-peer router admission remains rejected until authenticated
predecessor allocation access and router progress are integrated.

The gate separates successful inputs from external stream ordering. It binds
the exact outer consumer and child submission; terminal input results cannot
change and completed ordering cannot rewind. Native failure remains sticky.
A failed consumer waits for both stream prefixes, without unnecessarily waiting
for unrelated explicit inputs. Waiting retains the existing allocation/module
custody, completion reservation, dependency counts and stream position.

Immediate, observation, explicit progress, deadline, ordered-successor and flush
paths enforce the gate. Ready resumes existing native blocker progress and
early-successor behavior. It does not supply backing initialization authority.
Terminal observation and unwind restore the pending record before sealing;
ordinary cancellation releases only the consumer's owners.

Exact predecessor retirement is rechecked after a quiescent error. Foreign
blocker quiescence in the pending-compute poll prelude now leaves the requested
consumer Pending. This latter change is defensive attribution hardening: review
did not find a currently reachable scripted/native blocker Quiescent outcome
there, so no native regression reproduction is claimed.

## CPU Qualification

| Check | Result |
| --- | --- |
| Focused child-gate groups | 10 passed |
| Unfiltered all-feature runtime library | 1,600 passed, 3 failed, 28 ignored |
| All-feature doctests | 52 passed (8 + 44) |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-features check, formatting, whitespace | Passed |
| Source continuity and SSH commit signature | Passed |

The three failures remain the telemetry socket-permission failures at
`authorized_execution.rs:1317`: `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. They were not skipped
or counted as passes. Focused and broad counts overlap. Cargo incremental
compilation was disabled; the existing dependency build directory was reused.

Controls cover seven progress ingresses, same/cross-stream native parents,
failed-input ordering, cancellation, identity corruption, public release Busy,
success with invalid backing, native blocker progress, and injected Terminal
and panic retention. Scripted accepted three-binding work continues successfully
with zero user-data materializations after resolution. This is not GPU timing.

The ordinary ordered-successor control uses an exact physically retired recipe
fixture: Waiting/Failed stop before the attempt, while Ready reaches the precise
missing-native-lane error. It validates guard placement, not successful native
queue publication. Broader memory-path and native execution qualification remain.

## Formal Qualification

All 23 campaign phases pass. Signed opening, exact relocated and closing full
proofs each report 10 verified / 0 errors using the pinned default solver limits.
These are three measurements of the same 10 obligations, not 30 distinct ones.
All 15 executable-body mutations reject with clean logical diagnostics: seven
resolution mutations and eight action mutations. Resource/frontend failures
are not accepted as successful mutation rejection.

Four gate-specific and nine inherited classifier calibration groups pass.
Both 190-file verifier closure measurements match; all 6,070 measured signed
source inputs remain equal. All 23 owned process groups are absent at completion.
The production adapter and proof include the same executable decision macros.

The proof establishes scalar identity, monotonic resolution, ordering and failure
decisions under explicit native-success/order inputs. It does not prove those
inputs' observation adapters, concrete custody-map refinement, router ancestry,
permit issuance, publication-callsite completeness, native execution or speed.
No assume/admit/external-body shortcut or increased solver budget was introduced.

## Retained Evidence

`receipts.tar.xz` contains the signed implementation patch, CPU receipts, source
measurements, complete 23-phase campaign with relocated/mutated sources, reviews,
and network checks. The archive was compared with the complete receipt directory.
SHA-256: `33d15bd59dbc003c43de13ba99067af7a5c9be66fa57d17f137f301cbf6ca6dd`.

Exploratory proof attempt 1 failed on Verus's unsupported `unreachable!()` panic;
attempt 2 passed after an explicit proved-impossible branch replaced it. These
logs and earlier focused runs are retained, but are not substitutes for the
signed campaign; complete source closures for each exploratory attempt were not
recorded.

MI300X hostname resolution failed before execution; no shared-machine artifact
was created. A direct API read confirmed #182 open, while the final scripted API
read failed. No permission request, escalation, native timing or speedup claim
was made. Accepted Native R125, Admission R118B C1-C3 and Resources R116/V3
milestones are unchanged.

The [composition contract](../../runtime-peer-producer-composition-v1.md) keeps
exact predecessor copy access, bounded ancestry, all-memory-path integration,
router driving, native XGMI and matched HIP/HSA measurements open.
