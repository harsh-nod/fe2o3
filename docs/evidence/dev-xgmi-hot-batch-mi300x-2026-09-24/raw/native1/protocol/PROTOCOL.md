# Matched Hot-Batch Native Protocol

This is a prospective shared-host correctness and performance characterization,
not a performance acceptance gate or exclusive reservation. No native result is
claimed until a complete campaign, collection, cleanup and replay qualify.

## Fixed Workload

Use signed source `8dc128357ecd55e1ba4f2866eb075899481f9aa0`, already qualified
by the sibling hot-batch CPU packet. Every trial uses one MiB per slot, ten
warmup batches and thirty measured batches, in each direction. One untimed prime
batch in each direction precedes warmups. Timing is diagnostic-off only.

The ordered first half is depth 1 KFD/HSA/HIP, depth 16 KFD/HSA/HIP, then depth
32 KFD/HSA/HIP. The second half reverses the entire first half. Thus each of the
nine backend/depth cells has two separate process invocations. Preserve every
process/direction summary separately; do not pool percentiles. Bandwidth uses
one direction's bytes times depth. Amortized batch cost is not an individual
copy latency percentile. Final complete-buffer canaries do not witness every
intermediate repeated transfer.

KFD facade enqueue/aggregate close and HIP/HSA native enqueue/completion have
different host boundaries. Equal submitted depth does not imply equal engine
concurrency: KFD uses ordered single SDMA; HIP/HSA engine selection is unknown.
Do not hide currentness checks or weaken the whole-host snapshot contract.

## Build And Identity

Verify the source commit with the existing pinned trusted signer list. Export
one signed source checkout, validate every file and mode against Git tree/blob
identities, and require exact equality to the CPU-qualified selected-source
map. Authenticate both complete referenced CPU evidence packets against the
signed Git tree, including their exact file rosters, contents and executable
modes, before running the pinned CPU replay. Create a previously absent
exclusive Cargo target; reject existing or symlinked targets. Build/test the
musl release example with default features, two jobs, locked/offline Cargo and
`nightly-2026-04-03`. Bind the standalone ELF immediately after its build and
require it to remain identical through the subsequent test invocation. Preserve
tool identities and unchanged source checks.

Build HIP and HSA from the same signed comparator source on MI300X with the
existing gfx942/O3/warnings-as-errors commands. Preserve ROCm, hipcc and g++
identities before/after. Retain all three executable byte streams, checking
their hashes before every trial and finally. Parser bytes come from the signed
checkout and have a fixed qualified digest. Native bootstrap and helper imports
authenticate bytes before executing them.

The build source tar is written directly to an owned scratch file, not a
captured stdout stream. Retain its digest, exact source map, Git tree receipt
and signed source identity; after collection, remove the transient archive and
checkout with an exact owned cleanup record. Signed Git objects, not retained
tar bytes, are the source reconstruction authority. Retain the small remote
comparator/observer archive and executable artifacts. This is not a hermetic
compiler, library, kernel or firmware archive.

## Admission, Bounds And Cleanup

Select any two currently free endpoints with canonical index/BDF/unique-ID
identities. Before every workload, independently observe both endpoints using
the existing strict observer: zero GPU/memory-engine activity, less than
512 MiB VRAM, no selected process attachments and matching identity. Repeat
both endpoints after a two-second settling delay, then after another twenty
seconds. These are sequential point observations, not continuous monitoring.
Failure of one endpoint must not suppress observing the other. Any admission,
workload, parser or postflight failure stops the campaign after both postflight
stages; do not silently retry or include that cohort in an accepted comparison.

Each endpoint command is bounded at 100 seconds; workloads at 300 seconds;
comparator builds at 180 seconds; tool queries at 30 seconds. The outer remote
bound is 19,446 seconds: six tool commands and two builds, eighteen times six
endpoint commands plus one workload and 22 seconds of delays, 15 seconds of
process-group stop margin per command, and 300 seconds for other bounded-size
controller work. These are worst-case limits, not predicted campaign duration.
A complete run has 134 remote command receipts and 108 endpoint observations.

Use a unique mode-0700 directory and exact ownership marker under the declared
prefix. All workload commands run in owned process groups with core dumps off.
Collect the complete remote result inventory and compare every byte before
removing the owned remote directory. If collection fails after native execution
was attempted, retain remote receipts for recovery instead of deleting them.
Require a separate path/process absence check. Do not reset GPUs, terminate
foreign processes or remove any unowned files.

## Qualification Status

The prospective controller has CPU controls for exact ordering/arguments,
parser inputs, source and target integrity, bounds, both-endpoint observation,
failed admission and postflight behavior. Reuse the existing authenticated
bootstrap and cleanup controls. Native execution and sealed result replay are
separate required steps; no A1/A2, A7, formal-refinement or parity acceptance is
conferred by protocol tests.
