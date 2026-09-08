# MI300X R57 N3 persistent-compute qualification, 2026-09-08

Status: `Exact-product bounded native numerical qualification`. Two executions
of the exact signed source archive completed the R57 three-binding sequence and
emitted the same sole PASS record. This is not performance, formal-refinement,
or generic HIP/HSA parity evidence.

## Provenance and environment

- Exact signed candidate commit:
  `3c12068d6ff0cfab551a346bf1b0175551297cef`.
- Exact tree: `794fa560009bd6237fb2ed2f0081ce021ec92015`.
- Exact source-archive SHA-256:
  `42fdba7a7f6f32b49fe6b74351fbcc6b35149fdfacabcf0f391a878653d17011`.
- Host: `mi300x` (`sharkmi300x-1`), Linux kernel `6.8`, ROCm `7.2.4`.
- Device: GPU 1, `gfx942:xnack-`, unique ID `0xab83d2ffef0d3cdf`.
- Pre-run device observation: 0% sampled GPU use and 0% VRAM use.

The exact signed archive passed both source-gate checks. The independently
pinned qualifier then ran twice from that archive. Each invocation emitted one
PASS line and no additional PASS line, with schema
`fe2o3.runtime.gfx942-r57-n3-qualification.v1` and these identical bounded
observations:

| Field | Observed value |
| --- | --- |
| Expected prepublication rejections | 1 |
| Authority calls | 2 |
| Launches | 2 |
| Data path | `PersistentDeviceReused` |
| User-data materializations | 0 |
| Persistent control reused | `false` |
| Readbacks | 4 |
| Cleanup | `complete` |

The four full-buffer readback SHA-256 digests were:

- A: `1202ea159669597ce883b2af7745fc94afd5d9de7e1985cacb341cec3ae914a7`.
- B: `2c1e99dcde81f5dd034e1cda167af61dd9c7109c72c6a0ede80801f2e1d665d8`.
- C: `564b131bbd3f6eae41a23c72fea11ce4efa9ff0539b16eca66f4aa365d1ad8a7`.
- D: `275fe5e215ed3701e9f0d694119cf46d89846916ea29269dec0533d56e410925`.

This establishes the exact uninitialized-C prepublication rejection followed
by completed `A+B -> C` and resident `C+B -> D` launches, persistent device
storage reuse without user-data rematerialization, full A/B/C/D readback, and
explicit cleanup for this source, host, device, and driver stack.

## Verification and cleanup

Local all-feature validation passed 656 KFD unit tests, 22 KFD integration
tests, 27 KFD doctests, 367 runtime unit tests, two runtime source tests, six
runtime conformance tests, and seven runtime doctests. Three runtime hardware
tests remained explicitly ignored. Strict Clippy, formatting, and diff checks
were green.

After qualification, the private remote stage and local source archive were
deleted. A final host check found no KFD client.

## Claim limits

This evidence covers one fixed N=3 binding shape, two dependent vecadd
launches, one host, one GPU, one software stack, and two executions. It does
not cover general binding arity, three-binding control replay, partial or
padded ranges, auxiliary lanes, overlap, XGMI compute, a general full-write
initialization certificate, Rust-to-native refinement, latency, throughput,
HIP/HSA parity, or any speedup.
