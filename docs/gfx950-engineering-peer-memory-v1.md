# Gfx950 Engineering Peer Memory V1

This additive API is an explicitly unsafe, disposable-process engineering
profile. It does not grant protected runtime authority, convert gfx942 leases,
or enable the existing gfx950 backend's denied XGMI publication method. It is
not a hardware qualification or new Verus proof.

## Group Owner

`Gfx950EngineeringPeerGroupV1::open_unchecked` owns exactly two or eight checked
gfx950 contexts in one single-threaded process. Each context retains its own
device observations, VM, queue, allocations, event, doorbell, and runtime lease.
The process-global runtime lease implementation already admits multiple queues.
The original single-context worker entry and its wire protocol are unchanged.

All participants must have identical retained host topology snapshots. Every
directional pair must be gfx950, have distinct GPU identities in the same
nonzero XGMI hive, and have exactly one enabled IO XGMI link with positive
bandwidth. Noncoherent links, NO_PEER_TO_PEER_DMA and reserved flags reject.
NO_ATOMICS_32_BIT and NO_ATOMICS_64_BIT are allowed because this profile does
not authorize remote atomics. No SDMA engine selection is inferred or granted.
Full existing device currentness checks on every participant run before and
after mapping, dispatch, host access and release. Thus topology and aperture
equality, process/fd/XNACK/reset/DRM observations retain their full contract.

## Allocation And Access

An allocation uses the existing owner context's PUBLIC device-local VRAM path.
It is initially mapped only to its owner. Its entire backing VA range must
also fit every selected peer aperture. A canonical sorted peer GPU-ID roster
is mapped using the existing checked multi-GPU map wire operation. The record
retains its owner, exact logical byte extent, group incarnation, monotonic
buffer identity, peer roster, map/unmap progress and transaction phase.

Externally visible buffer/kernel tokens have private group-bound fields. No
native address, handle, fd, or gfx942 capability escapes. Kernel arguments are
resolved from those retained tokens; a peer may bind a buffer only as read-only,
while its owner may read or write it. This is argument-binding enforcement,
not a hardware read-only page permission. Unauthenticated kernels remain
subject to the caller's explicit obligation to honor declared accesses.

`dispatch_unchecked` runs synchronously. All queues must be complete and idle
before entry, every backing allocation and context remains owned until its
completion, and no free/write can interleave with the exclusive group borrow.
The dispatch reuses the original geometry, ABI, pointer-range, aliasing, COV6,
descriptor, completion, exception, and queue checks. The baseline resolver
remains unchanged when no group binding table is supplied.

Host reads/writes are bounded to 4 MiB per call. Kernel objects remain bounded
to 64 MiB, kernargs to 65536 bytes, pointer fixups to 256, and individual
dispatch waits to 600000 ms. Existing per-context allocation limits apply,
with at most 2048 group buffers. IDs never recycle within a group.

## Failure And Release

The shared map/unmap transaction implementation is exercised with injected
native outcomes in host tests. It requires exact full progress and success;
zero/short/overshot progress, errno even at a full prefix, or a failed pre/post
currentness check quarantines the record and group. There is no native retry,
rollback or owner-free after an uncertain peer transition.

Normal release checks all participants, unmaps the complete peer roster,
checks all participants again, and only then calls the existing owner unmap /
free operation. Close releases every group buffer before destroying any
context. On any error, future operations reject; an unclosed group's native
owners are retained until process exit. The caller must exit its disposable
process immediately on failure, not continue independent GPU activity.

## Host Tests And Probe

Host tests cover actual transaction ordering, all partial-map/unmap progress
and errno combinations, pre/post currentness failures, owner-release failure,
local-only allocation handling, route target/hive/direction/flags, duplicate
rosters, read-only peer binding, stale group tokens, and terminal poisoning.
These tests do not establish physical peer accessibility or cache coherence.

The integration lead must first run a two-device probe and then an eight-device
probe. The evidence-only client uses an exact digest-bound emitted RMSNorm
image to read one owner's allocation on every reader, then changes the shared
input and repeats. It checks exact outputs, unchanged input and all guards,
then unmaps peers before freeing each owner and explicitly closes the group.
The two-device case has eight owner/reader/phase observations; eight devices
have 128. These are qualification probes, not performance measurements.

## Ferric Integration

The immediate client interface is the typed in-process API. A dedicated Ferric
worker can own this group and accept bounded group-scoped allocation, load,
dispatch and release commands, returning only completion/timing records.
Projection partials can be allocated on their owner and shared read-only with
the other ranks. An ordered device reduction then reads rank 0 through rank 7
partials, checks every FP32 intermediate, adds the local residual once and
rounds once to BF16, without copying tensors through controller IPC.

This group currently serializes individual dispatches and performs full
group-wide currentness checks. It is a foundation for qualified peer memory,
not a claim of faster tensor parallelism, asynchronous collectives, symmetric
memory, or overlap. Ferric's existing host-staged mode remains explicit until
the new group, kernel image, transport and model outputs pass hardware checks.
