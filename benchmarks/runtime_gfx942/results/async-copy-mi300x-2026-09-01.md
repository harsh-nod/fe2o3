# gfx942 Async Copy Result: MI300X, 2026-09-01

Status: `Measured` for the exact configuration below. This is hardware evidence,
not a formal proof or a general HIP/HSA parity claim.

## Provenance

- Measured source commit: `a7b427e2172d037c96b63b9e0a78d030e9d03d22`
- Operator-recorded host: `sharkmi300x-1`, Linux `6.8.0-124-generic`
- Devices: AMD Instinct MI300X `gfx942`, indices `0,1`
- KFD unique IDs: `6ced1647a296545c`, `ab83d2ffef0d3cdf`
- Target: `gfx942:xnack-`; HSA reported XNACK disabled and HIP reported
  `gfx942:sramecc+:xnack-` for both devices
- ROCm: `7.2.4`
- Rust: `1.97.1 (8bab26f4f 2026-07-14)`
- SDMA manifest SHA-256:
  `a1a2f3cb07b67e8f66d89578d278853d5750b1a0ad862f0edd27c2fb1ef7b4ec`
- Traffic: 1 MiB per copy, depths 1 and 16, 10 warmups, 30 samples
- Raw result: [async-copy-mi300x-a7b427e21.txt](async-copy-mi300x-a7b427e21.txt)
- Raw result SHA-256:
  `259b43c1b27d3eb233403b2c73eb50414bd8171dc299ec75ef9d6d5a399b28c9`

Every phase began and ended with every GPU used by that phase at 0% load. Each
warmup and measured round used a new per-slot/per-device pattern, poisoned the
download storage, and validated every returned byte. The runner's
frozen-manifest test passed, every physical identity and XNACK gate matched, and
no phase reached its 120-second timeout.

## Throughput

Throughput in GB/s computed from nearest-rank p50 latency over the aligned host
submit-plus-wait intervals:

| Scope | Depth per device | Direction | KFD | HSA | HIP |
| --- | ---: | --- | ---: | ---: | ---: |
| Single device | 1 | H2D | 7.971 | 23.148 | 21.840 |
| Single device | 1 | D2H | 8.040 | 40.518 | 38.338 |
| Single device | 16 | H2D | 21.435 | 24.774 | 31.259 |
| Single device | 16 | D2H | 26.476 | 37.501 | 54.267 |
| Two-device aggregate | 1 | H2D | 7.845 | 25.568 | 24.822 |
| Two-device aggregate | 1 | D2H | 7.892 | 62.959 | 58.053 |
| Two-device aggregate | 16 | H2D | 36.801 | 58.335 | 64.426 |
| Two-device aggregate | 16 | D2H | 38.141 | 65.220 | 76.929 |

KFD is slower in every measured copy row. At depth 16 it reaches 86.5%/70.6%
of HSA single-device H2D/D2H throughput and 68.6%/48.8% of HIP. Its two-device
aggregate reaches 63.1%/58.5% of HSA and 57.1%/49.6% of HIP. Depth improves KFD
utilization substantially, but the result does not establish performance
parity.

The allocation rows are intentionally not ratioed. KFD measures a pooled
host-plus-device checkout/recycle pair, HSA one device-pool allocate/free pair,
and HIP one stream-ordered device allocate/free pair; these are different
operations.

## Limits

- This is one run on two devices and one ROCm/kernel stack, not a distribution
  across hosts or releases.
- Load was gated at every phase boundary, not observed continuously. A competing
  job wholly contained inside a phase would not be detected.
- The timing boundaries align host submission and waiting, but the native APIs,
  signal/currentness work, and allocation policies differ. Multi-device HIP also
  includes single-threaded `hipSetDevice` transitions.
- This harness does not compare peer copy. The KFD runtime peer route is
  synchronous and host staged, unlike native HIP/HSA peer mechanisms.
- Verus proves the abstract resource model only. Driver, firmware, coherence,
  completion, liveness, and these performance values remain contracted or
  measured evidence.
