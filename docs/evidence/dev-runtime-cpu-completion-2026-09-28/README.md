# Runtime CPU Completion Custody

Follow-up to [runtime CPU receipt composition](../dev-runtime-cpu-receipts-2026-09-28/README.md).
This qualifies additional backend-SPI completion and failure paths with genuine
lower CPU receipts. It is not native CODE/DATA binding, signal currentness,
Context/Worker qualification, formal correspondence or HIP/HSA parity evidence.

## Source

Signed source: `f4b8985e7409055c1c22e9eef4c81de97a90ef96`.
Tree: `66c6e3b9e90bc69de48474ef9056635dc471fe80`.

Both remotes' `codex/r65-runtime-drain-versions` heads were observed at this source
after publication at 17:55:06 UTC (harsh-nod) and 17:55:09 UTC (powderluv).

## Changes

The test-only CPU lane records which real lower operation returned. An outer
Error/Unwind is consumed only after the matching Submit/Poll/Recycle callback
returns and the runtime has deposited its receipt/phase. Recycle faults survive
a preceding Ready poll. Native dispatch code and callback signatures are unchanged.

A one-shot pin control publishes through the existing lower `submit_pinned`
fixture, creating genuine bound-event pins. It grants no arbitrary receipt or pin
constructor. Unclassified pin-setup errors are conservatively terminalized by
runtime callers, not treated as a lower classification oracle. The tests use
successful pin setup and genuine recycling failures. A borrow-only observer counts
actual `SignalPinned` errors with one event pin and zero native-reader pins while
returning the original failure and completed receipt unchanged.

## New Coverage

Three new groups extend the seven inherited runtime composition groups:

- Four primary/AUX, frontier/pipeline scenarios repeatedly poll genuine Completed
  pinned receipts. Exact receipt identity, first-Ready timing, runtime custody and
  lower snapshots survive each retry. Pipeline A and B are both Completed/pinned,
  so each B poll observes exactly two actual pin refusals and no signal I/O.
  Unpinning B retires it physically without logical success; unpinning A allows
  ordered logical completion. Public releases, shutdown and ordinary Drop follow.
- Forty-eight isolated completion cases cross six states, primary/AUX lanes,
  frontier/pipeline positions, and Error/Unwind. States are Pending poll, Ready
  poll, Ready-to-pinned-Recycle, Ready-to-successful-Recycle, already-Completed
  pinned Recycle, and already-Completed successful Recycle. They preserve exact
  Published/Completed receipts or actual Retired observations without logical
  settlement, retain release or completion events. Pipeline entries quarantine.
- Eight isolated Submit outer-fault cases cross initial flush/Prepared retry,
  primary/AUX, and Error/Unwind. Genuine signal saturation produces Retryable,
  which remains indexed in the unconfirmed binding root rather than becoming
  Prepared after a failed outer close. Pending handoff happens once, trailing
  Pending custody survives, and no new receipt, publication event or result appears.

Completion cases frame immutable Active metadata, recipe/descriptor storage,
retained allocations, module/dependency/event retains, reservations and trailing
Pending records. New host Ready timing must fall between elapsed-time observations
around the public poll; previously recorded Ready timing remains exact. Publication
time is unchanged, and failed outer close does not accumulate recycle duration.
Post-terminal poll/cancel/shutdown refusals cannot mutate retained custody.

Passive identity/snapshot checks need no post-poison authority. A Retired
observation has packet count, not the former receipt identity; its provenance is
framed by genuine recycle and the captured lower state. Snapshots do not include
the full dependency ledger or completion-owner phase. Terminal children suppress
core dumps, require explicit completion markers, and retain poisoned state until
process exit. They do not demonstrate recoverable cleanup.

## Remaining Work

Prepared cancellation, native binding/rebinding, device writes/readback,
currentness faults, populated-arena ordered retry, shared-source caller refinement
and retained-resource/Context proofs remain open. Protected Worker/compiler,
multi-device/distributed qualification, atomics/collectives and matched HIP/HSA
performance remain separate outstanding work. This does not promote A1/A2 or
accepted Native R125, Admission R118B, Resources R116/V3 checkpoints.

No solver, MI300X, GPU workload, benchmark or remote cleanup was run. Existing
native behavior and lower KFD bodies were not changed or newly qualified here.

## Qualification

The frozen source run completed on 2026-09-28 with aggregate status zero:

| Gate | Result |
| --- | --- |
| Focused CPU receipt groups | 10 passed, 0 failed or ignored |
| All-feature runtime library | 1816 passed, 0 failed, 28 existing ignored |
| Runtime doctests | 8 compile-positive and 44 compile-fail passed |
| Strict all-feature/all-target Clippy | Passed |
| Runtime no-default-feature check | Passed |
| Workspace formatting and diff check | Passed |
| Signed source and implementation continuity | Passed |

The focused groups include inherited cases as well as the four positive scenarios,
48 completion-fault children and eight Retryable-fault children added here. Those
56 new terminal children are not the total child count across all ten groups.
Issue #182 was observed open through the GitHub API at 17:52:25 UTC. No milestone
closure follows from these CPU-only checks.

## Reproduction

The archived `qualify.sh` serializes all ten focused groups, the full all-feature
runtime library suite, doctests, strict all-target Clippy, no-default-feature
checking, workspace formatting, source/signature/continuity checks and issue #182
API observation. No implementation changes during the frozen run. Focused counts
overlap the full suite; CPU test timing is not a GPU performance measurement.

Raw campaign: `/home/harsh/.codex-tmp/fe2o3-runtime-cpu-completion-20260928-qsZoMdU0`.
`receipts.tar.xz` retains preparatory results, final logs, review notes and source
publication observations. Verify with `receipts.tar.xz.sha256`. Raw files are frozen
after archive creation; documentation publication receipts are stored separately.

Archive SHA-256:
`8cf4ab2b234c6f1cad504eb3f67a438afcde09570c8107386e321e788251c623`.
