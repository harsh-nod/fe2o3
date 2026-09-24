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

Allocation reduction is not a measured copy-latency gain. Matched native testing
must bind signed baseline and candidate sources, identical release settings,
exact endpoint identities and fresh admission before every process. The planned
1-MiB, depth-one campaign uses one prime, ten warmups and thirty samples per
direction. Diagnostic baseline/candidate/candidate/baseline trials are separate
from reversed-order diagnostics-off trials containing current HIP/HSA controls.

Every process requires complete canaries, explicit teardown and settled/delayed
endpoint checks. Shared-host observations are not an exclusive reservation.
Aggregate accounting, broad fault qualification, production executable
refinement, A1/A2 and HIP/HSA parity remain open.
