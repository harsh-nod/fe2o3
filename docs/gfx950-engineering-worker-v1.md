# gfx950 Engineering Worker V1

This optional direct-KFD process is a contracted engineering execution boundary
for issue #274. It grants no protected load, dispatch, model, publication, or
service authority. It does not convert an MI350 observation token to gfx942.
The default runtime API and its checked gfx942 routes are unchanged.

Build `fe2o3-kfd` with feature `engineering-gfx950` and run the
`fe2o3-gfx950-engineering-worker` executable with
`--device-unique-id N --allow-unauthenticated-machine-code`. Its stdin/stdout
must be private framed pipes. The executable owns a single process-bound
selected device, explicit VM acquisition, queue, allocations, and native-code
trust boundary. Separate child processes may execute on independently selected
devices; this is not peer access, collective transport, or shared-VM admission.

## Trust And Protocol

The public library entry is deliberately **unsafe**. Structural ELF/AMDHSA
validation does not prove arbitrary machine code respects its declared ABI,
pointer extents, or access modes. The operator must trust the supplied code and
authorize the selected hardware. The executable is disposable, single-threaded,
and must not contain unrelated work or share Rust memory with its parent. A
parent written in safe Rust can use the wire types and process pipes without
acquiring raw pointers. Process isolation is not a security sandbox for hostile
GPU code, and this interface must not accept untrusted network submissions.

`engineering_wire` defines protocol version 1: a bounded little-endian u32 JSON
header length, exact JSON header, then the declared binary payload. Headers are
at most 64 KiB, copies 4 MiB, objects 64 MiB, and kernargs 64 KiB. Buffer/kernel
IDs are child-local and monotonic. There are at most 2048 live user buffers,
256 loaded kernels, 32 GiB per allocation, and 128 GiB total backing per child.
The 8 MiB ring has 131072 packet slots, also exposed as
`MAX_UNRETIRED_RING_PACKETS_V1`. Parents should preflight the complete workload
against that conservative budget. A full ring is terminal; queue rollover is
not implemented and slots are never reused merely because a signal completed.
Parents can submit dispatch commands to multiple children before receiving
their replies; each child itself handles one command synchronously.

Loading retains bytes and validates the requested SHA-256, symbol, exact
`gfx950:xnack-` COV6 envelope, wave64, no private segment/scratch, bounded LDS,
and supported explicit argument metadata. Returned metadata lets a parent
independently match its expected ABI. Each dispatch supplies the full zeroed
kernarg segment with explicit scalars. Exactly one owner-relative pointer fixup
is required per global-buffer argument; raw pointer slots must initially be
zero. Nonnull zero-extent slices are allowed at an owned buffer's endpoint.
Alignment, bounds, declared access, and mutable overlap are checked before any
pointer slot changes. COV6 implicit arguments are initialized from checked
geometry by the existing pure layout helper, not copied from caller addresses.

## Exact Queue Profile

The existing MI350 checked-observation token establishes the exact published
kernel/module/UAPI/PCI/DRM/firmware/SPX/NPS1/XNACK profile. Additional queue
admission requires 1024 SIMD, four SIMD/CU, eight XCC, 32 arrays, one array per
engine, 160 KiB LDS, eight waves/SIMD, 24 compute queues, wave64, and observed
module parameters `mes=0`, `sched_policy=0`, `cwsr_enable=1`. No setter is used.

The active driver source at `amdgpu-6.16.13-2303411.24.04/amd` was inspected:

| Source | SHA-256 |
| --- | --- |
| `amdkfd/kfd_queue.c` | `fb4b2a5c9e6981222873bcd7aca7e9c1397cba8f1a6b33634d2a48d4427fe062` |
| `amdkfd/kfd_doorbell.c` | `de30437ee1ed9ccbdaf855899482c0bebb7f55adc120ac712c96cadef1a0ec6d` |
| `amdkfd/kfd_mqd_manager_v9.c` | `21166e9dbe2a4c24cbcd6f9ff6193aa093230e91fbafc8b4ac4eee1465cd2c9e` |
| `amdkfd/kfd_process_queue_manager.c` | `8526e258824dbe145e4209cf0fed26463729234ba24369f39e3413e7e6e028db` |
| `amdgpu/amdgpu_amdkfd_gpuvm.c` | `c7cca2ee47a08c99bb73906662d82dd7d0b5738468fbef54848e5e6dd62ba50d` |

These hashes identify reviewed contracts, not authentication of the running
driver. The gfx950 per-CU VGPR save size is 0x80000; SGPR is 0x4000, LDS is
160 KiB, and hardware registers are 0x1000. There are 32 CU/XCC and 1280 saved
waves/XCC. The page-rounded control stack is 0x3000, context/XCC is 0x15a3000,
debug total is 0x50000, and total CWSR mapping is 0xad68000. The exact 4 KiB EOP
backing uses an executable GTT allocation in this engineering path. This is
not a claim of ROCr VRAM backing or syscall parity. CWSR and ring are retained
USERPTR mappings; the completion signal and kernargs use coherent GTT backing.

The shared KFD 1.18 UAPI, Linux identity mappings, mandatory DONTFORK ordering,
atomic AQL publication, AMD signal layout, and MMIO release-store mechanisms
are reused. Queue geometry, returned doorbell target binding, ownership, and
kernel target admission are independently gfx950-specific. XGMI publication
through this backend is explicitly rejected.

## Completion And Teardown

CPU copies occur only when the retained completion frontier equals the submitted
frontier. A dispatch reserves one slot using the existing single-producer AQL
ring model, publishes one bounded packet, and observes its exact acquire-loaded
completion signal. The active driver sets `NO_UPDATE_RPTR` for AQL; a read-pointer
report may lag completion, as the existing runtime's barrier contract also
allows. Read reports must remain monotonic, not exceed the exact write count,
and leave no more than one ring's capacity outstanding. Signal completion does
not fabricate a hardware read report or grant ring-capacity reuse. The exception
payload and retained currentness are checked before output can be read or the
next command proceed. Currentness is contracted
process/fd/topology/XNACK/aperture/reset-event/VRAM-loss observation, not an
all-reset or ABA proof.

Every error is terminal. Uncertain resources are retained until process exit;
the worker does not retry or free potentially live resources. A successful
close destroys the fully completed queue, destroys its exception event, disables its
process-local runtime session, unmaps the doorbell, unmaps/frees each owned BO,
and releases its VA reservation. No global host state is modified.

Pure tests cover profile mutation rejection, CWSR derivation, encoded doorbell
identity, exact empty extents, pointer ownership/aliasing/alignment, fail-atomic
fixups, and bounded framing. Existing runtime-model Verus proofs do not prove
this new executable path or the hardware. Native execution evidence must be
reported separately from source tests and protected verification claims.
