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

That script intentionally does not invoke the separate raw hardware smoke test.
The explicitly non-authoritative gfx942 check can be run with:

```text
FE2O3_RUN_GFX942_SCALAR_GEMM_SMOKE=1 \
FE2O3_GFX942_SCALAR_GEMM_HSACO=/absolute/canonical/scalar-gemm-v1-gfx942.hsaco \
FE2O3_GFX942_SCALAR_GEMM_SHA256=<64-lowercase-hex-digits> \
cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks \
  --test gfx942_scalar_gemm_hardware \
  gfx942_scalar_gemm_v1_raw_smoke_bypasses_production_prerequisite_authentication_and_grants_no_protected_evidence \
  -- --ignored --exact --nocapture
```

This direct raw-adapter smoke test bypasses production prerequisite
authentication and grants no protected evidence.

The MI300X controller remains externally driven until the scalar Worker V2
pipeline publishes an inspected, current, authenticated Scalar GEMM V1 COV6
capability. Each hardware case uses one physical output allocation containing
the left canary, checked mutable C subview, and right canary, so both adjacent
canaries are checked after synchronous completion.
