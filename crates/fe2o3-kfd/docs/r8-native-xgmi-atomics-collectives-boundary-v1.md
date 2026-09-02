# R8 native XGMI, atomics, and collectives boundary

This document separates the executable R8 model from native KFD authority.
It is not a native-support claim.

## Implemented executable model

`fe2o3-runtime-model::kernel_semantics` admits a caller-declared gfx942
semantic roster only when it is bound to:

- one current runtime device generation;
- one live loaded-code key and its exact artifact identity;
- the exact async-operation resource set;
- live same-VM global mappings with sufficient read or read-write access;
- aligned, in-bounds 32-bit or 64-bit atomic storage, measured at the checked
  `mapping.gpu_va + byte_offset` address for global objects;
- exact-object compatibility for overlapping atomic declarations, rejecting
  partial overlap and mixed-width aliases;
- legal load, store, read-modify-write, and compare-exchange orderings;
- workgroup, device, or system scope, with a separate caller-declared
  system-coherence prerequisite for system-scope global storage; and
- the exact wave64 or power-of-two workgroup geometry, and for subgroup calls a
  separate power-of-two tile width in `1..=64` while all 64 physical wave lanes
  execute convergently, plus the scalar type and LDS-slot count required by the
  bounded collective APIs in `fe2o3-device`.

The atomic roster covers load, store, swap, compare-exchange, add, subtract,
and, nand, or, xor, signed/unsigned minimum, and signed/unsigned maximum for
the reviewed 32-bit and 64-bit integer subset. The collective roster covers the
currently exposed gfx942 wave64 reductions/scans, active-lane reduction,
floating-point subgroup sum/maximum, and workgroup reductions/scans through
256 work-items.

Every input is caller-constructible. Admission does not inspect machine code,
prove convergence, create LDS, authenticate coherence, publish a packet, or
observe hardware execution. The admitted value remains `ModelOnly`.

## Why atomics and collectives are not KFD operations

The KFD UAPI creates memory mappings and queues and accepts queue packets. It
has no ioctl that means "perform this Rust atomic" or "run this collective."
Those semantics come from the code object dispatched on a compute queue.
Consequently, native support requires a refinement chain from an authenticated
compiler semantic manifest to the loaded artifact and the exact dispatch. A
successful KFD completion establishes neither the instructions in the kernel
nor their memory scope by itself.

System-scope global atomics additionally need sealed allocation/mapping
evidence for coherent system access. The model's `SystemCoherent` declaration
is intentionally not that evidence.

## Native XGMI peer-copy prerequisites

Linux KFD 1.18 exposes `AMDKFD_IOC_MAP_MEMORY_TO_GPU` and
`AMDKFD_IOC_UNMAP_MEMORY_FROM_GPU` with an array of GPU IDs, a device count,
and cumulative prefix progress. The frozen fe2o3 UAPI layout represents those
fields. The current native memory adapter does not expose that capability:
`LinuxMemoryBackend::exact_progress` builds a one-element array and rejects
any result whose `n_devices` is not one.

Direct peer support must complete these gates in order:

1. Parse every directional `io_links`/`p2p_links` record, validate its exact
   source, destination, link type, weight, and flags, and retain it under the
   same topology-generation/currentness fence as device admission. The current
   topology adapter validates the directories and link counts but does not
   retain their contents. Multi-device presence or a nonzero link count is not
   peer authority.
2. Add a move-only multi-node map owner that retains the canonical GPU-ID
   array and treats every failed or interrupted map/unmap according to KFD's
   cumulative successful-prefix contract. A partially mapped VRAM allocation
   cannot return to a general pool or be freed.
3. Admit an allocation profile suitable for remote GPU access and bind the
   allocation's owning GPU, every mapping GPU, and the queue's executing GPU.
   `KFD_IOC_ALLOC_MEM_FLAGS_PUBLIC` alone is not a topology, coherence, or
   accessibility proof.
4. Refine the exact gfx942 SDMA remote-address, release-fence, completion, and
   acquire-observation behavior. Queue completion without the required remote
   visibility is insufficient.
5. Exercise directional copies for every admitted pair, both directions,
   overlapping independent pairs, failure cleanup, reset/currentness loss, and
   stale device generations before publishing a native claim.

Until those gates are implemented, fe2o3's multi-device runtime may provide a
host-staged or otherwise contracted peer path, but it must not label that path
native XGMI.

## Evidence levels

- The Rust executable admission model and unit tests are **Checked**.
- Any separate Verus abstraction is **Proved** only for its stated mathematical
  predicates and is not a refinement of this Rust module unless explicitly
  linked.
- KFD packet/UAPI assumptions are **Contracted** at the kernel/firmware and
  coherence boundary.
- Native correctness and performance become **Measured** only through gated
  hardware runs on the exact admitted platform.
