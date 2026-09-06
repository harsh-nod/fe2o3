# MI300X R37 native SDMA wait activation, 2026-09-05

Status: `Measured regression` for the exact bounded R26 V4 runs below. R37
activated the native bounded wait for Published directional and same-device
SDMA submissions. Against the exact R36 proof baseline, KFD E2E p50 regressed
by 22.885%, 17.956%, and 25.449% in the three matched slots. This record also
retains a diagnostic-only spin-budget experiment; that experiment is not an
R37 production result.

## Provenance

- Exact baseline:
  [`8b6fe6b307ac1ef60123bd1081623670be6cef87`](https://github.com/harsh-nod/fe2o3/commit/8b6fe6b307ac1ef60123bd1081623670be6cef87),
  whose runtime production is R36.
- Exact R37 production:
  [`f81d67fa603ecf23ebd101556b327ae80f13c5ec`](https://github.com/harsh-nod/fe2o3/commit/f81d67fa603ecf23ebd101556b327ae80f13c5ec).
  Its production delta is confined to `crates/fe2o3-runtime/src/kfd_backend.rs`
  and `crates/fe2o3-runtime/src/kfd_backend/kfd_backend_sdma_seam.rs`.
- Exact R37 proof:
  [`19602f5a7dfdaa76e6a96fbc890deb748afb2d65`](https://github.com/harsh-nod/fe2o3/commit/19602f5a7dfdaa76e6a96fbc890deb748afb2d65).
- Host and workload: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm
  `7.2.4`, GPU 2 (`gfx942:xnack-`, unique ID `0xd2e26fef80cf5c33`), one
  1 MiB in-place `u32` transform, 10 warmups, 30 samples per backend per
  slot, and 10 iterations averaged into each sample. The three slots use the
  R26 cyclic Latin backend order.

The external baseline archive is
`/tmp/fe2o3-r37-baseline-8b6fe6b-r26-20260905-evidence.tar.gz`, SHA-256
`8e6b1ca01d529cca444adf3f9024e4850d53f7346df97aec04dcdb511d679b31`,
size 94,644 bytes. Its set ID is
`1c89e8878628f542bc2ebb7afeff8c4b5bbe45816ea84e9fff0c0262af067aec`;
manifest SHA-256 is
`34a861779a7e4791851508bc2a29ac2cafceed3e7ca2d3dc291ae79552a30840`.

The external R37 archive is
`/tmp/fe2o3-r37-f81d67fa-r26-20260905-evidence.tar.gz`, SHA-256
`5cac9f4929ee1d3c56bd147d625a410a70ff23b09c5b6e96c05a983761a92e25`,
size 94,566 bytes. Its set ID is
`685970faa3fd1a094b630bf8508b624604b77f4e995c464dd3f16c9398804dbb`;
manifest SHA-256 is
`18ede79e067d6b7f400fdcdaa26b575b95027a2d48a788d8cbe0de4d8f7787c3`.
Both set validations report all three slots and the set as passing. These
`/tmp` archives are external and non-durable.

## Measured path

The R26 workload submits persistent H2D and D2H copies through the public
asynchronous runtime and waits separately. R37 recognizes Published
directional and same-device SDMA entries in `wait_v1`, calls the typed native
wait through the runtime seam, and advances only an exact completion. Exact
timeout restores Published custody; other native failures are terminal. Poll
and residual non-Published waits retain their earlier routes.

## Results

All values are host-monotonic p50 nanoseconds. Lower is better.

| Slot | Revision | KFD H2D | KFD compute | KFD D2H | KFD E2E |
| --- | --- | ---: | ---: | ---: | ---: |
| 0 | baseline | 141171 | 82936 | 50422 | 274535 |
| 0 | R37 | 206374 | 84562 | 46448 | 337362 |
| 1 | baseline | 147271 | 83437 | 53986 | 285525 |
| 1 | R37 | 205282 | 84671 | 46272 | 336795 |
| 2 | baseline | 137351 | 81836 | 50061 | 269842 |
| 2 | R37 | 207256 | 84539 | 46643 | 338514 |

R37 H2D p50 regressed by 46.187%, 39.391%, and 50.895%. D2H movement was
lower in every slot, but it did not offset the H2D and
compute increases. R37 KFD E2E remained 3.386x-3.408x slower than its
same-slot HIP reference and 3.707x-3.716x slower than HSA.

## Diagnosis

The diagnostic archive is
`/tmp/fe2o3-r37-wait-spin16384-01c896dc-20260905-diagnostic-evidence.tar.gz`,
SHA-256
`da09fb9944bd50ccaff29f851676b7bf28d978c9300717994d05e9b9a3469999`,
size 110,932 bytes. It changes only `SPIN_ATTEMPTS_V1` from 64 to 16,384 in
private experimental commit `01c896dc5226f070aeab23378e93b95d3a918cf4`;
the patch SHA-256 is
`cdf1d1ea46c878fd4a31c31dc8ac021c1ebec086e2dfb8c4bee5ed1087aa3119`.

The candidate's KFD E2E p50 was 265557, 263610, and 264621 ns, 21.284%,
21.730%, and 21.829% below R37. A bounded `strace` probe observed two
`sched_yield` calls for R37 and none for the candidate, but ptrace changes the
observation rate and cannot establish untraced syscall counts. Whole-process
CPU measurements were dominated by setup and validation. The evidence
supports investigating premature yield/backoff on short persistent copies; it
does not establish the experimental spin count as a production policy.

## Formal boundary

The independent R37 finite model proves 15 obligations and rejects seven
pinned mutations. It covers exact typed-timeout classification, one abstract
observation including zero deadline, timeout restoration, terminal custody,
success-only settlement or Ready continuation, and route selection. The full
authenticated runner at `19602f5a` reports 838 obligations and 333 rejected
mutations; transcript SHA-256 is
`fcc0e0302f1b14d0ceff8fd5cec89b021f115fd35c25084e631169cfeedf7471`.

The proof uses contracted mathematical inputs. It has no Rust-to-Verus or
native refinement theorem and proves no real clock, polling, syscall, driver,
firmware, hardware, liveness, parity, or performance property.

## Archive integrity

The retained external containers can be checked without rebuilding or rerunning
the workload:

```bash
printf '%s  %s\n' \
  8e6b1ca01d529cca444adf3f9024e4850d53f7346df97aec04dcdb511d679b31 \
  /tmp/fe2o3-r37-baseline-8b6fe6b-r26-20260905-evidence.tar.gz \
  5cac9f4929ee1d3c56bd147d625a410a70ff23b09c5b6e96c05a983761a92e25 \
  /tmp/fe2o3-r37-f81d67fa-r26-20260905-evidence.tar.gz | sha256sum --check
gzip -t /tmp/fe2o3-r37-{baseline-8b6fe6b,f81d67fa}-r26-20260905-evidence.tar.gz
```

## Claim limits

The revisions ran sequentially with three slots on one MI300X and one fixed
workload. P50s are descriptive statistics, not confidence intervals. The
diagnostic candidate is private and is neither the production R37 code nor a
general CPU-utilization study. This record establishes a bounded regression
and a plausible wait-policy diagnosis, not causality, application behavior,
HIP/HSA parity, or an orders-of-magnitude result.
