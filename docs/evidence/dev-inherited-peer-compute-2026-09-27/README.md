# Peer Custody Through Native Compute Intermediates

Development checkpoint, not #182 closure, A1/A2 acceptance, full HIP/HSA parity,
concrete formal refinement, hardware qualification, or performance acceptance.

## Source And Receipts

Implementation: `9c688f1586243d41d0216fcaed0a022cf803129a`, SSH-signed and verified.
The primary agent implemented and tested; two read-only agents reviewed the
dependency roles, ownership, fixtures, and healthy/error paths.

Frozen raw directory:
`/home/harsh/.codex-tmp/fe2o3-inherited-peer-20260927-JcoTZv4V`.
[Receipts archive](receipts.tar.xz), SHA-256:
`05d530d55a330c03932beac2a6fd2b8730ef54278e4443724123ead48d64f507`.
Archive comparison passed. Receipts include the signed source patch, source and
test-binary hashes, compiler identity, command notes, and development/final logs.

## Implementation

- Producer-aware native C can inherit directed-peer ancestry through native B.
  Direct native success dependencies and native FIFO ordering are not flattened
  into peer-success dependencies. Inherited peer roots require quiescence only;
  they do not increase explicit dependency depth or propagate peer failure as an
  independent C failure.
- C independently captures, retains, and authenticates every ancestor node, not
  just B's direct peer roots. A failed peer root can leave another ancestor live.
  Releasing B's event, cancelling B, or retiring B's ancestry does not revoke C's
  access permits while such an ancestor is still writing.
- An exact native-route index locates still-live ancestry even after B has
  settled in its child backend. Retired history is not reconstructed. Native
  prefix traversal follows immutable retained child rosters, checks monotone
  identities, FIFO stream identity, and matching positive retain counts, and is
  bounded by submission/dependency limits. Cumulative imported snapshot work is
  bounded before validation, including repeated shared closures.
- Source ancestry, child gates, route stamps, consumer rosters, and returned
  child handles are checked. Retirement validates links before mutation, returns
  terminal failure on mismatches, and preserves custody. Stream retirement stays
  scoped to its indexed consumers. Unrelated allocation owners remain rejected.

## Qualification

| Lane | Result |
| --- | --- |
| Final all-feature runtime library | 1643 passed, 3 existing failures, 28 ignored |
| Seven new scripted test groups | All passed in the final full run |
| Runtime doctests | 52 passed |
| Strict all-feature/all-target runtime Clippy | Passed |
| No-default-feature runtime check | Passed |
| Formatting, whitespace, source continuity, signature | Passed |

The three failures remain `authorized_execution::tests` cases
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`:
`InspectSocket(PermissionDenied)` in unchanged `authorized_execution.rs`.
They are not passing or waived qualification.

Tests cover Pending/Published P -> B -> C, explicit versus FIFO dependency roles,
observation-only calls, expired drain, cancellation, event release, late
admission after child B settles, failed roots with live ancestors, partial-copy
bytes and untouched suffixes, depth, bounds/deduplication, missing ledgers,
corrupt gate/routes, callback mismatch, and pre-mutation retirement failure.
Scripted compute does not establish GPU kernel outputs. Deliberately corrupted
CPU metadata is restored for teardown; that is not native terminal recovery.

Development failures and corrections are retained in `commands.md` and logs.
Two full runs aborted because an older low-level fixture stamped a source-side
child with a destination-side ancestry stream. The new callback identity check
exposed that mismatch. The fixture now uses an actual routed child stream. Other
fixture corrections covered FIFO cancellation, reporter native backing, and its
required recycle step. Only the final run supports the counts above.

## Open Work

Native-only transitive shared-buffer custody still requires a separate immutable
child quiescence roster. C must retain transitive Compute owners independently
before their intermediates can cancel/fail, observe them before publication,
and release them on every terminal/cancellation/publication path. Do not turn
those owners into success dependencies or enable unordered overlap.

Native-prefix traversal and snapshot inheritance need workload-scale measurement
and fast-path work, including producer-aware admissions without peer ancestry.
This checkpoint establishes bounded work, not a speed improvement. General
mixed DAG/fanout qualification, compute-to-peer and native XGMI composition,
concrete Verus refinement, ELF audits, and matched storage-origin first-conversion
versus steady-replay KFD/HIP/HSA measurements remain open. No new native KFD suite,
Verus campaign, ELF audit, or GPU benchmark ran. Worker, device-language,
atomic/collective, distributed execution, and release gates remain in scope.

MI300X DNS failed before connection, so no remote artifacts or jobs required
cleanup. The frozen packet records both initial source pushes failing DNS.
A later retry published implementation and evidence through
`952e92bd8905a6082205bb4bded1057d380ba7e3` to origin's
`codex/r65-runtime-drain-versions`; `ls-remote` confirmed that head. The upstream
push still failed DNS and its independently queried head remained
`2307ff7d88e093019208e2f8391e498956f415f6`. Dual publication remains pending.
The [issue update](https://github.com/harsh-nod/fe2o3/issues/182#issuecomment-5855020390)
was posted on retry; a subsequent API read verified #182 remains OPEN.
