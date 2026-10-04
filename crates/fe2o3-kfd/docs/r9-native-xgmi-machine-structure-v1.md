# R9 native XGMI and machine-structure boundary

R9 adds a bounded native direct-KFD XGMI copy path and authenticated
machine-instruction structure evidence for gfx942 atomics and collective
building blocks. It does not claim that opcode classification proves machine
semantics or that the community runtime facade now routes peer copies through
this lower-level path.

## Native directional XGMI

The native path retains every exact KFD `io_links` and `p2p_links` record under
the topology generation. A directional route is admitted only when both
gfx942 devices have the same nonzero hive, the source reports the exact
ordinary and XGMI SDMA engine inventory, one enabled type-11 `io_links` record
connects source to destination with nonzero maximum bandwidth, and its
recommended engine mask names exactly one XGMI engine in the reviewed 2..15
BY_ENG_ID space. Reverse direction requires a separate retained route.

PUBLIC device-local allocations have a separate move-only API. Before an
allocation can enter copy custody, `AMDKFD_IOC_MAP_MEMORY_TO_GPU` maps the exact
canonical ascending two-GPU array. Map and unmap retry only an advanced
cumulative prefix. An errno at a full map prefix grants cleanup custody but no
copy authority. An errno at a full unmap prefix is indeterminate: the session
is quarantined and no free authority is created. These rules follow the Linux
KFD 1.18 `n_success` contract in
[`linux/kfd_ioctl.h`](https://github.com/torvalds/linux/blob/master/include/uapi/linux/kfd_ioctl.h).

`Gfx942NativeXgmiSdmaQueueV1` creates a directional
`KFD_IOC_QUEUE_TYPE_SDMA_BY_ENG_ID` queue on the route's exact recommended
engine. Nonblocking submission transfers both mapped allocation owners into
the queue. Tickets bind the non-reused queue occurrence, native queue ID, ring
slot, and generation. Only an acquire-observed exact completion returns both
owners. Bounded depth uses one full topology-currentness envelope per batch;
this means one full observation envelope around the bounded batch, not one
native queue publication. Each packet advances the queue and rings the doorbell
separately. The retained process identity and prospective KFD reset stream are
checked around every packet publication and completion; the full descriptor,
UAPI, XNACK, DRM-loss, and topology observations remain at the batch edges.

The repository benchmark compares the KFD queue with
`hsa_amd_memory_async_copy` and `hipMemcpyPeerAsync` in both directions. All
allocation, mapping, pattern setup, poisoning, readback, and validation are
outside the measured interval. The timed interval begins immediately before
submission and ends only after every completion is observed. Every round uses
new source patterns, poisons destinations, and validates payload and canaries.
The runner requires two idle GPUs, exact unique-ID correlation, a clean Git
commit, and explicit cleanup.

This is a low-level direct-KFD primitive. `RuntimeContextV1` peer copies still
use their existing bounded host-staged router because its persistent facade
allocations do not yet share this XGMI ownership representation.

## Authenticated machine structure

The LLVM/MC worker now emits exact instruction encodings and closed memory
classifications for the reviewed gfx942 subset:

- 32-bit and 64-bit global and LDS integer read-modify-write atomics for swap,
  compare-exchange, add, subtract, bitwise and/or/xor, and signed/unsigned
  minimum/maximum;
- reviewed 32-bit LDS read, write, and permutation primitives;
- the exact workgroup barrier spelling.

Unknown atomic, DS, or barrier spellings and every opcode spelling containing
`_DPP` fail closed; no DPP primitive is in the reviewed roster. The Rust checker
consumes the authenticated worker execution and retains the exact HSACO
payload identity, selected kernel and descriptor, entry byte range and digest,
closed reachable call graph, instruction offsets, opcode spellings, encoding
digests, widths, storage classes, and primitive classes. Its move-only
application transition additionally matches that receipt to the exact loader
prepared dispatch object, descriptor, entry, and kernel identity.

This is **authenticated machine-structure application**, not semantic
refinement. Opcode names and MC load/store flags do not prove an instruction's
mathematical effect, atomic ordering or scope, barrier convergence, high-level
collective algorithm, compiler preservation, or hardware coherence. Accordingly the
receipt and its prepared-dispatch binding explicitly grant neither load nor
launch authority. The integration-only `fe2o3-runtime-machine-adapter` owns
`execute_machine_structure_applied_gfx942_runtime_dispatch_v1`, which consumes the result of
`apply_gfx942_atomic_collective_machine_structure_v1`, an independently
authorized Worker V3 value, and the checked device. It delegates to the sole
authorized runtime dispatch transition and returns the retained structure with
the normal completion result. Worker V3 remains responsible for semantic and
launch authority; the native owner still binds device generation, queue
occurrence, dependencies, publication, and completion.

Atomic load/store operations are not admitted by an RMW opcode name, and a
primitive roster is not a one-to-one proof of a reduction or scan. Those cases
require source-to-machine correspondence rather than broader string matching.

## Verification claims

| Surface | Level | Claim |
| --- | --- | --- |
| Abstract mapping prefixes, compensation, route currentness, copy custody, exact evidence equality, and dispatch publication predicates | **Proved** | Fourteen Verus obligations, with fifteen R9 expected-negative mutations. These are mathematical abstractions. |
| Rust topology, mapping, queue, analyzer receipt, prepared-dispatch binding, and model rejection tests | **Checked** | Executable tests cover exact identity, range, prefix, classification, custody, and stale/substitution failures. There is no Rust-to-Verus refinement theorem. |
| KFD ioctls, packet consumption, reset events, firmware, XGMI routing, and CPU/GPU/system coherence | **Contracted** | Frozen layouts, primary-source contracts, retained observations, and fail-closed state transitions constrain but do not prove the external system. |
| Native correctness and performance on MI300X | **Unsupported** | No retained clean-commit, load-gated result from `benchmarks/runtime_gfx942/run-xgmi-peer-mi300x.sh` exists yet. |

The cumulative Verus runner reports 81 proved obligations and 60
expected-negative mutations. The machine checker is deliberately excluded from
the Proved row: its output is authenticated exact structural evidence, not a
formal decoder or ISA semantics proof.

## Remaining work

1. Connect facade-owned persistent device allocations and events to the native
   XGMI owners without weakening terminal custody.
2. Prove or independently validate instruction semantics, ordering/scope,
   compiler correspondence, and collective convergence for exact generated
   artifacts.
3. Produce native system-coherence evidence for system-scope atomics.
4. Expand the closed instruction and language subset only with exact fixtures,
   mutation tests, and source-to-machine correspondence.
5. Retain idle, clean-commit MI300X correctness and performance measurements
   before making parity claims.
6. Expose retained-mapping, multi-packet single-doorbell publication through the
   public facade and add topology-safe striping across multiple recommended XGMI
   engines; the current bounded batch uses one directional queue and one final
   doorbell store per batch.
