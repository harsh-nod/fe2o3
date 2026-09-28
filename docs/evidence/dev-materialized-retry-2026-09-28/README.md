# Ordinary Prepared Retry Custody

Development implementation and CPU qualification only. A1/A2, accepted lane
checkpoints, protected Worker application and HIP/HSA parity remain incomplete.

## Source

- Signed source commit: `54a1eb389aa990f633b6a6915474976afb07f83d`.
- Qualified source tree: `46c698edd1636546cbf433da29eae89c9aee5992`.
- The SSH signature and sign-off were verified. All final checks used this
  unchanged source revision. The unrelated untracked owner-inspection packet
  was preserved.

## Implementation

Ordinary Prepared retry now stays indexed while validating logical custody,
descriptor geometry and exact writable-binding projection. It moves into the
same submit-only Binding phase used by initial publication, without repeating
preparation, native binding, materialization or the one-time Pending handoff.

The native callback records attempted consumption before submission and roots
each returned batch/retry classification before its outer lane loan closes.
Only confirmed retry re-arms Prepared. Terminal/outer failure or unwind retains
its actual attempted state; repeated poll/cancel/shutdown cannot advance it.
Published custody is installed before profiling. Confirmed retries preserve
preparation, backing identities and binding/materialization counters; successful
publication refreshes its timestamp and emits one publication event.

The common cancellation/retry descriptor guard replaces binding-prefix scans
with a fixed native-DATA-sized stack array. Geometry takes O(B*D+B+D) work with
expected-constant allocation-table lookup, D <= 16 and B <= 128. Full custody
preflight also performs allocation-owner binary searches; writeback projection
has its own bounded B*D scan. The new explicit D limit rejects malformed synthetic
states above native capacity. This is a complexity claim, not measured speedup.

## Qualification

All Cargo checks use `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`. Commands, UTC
timestamps, complete logs and terminal statuses are retained in the archive.

| Check | Result |
| --- | --- |
| Final all-feature materialized selection, serial | 22 passed |
| All-feature runtime backend, serial | 880 passed, 28 ignored |
| Full all-feature runtime library, serial | 1777 passed, 3 failed, 28 ignored |
| Runtime + KFD all-feature/all-target strict Clippy | Passed |
| Runtime no-default-features check | Passed |
| Workspace format check | Passed |
| Signed source identity and post-check source continuity | Passed |

Selections overlap and cannot be summed as distinct totals. The full KFD suite,
runtime-model suite and doctests were not rerun. Preliminary 19- and 21-test
passes precede the final source revision and are retained, not substituted for
the final 22-test pass.

The three unwaived full-runtime failures occur at `authorized_execution.rs:1317`
with `InspectSocket(PermissionDenied)` / `Operation not permitted`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`.
No admission guard was bypassed or failing test skipped.

## Evidence Scope

Retry tests cover 32 lane/origin/fault combinations, all 32 unrepaired-Drop
subprocesses, eight healthy retry/publication/cancellation controls and 30 corrupt
preflight cases. They inspect accepted recipe and backing identity, descriptor
and writeback storage, exact retains, reservations, stream maps, event/profile
counts and terminal reentry. Normal scripted cleanup also runs through the
existing cancellation fixture for both origins and both lanes.

Three confirmed retries in each healthy control are allocation-counted around
public poll. Snapshot formatting lies outside that count. This is scripted CPU
retry evidence, not native submit, successful publication, cancellation or
end-to-end latency evidence. Valid multi-binding/aliased-writeback retries still
need dedicated runtime workflow coverage.

Independent prior-algorithm differential tests enumerate 1093 small alias rosters,
descriptor corruption, 128-binding/16-DATA bounds, overflow/invalid ranges,
missing records, invalid alignment and isolated 1/8/4096 alignment edges. The
production projection is allocation-counted. Both guards intentionally ignore
content hashes; positive digest controls preserve that boundary. These synthetic
allocation records prove no native storage or GPU behavior.

Read-only swarm review found no production blocker. It identified and corrected
an overly strong reference comparison and masked alignment tests before final
qualification. Review is not a formal correspondence theorem. Scripted recycled
generation 7 tests provenance retention, not a prior real native recycle.

| Acceptance dimension | This packet |
| --- | --- |
| CPU/source | Qualified within the scope above; full suite has three failures |
| Formal correspondence | Not established |
| Live Linux/KFD | Not run; MI300X DNS failure |
| Matched HIP/HSA performance | Not measured |

SSH failed resolving `sharkmi300x-1`; no remote jobs or files were created. Source
pushes to both `origin` (harsh-nod) and `upstream` (powderluv) failed resolving
`github.com`. The source-publication receipts are retained. Nothing here closes
native coupling, protected Worker application or behavioral/performance parity.

## Next Work

Ordinary frontier/pipeline poll and recycle still move Active off-index, and
logical completion needs checked, allocation-free settlement before profiling.
Only an actually returned completed receipt permits recycle retry; an error
without that receipt does not. Preserve first-Ready timing across recycle
attempts and preserve the existing prepublication dirty-vector reservations.

Ordered-successor submission also still precedes pipeline-owner installation.
It needs its own indexed publication state and Pending error-handoff changes,
not replacement of the current predecessor frontier. Native coupling, allocator
failure qualification, Context composition and formal refinement remain open.

`receipts.tar.xz` contains the frozen campaign and review/design notes. Verify it
with `sha256sum -c receipts.tar.xz.sha256` in this directory.
