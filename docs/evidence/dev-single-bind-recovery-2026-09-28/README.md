# Single Persistent Bind Recovery

CPU developer evidence, not GPU execution, executable formal refinement,
complete #182/A1/A2 closure, HIP/HSA parity, or matched performance evidence.

Baseline: `f5acac253914795516c24ae6d545632f1e6287bc`.
Signed implementation: `276c42c023968b6d9543722565b5fbd36ac3c007`.
Source tree: `7e9fea7ffdc0481b105aefda3d35a3182b397469`.

## Change

Single-binding rejection previously reused the successful-completion restore
helper. That helper allocated a new Box after the lower binder returned typed
retryable input. The existing public scripted H2D fixture reproduced one heap
allocation in that restore body before this change.

Extraction now returns the typed input and a linear restoration ticket carrying
its original allocation, source, access, submission, promotion metadata and box.
H2D-ready and replay extraction reuse the original Box allocation through the
existing `take_restore_shell_v1` helper. Initialized storage keeps its existing,
record-owned multi-shape shells; the local ticket uses None for that origin.

Native Retryable, scripted Unsupported and the scripted clean-bind-refusal hook
share the new restoration body. It validates the exact slot and original shell
before allocation-free reinsertion. Mismatches retain the returned typed input
in terminal custody before formatting diagnostics, without replacing storage.
Initialized-storage bind rejection accepts only its original storage input;
successful completion remains separately capable of returning replay input.
Metadata, including scripted replay provenance, is left unchanged on rejection.

No new allocation is added to healthy H2D/replay extraction. The empty local
shell remains alive through binding and is dropped only after successful bind
custody is installed in Active. Published descriptors and allocation ledgers do
not grow. This does not remove successful-completion restoration allocations or
claim that diagnostics, complete launches or every failure path allocate zero.

## Coverage

The H2D allocation regression now counts the same extraction and bind-recovery
bodies used by production: zero allocations in each, with the exact original
Box address, device identity, byte content, certificate and promotion preserved.
It still performs complete scripted cleanup after the public write/copy/poll.

New fixtures use public scripted H2D transport, explicit dependency events and
admitted pending compute descriptors. Replay comes from completed scripted
compute; initialized storage comes from the existing typed conversion. These
are CPU owner-state witnesses, not native GPU uploads or kernel execution.

Nine direct extract/restore round trips cover ready, initialized-storage and
replay origins three times each. All restoration commits allocate zero; ready
and replay extraction also allocate zero and restore into the original box.
Initialized-storage extraction still allocates three test-layout shells before
input extraction and restores into its preallocated initialized shell. Tests
compare owner IDs, byte pointers/digests, certificates, promotion, metadata,
pending recipes/rosters and driver ownership counts, then cancel and clean up.

Three public deferred submit/flush controls inject a clean bind refusal into
the same recovery body. The first submission settles Failed(-1) without a
dispatch-publication event; its resources are refunded exactly once. The
unchanged queued successor retains its dependency/owner counts and completes
successfully. Failure of an ordered predecessor does not become an implicit
success dependency. No native bind receipt is fabricated by these controls.

Eighteen subprocess cases corrupt the slot marker, record membership, shell
presence/shape, ticket source or returned input variant for each origin. They
require exact terminal input custody, unchanged slots and surviving record-shell
addresses, retained pending recipes/rosters, no scripted owner loss, and actual
SIGABRT on unrepaired backend Drop after an inspection marker. Core dumps are
disabled. They test private adapter contract violations, not arbitrary native
memory corruption or a formal proof of the entire runtime ledger.

## Qualification

All final-source checks ran sequentially against the signed implementation
above, with `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`. Source continuity was
checked after the campaign. The compressed receipts include commands, output,
exit status, timestamps, source identity and signature verification.

- Focused bind recovery: 3 tests passed, including all 18 fault subprocesses.
- Existing H2D allocation regression: 1 test passed.
- Lower KFD persistent selection: 166 tests passed.
- Runtime unit tests: 1,740 passed, 3 failed, 28 ignored.
- Runtime model unit tests: 1,080 passed, 19 ignored.
- Integration tests: 11 passed, 3 hardware tests ignored.
- Doctests: 52 runtime and 29 runtime-model tests passed.
- Strict all-feature/all-target Clippy, runtime no-default-features check and
  workspace formatting check passed.

The full test command exited 101. The same three authorization/telemetry tests
failed at `authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. These failures are
neither waived nor represented as a green full suite.

Development receipts retain the expected pre-fix one-allocation failure and an
initial compile failure caused by removing a helper still used by older tests.
That helper was restored unchanged before the passing final-source campaign.
Read-only reviewers found no blocking issue in the bind-recovery change.

## Open Boundaries

Lower KFD Retryable variant preservation has separate CPU contract tests; this
packet does not execute a coupled native bind/reject/retry on MI300X. Native bind
terminal capsules, internal bind unwinding, initialized-storage shell allocation
failure injection, completion-time restoration allocation, Context composition,
generated execution/profiling, formal native correspondence and matched HIP/HSA
benchmarks remain open. Accepted checkpoints, A1/A2 and parity status are unchanged.

MI300X access again failed DNS resolution for `sharkmi300x-1`; no remote files or
jobs were created. This is not a new hardware or external issue-state revalidation.
