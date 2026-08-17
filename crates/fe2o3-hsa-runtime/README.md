# fe2o3-hsa-runtime

This crate is the reviewed production adapter between fe2o3's linear Worker V2
authority types and the AMD HSA runtime. It loads already-finalized AMDHSA code
objects and submits AQL directly. It does not compile or link code and has no
COMGR or command-line tool path.

The production backend is enabled only when matching HSA and HIP headers and
runtime libraries are found under `ROCM_PATH`, `HIP_PATH`, or a known ROCm
installation. Builds without that ABI fail closed at adapter construction.
Construction locates the HSA and HIP runtime images through `/proc/self/maps`,
opens those paths, verifies that each opened file has the mapped device and
inode, checks stable metadata around hashing, and binds both file digests into
the runtime identity. The descriptors are closed after measurement. This is
path/device/inode evidence for the runtime files at observation time, not
authentication of the executable pages already mapped into the process. In
particular, a same-UID actor that can mutate a runtime file in place after the
observation is outside this model and may invalidate the evidence. This is also
not an operating-system attestation or rollback anchor.

The generic reviewed COV6 initializer accepts a bounded profile-supplied
explicit prefix followed immediately by the exact 256-byte implicit-argument
layout and requires zero dynamic LDS. The workgroup-sync specialization instead
requires exactly 256 dynamic LDS bytes for the LDS reduction profile and zero
for the scoped-atomic profile. The reviewed layout initializes block counts,
group sizes, zero remainders, zero global offsets, grid rank, and the exact
`hsa_queue_t *` 200 bytes into the implicit suffix. Queue creation therefore
happens during hidden-argument initialization; the adapter privately retains
that queue and binds it to the executable, kernel, geometry, and a digest of
the complete kernarg bytes until the immediately following launch consumes it.

The reviewed COV6 metadata profile has no hidden dispatch-pointer kernarg. The AQL
packet supplies dispatch identity through the AMDHSA dispatch mechanism. Its
hostcall, multigrid synchronization, heap, default device queue, and completion
action fields are null because the current reviewed profiles use none of those
services. A profile that requires any of them must add authenticated runtime
observations and a separate reviewed initializer; this adapter fails closed on
every other hidden-argument layout, unsupported dynamic LDS value, missing
queue binding, or binding substitution.

One loaded executable may resolve a fixed, const-generic set of distinct kernel
symbols. The non-clone set owns each kernel token and borrows the executable,
so safe Rust must release the whole set before unloading that executable. It
rejects duplicate requested names and native symbol, kernel-object, or derived
kernel-identity aliases. This is lifecycle and identity authority only: it does
not claim that arbitrary resolved kernels have a typed ABI. Safe dispatch
remains profile-specific: host-side typed lifecycles validate their own ABI and
resources before using this adapter. Resource-observation implementations exist
for the exact workgroup-sync, row-softmax, FlashAttention, MoE top-2,
Wave64-collectives, and LDS-GEMM profiles. Their configured hardware tests
remain ignored under their explicit worker, artifact, wrapper, receipt, target,
or MI300X prerequisites; those lanes do not establish general gfx942 support or
GPU execution for every profile. Each prepared queue is bound to one executable
identity, one kernel identity, one geometry, and one digest of the complete
kernarg bytes.

Dispatch has explicit pre-submit, submitted, and quiesced states. Packet
publication atomically releases the combined 32-bit AQL header/setup word.
After publication, completion is polled with nonblocking acquire signal loads,
asynchronous queue status checks, and a five-second host monotonic deadline.
No HSA wait call can extend that deadline. Only an exact zero completion value
establishes quiescence. A queue fault or expired deadline terminates the process
with `SIGABRT`, because returning would release caller authority while the GPU
may still reference its allocations. Errors return only on definite
pre-publication paths or after quiescence, where native queue, signal, and
kernarg resources can be cleaned in reverse order.

The adapter is `Send` so a unique owner may move it between threads, but it is
intentionally not `Sync`. Native ABI record sizes and alignments are asserted in
both C and Rust. HSA queue ID zero is accepted as valid; queue validity depends
on the returned queue pointer, size, and callback state. If a malformed queue is
returned after successful creation and its destruction fails, the callback and
queue record remain allocated and adapter construction terminates rather than
freeing callback state that a live queue could still invoke.
