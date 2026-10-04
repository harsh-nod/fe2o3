# Original Application Startup Custody

Source: `cc06d964abd49624ca57d6616b737c7b42ee77d1`.

Cargo now captures original application custody before compiler-service readiness.
This closes a startup prerequisite for authenticated multi-GPU proof registration;
it does not implement that registration, remote proof ownership, or GPU authority.

## Implementation

`PendingApplicationAck::after_spawn` drops the parent's ACK and test-readiness
writers before any service wait, captures an original child pidfd while the Child
remains unreaped, and completes sandbox admission exactly once. The spawned state
can either await ACK or move infallibly into existing containment cleanup.
Compiler-readiness failure no longer repeats sandbox admission.

The owner retains the complete occurrence, historical descriptor coordinates,
expected envelope/application commitment and challenge. Its binding allocation is
created before spawn and moved through pending, spawned, active and reaper states
without cloning. The original pidfd remains in cleanup on sandbox/ACK failure,
cleanup timeout and reaper-worker failure. Valid fast ACK-and-exit remains accepted;
capture is not a live-process observation. Foreign Child use rejects before reading.

No publication token is acquired, no sandbox rule is relaxed, and no protocol
environment or ACK wire format changes. The separate compiler-service channel
still opens its own pidfd while unreaped Child custody prevents PID reuse. Future
proof registration must duplicate this retained original application pidfd rather
than rediscovering it. Process exit and containment do not establish GPU settlement.

## Qualification

| Check | Result |
| --- | --- |
| Handoff unit tests | 37 passed |
| Sandbox unit tests | 7 passed |
| Selected strict V3 integration tests | 16 passed |
| Cargo all-target Clippy, package only | Passed with `--no-deps -- -D warnings` |
| Formatting, whitespace and frozen source hashes | Passed |
| Full Cargo binary unit run | 398 passed, 2 inherited failures, 5 ignored |

Five new startup tests cover writer EOF before a service wait, fast ACK-and-exit,
missing/substituted ACK, substituted Child and sandbox-admission failure. Existing
delayed-cleanup and reaper-panic tests now inspect the retained original pidfd and
binding. These tests use descriptive protocol fixtures and an immediate test-only
sandbox result; they do not authenticate an application or deployed service.

The integration run executes actual static single-kernel and roster host consumers
through Cargo's sealed application/sandbox path. It also covers descriptor and
protocol substitution, stale publication, ACK timeout/reaping, permitted auditor
operations, and rejected process/session/exec escapes. Its explicit envelope-only
test profile does not qualify production compiler readiness, a protected custodian,
Worker/Verus inside the no-fork application, or any GPU operation. Initial pidfd-open
failure and a live blocking production-service registration are not fault-injected
by this checkpoint. No new formal theorem or performance result is claimed.

## Inherited Failures

The full Cargo run fails the two reviewed-workspace macro source-pin tests. The
macro tree and pin source are unchanged from parent `0b896d925`; the independent
baseline script reproduces the mismatch directly from committed Git objects.
The pin at `3494de45f` is
`83559900a74b536f33006952ff4f0248286352d107f99acf3c5810e18252c377`;
nine later committed macro changes produce
`87b987e8d82629cb976e8b31a644080c9de49e632362fd678f32b0b5d2784f3a`.
That separate build-admission review remains required. Security pins are unchanged.
Follow-up read-only review found no production-code defect in the macro delta, but
requires direct structural tests for the new unsafe owned-argument adapter's exact
accounting, argument ordinals, mapped indices and unsupported-profile rejection
before refresh. Existing typechecking and compile-fail fixtures do not directly
exercise that emitted packing/accounting implementation.

Dependency-inclusive Clippy also stops at the existing manual Default implementation
in `fe2o3-semantic-import/src/profiler_bundle.rs`. Package-scoped strict Clippy passes.
A development run caught an oversized startup error type; moving the protocol's
allocation before spawn fixed it without a lint suppression.

All runs used the pinned nightly, four build jobs, disabled incremental/debug info
and HIP discovery, test optimization 1 and enabled debug/overflow checks. Ordinary
tests used four threads; static integration used one. No MI300X resources were used.
The evidence records exact commands, source/binary hashes, final and development
logs, and the reproducible baseline audit. Temporary static builds and this run's
scratch are removed after packaging; normal repository build caches are retained.

[Evidence archive](evidence.tar.gz) SHA-256:
`06fd0297942f747ff8585c0dedf1e2d274c3b8b768ca1dcbb5c3299aefa5adfc`.
Source patch SHA-256:
`a494963a895c506c8b51ce0b725120f3bbb54bd8ecc8d4e168d318435bd43c90`.
The complete archive manifest and archive-to-source comparison passed.
