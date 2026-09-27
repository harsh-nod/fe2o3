# Queued Native Failure Custody Checkpoint

CPU developer evidence over scripted native owners. This is not GPU execution,
formal unwind proof, complete #182/A1/A2 closure or HIP/HSA parity evidence.

Final signed source: `84417cfec88563cee57ad69c0625ec27a51ee411`.
Source tree: `93c3aef6260318424b8e0b833b57509ef8e99211`.
Initial test checkpoint: `01e5568998a1c7382be00bfb1c86c96f2253f9a3`.
Baseline: `27ebb17284c77b7029f22ea9acceeb7af5851176`.
Both source signatures were verified for `harmenon@amd.com`.

## Reproduced And Fixed

A public same-stream Context flush removed the middle pending submission before
polling its active producer. An injected producer panic propagated through that
observation without reinserting the middle submission. The regression observed
the missing pending record while its FIFO entry, dependency/module retains,
allocation custody and completion reservation remained live. This is a logical
accepted-operation custody defect, not a demonstrated GPU failure or native
allocation use-after-free.

`poll_retained_pending_dependency_v1` now holds the exact pending owner outside
the caught dependency poll. On unwind it restores the index before sealing the
backend and resuming the original panic with the existing payload-preserving
helper. It covers six formerly unguarded observations: explicit and ordered
dependencies during progress and observation, failed-submission FIFO ordering,
and a conflicting SDMA copy during progress. Normal poll results and each caller's
error classification are unchanged. The guard does not clone the launch/rosters.

Already indexed blocker observations, protected peer/quiescence observers and
publication/staging guards retain their existing structure.

## Failure Matrix

The Context test contains ten subprocess scenarios:

- Terminal restoration-marker mismatch and pre-active-move panic, with either A
  active and B/C indexed or C active after physical-only A/B completion.
- Both same-stream and cross-stream layouts for those eight combinations.
- Same-stream middle-submission flush with terminal error and unwind, including
  the reproduced pending-owner loss after removal from the index.

These check exact active/terminal device-owner identities, pending recipes and
rosters, native completion records, FIFO state, lane leases, dependency/module/
event retains and completion reservations. Context retains six reader records,
three writer records, its event and callback, and five allocation charges totaling
320 requested bytes. Credits move from retained to quarantined without refund or
poisoning the credit account. Valid local queries remain Pending; repeated public
ingress and cleanup remain inert. Callback reference counts check retention as
well as nonexecution.

Five additional backend cases cover explicit observation, ordered observation,
ordered progress, and failed-explicit handling behind an active FIFO predecessor
through both observation and progress. Direct observer cases intentionally enter
the adapter after the SPI's normal ownership move, bypassing its earlier blocker
poll. Dependencies and cancellations are admitted through the real SPI.

Final snapshots clone launch values, not owning Arcs, and retain weak recipe
references plus original boxed-roster addresses. They cannot themselves keep the
pending recipe alive or accept replacement roster storage. Every subprocess
checks custody before explicitly dropping the unrepaired Context/backend. Its
parent requires that inspection marker and SIGABRT; core dumps are disabled.
No fixture repair, Drop disarming or parent-process owner leak is used.

The test-only panic hook applies only to scripted three-binding execution before
`active.take()`. The guard protects the waiting node, not every panic point of the
operation being polled. A dedicated conflicting-SDMA-copy panic case and full
native-owner unwind qualification remain open.

## Qualification

Final signed-source command, with no failed target skipped and no concurrent
compilation/proof campaign during Worker deadline tests:

```sh
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test \
  -p fe2o3-runtime -p fe2o3-runtime-model --all-features --no-fail-fast
```

- Runtime unit tests: 1,718 passed, 3 failed, 28 ignored.
- Model unit tests: 1,080 passed, 0 failed, 19 ignored.
- Runtime integration tests: 11 passed, 3 hardware tests ignored.
- Runtime doctests: 52 passed. Model doctests: 29 passed.
- Strict all-feature/all-target runtime/model Clippy passed.
- Formatting and runtime/model no-default-feature checks passed.
- Final source continuity against the signed commit passed.

The complete test command exits 101. The unchanged failures remain at
`authorized_execution.rs:1317`, each with `InspectSocket(PermissionDenied)`:

- `cooperative_debug_telemetry_emits_only_bounded_logical_records`
- `failed_session_end_is_explicit_and_terminal`
- `pre_native_telemetry_failure_is_returned_and_poisoned`

These are failed, not waived. No permissions or escalation were requested.

The archive preserves development failures: an initial snapshot incorrectly
expected Device instead of authenticated H2D-ready storage; the public flush
regression failed before the production fix; the additional ordered fixture
initially allowed its producer to complete during admission. Corrected focused
runs passed all 15 child cases, and both parent tests passed in `qualified-test`.
Earlier `final-test` receipts describe the initial test-only commit; the final
production fix is bound by `guard-source-*` and `qualified-*` receipts.

## Open Gates

No GPU, matched HIP/HSA performance measurement or new formal proof ran here.
The complete queued-owner representation/invariants, admission/activation,
release/refunds/disposal and active-then-queued Context release composition still
need qualification. The wider Worker/device-language/multi-device/memory/atomic/
collective/profiling objective remains open. See the
[successor plan](../../runtime-successor-writer-plan-v1.md) and the separate
[conditional resolution proof](../../runtime-queued-read-resolution-proof-v1.md).

MI300X SSH and pushes of both source checkpoints to both GitHub remotes failed
DNS resolution. No remote jobs or files were created. Unrelated inspection
evidence remains untouched. Publication of the evidence commit is attempted
separately after freezing this packet.

`receipts.tar.xz` contains source identities, signatures, patches/digests,
commands, timestamps, exit statuses and development/final logs. Its companion
SHA-256 identifies the archive.
