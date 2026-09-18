# Nonblocking Observer Registration Development

Development above `5d70cb0a6e16fb265fe224690274fdb0be2b0055`. This packet covers
CPU registration and lifecycle behavior, not protected/native execution, formal
Rust/native refinement, performance acceptance or HIP/HSA parity. Accepted
R125 Native CPU/test, R118B C1/C2/C3 and R116/V3 checkpoints are unchanged;
A1/A2 and #182 remain incomplete.

## Change

Three APIs enqueue event observation, stream progress, or atomic paired
event/source-stream registration without blocking for admission:
`enqueue_event_registration`, `enqueue_stream_registration` and
`enqueue_event_registration_with_progress`. Each reserves one reply cell before
bounded command admission. The acknowledgment future separates command failure
(including reply/queue capacity and discarded-command Stop) from the existing
registration validation/capacity errors. Success moves the original observer,
not a replacement and not evidence of GPU completion. Its reply credit remains
charged until the acknowledgment future is dropped, including after readiness.

The same three scheduler commands and existing registry bounds serve both
blocking and nonblocking callers. Synchronous APIs keep their signatures,
validation order and owner-thread self-wait rejection. New APIs allow admission
between current-owner ticks but reject callback and acknowledgment-wake reentry.
No extra thread, scheduler, native mechanism, receipt authority or liveness
allocation is introduced.

The acknowledgment owns the provisional observer and drops it before disposing
its reply/waker. Owner admission checks its original abandonment bit immediately
before pending registration commit. A racing Drop after this check can leave an
abandoned entry for the ordinary bounded poll/flush cleanup; it cannot turn Drop
into GPU cancellation or release. Paired admission retains its existing
insert-both/acknowledgment/rollback transaction and exact paired-progress removal.
Terminal cached events need neither registry capacity nor a new flush.

Thirteen tests cover all three APIs on current and background owners, exact
reply/queue exhaustion, event/progress capacity and duplicate precedence, paired
mismatch/rollback, pre-acknowledgment Drop and deterministic Drop during the
acknowledgment wake, queued-versus-admitted Stop, callback reentrancy, deadline
preservation, already-terminal observation, and foreign-thread admission.
The public compile-only example links paired admission and event completion
through the caller-driven engine; it does not run a kernel.

The protected verifier/refinement provider and proof artifacts remain missing.
This packet neither relaxes application seccomp nor qualifies actual protected
typed bundle execution. Formal refinement and native fault coverage remain
separate. A concurrently attempted native-wait comparison uses the earlier
committed source, not this registration candidate, and is recorded separately.

## Development History

Recorded exploratory compilation and the initial eleven focused tests pass.
The two later tests add background-owner compatibility and standalone
capacity/duplicate checks. Formatting commands before the qualification snapshot
were not retained as command receipts. No failed exploratory result is omitted
from this packet's raw receipt directory.

This registration CPU campaign creates no remote workload or scratch directory.

## Qualification

| Gate | Result |
| --- | --- |
| GNU host library | 282 passed, 4 ignored |
| GNU runtime library | 1,101 passed, 17 ignored |
| Scoped musl host library | 282 passed, 4 ignored |
| Scoped musl runtime library | 1,101 passed, 17 ignored |
| Host/runtime doctests | 70 passed: 7 compile, 63 compile-fail |
| Unsafe-source policy | 5 passed, 1 ignored maintenance command |
| No-default-feature check, strict Clippy, workspace format | Passed |
| Evidence Python lint/format and shellcheck | Passed |

Both runtime harnesses contain exactly thirteen new registration tests, all
passing. Every prior host/runtime test name and outcome is unchanged; ignored
tests are not execution evidence. Musl uses `FE2O3_HIP_SYS_DISABLE=1` and is not
HIP/native qualification. Doctests compile or reject examples, not kernels.

The verifier parses complete harness transcripts, compares GNU/musl rosters
against the pinned prior archive, and checks all 5,540 source hashes before and
after qualification against the current tree. The four exact library test
executables are bound to both build and execution paths and have identical
before/after/current hashes. Qualification command receipts preserve arguments,
UTC timestamps, logs and exit codes. Independent source/API review found no
blocker. `seal.sh` reruns the final verifier and creates `SHA256SUMS`; no existing
sealed archive was modified.
