# Scalar GEMM V1 hardware harness

This standalone crate keeps the concrete HSA test controller inside the scalar
GEMM example without changing the workspace dependency graph.

The controller has no path, environment-variable, raw-HSACO, digest-string, or
raw-pointer entry point. A caller must supply an already loaded
`LoadedHsaExecutableV1<scalar_gemm_v1_gpu::Marker,
ReviewedHsaRuntimeAdapterV1>` and a reviewed Worker V2 prerequisite
authenticator. Those types are obtainable only after current-publication
admission, compiler and proof prerequisite authentication, exact-byte loading,
and exact-symbol resolution.

Run the CPU and fail-closed checks from the repository root:

```text
examples/scalar_gemm_v1/scripts/test-harness.sh
```

The MI300X controller remains externally driven until the scalar Worker V2
pipeline publishes an inspected, current, authenticated Scalar GEMM V1 COV6
capability. The exact scalar generated slice wrappers also currently accept
whole `DeviceBuffer`s only. Hardware checks therefore retain independent
device canary allocations; truly adjacent prefix/suffix canaries require
generated scalar checked-view constructors.
