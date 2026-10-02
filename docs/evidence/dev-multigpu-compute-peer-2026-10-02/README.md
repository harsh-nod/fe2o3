# Queued Multi-GPU Compute, Peer Transfer And Capture

Increment above `f0eaff8efc2cd53e4f8da4e12a56881a74537ca9`, prioritizing A3.
Ten queued MI300X cases and two joined controls pass. Exact compute -> native
peer -> D2H chains now run without a host-side compute join. A3, general kernel
authority, HIP/HSA parity, overlap and whole-adapter refinement remain incomplete.

## Implementation

A default-false backend capability enables journal-aware admission of an exact
pending producer-aware compute writer as the input to an ordinary scalar peer
copy. Its retained Write binding covers the entire DeviceLocal source; an explicit
event names the current writer. Other control dependencies must already succeed.
Partial ranges, ReadWrite/Read aliases, queued journal writers and pending
router-deferred compute are outside this profile.

The Context retains the existing journal read lease and authenticates every
descended completion node before observation. Checked parent ranks bound recursive
validation and reject invalid forward/self edges. Compute is actually observed
before peer logical success; dependent D2H retains the complete checked chain.
Public event release cannot discard retained producers.

The multi-KFD backend retains one accounted immutable launch recipe even for
eager persistent compute. Peer admission prepares route metadata without
extracting owners, acquiring endpoint occupancy, allocating host payload staging
or publishing DMA. Explicit readback-stream progress services the exact compute
producer and necessary FIFO prefix. Native transfer requires successful quiescent
completion, both owners' restoration and concrete pre-extraction checks.
An admitted native candidate never silently falls back to host staging.
Uncertain failures retain ownership; pre-effect cancellation releases only
consumer custody. Scripted completion does not count as native DMA.

The existing finite 70-recipe policy and object remain unchanged. The witness
adds leading `--queued-compute` while preserving the default joined schema.
All compute, peer and readback submissions are admitted before the group cutoff.
Only final readback streams are registered for progress, and all 3N exact
callbacks must succeed once. Admission may eagerly publish compute; no physical
overlap or effects-free admission claim follows.

## Native Results

Each queued profile runs both changed-content rounds in separate processes.
Every run partitions the same 65,537 f32 elements, checks all logical output and
padding bytes, verifies the independent global digest and explicitly shuts down.
No expected computed output is installed from the host.

| Profile | Ordered GPU ordinals | Queued runs | Compute/peer/readback successes each |
| --- | --- | --- | --- |
| Two devices | 6, 7 | 2 | 4 |
| Three devices | 5, 6, 7 | 2 | 6 |
| Five devices | 3, 4, 5, 6, 7 | 2 | 10 |
| Seven devices | 1, 2, 3, 4, 5, 6, 7 | 2 | 14 |
| Reverse seven | 7, 6, 5, 4, 3, 2, 1 | 2 | 14 |

Joined controls cover two GPUs in round 0 and seven GPUs in round 1. Across all
twelve processes, 57 computes, 57 peers and 57 readbacks have exact successful
receipts. The meaningful global output is always 262,148 bytes. Fresh pre/post
UID/BDF, activity, memory, counted PID-to-device and host-memory checks pass.
GPU 0's foreign work is untouched. The exact uploaded executable and owned
scratch directory are removed, their absence is checked, and GPU/process
baselines are restored. Point observations are not an exclusive reservation.

Local/uploaded/final witness SHA-256:
`adee9d8e589445de6f42912e93fc1ebc8ac260fe751ea57030696b184f19bffd`.
Runtime test executable SHA-256:
`5d990d67637550eaf1b58cc211295c179b3711cae1bafbd0cbf3da01cc7aaa2c`.

## CPU And Source Qualification

- Runtime: 2,069 passed, zero failures/filtering, 32 unchanged hardware ignores.
  All prior tests remain; twelve Context and nine backend tests are added.
- Examples: 34 passed, including three new queued-mode/schema/receipt tests.
- Strict Clippy, no-default and hardware-feature checks, formatting and
  whitespace checks pass. All 32 exact source-control workflow commands pass.
- Nine guard files refresh only 18 SHA literals and seven inventory counts.
  All 76 associated proof files, predicates and proof counts remain unchanged.
- KFD: 1,925 previous passes are reused through exact current executable,
  source, roster and authenticated original shard receipts, not freshly rerun.
- The unchanged policy/source/object and earlier pinned ROCm compiler rebuild
  are authenticated reuse, not a new compiler invocation or semantics proof.

The historical empty-hook completion-planner proof does not qualify the new
per-node adapter. Shared bodies and source checks do not establish whole-adapter
or machine-code refinement. Scripted fault tests do not replace native fault
injection, and scripted compute does not simulate or qualify GPU arithmetic.

## Evidence And Limits

`raw.tar.gz` contains `fe2o3-multigpu-compute-peer-20261002/`. The final auditor
selects CPU `attempt-04`, metadata `proposal-02`, source workflow
`attempt-04-after` and `hardware-01`. It independently checks exact rosters,
commands, minimal environment, source and executable identities, historical
reuse, output digests and cleanup before emitting accepted `qualification.json`.
The auditor SHA-256 is
`ec314449f382dc9c73efa2ff14b8cfbdea255a500d771a8b1bc03d7f5f3a136a`.
`SHA256SUMS` binds this README and archive.

Non-selected diagnostics preserve the initial missing import, invalid dirty-input
queued fixture, Scripted/native counter distinction, mock-reader expectation
and cancellation-flush expectation. These were corrected without weakening
production readiness or Native-only counting. Diagnostic inherited environments
were sanitized before publication; selected runs use a minimal environment and
an exact allowlist audit. `diagnostics.md` records the scope and corrections.

This is finite trusted-artifact correctness, not general application authority.
Post-cutoff native counters and post-copy source preservation remain unobserved.
Rounds do not yet reuse a live Context or allocations. Same-process device reopen,
eight-GPU hardware, native partial-failure isolation, overlap and matched HIP/HSA
performance remain unqualified. The next priority is two changed-content queued
batches within one live Context before one final drain, without resetting VM
admission history.
