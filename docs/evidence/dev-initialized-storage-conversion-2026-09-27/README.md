# Native Initialized-Storage Conversion

Development evidence, not #182, A1/A2, full HIP/HSA parity or performance
acceptance. Native conversion is implemented; the deferred high-level runtime
adapter and pending-peer lifecycle integration remain open.

## Source

Implementation: `e60ff12a441167361fe63de8c3a591950c12bf68`, SSH-signed by
`harmenon@amd.com`. The primary agent implemented, integrated and tested the
changes. A read-only agent reviewed the production custody boundary and reported
no concrete correctness or custody regressions; it did not run tests.

Raw directory: `/home/harsh/.codex-tmp/fe2o3-storage-conversion-20260927-4WEoJ3jP`.
Frozen archive: `receipts.tar.xz`, SHA-256
`df0a5b63c3cfea8b2b31555afc33928ecf7840cf5d85481f24cd30128c51bf32`.
Archive comparison passed. Source identity, signature, committed-patch equality,
final input hashes and the final KFD test-binary hash are recorded.

## Behavior

- A sealed, move-only `Gfx942PersistentComputeInitializedStorageV1` authenticates
  exact local native storage with full initialization coverage, retired use
  frontiers and matching queue, pool generation, storage identity and extents.
  This is not a content-digest or previous-compute-dispatch claim.
- The conversion roots the original allocation before model ownership is loaned,
  brackets native mapping validation with operational-currentness checks, and
  retakes the model before returning the payload. Failures preserve retryable,
  foreign-queue or opaque terminal custody. Panics retain queue-owned custody;
  lifecycle guards and queue Drop prevent normal teardown of an unfinished root.
- Conversion performs bounded metadata checks with no allocation, demotion,
  generation advance or outstanding-buffer debit change. This source property
  is not a measured latency or throughput result.
- Initialization origin is represented by an enum rather than independent hash
  and boolean fields. Bind/replay/cancellation preserve the exact origin.
- Retained write-only replay replaces the previous initialization premise with
  the actual incoming storage state. It no longer resurrects stale initialization
  after detached storage was overwritten. Cancellation validates initializedness
  before moving any returned storage and cannot upgrade a cold input.
- Runtime compatibility matches remain fail-closed. They do not install deferred
  conversion or relabel initialized storage as completed compute replay.

## Qualification

| Lane | Result |
| --- | --- |
| Initial full KFD library suite | 1669 passed, 3 failed |
| Corrected persistent-cancellation tests | 17 passed |
| Final conversion tests | 10 passed |
| Final stale-replay regression | 1 passed |
| Initial cancellation-filter tests | 15 passed |
| Runtime all-feature library suite | 1621 passed, 3 failed, 28 ignored |
| KFD and runtime doctests | 42 and 52 passed |
| Runtime all-feature/all-target check; minimal runtime check | Passed |
| Final strict KFD and runtime all-feature/all-target Clippy | Passed |
| Formatting, staged/unstaged whitespace and final input audit | Passed |
| Layout bound | Passed without changing any limit |

Two initial KFD failures were old cancellation tests that explicitly accepted
an initialization mismatch. Their expectations were corrected to match the
stricter production contract, and a constructed single/three-binding storage-origin
cancellation regression was added. The focused rerun passed all 17 tests. The
full KFD suite was not rerun after these test-only corrections.

The remaining KFD failure is
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
at `target_debug_telemetry_v2.rs:1173`, with `SocketAdmission`. Runtime failures
are `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal` and
`pre_native_telemetry_failure_is_returned_and_poisoned`, each at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
These four baseline failures recur from the preceding checkpoint, and both
failure-source files are unchanged. No permission escalation was attempted.

The initial KFD Clippy run rejected one nested conditional in the new test
driver. The style-only fix and successful rerun are retained separately.
The original source-continuity check correctly failed for the runtime companion
and revised cancellation tests. `source-revision.md` records these boundaries
and the later test-only lint fix. The final audit verifies all other initial
inputs unchanged plus explicit new hashes for those three files; the committed
source matches that final input set. The initial check is not reported as passing.

Layout: owner inline 136 bytes, owner heap 3328 bytes, ledger 3072 bytes,
queue inline 39248 bytes, up from 38976 bytes. This is not a performance result.
MI300X DNS failed before connection, so no remote artifacts were created. The
final issue-state probe also failed to connect to GitHub; it is not acceptance.

## Verification Boundary

The conversion fixtures use registered mappings, actual host-buffer writes,
production SDMA bookkeeping and injected completion observations. They cover
contiguous/partial/gapped/unknown initialization, retained frontiers, wrong
scope and identity, quarantined or raw-restored storage, reserved/prepared uses,
loan/currentness/mapping/retake failures and panics, public guards and subprocess
Drop abort. Cancellation and replay use real constructed dispatch/control data.
Five compile-fail examples cover sealing, mutation, cloning and reuse.

These CPU tests do not execute GPU copies or kernels, prove native adapter or
machine-code refinement, or establish copy/launch parity. The runtime-model
proof sources are unchanged, and no new formal campaign is claimed. The prior
initialized-prefix scalar proof does not prove this concrete conversion.

Remaining work includes typed eligibility refusal distinct from integrity
failure, deferred runtime conversion after predecessor ordering, origin-aware
restoration/normalization, guarded early replay, pending-peer admission, durable
mixed-depth history, native XGMI composition and matched hardware qualification.
`next-runtime-integration.md` in the archive records the adapter ownership and
fallback constraints. Broader Worker, device-language, collective, distributed
and release gates are unchanged.
