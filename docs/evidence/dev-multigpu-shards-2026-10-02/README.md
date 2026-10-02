# Fixed-Total Multi-GPU Shards

Increment above `bafbfe73981aa177fc052a4dcc7120c66a34cfb6`, prioritizing A3.
This is not A3 completion, HIP/HSA parity, physical-overlap evidence or a
performance result. Five MI300X native ring cases pass on 2/3/5/7 devices,
including both seven-device directions. GPU 0's foreign work was left untouched.

## Implementation

The ordinary public native-peer constructor admits 2..8 explicit distinct GPUs
with a deny-all compute authority. The new
`gfx942-runtime-sharded-peer-copy-smoke` example partitions one fixed
67,108,901-byte payload into balanced, contiguous, nonempty shards. GPU i sends
its outgoing shard to a separate incoming allocation on GPU (i+1) modulo N.
One current-thread owner queues all N peer operations before driving any of them.
Neighboring routes may serialize because they share child ownership.

Two changed-content rounds reuse all 4N allocations and N streams. Both rounds
check every source and destination byte before and after copying, plus ordered
whole-payload SHA-256 digests and exactly N additional native logical completions.
Packet counts are derived from the production packet plan, not raw queue telemetry.
Explicit submission/resource release precedes completed-group drain and native
shutdown. No pending-peer group-drain claim is made.

The multi-device backend also forwards Context-authorized single-range host
capture. It rejects terminal or live execution custody on any child, preserves
the exact device identity and translates only the allocation handle, then uses
the existing native host-storage reader. Completed results and events may remain
retained. The path does not synchronize, materialize a shadow or allocate capture
storage. This forwarding increment is CPU-tested, not hardware-qualified.

## Verification

- Full final-source runtime library: 2,011 passed, 32 existing hardware ignores,
  zero failures and zero filtering.
- Strict combined Clippy, no-default checks, formatting and all 19 example tests
  pass, including seven new sharded-witness tests. All three runnable examples build.
- New CPU ring tests cover 2/3/5/8 children, uneven fixed-total shards, shared-child
  progress, cancellation, deadline resumption, terminal error/unwind custody and
  explicit cleanup. Scripted storage is not native DMA evidence.
- Six new capture-router tests cover exact translation, all-child preflight,
  retained completed native/cooperative results, terminal propagation and unwind.
- All 32 source-control workflow commands pass. Nine metadata files change only
  16 SHA literals and seven roster counts; 76 proof files remain unchanged.
  No new solver run or formal refinement is claimed.
- Prior KFD qualification is reused only through an authenticated committed
  archive, unchanged KFD source inventory, identical test roster and exact test
  executable SHA-256: 1,925 passes, not a new KFD test run.

The initial build compiled successfully but failed its enclosing source-continuity
check because the final fixture/example edits overlapped compilation. Its focused
tests are diagnostic only. The replacement `attempt-02` uses frozen source.

## Native Results

| Case | GPUs | Logical native copies | Full-buffer readbacks |
| --- | --- | --- | --- |
| Two-device ring | 6, 7 | 4 | 16 |
| Three-device ring | 5, 6, 7 | 6 | 24 |
| Five-device ring | 3, 4, 5, 6, 7 | 10 | 40 |
| Seven-device ring | 1, 2, 3, 4, 5, 6, 7 | 14 | 56 |
| Reverse seven-device ring | 7, 6, 5, 4, 3, 2, 1 | 14 | 56 |

Every case copies the same fixed total, not N replicas of the workload. The
controller independently computes both complete-payload digests in Python and
requires every PASS field to match. All run stderr streams are empty. Fresh
UID/BDF, utilization, memory, counted PID-to-device mappings and host-memory
checks precede every case. Point observations are not exclusive reservations.

The local/uploaded/final witness SHA-256 is
`64c093ed8166adaaba3cac3687f16ed7f509740bf68da708ced6493112cb6c38`.
The final runtime test executable SHA-256 is
`2d47af71604ef15d223e4a3f4d2fd9b9d3700b2834bd01deb4b5875367f75891`.
Final checks find no owned process; the single uploaded executable and exact
scratch directory are removed and absence checked. Selected GPU memory and the
whole process roster return to baseline. No reset, foreign process termination
or unrelated file cleanup is performed.

`raw.tar.gz`, authenticated by `SHA256SUMS`, contains source reconstruction,
immutable command/output receipts, selected `attempt-02`, source-CI proposal01
and all 32 commands, the five-case hardware campaign, independent `audit.py`,
and `qualification.json`. The audit checks actual rosters, commands, source and
executable identities, metadata-only AST changes, prior archive members, hardware
admission, chronological execution and cleanup. It does not accept summary flags
in place of their underlying evidence. `next-work.md` decomposes the next gate.

## Remaining Work

Pending peer-to-D2H continuation, bounded multi-range capture after admission
cutoff, genuine compute sharding under a finite kernel authority, native partial
failure campaigns, and matched HIP/HSA scaling remain open. The current 64 MiB
single-range capture cap cannot contain this whole payload. GPU 0 has foreign
work; eight-GPU hardware execution is not authorized by idle utilization alone.
