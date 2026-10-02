# Sharded Multi-GPU Compute And Capture

Increment above `76dbbb03d65529d28bb6d15d2970d020cc7ef48b`, prioritizing A3.
Ten MI300X cases pass: genuine sharded compute, joined native peer transfer,
pending event-bound D2H and group capture. A3, general kernel authority,
HIP/HSA parity, physical overlap and whole-adapter refinement remain incomplete.

## Implementation

The hardware-qualification-only indexed constructor admits two to eight distinct
device UIDs and all finite recipes before opening native devices. Each child has
an independent atomic one-shot gate. The separate policy pins exact source,
object, ABI, geometry, full allocation identities and initial content digests.
Old vecadd, repeat and R57 policies/artifacts are unchanged.

Exactly 70 recipes cover each shard of N=2..8 for rounds 0 and 1. Every workload
contains 65,537 f32 elements. Uneven shards retain full page-padded allocations
and bindings; the 48-byte ABI carries logical lengths and the grid rounds to
256-thread workgroups. Existing full-extent readiness is not weakened.

The witness checks HostVisible inputs before upload and preserves authenticated
H2dReady input owners through compute authorization. It admits all N launches
before explicit progress (admission may publish eagerly), joins exact success
and owner restoration, then queues all N native ring copies and N dependent
readbacks before one group cutoff. Public peer events are released before it.
No host-computed expected output is installed into the device after launch.

## Native Results

Each row runs both changed input rounds, each in a separate process. Every run
checks every logical output and padding byte, exact N compute plus 2N transfer
completion receipts, quiescent drain, explicit owned shutdown and result-credit
disposal. The meaningful gathered output is always 262,148 bytes; global digests
are independently derived by both the controller and final auditor.

| Profile | Ordered GPUs | Processes | Compute successes | Peer/readback successes each |
| --- | --- | --- | --- | --- |
| Two devices | 6, 7 | 2 | 4 | 4 |
| Three devices | 5, 6, 7 | 2 | 6 | 6 |
| Five devices | 3, 4, 5, 6, 7 | 2 | 10 | 10 |
| Seven devices | 1, 2, 3, 4, 5, 6, 7 | 2 | 14 | 14 |
| Reverse seven | 7, 6, 5, 4, 3, 2, 1 | 2 | 14 | 14 |

Every run has fresh pre/post UID/BDF, activity, memory, counted PID-to-device and
host-memory checks. GPU 0's foreign work is untouched. Owned executable processes
are absent; uploaded files and the exact scratch directory are removed and their
absence checked. Selected GPU memory and the whole process roster return to
baseline. Point observations are not an exclusive reservation.

Local/uploaded/final witness SHA-256:
`b1c83337877a6a235c8219dbc5aaef65a39cc5b02b26202d4e8411618af86cf4`.
Runtime test executable SHA-256:
`dd060e2e827821e484ea3c9d5004a4138c36a6a75bb002e43353aeb3e5fb3fe4`.

## CPU And Source Qualification

- Runtime: 2,048 passed, zero failures/filtering, 32 unchanged hardware ignores.
  All prior tests remain; eight policy and four backend tests are added.
- Examples: 31 passed, including eight new partition/ABI/reference/gather tests.
  Scripted CPU completion tests do not simulate vecadd output or prove native DMA.
- Strict Clippy, no-default and hardware-only feature checks, formatting and
  whitespace checks pass. All 32 exact source-control workflow commands pass.
- Ten source-control files update only 17 SHA literals and seven inventory
  counts. All 76 associated proof files and their contracts remain unchanged;
  no new solver or whole-adapter refinement qualification is claimed.
- KFD: the previous 1,925 passing tests are reused through byte-identical current
  executable/source/roster and authenticated prior raw receipts. This is not a
  fresh KFD suite run.
- The pinned ROCm 7.2.4 MI300X compiler reproduces the existing vecadd object
  byte-for-byte. Rebuild inputs, exact commands/hashes and owned cleanup have
  separate receipts. Reproducibility does not prove compiler or kernel semantics.

## Evidence And Limits

`raw.tar.gz` contains `fe2o3-multigpu-compute-shards-20261002/`. The final auditor
selects CPU `attempt-03`, metadata `proposal-02`, source workflow
`attempt-01-after`, hardware `hardware-01` and `rebuild-01`. It independently
checks exact rosters, command/environment/source/executable identities, prior
qualification reuse, output digests and cleanup before emitting
`qualification.json`. `SHA256SUMS` binds this README and the raw archive.

Non-selected diagnostics are preserved: the first build found two unsupported
test iterator calls; the next focused run found an incorrect test expectation
that read-only H2dReady inputs would become Device owners. The corrected test
requires the exact read/write variants and original identities/content. Review
also removed precompute native input readbacks that would clear required
digests, before any hardware campaign. No runtime guard was relaxed.

This is finite trusted-artifact qualification, not arbitrary application kernel
authority or machine-code refinement. Compute is explicitly joined before peer
admission; physical overlap is unmeasured. Post-cutoff native counters and
post-copy source preservation are unobserved. Each round uses a fresh process:
there is no across-round allocation reuse or same-process Context reopen. Eight
GPUs, native partial-failure isolation and matched HIP/HSA performance remain
unqualified. Next priorities are exact prequeued compute-to-peer composition and
repeated useful batches within one live Context, not resetting VM admission.
