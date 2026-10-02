# Live Multi-GPU Compute Batches

Increment above `56d79d3ae58847c79ca6b0492f0f3f21e3d6883b`, prioritizing A3.
Five MI300X cases execute two changed-content compute -> native XGMI -> D2H
batches inside one live Context, reusing logical allocations, modules, kernels
and streams. Two existing queued controls also pass. This closes the finite
live-batch qualification gap, not A3, application authority or HIP/HSA parity.

## Implementation

The separate two-round gate composes unchanged one-shot authorities with a
distinct policy signature. It latches the original child-local A/B/C identities
and serializes inner acceptance and phase advancement under one mutex. Rejected
requests do not consume a phase; accepted requests never reopen after failure,
cancellation or uncertainty. Poison denies future authorizations. Existing
policies, artifacts and public production constructors remain unchanged.

The new example refreshes every padded input byte and incoming sentinel before
each batch. It admits all compute, peer and dependent readback submissions in
one bounded owner command, releases public events after admission and drives
only the final readback streams. It observes the original successful results,
then validates stream quiescence and reads at most 286,720 settled HostVisible
bytes in one owner command. Partial snapshots never escape on failure.
This is serialized settled reading, not a new coherent-capture API or a
drain-capture byte-credit qualification.

Every logical output and padding byte is independently checked in source-shard
order. No expected computed output is installed from the host. Results are
released readback -> peer -> compute before the next refresh. Completed command
futures are dropped to refund reply credits. An intentionally expired command
observation resumes the same accepted future without duplicate enqueue; this is
not a GPU timeout or hard-preemptible Context operation. Only the end of the
second batch creates a completed-only drain, followed by explicit native shutdown.

## Native Results

Each live process partitions the same 65,537 f32 elements per round. Inputs and
outputs change between rounds, without reopening the Context or replacing its
logical handles. The native completion count is observed as 0, N, then 2N.

| Profile | Ordered GPU ordinals | Batches in one Context | Compute/peer/readback successes each |
| --- | --- | --- | --- |
| Two devices | 6, 7 | 2 | 4 |
| Three devices | 5, 6, 7 | 2 | 6 |
| Five devices | 3, 4, 5, 6, 7 | 2 | 10 |
| Seven devices | 1, 2, 3, 4, 5, 6, 7 | 2 | 14 |
| Reverse seven | 7, 6, 5, 4, 3, 2, 1 | 2 | 14 |

The existing queued controls cover two GPUs in round 0 and seven in round 1.
All seven processes account for 57 computes, 57 peer copies, 57 readbacks and
171 exact successful callbacks. The 48 live-case peer completions have directly
observed native counters; the nine control peers retain their explicit
post-cutoff counter limitation. Both global logical digests and each roster's
full padded digests are independently recomputed by the auditor.

Fresh UID/BDF, activity, memory, counted PID-to-device and host-memory admission
checks pass before and after every case. GPU 0's foreign work is untouched.
Both exact uploaded executables and the owned scratch directory are removed;
their absence and restored GPU/process baselines are checked. Point observations
are not exclusive reservations or continuous attachment monitoring.

Live witness SHA-256:
`ca22588cd66ab611fced1f9e1af7018dcb097c23e975474aac2677d05dea325d`.
Queued-control witness SHA-256:
`239d8b480e156ff6921ef17e5f47d3aa8d4fa1fe869bc394bbf6d6a49e548393`.
Runtime test executable SHA-256:
`b80b85c2cfec1c7df29133017681333e36931c3bd6cc3effa26311168ea45556`.

## CPU And Source Qualification

- Runtime: 2,082 passed, zero failures/filtering, 32 unchanged hardware ignores.
  All prior tests remain; eight authority, three constructor/argument and two
  Context/owner regressions are added.
- Examples: 39 passed, including five new CLI, recipe, output/padding, bounded
  snapshot and exact-callback tests.
- Strict Clippy, no-default and hardware-feature checks, formatting, whitespace
  and all 32 exact source-control workflow commands pass.
- Ten guard files refresh only 17 SHA literals and seven inventory counts.
  All 76 associated proof files, predicates and proof counts remain unchanged.
- The prior 1,925 KFD passes are authenticated reuse through identical executable,
  source and original complete shard receipts, not a fresh KFD test run.
- The unchanged artifact/source and pinned compiler rebuild are authenticated
  historical reuse. No fresh rebuild or compiler-correctness proof is claimed.

The Context tests use actual mock operations and normal completion observation;
they do not qualify GPU arithmetic. Neither old component proofs nor the source
manifest refresh formally refine the new authority, example or whole runtime.

## Evidence And Remaining Work

`raw.tar.gz` contains `fe2o3-multigpu-live-batches-20261002/`. The final audit
selects CPU `attempt-03`, metadata `proposal-03`, source workflow
`attempt-03-after` and `hardware-02`. It validates exact test rosters, commands,
allowlisted environments, 5,862 source files, historical reuse, both executable
identities, all output digests and cleanup before accepting `qualification.json`.
Auditor SHA-256:
`f6bc8f780a880d1c30aca555a6455202390346f004f117e133ae2f426124dacc`.
`SHA256SUMS` binds this README and the raw archive. Diagnostics retain the initial
test-cleanup warning and formatting-only rejection without counting either as
selected qualification; the unused first hardware packet contains only a
read-only availability observation.
All 1,168 raw files were independently read back after archiving, and all nine
captured build/test environment files passed the explicit variable allowlist.

Stable logical handles do not independently prove physical mapping identity.
Source preservation after peer transfer, physical overlap, native partial-failure
isolation, eight-GPU hardware and matched HIP/HSA performance remain unqualified.
The finite gate does not authenticate general application/compiler effects.
Same-process device reopen remains unsupported and is not needed for these live
batches. Next prioritize a pending deferred-compute output feeding a native peer
copy, then an evidence-backed application kernel through the existing production
constructor. Do not substitute another fixture or a permissive authority for
that application evidence.
