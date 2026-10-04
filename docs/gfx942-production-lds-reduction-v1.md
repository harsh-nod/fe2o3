# gfx942 production LDS reduction V1

This checkpoint qualifies one genuine Rust workgroup kernel through the single
production compiler and finalizer transaction for `gfx942:xnack-`. It does not
use COMGR, command-line LLVM tools, a workload-profile selector, or prebuilt
Kernel IR. It grants no load or launch authority.

## Source contract

`examples/workgroup_sync_v1/src/kernel.rs` defines
`lds_publish_read_reduce_i32_v1` with `#[kernel(typed, launch(...))]`:

- required and maximum workgroup: `64x1x1`;
- static shared memory: 256 bytes;
- input ABI: shared `&[i32]` pointer and length;
- output ABI: grid-exclusive `DisjointSlice<i32>` pointer and length; and
- one `DynamicLds::<i32>::exact_current::<64>` allocation consumed by the
  public workgroup reduction.

The macro emits the canonical kernel-resource sidecar. Rust collection,
semantic MIR, and Kernel IR must agree exactly on launch dimensions and LDS
bytes. Drift, overflow, missing workgroup-memory capability, or a nonzero
dynamic-LDS requirement fails before LLVM.

## Production path

The protected fixture exercises this custody chain:

1. pinned rustc collection and complete reachable portable-MIR closure;
2. semantic MIR with the source resource contract;
3. ranked PLIRON checks and proof-bound bounds-edge elimination;
4. verified Kernel IR plus composed formal/ranked memory admission;
5. deterministic gfx942 LLVM using `llvm.amdgcn.dispatch.ptr` and HSA dispatch
   packet grid-size fields;
6. compiler-bound inert handoff with an exact symbol manifest;
7. measured upstream LLVM target APIs and in-process LLD, executed twice with
   byte-identical COV6 output;
8. ELF, AMDHSA metadata, descriptor, target, ABI, and resource inspection.

The inspected kernel has a 288-byte complete kernarg segment, 256-byte static
group segment, wave64 execution, maximum flat workgroup size 64, and required
workgroup size `64x1x1`. Observed machine code contains seven LDS writes,
thirteen LDS reads, and fourteen physical `s_barrier` instructions.

## Execution boundary

The repository intentionally removed the historical HSA qualification harness.
The exact source-bound HSACO now completes one current pure-Rust KFD diagnostic
on the qualifying MI300X: the 64 values `1..=64` reduce to `2080`, input and
output canaries remain unchanged, and queue/completion teardown succeeds. The
runtime uses identical CPU/GPU virtual addresses for every BO and places the
complete 256-byte static-plus-dynamic group allocation in the AQL packet.

That diagnostic now invokes the private mechanics through the runtime's sole
safe, consuming execution transition. The transition independently matches the
final object, kernel, complete address-free invocation identity, and checked
KFD GPU identity, and aborts on any post-mutation failure. The diagnostic still
grants no production authority because its trait implementation is an explicit
unsafe manual assertion. The sole Worker V3 application and production verifier
must authenticate the artifact, generated arguments, memory effects, launch,
completion, and evidence chain before production can construct that authority.
The scoped-atomic kernel is requalified only through the same compiler, worker,
descriptor-inspection, and authorized runtime path.

## Defects closed

A development-time MI300X observation before the HSA harness was retired found
two defects that LLVM-text tests did not expose:

- the invented `llvm.amdgcn.grid.size.x` declaration was replaced by the real
  upstream `llvm.amdgcn.dispatch.ptr` intrinsic and ABI-defined packet loads;
- unique enum payload custody now initializes private fallback storage when a
  later variant refinement is not strictly dominated by the source. This
  prevents uninitialized LDS pointer and length loads.

Both changes are workload-neutral and fail closed under existing hostile tests.
That observation is diagnostic history, not current hardware evidence.

## Validation

The checkpoint passed the complete `fe2o3-lower-mir-kernel` and
`fe2o3-amdgcn-model` test suites, all active `rustc-codegen-fe2o3` library
tests, the protected source-to-LLVM fixture, the
deterministic LDS HSACO finalization/inspection test, and the deterministic
scoped-atomic HSACO finalization/inspection test. Worker and LLVM build
identities are read from the measured worker build directory rather than
accepted from source defaults. The pure-Rust runtime additionally passed its
strict loader/request tests and the exact-artifact KFD canary diagnostic on
MI300X `gfx942:xnack-` unique ID `6ced1647a296545c`.

## Boundary

This proves one bounded source-to-HSACO path and records one current measured
execution of the exact bytes. It is not authenticated Worker V3 launch
authority, compiler refinement, race freedom for arbitrary kernels, general
barrier convergence, broad reduction/scan coverage, dynamic LDS, performance
parity, or support beyond the admitted gfx942 target.
