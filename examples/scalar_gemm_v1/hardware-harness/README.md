# Scalar GEMM V1 hardware harness

This standalone crate keeps the concrete HSA test controller inside the scalar
GEMM example without changing the workspace dependency graph.

The controller has no path, environment-variable, raw-HSACO, digest-string, or
raw-pointer entry point. A caller must supply an already loaded
`RecoveredWorkerV2SynchronousHsaHandoffV1<scalar_gemm_v1_gpu::Marker,
ReviewedHsaRuntimeAdapterV1>` and a reviewed Worker V2 prerequisite
authenticator. The recovered handoff retains the exact publication and
application-descriptor bindings through generated preparation, synchronous
dispatch, and unload. Stale bindings fail closed before dispatch.

Run the CPU and fail-closed checks from the repository root:

```text
examples/scalar_gemm_v1/scripts/test-harness.sh
```

The MI300X controller remains externally driven until the scalar Worker V2
pipeline publishes an inspected, current, authenticated Scalar GEMM V1 COV6
capability. Each hardware case uses one physical output allocation containing
the left canary, checked mutable C subview, and right canary, so both adjacent
canaries are checked after synchronous completion.
