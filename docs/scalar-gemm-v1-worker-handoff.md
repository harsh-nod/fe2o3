# Scalar GEMM V1 Worker V2 Handoff

This checkpoint implements the worker-side half of the scalar GEMM V1
source-to-artifact join. It is intentionally narrow and fail-closed.

## Implemented boundary

`validate_scalar_gemm_v1_worker_exchange_v1` reconstructs the canonical scalar
GEMM V1 Kernel IR internally, lowers it with
`lower_scalar_gemm_v1_to_gfx942_llvm_ir`, and requires the sealed Worker V2
compiler input to equal those LLVM bytes exactly. It also requires:

- target `gfx942:xnack-`;
- requested code-object version 6;
- Worker V2 options `O0`, `strip-debug=true`, and `verify-each=true`;
- no external providers and no device-FFI imports or exports;
- exactly `scalar_gemm_v1` and `scalar_gemm_v1.kd` in the final symbol closure;
- the canonical FFI-free compiler-module envelope and symbol manifest;
- exact measured worker/toolchain identity agreement with first-build evidence;
- a complete, request-bound response whose diagnostics exactly match the five
  canonical post-link target, export, unresolved-symbol, metadata, and kernel
  observations emitted by the measured worker;
- an output identity that matches the exact response bytes and output bound.

The compiler-FFI crate now has a distinct canonical envelope constructor for a
module with no device-FFI contracts. The ordinary count-first FFI builder still
rejects an empty FFI closure. This allows kernel-only Worker V2 handoffs without
inventing a fake import or export.

`inspect_scalar_gemm_v1_worker_v2_hsaco_v1` first performs the exact exchange
validation and then consumes the evidence through the existing independent raw
HSACO inspector. Only this second API reports COV6 as observed. It also requires
the inspected target and sole kernel/descriptor pair to remain exact, and
rechecks the nine explicit ABI fields, 64-byte explicit span, 256-byte COV6
implicit suffix, 320-byte total kernarg, and 8-byte kernarg alignment. Neither
API grants publication, loading, or HSA launch authority.

The path uses the existing Worker V2 upstream LLVM and in-process LLD boundary.
It adds no COMGR dependency and no command-line linker fallback.

The ignored `scalar_gemm_v1_direct_llvm_worker` integration test constructs the
canonical Kernel IR and textual LLVM handoff, runs independent candidate and
authorized Worker V2 links twice, requires deterministic HSACO bytes, and
passes each output through this scalar-specific inspection. The focused native
runner is `tools/fe2o3-llvm-link-worker/run-scalar-gemm-v1.sh`.

## Remaining frontend join

The parallel rustc frontend work currently retains its admitted exact scalar
GEMM identity in a crate-private value. A downstream crate cannot consume that
value without a frontend-owned transfer API. Accepting a public digest, symbol,
or caller-constructed record here would let untrusted code mint the canonical
Kernel IR, so this checkpoint does not add such a constructor.

The remaining join must be implemented in the rustc backend after its portable
MIR identity is pinned:

1. consume the crate-private admitted scalar GEMM value directly;
2. move it into a single-use opaque frontend-to-lowering receipt;
3. map that receipt, and only that receipt, to `scalar_gemm_v1_module()`;
4. lower through the exact scalar GEMM lowering API;
5. publish a V2 handoff with the FFI-free envelope and exact two-symbol manifest;
6. require the worker-side validator before any artifact inspection or later
   publication step.

The receipt must bind the reviewed portable MIR identity, compiler-semantics
identity, target, COV requirement, ABI, launch contract, root identity, and
export symbol. A serialized digest claim alone is not sufficient provenance.

Until that join lands, the worker validation is independently useful but does
not authenticate Rust-source-to-Kernel-IR refinement. COV6 is only a request
before raw response inspection, and no HSA launch claim is made.
