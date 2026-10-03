# Ordered Destination Segment Lists

## Scope

This checkpoint extends `c37c04d60` with success-ordered native peer-copy lists
from settled sources into one initialized destination. Context requires the
version journal and the default-false `supports_ordered_peer_copy_segments_v1`
capability. Each successor supplies the exact latest destination-list event on
the same stream. It retains the predecessor result and immutable descriptor and
frame identities independently of public event lifetime.

Each source keeps its ordinary read lease. Queued writers serialize ownership
of the whole destination, preserving initialized holes, descriptor order,
duplicate descriptors and overlapping writes. A full-frame D2H or Read-only
compute consumer can depend on just the latest list. A list envelope does not
become scalar coverage or a fabricated compute producer.

Backend ancestry validation uses bounded, iterative traversal with older IDs,
decreasing frame ranks and exact retained plans. Transfer scheduling depth stays
separate from frame depth. A completed restored ancestor no longer needs its
disposed source; a pending ancestor still authenticates the original source.
Late consumer admission checks both native endpoint markers and child slots.

Failed or canceled writes retain conservative journal Unknown semantics.
Uncertain native outcomes keep the existing fail-stop custody policy. This
change neither adds concurrent destination slices nor composes pending-compute
source lists behind a pending destination-list writer.

## Qualification Design

The new copy-only example uses three distinct GPUs and production authorities
that deny every kernel. Two source lists and a guarded full-frame D2H are
admitted before explicit progress. Both public events are released after
dependent admission, and caller descriptors are overwritten after submission.
Two rounds in one Context change every source pattern and destination sentinel.
Full source, destination, initialized complement and host guard bytes are checked.

Disjoint, overlapping, duplicate and packet-spanning cases run with forward
and reversed three-device rosters. Two additional source-disposal cases settle
the first list, verify and dispose only its source while the second list and
readback remain Pending, then drive the final readback stream. The next round
allocates a distinct first source. Other owners are reused only after reverse
dependency-order release.
Native completion counts must advance `0 -> 2 -> 4` with six exact callbacks.

The existing finite R57 example adds two settled-source lists feeding its
unchanged destination consumer, followed by a native return peer and guarded
D2H. Sources contain different previously completed C and D results; all source
producer results are released before list admission. Four setup kernel launches
and one pipeline consumer launch are explicit. The pipeline checks full output
and guards, five callbacks and native counts `0 -> 3`, driving only the final
readback stream after admission. Late variants first observe one retained native
ancestor and zero completed copies before admitting the compute consumer.

The matrix has ten copy cases, six new compute cases covering 4, 65 and
4096 descriptors per list, and two existing scalar-gather regression controls.
An independent Python oracle reconstructs complete expected buffers, domain
separators and length-prefixed digests. Compute expectations use exact integer
quarters and are cross-checked by a separate float/byte implementation.

Every hardware case requires fresh device identity, attachment, activity and
memory checks on the shared host. Only the two uploaded executables and their
exact private scratch directory may be removed. No device reset or foreign
process termination is permitted. Point observations are not a reservation.

## Acceptance

Final generation-03 CPU qualification passes 2294 runtime tests with the same
32 hardware-only ignores, 40 example tests and 61 runtime doctests. Strict
Clippy, default/no-default library checks, all 32 source controls and eight
independent oracle/controller tests pass. The CPU auditor checks exact rosters,
source continuity, witness executable identities and literal-only guard changes.

The 1950 KFD tests are explicitly reused from the accepted recovery checkpoint,
not freshly executed or rebuilt here. The auditor replays all five historical
shard receipts and checks the identical executable, ten local dependency
packages, shared runtime bodies, fixture assets and build configuration. Only
separately audited, noncompiled guard metadata changes are exempted.

Native generation-03 qualification passes all 18 cases on MI300X GPUs 1/6/7
with forward and reversed rosters. Independent offline replay accepts all 165
transport receipts, exact executable identities, full-buffer digests, callbacks
and logical/native counts. Both uploaded executables and the owned directory
`/tmp/fe2o3-destination-segments-20261003-U7JOKO` were removed. No owned process
remained, and device/process baselines were restored without touching foreign
work. These rosters do not cover every directed pair among the three devices.

The aggregate auditor binds CPU and native acceptance to the same unchanged
source snapshot. The packet includes `raw.tar.xz`, its per-file SHA-256 manifest
and `SHA256SUMS`. Raw contents retain controllers, command/stdout/stderr receipts,
source identities, the candidate source patch, independent oracle checks and
superseded diagnostics. Native executables are identified by hash, not bundled.

## Retained Diagnostics

Initial compile diagnostics found test API mistakes and two example typing/
mutability mistakes. A focused test run then exposed insufficient journal
capacity in the depth-bound fixture: capacity exhaustion masked the intended
dependency-depth rejection. The fixture now admits one extra journal slot so
the 257th list reaches that boundary; production limits are unchanged.
Superseded captures remain diagnostic, including commands whose source snapshot
changed during metadata maintenance. They are not selected as final evidence.
The initial CPU auditor also incorrectly expected standalone proof fields on
two dependent Python-driver metadata rows. Its failed receipt and exact script
are retained; the corrected auditor checks each row's accepted schema explicitly.

The first native campaign passed thirteen cases, then exceeded the 180-second
deadline for two serialized 4096-descriptor lists. It remains rejected. Pending
resources were retained until process exit; the controller confirmed no owned
processes, removed both uploaded executables and their private directory, and
checked restored device baselines. No native terminal failure was reported.

The witness now gives only that maximum-size two-list case 360 seconds, retaining
the prior allowance per list, one absolute deadline and the 60,000-poll cap.
Other witness budgets and all runtime admission/currentness checks are unchanged.
Read-only snapshots every 256 polls retain elapsed time, native counters and
exact logical statuses for timeout diagnostics. The replacement campaign has
420-second native and 480-second transport bounds and repeats the full matrix.
Per-segment full topology/currentness checks remain a concrete cost; increasing
a correctness-witness deadline is not a runtime optimization or performance result.
The replacement maximum-size case completed successfully in 231.8 seconds of
transport-inclusive elapsed time. This is not an isolated copy benchmark or
proof that currentness discovery dominates the elapsed time.

## Limits

- Sources in the new ordered destination profile are already settled.
- Compute uses existing finite qualification authority, not general kernels.
- Scripted cancellation/corruption/fault tests are not native GPU fault injection.
- Indeterminate native failures still fail-stop the router.
- Retained work does not establish physical overlap or HIP/HSA performance parity.
- Deep chains repeat ancestry validation; this checkpoint does not optimize
  worst-case reconciliation cost at the 256-node bound.
- No new solver campaign, machine-code refinement or whole-adapter proof is claimed.
- A3 remains incomplete; application authority, broader failure isolation and
  device/topology coverage remain separate gates.

Source guards change only audited hash and inventory literals. Existing proof
bodies and predicates remain unchanged; their historical qualification does
not prove this new adapter composition.
