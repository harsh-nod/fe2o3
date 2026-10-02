# Ordered Multi GPU Gather

Implementation baseline: `dd3fad75f7c471b9847de48a8e546b3c3a2cb171`.
Status: current-source CPU and finite native qualification passed on 2026-10-02 UTC.

## Scope

Producer-aware compute outputs on several devices can feed ordered peer copies
into one initialized destination allocation. All compute, peer copies and a final
whole-allocation readback can be admitted before explicit progress. Each peer
names its exact pending compute producer. A successor also names the exact
latest destination writer; pending destination predecessors must share the peer
stream. Only successful completion and restoration of the original owners
authorize a successor. Released public events do not remove retained dependencies.

The existing version journal queues the writers and final reader. Each checked
partial write preserves bytes outside its window. This initialized-frame contract
allows whole-allocation readback without pretending that the last partial copy
wrote every byte. Whole-owner custody remains serialized, including overlapping
windows. Backends must explicitly opt in; the default is false. Source compute
bindings remain exact full-allocation Write operations. No Worker schema, kernel
authority, shared model or proof body changes are included.

The live sharded vecadd witness adds `--gather` and `--gather-overlap`. The first
devices compute separate source shards; the last device is a dedicated sink.
Two batches change inputs while reusing the Context, allocations, modules,
kernels and streams. Source windows have unequal lengths and nonzero offsets;
destination windows are either separated by guard bytes or overlap in dependency
order. The sink starts with A5 bytes. One full-sink D2H writes into a larger
5A-initialized host allocation with 13-byte and 29-byte guards.

The final readback stream alone drives progress after public event release.
The witness checks all source bytes, the complete destination and the full host
allocation, exact completion callbacks, native logical peer-copy counters,
retained results and release before the next batch. An independent Python oracle
derives the source arithmetic, ordered byte overlays and domain-separated,
length-framed SHA-256 values. Expected computed output is not uploaded.

## CPU Qualification

The complete current-source runtime suite passes 2,181 tests with the unchanged
32-test hardware-ignore roster. All 21 added tests pass: eight Context gather
tests, six Context readback tests and seven native-backend scripted tests. They
cover exact latest events, opt-in and stream identity, 2/3/5-writer queues,
overlap and guard preservation, final-readback-only progress, canceled and failed
ancestors, queued references, active epochs and lineage, bounded ranks, disposed
predecessor sources, refunds, terminal errors and unwind custody. A seven-peer
regression checks linear predecessor visits within each validation traversal.
Scripted native owners test routing and custody, not compute arithmetic or DMA.

The first full runtime attempt passed 2,179 tests and failed two new backend
tests. Both used incorrect observation boundaries: a stream-wide flush could
already acquire the successor's source, and a failed descendant need not observe
an independent pending ancestor. The tests now stop at the exact predecessor
completion and explicitly reconcile that independent ancestor, respectively.
Production code did not change for these corrections. The final build and full
suite were rerun; failed and superseded receipts remain in the archive.

Strict Clippy, no-default/hardware-qualification checks, formatting and whitespace
checks pass. Directed/deferred/live-sharded examples pass 11/8/7 tests, for 2,207
fresh library/example passes. Selected CPU receipts are
`attempt-02/{build,runtime,checks}`. Cargo commands explicitly use four jobs;
the retained environment and exact argv record that override.

All 32 source-control commands pass in `source-ci/attempt-02-after`. The reviewed
refresh changes only 18 hash literals and seven source counts in nine files;
all 76 historical proof-closure files remain unchanged. The four changed runtime
adapter method bodies are separately retained and reviewed; the refresh does
not assert their formal qualification or weaken solver and mutation predicates.

`reuse-check.py` authenticates the prior native-peer-subranges archive, complete
1,934-pass/no-ignore KFD result, exact unchanged KFD test executable, 3,499
conservative dependency-source pins and 14 arithmetic proof inputs. This is
historical evidence reuse, not a fresh KFD test or solver execution.

## Native Results

All 20 MI300X cases pass: eight new gather cases and 12 current-source controls.
The campaign ran from 18:35:12 to 18:45:44 UTC on 2026-10-02. New cases use GPUs
5/6/7 with two compute sources, and GPUs 4/5/6/7 with three compute sources, each
with a dedicated sink. Both device orders and disjoint/overlapping windows pass.
This is a finite partial-shard gather profile, not execution of the entire
original N-shard vecadd workload.

Each new case executes two changed-input batches, with all submissions queued
before explicit progress, released public events, final-readback-stream-only
progress and no intermediate host joins. Two-source cases observe native peer
completion counts 0/2/4 and ten completion callbacks; three-source cases observe
0/3/6 and fourteen callbacks. Complete source, sink and host bytes match the
independent oracle, including ordered overlap and untouched guards.

Controls cover ordinary and late pending-compute windows, three-GPU directed
peer-window readback, both full-buffer deferred-chain variants and repeated
two-GPU live sharded execution, all in both device orders. The gather executable
SHA-256 is
`f0463b3bd97a13e6be4cf478f2826e0d9d6b10f19da7582cf594c8a3628901dd`.
All three uploaded executable identities agree with the local builds before
and after the campaign.

Fresh UID/BDF, activity, VRAM, complete process/attachment census and host-memory
checks bracket every case. Final observations match the shared-host baseline.
No owned executable process remains; all three uploaded binaries and
`/tmp/fe2o3-queued-gather-20261002-REWOoP` were removed. No device reset, foreign
process termination or native fault injection occurred.

## Verification Boundary

The unchanged queued-writer model requires authenticated dependencies and permits
journal activation only after recorded predecessor success; queued readers
resolve the activated writer's actual epoch and lineage. The new runtime adapter
supplies the exact destination predecessor and initialized-frame premises. Its
expanded Copy validation and acquisition are not newly proved by the historical fold and
observer proofs. No new solver execution, whole-adapter refinement, machine-code
proof or DMA-engine proof is claimed.

Lower KFD source and arithmetic proof inputs are unchanged. Historical KFD and
arithmetic evidence is authenticated separately from newly executed runtime and
example tests; those historical passes are not counted as fresh results.

## Provenance

The working evidence root is
`/home/harsh/.codex-tmp/fe2o3-queued-gather-20261002`.
The accepted final receipt is `audit-01.json`. It rechecks current source and ELF
identities, complete test rosters, exact source-control commands, authenticated
historical reuse, and each native case's argv, oracle, admission and cleanup
chronology. The auditor reuses the reviewed Python oracle; it is not a second
independent host oracle and does not rerun hardware or a solver.

`raw.tar.xz`, `raw-manifest.json` and `SHA256SUMS` retain commands, outputs, exits,
source inventories, controllers, failed/superseded attempts and the source
`candidate.patch` against the baseline above. No test executable or verifier
distribution is included. Follow-up notes prioritize gathered-frame input to
queued compute, a genuine compiled-kernel evidence provider and the generated
application's PUBLIC multi-device binding. Those notes describe remaining work,
not newly implemented or qualified behavior.

## Limits

This is an ordered gather profile, not arbitrary dependency-graph support or
concurrent slice ownership. Compute may publish eagerly during admission;
all-before-explicit-progress does not establish physical concurrency. The native
compute witness retains finite qualification authority, not a general production
compiler/effects proof provider. A concrete production artifact/provider bridge
remains required for ordinary applications. Native fault qualification,
physical overlap and matched HIP/HSA performance parity are outside this
increment. Shared-host point observations do not constitute an exclusive GPU
reservation. Guards cover logical allocation bytes, not hidden physical padding.
