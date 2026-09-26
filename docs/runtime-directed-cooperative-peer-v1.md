# Directed Cooperative Peer Copies

Development implementation of `RuntimeDirectedScalarPeerCopyBackendV1` for
`KfdMultiDeviceRuntimeBackendV1`. This uses host staging and the existing child
runtime, not native XGMI or a new transport. It does not accept A1/A2, issue
#182, pending peer-to-compute, formal refinement or HIP/HSA performance parity.

## Admission And Identity

The router authenticates exact endpoint devices, destination-owned stream,
allocation ranges/access, event variant, event child and expected producer.
Every explicit producer and implicit FIFO predecessor must use this directed
profile, including completed predecessors. Event aliases naming one producer
reject. Legacy parents remain unsupported; missing retained parents, malformed
event routing or a broken retained tail seal the router before admission.

The immutable route, allocation extents, ordered event/producer roster, original
FIFO predecessor and depth are embedded in the ordinary cooperative copy owner.
All fallible reservations precede its ownership commit. No native event,
completion record or provisional GPU submission is fabricated. Public events
may be released after admission. Terminal provenance survives parent and
allocation retirement until guarded submission release succeeds. A retained
completed FIFO tail still prevents stream destruction; historical route
matching does not override the existing stream lifetime contract.

Depth and dependency count are bounded to 256, including completed ancestry.
Exact directed read/read source sharing requires no sibling dependency. Both
selected allocation-owner rosters are bounded to 256, including legacy copies
joining a roster with directed owners. Legacy-only admission is unchanged.
The existing full-copy staging budget includes one additional scratch window
for native endpoints; scratch remains charged through residual disposal.

## Progress

Each request repeats the exact original route and ordered explicit producer IDs.
Validation occurs before action, including consumed-producer success, phase,
execution roster, local routes, extents and live allocation custody. An implicit
FIFO dependency is not added to caller provenance. Read/Write cannot bypass
pending, failed or cancelled dependencies by advancing their cursor or phase.

Selection iteratively visits at most 256 dependency roots. It may additionally
select one exact private owner blocking source DMA admission or source/destination
generation reconciliation, including a write-side pin on another allocation.
This selection uses child custody/pins and bounded
outer owner indexes, not a synthetic event or recursive dependency. A private
owner is authenticated against its actual stream, handles, offsets, direction,
scratch and phase. Readback, retirement, cleanup and an already-owned
reconciliation advance themselves rather than waiting on a newer sibling.
An unrelated legacy reconciliation remains under its existing progress API.

Only one selected step executes: a dependency observation/settlement, private
allocation, DMA publication/observation, a transfer/reconciliation window of at
most 64 KiB, or private cleanup. Poll and deadline wait remain observational.
One step does not imply constant work or a hard latency bound: metadata
validation is bounded by depth, dependency and owner limits, and allocation,
copy-on-write, profiling and driver calls retain their existing costs. Duplicate
roster validation uses fixed scratch and sorting; consumed terminal parents use
local success checks, not repeated traversal of historical graphs.

A terminal result belongs only to the requested copy. A selected ancestor's
conclusive cleanup error leaves the requester Pending; later progress settles
one failed dependent at a time. An independent resource sibling is not a success
dependency. Ordinary flush preserves its existing whole-path failure behavior.
Returned terminal failure and unwinding seal the shared progress path, retaining
possibly live custody. Ordinary cancel/release also authenticate retained roots.

## Evidence Boundary

The [development receipts](evidence/dev-directed-router-2026-09-26/README.md)
cover real router/Context adapters using CPU and scripted child drivers. They
exercise pending chains with and without the version journal, ordered fan-in,
fanout, depth/capacity limits, provenance corruption, requested-result attribution,
private DMA/reconciliation and cleanup. They do not prove hardware behavior,
positive native composed-account execution or semantic-to-machine refinement.
The [native XGMI profile](runtime-directed-scalar-peer-v1.md) remains distinct;
its batching, provisional roots and error handling must not be inferred for this
router. Pending peer-to-compute and unified native XGMI/compute remain open.
