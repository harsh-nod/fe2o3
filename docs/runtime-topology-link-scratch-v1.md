# Fresh Topology Link Scratch Reuse

Each IO or P2P link-set traversal now reuses one local byte buffer for its
properties files. The buffer is discarded when that traversal returns. No
topology, currentness, link, generation or device observation is cached across
operations or even across link sets.

The shared bounded reader clears the buffer before opening each observed file.
It establishes the existing initial capacity only after the opening file
identity check. All callers still use no-follow/nonblocking open, opening
metadata comparison, a bounded read, and the closing metadata comparison.
Oversize still precedes closing identity rejection, which still precedes UTF-8
validation. Parsing borrows the checked text and returns only owned integers.
Other readers retain their fresh-buffer wrapper. Empty link sets allocate no
text buffer.

For L valid links across S nonempty sets, this removes L-S text-buffer
allocations per discovery. The previous String conversion was zero-copy and
must not be counted as another allocation. The largest valid link text is 367
bytes, below the unchanged 1024-byte initial capacity. Invalid text still stops
the traversal immediately; no failed observation can authorize the next link.

The focused tests cover long-to-short reuse, residue after read/parser failures,
refusal before read storage allocation, bounded I/O observation order, competing
error precedence, and actual allocation counts. Existing discovery, fixed-schema
differential, hostile-file and currentness tests remain applicable. Diagnostic
traces describe Rust I/O boundaries, not exact kernel read syscall counts or
sizes. This change introduces no new unsafe code or formal-proof claim.

## Performance Boundary

Allocation reduction is not a demonstrated copy-latency gain. The
[matched MI300X comparison](evidence/dev-topology-link-scratch-mi300x-2026-09-24/README.md)
binds signed baseline/candidate sources, separate cold builds, matching release
settings, exact endpoint identities and fresh admission before every process.
It uses one MiB at depth one, one prime, ten warmups and thirty samples per
direction. Diagnostic baseline/candidate/candidate/baseline trials remain
separate from reversed-order diagnostics-off trials with HIP/HSA controls.

Candidate process/direction p50 summaries span 14.319-14.348 ms, overlapping the
baseline's 14.305-14.388 ms. HSA spans 30.155-30.806 us and HIP 38.347-38.678 us
on the declared native API surfaces. No scratch-reuse latency gain is established.
The independent diagnostic population still attributes approximately 94.6-94.7
percent of backend time to fresh topology discovery. The initial shared-target
campaign is retained as rejected history and contributes no timings.

Every process requires complete canaries, explicit teardown and settled/delayed
endpoint checks. Shared-host observations are not an exclusive reservation.
Aggregate accounting, broad fault qualification, production executable
refinement, A1/A2 and HIP/HSA parity remain open.

## Next Performance Work

Qualify matched persistent-hot batches at depths sixteen and thirty-two. The
KFD aggregate path already batches the full submission roster, but HIP/HSA's
current hot callbacks access slot zero only. Before relaxing any benchmark depth
guard, their preparation, enqueue/completion and full source/destination guard
validation must cover every slot. Enqueue the whole batch before waiting; retain
distinct streams/signals, slot-qualified patterns and the existing no-host-access
timed lifecycle. Keep diagnostic modes depth-one scoped until separately extended.

Fresh pair-only topology reads are not a drop-in optimization of the current
API. `currentness/full.rs` compares a fresh complete `HostTopologySnapshot`
against both retained snapshots, intentionally detecting unrelated-node drift.
Generation equality cannot replace property observations under that contract.
A narrower pair-projection API would need an explicitly versioned predicate,
new route/currentness model identities, R9/R28 proof coverage, custody auditing
and hostile mutation tests. Preserve full admission and full batch checks while
developing that separate contract; do not report projected reads as full audits.
