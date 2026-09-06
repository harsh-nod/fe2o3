# MI300X R36 completion/recycle fusion, 2026-09-05

Status: `Measured` for the exact bounded R26 V4 runs below. Against the exact
R35 production baseline, R36 reduced the directly targeted KFD
`completion_signal_recycle` p50 in all three matched slots by **46.990%**,
**46.592%**, and **47.136%**; the median slotwise reduction was **46.990%**.
The inclusive recycle p50 fell by **30.979%**, **31.059%**, and **31.793%**;
the median was **31.059%**. Median slotwise KFD E2E movement was **1.756%**
lower raw, **2.440%** lower after HSA ratio-of-ratios adjustment, and **1.666%**
lower after HIP adjustment. This is one clean `n=3` descriptive comparison.
It is not a causal, parity, orders-of-magnitude, application-speedup, or
workload-general result.

## Provenance

- Exact baseline:
  [`0a8555e4fb029ff5317011f0063549d1a49ff541`](https://github.com/harsh-nod/fe2o3/commit/0a8555e4fb029ff5317011f0063549d1a49ff541).
  Its KFD/runtime production sources are byte-identical to R35 production
  commit `4b324bbd53e4c6e767c5c5f2f18817c133edbe03`.
- Exact R36 production implementation:
  [`d32aa6e61e49fb16e44ba3cd715563e9e452b23f`](https://github.com/harsh-nod/fe2o3/commit/d32aa6e61e49fb16e44ba3cd715563e9e452b23f)
  (`perf(kfd): fuse completion recycle currentness`). The complete production
  delta is confined to `persistent_compute.rs`, `queue_completion.rs`,
  `queue_live.rs`, and `shared_memory.rs` in `fe2o3-kfd`, plus runtime
  `kfd_backend.rs`.
- Exact R36 proof:
  [`8b6fe6b307ac1ef60123bd1081623670be6cef87`](https://github.com/harsh-nod/fe2o3/commit/8b6fe6b307ac1ef60123bd1081623670be6cef87)
  (`proof(runtime): verify fused completion recycle`).
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm `7.2.4`, amdgpu
  version `6.16.13`, build ID
  `4cd22e1f91450b8d9da1fc7bbbc02ee412e202d9`.
- Device: host GPU 2, AMD Instinct MI300X, `gfx942:xnack-`, PCI
  `0000:46:00.0`, unique ID `0xd2e26fef80cf5c33`, KFD GPU ID `29122`, NUMA
  node 0.
- Workload: one 1 MiB in-place `u32` transform, 10 warmups, 30 samples per
  backend per slot, and 10 iterations averaged into each sample. The three
  cyclic Latin orders were KFD/HSA/HIP, HSA/HIP/KFD, and HIP/KFD/HSA.
- Corrected runner SHA-256:
  `8c8b59b77705072b83866076d38260f38951e1f66ef7dd6ffc384e5372c13c2f`;
  checker:
  `46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f`;
  host guard:
  `877c2b9199c5594a23c681dccdc1e58c2bef7228a87b16e22150baebc21af8b6`;
  system-identity collector:
  `6a80769bde37c41787a28c725d9c6eeb04ec0837a752924969bed952af8e036f`.
- Both private archived builds record Rust
  `1.96.0-nightly (55e86c996 2026-04-02)` and Cargo
  `1.96.0-nightly (888f67534 2026-03-30)`. Both record HIP
  `7.2.53211-97f5574fe2` and GCC `13.3.0`.
- HSACO SHA-256:
  `8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9`.
  Kernel source, policy, and fixture-recipe SHA-256 values were respectively
  `1185d4cd931c1bb43d113e66714af3d98bd96f7d036f5c610a909abf34ba87d5`,
  `c060c3c4a96012fc6661b0585f4ff8ffe7b7f8483eb40262e4a018133c0ea585`,
  and `29c6db8ea2a86392eb980b78e42fa1c049a6f92ca8dd3dc8224f90cf66254ab5`.
- HSA source, HIP source, binary-reader, HSA-pool-policy, and common-header
  SHA-256 values were respectively
  `a1470c846474dcb10354202a5abd028a7ef9f13e9f36271eedec557953ff523e`,
  `da7839dbbf12b18421e01c32e35d3b33935846deeef6d0210dfa725179bed542`,
  `f41fc9211a317728cebff51197af52e6fde30efd3ea6a177dc22b8f71e92cf1c`,
  `30279269a8a6d4b20f9be38c7d1a35000dfd68562ee165e8406596af4f2177a3`,
  and `83bb2a5a81aff2ddfdd2e7993780d00d175e00ca786556febcefb6d76b2f1e9d`.

The baseline external archive is
`/tmp/fe2o3-r36-baseline-0a8555e-r26-20260905-evidence.tar.gz`, SHA-256
`ff8e3b795ce1f62b52afab5ebb8924269e79350fa0813fce00b85ce1692118c6`,
size 94,477 bytes. Its counterbalance set ID is
`102493e81644b4b4be6289b1a35ef5dfb2769662f81befbc24d5e6ecd2e51654`.
The set records manifest and validation SHA-256 values
`3787a126f37665054f2d4e773906b45ce8d1e5a4c3539661e98c872a0d5c6b74`
and `1e4f1d04df9291be293de66be9dca4d321382990b4c375e6c67074c55d10720d`.

The R36 external archive is
`/tmp/fe2o3-r36-d32aa6e-r26-20260905-evidence.tar.gz`, SHA-256
`1a3170f88db6235d1a1a7f28042d50a27f347a43c1f0f7971ec897f28bfed407`,
size 94,850 bytes. Its counterbalance set ID is
`a9f0281634f8b57499e537a02bf5d5b5ddab01ab3bb3b57b59dc983d269d27a0`.
The set records manifest and validation SHA-256 values
`2c94cfb0b715ff5d2e59aee09e097cd3661d9fcf7f3ada8d63370336b579879c`
and `a6b164f1e91cfa733765237e81e2a8d8c5d941650c2c6605d9dfc05244a879b1`.
These `/tmp` archives are external and non-durable.

Baseline executable SHA-256 values were KFD
`c9a94d231472dd5b5ffa215f586a85cf6cf3e555204a02357f41f73c075e0111`,
HSA `9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83`,
and HIP `3a229bf9c70403a7d272204d234f3d3f509af7a5a71636ccd87cf7bc2d9a0238`.
R36 values were KFD
`8bcb681fbcddede0566beeeedfd982c4b232d44a4f05d569f751bedd38dbc5fc`,
the same byte-identical HSA executable, and HIP
`75a5feb54c301559555869316996574a1a663df07043d31eb6983a9673d88acb`.
HIP source, compiler, and loaded library identities match, but the executable
bytes do not; HIP is a matched-source control, not a byte-identical one.

Stable recorded system identities also match: boot ID
`317d0f9a-4f05-4ab0-8922-3ebfd7354c8b`, topology SHA-256
`43538be8d641b68ec9cfe545f0b64e42e0b1404de6678dce43752824a91c0c37`,
amdgpu module SHA-256
`e5a327a8f46459e07ee3f59cc991d16feee17103e199d39149823879b7fcff0b`,
decompressed module SHA-256
`61317154cee502ea97a74818879dff4b20abf8f074a2f4d19a94288e25d4ac3a`,
HSA library SHA-256
`b8cdfe93d343649a35c1daf73a0a3a6840f09379ebeee9be65670461ffea43f4`
with build ID `cbe2c420f8c65e4710580d19cfd7950db722ea9f`, HIP library SHA-256
`f1043337461c8e54ee135e95fa979a7d0e4344676ad5b0554652f844f8f098ac`
with build ID `db1aaf11568a2d99249b8c24ff700694ff6857dd`, and ROCm-SMI library SHA-256
`cca245677e869de87b11b3f4c0358be63c60190f26a0821ed06f6801764125a5`.

## Measured path

The exact R36 sources establish the path measured by R26:

1. The [R26 compute interval](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs#L405-L452)
   calls the public typed launch, flushes the stream, waits separately, and
   stops its host-monotonic timer after completion.
2. The benchmark [requires every measured KFD iteration to report persistent
   HBM and retained-control reuse](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs#L455-L473).
   Its [constants and measured loop](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs#L32-L37)
   select 10 warmups, 30 samples, and 10 iterations per sample, while the
   [execution block](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs#L647-L724)
   runs warmups before the 300 measured iterations.
3. The runtime's [persistent completion branch](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-runtime/src/kfd_backend.rs#L5107-L5204)
   invokes the fused queue operation and attributes the returned midpoint to
   `publish_to_completion`; time after that midpoint is
   `completion_signal_recycle`.
4. The [move-only public KFD result](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-kfd/src/persistent_compute.rs#L382-L405)
   distinguishes Pending from Recycled and Poll from Recycle failure. The
   midpoint is explicitly after completion-ledger advancement and before
   signal reset.
5. The queue's [private orchestration](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-kfd/src/queue_live.rs#L2894-L2929)
   short-circuits Pending, captures the midpoint only after Ready, then invokes
   recycle. The [public fused queue method](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-kfd/src/queue_live.rs#L11449-L11517)
   carries exact Published/Completed/Recycled custody through that sequence.
6. The [one-packet completion specialization](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-kfd/src/queue_completion.rs#L1282-L1370)
   avoids slot-index and observation `Vec` construction. Its private currentness
   handoff skips only recycle's duplicate opening check; it preserves signal
   reset and the closing currentness check before reuse. The native backend's
   [currentness check](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-kfd/src/queue_live.rs#L525-L542)
   still covers operational memory, opener-process validation, and exception
   event shadows.
7. The [performance record contract](https://github.com/harsh-nod/fe2o3/blob/d32aa6e61e49fb16e44ba3cd715563e9e452b23f/crates/fe2o3-runtime/src/kfd_backend.rs#L172-L198)
   defines `recycle` as signal recycle plus detach/restore and states that the
   host midpoint is not a device clock.

The successful retained-control path above was exercised. The archive does not
exercise or validate the fused operation's failure routes.

## P50 results

All values are nanoseconds. These are the exact p50 fields from each retained
slot log; there is no cross-slot sample pooling.

| Revision | Slot | Backend | H2D | Compute | D2H | E2E |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| Baseline | 0 | KFD | 144335 | 86533 | 48996 | 280280 |
| Baseline | 0 | HSA | 44908 | 20667 | 25370 | 91189 |
| Baseline | 0 | HIP | 47275 | 25082 | 26494 | 98944 |
| Baseline | 1 | KFD | 144335 | 86590 | 49589 | 281443 |
| Baseline | 1 | HSA | 44841 | 20758 | 25319 | 91007 |
| Baseline | 1 | HIP | 47146 | 24970 | 26793 | 99132 |
| Baseline | 2 | KFD | 144780 | 87370 | 49472 | 282064 |
| Baseline | 2 | HSA | 44854 | 20836 | 25417 | 91226 |
| Baseline | 2 | HIP | 47010 | 25013 | 26656 | 98768 |
| R36 | 0 | KFD | 141394 | 83228 | 50113 | 275358 |
| R36 | 0 | HSA | 44779 | 20810 | 25265 | 90945 |
| R36 | 0 | HIP | 46766 | 25065 | 26798 | 98853 |
| R36 | 1 | KFD | 137266 | 83718 | 50155 | 271424 |
| R36 | 1 | HSA | 45118 | 20736 | 25333 | 91289 |
| R36 | 1 | HIP | 47096 | 25003 | 26796 | 99015 |
| R36 | 2 | KFD | 144603 | 83448 | 49574 | 278431 |
| R36 | 2 | HSA | 45411 | 20791 | 25847 | 92303 |
| R36 | 2 | HIP | 47061 | 25008 | 26685 | 98964 |

Cross-slot medians of the three KFD p50 fields were
`144335/86590/49472/281443` for baseline H2D/compute/D2H/E2E and
`141394/83448/50113/275358` for R36. These summary medians are descriptive;
all revision effects below are medians of matched slotwise ratios.

## KFD components

| Revision | Slot | Promotion | Preparation | Bound snapshot | Authority | Native binding | Publication | Publish to completion | Completed readback | Signal recycle | Detach/restore | Recycle inclusive |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Baseline | 0 | 13218 | 4200 | 186 | 2860 | 16895 | 16233 | 30843 | 0 | 9800 | 4229 | 14074 |
| Baseline | 1 | 13258 | 4121 | 199 | 2833 | 16982 | 16340 | 30738 | 0 | 9757 | 4318 | 14128 |
| Baseline | 2 | 13215 | 4227 | 199 | 2837 | 17129 | 16375 | 30989 | 0 | 9793 | 4420 | 14264 |
| R36 | 0 | 13451 | 4296 | 211 | 2824 | 17574 | 16382 | 30817 | 0 | 5195 | 4502 | 9714 |
| R36 | 1 | 13444 | 4312 | 259 | 2848 | 17561 | 16449 | 30931 | 0 | 5211 | 4524 | 9740 |
| R36 | 2 | 13275 | 4421 | 231 | 2867 | 17407 | 16366 | 30976 | 0 | 5177 | 4543 | 9729 |

Positive percentages mean lower latency. The median is across the three
matched slotwise percentage changes, not a ratio of cross-slot medians.

| Component | Slot 0 | Slot 1 | Slot 2 | Median slotwise reduction |
| --- | ---: | ---: | ---: | ---: |
| Promotion | -1.762748% | -1.402927% | -0.454030% | -1.402927% |
| Preparation | -2.285714% | -4.634797% | -4.589543% | -4.589543% |
| Bound snapshot | -13.440860% | -30.150754% | -16.080402% | -16.080402% |
| Authority | 1.258741% | -0.529474% | -1.057455% | -0.529474% |
| Native binding | -4.018941% | -3.409492% | -1.622979% | -3.409492% |
| Publication | -0.917883% | -0.667075% | 0.054962% | -0.667075% |
| Publish to completion | 0.084298% | -0.627887% | 0.041950% | 0.041950% |
| Completed readback | 0.000000% | 0.000000% | 0.000000% | 0.000000% |
| Completion signal recycle | 46.989796% | 46.592190% | 47.135709% | **46.989796%** |
| Completion detach/restore | -6.455427% | -4.770727% | -2.782805% | -4.770727% |
| Recycle inclusive | 30.979110% | 31.058890% | 31.793326% | **31.058890%** |

The targeted signal-recycle absolute reductions were 4,605 ns, 4,546 ns, and
4,616 ns. Inclusive recycle fell 4,360 ns, 4,388 ns, and 4,535 ns. Other
components moved in both directions. Component p50 values are independently
computed; they are not expected to add exactly to an independently ranked
compute p50.

## Phase effects

For backend `B`, adjusted reduction in slot `s` is
`100 * (1 - (KFD_R36/B_R36) / (KFD_base/B_base))`. Positive is favorable.
This ratio-of-ratios controls only for matched contemporaneous reference
movement; it does not make the comparison causal.

| Phase | Comparison | Slot 0 | Slot 1 | Slot 2 | Median slotwise |
| --- | --- | ---: | ---: | ---: | ---: |
| H2D | Raw KFD | 2.037621% | 4.897634% | 0.122254% | 2.037621% |
| H2D | HSA-adjusted | 1.755409% | 5.481511% | 1.347330% | 1.755409% |
| H2D | HIP-adjusted | 0.971401% | 4.796667% | 0.230492% | 0.971401% |
| Compute | Raw KFD | 3.819352% | 3.316780% | 4.488955% | 3.819352% |
| Compute | HSA-adjusted | 4.480276% | 3.214204% | 4.282231% | 4.282231% |
| Compute | HIP-adjusted | 3.754119% | 3.444387% | 4.469859% | 3.754119% |
| D2H | Raw KFD | -2.279778% | -1.141382% | -0.206177% | -1.141382% |
| D2H | HSA-adjusted | -2.704847% | -1.085488% | 1.460889% | -1.085488% |
| D2H | HIP-adjusted | -1.119503% | -1.130059% | -0.097278% | -1.119503% |
| E2E | Raw KFD | 1.756101% | 3.559868% | 1.288006% | 1.756101% |
| E2E | HSA-adjusted | 1.492519% | 3.857781% | 2.439786% | 2.439786% |
| E2E | HIP-adjusted | 1.665662% | 3.445911% | 1.483506% | 1.665662% |

Exact R36-minus-baseline raw KFD deltas, where negative is lower/faster, were:

| Phase | Slot 0 | Slot 1 | Slot 2 |
| --- | ---: | ---: | ---: |
| H2D | -2941 ns | -7069 ns | -177 ns |
| Compute | -3305 ns | -2872 ns | -3922 ns |
| D2H | +1117 ns | +566 ns | +102 ns |
| E2E | -4922 ns | -10019 ns | -3633 ns |

The targeted recycle reduction is directionally consistent across all slots,
as is the broader compute reduction. The experiment did not instrument a
currentness-call counter or system calls and has no GPU timestamps, so it does
not isolate the removed opening check as the cause of either change.

## Residual gaps

Ratios below are R36 KFD/reference p50; lower is better and `1.0x` is equality.

| Slot | H2D/HSA | H2D/HIP | Compute/HSA | Compute/HIP | D2H/HSA | D2H/HIP | E2E/HSA | E2E/HIP |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 3.157596x | 3.023436x | 3.999423x | 3.320487x | 1.983495x | 1.870028x | 3.027742x | 2.785530x |
| 1 | 3.042378x | 2.914600x | 4.037326x | 3.348318x | 1.979829x | 1.871735x | 2.973239x | 2.741241x |
| 2 | 3.184317x | 3.072672x | 4.013660x | 3.336852x | 1.917979x | 1.857748x | 3.016489x | 2.813457x |

R36 therefore remains 2.74x-2.81x slower than HIP E2E and 3.32x-3.35x slower
than HIP compute in these slots. D2H remains 1.86x-1.87x and H2D
2.91x-3.07x slower than HIP. The HSA gaps are also material. This run does not
establish HIP/HSA parity.

## Validation and cleanup

Retained file SHA-256 values were:

| File | Baseline | R36 |
| --- | --- | --- |
| `run.log` | `096c315f07f68bdbe1f88c3490764d926e711e42a9269edcaecdf5ee9cf70f3a` | `74b9a2dcf2b73421479f0afc45dd3e6c5d1cc6c583a51432eee641b2efd69317` |
| `preflight.txt` | `9cba9580b01322427018c90f5b9c6893951676f8e4f30d2f9b60ff14e9aea2b8` | `5df70d22c332a58893d4f0908b63d75b0032bea354da2a338af2d0373491a6a2` |
| `postrun.txt` | `be66e212f50ba2e44939903d36c180ad13ec15e4d5beffcdae4b72b883e34943` | `64cceb6a03deb2d66333488217895181b1be4ec952063c5f263c80500e1ab90c` |
| `set-validation.txt` | `1e4f1d04df9291be293de66be9dca4d321382990b4c375e6c67074c55d10720d` | `a6b164f1e91cfa733765237e81e2a8d8c5d941650c2c6605d9dfc05244a879b1` |
| `slot-0.log` | `ced6ef198cd9eb81a67167e1b1221846080f1d8b7dcbf85ffe04094a7f28f7f9` | `8c02b1b00463e432d28f480fdde016796c6085a30edbd2dfa07baa82beaba1c7` |
| `slot-1.log` | `3a8bfccd592e400c69503fb3ae5d17ddb57b7e420ebb3dd80060d3e8321f0568` | `312fed966dad66c53cb4a9a1dc503e3e2f3ee743815ad8aef04d821ad39864e8` |
| `slot-2.log` | `bac872d81495c3faaad9a35a68ce937f81b95c306cbe292cf18e0c8eb7d1c9e6` | `0d6895bb18339d82a7458353e5a72f6063a309c6519783354ac5df7ed30efd5e` |

All 18 backend-phase monitors were clean. Every row recorded zero foreign and
terminal selected queues, target exit 0, a reaped target, and an absent process
group. The requested interval was 2,000 us and the authenticated maximum was
10,000 us.

| Revision | Slot | Backend | Observed max gap | Monitor SHA-256 |
| --- | ---: | --- | ---: | --- |
| Baseline | 0 | KFD | 3321 us | `904a1c4ea4bbe8dabfbe225a26230fdd5b500da7a65a3e1e00e889b350db7331` |
| Baseline | 0 | HSA | 3419 us | `f0206784187885343478c74d17c12a2f40e6fe3040e62b658c438a706ddc79b2` |
| Baseline | 0 | HIP | 3974 us | `6b1d7ae8de47c2df674c9f9ee9af4800a5f48cd04f559329e936668f8b2ac030` |
| Baseline | 1 | KFD | 3834 us | `35b3158340ad2ff1c32f215261c163f37502150b8305b9752c30725d9fc10792` |
| Baseline | 1 | HSA | 3874 us | `9671b8bafc30b29c5c4b5f721ea9166ea966561404a4b522eb869d79e63d33f5` |
| Baseline | 1 | HIP | 3668 us | `394b600e1bea95aea52e328bcb6495a18ebce7ad2b2b5676e401d04d96f8a93e` |
| Baseline | 2 | KFD | 4071 us | `7a7d07062940f0b70da785886df9ae93958c9631f7c759c6c472cfa0ae40776a` |
| Baseline | 2 | HSA | 3583 us | `629cafc9a6210c3b451d4a4c2d996474c5d4c4199dd1e36b8e5e5effedc37728` |
| Baseline | 2 | HIP | 3173 us | `4a5516bfe2c911cdd50a240c65a8e687ebacc806a5d3001da74ac46c6209c897` |
| R36 | 0 | KFD | 3548 us | `e227f405d69e4c62a02330efb21ae5c54fe388954391fc8a2b522eecd93a83b8` |
| R36 | 0 | HSA | 3704 us | `188f678ff54aaf64dd3feef81710e5d0a373dba63ab81983f9cafa47fddc1d3e` |
| R36 | 0 | HIP | 3722 us | `953f34c22a07b1dc9777d6372872a4fd0a37bf96345f2685e3a171391b84ee9c` |
| R36 | 1 | KFD | 3763 us | `e282066f8a036b4f52d7462ae26d04dc69c9354dbed2da797412016916603207` |
| R36 | 1 | HSA | 3569 us | `e283d684df19ccb9bf8ead8268ba5e8def74fa68dcff3dcf25f661606e7b41db` |
| R36 | 1 | HIP | 3580 us | `96bc1879f5e0a49a5a74c1fcdfa880a43c0e381068112b8f19e6ed45001ff4fd` |
| R36 | 2 | KFD | 4513 us | `b86b4ed010bf9d36abed102a893ba98bc35685eacf0d4cef5f9edeb726b21bf2` |
| R36 | 2 | HSA | 3704 us | `65f03a92f1d51849b25814c13f05f009c322a3dab9aab0ff1b13d0e3009d97a4` |
| R36 | 2 | HIP | 3672 us | `24cadfea1d6de4d3dbe0d6997ff84105add808f5674080f66d6a4ad64c897a4e` |

Both authenticated preflight records report the exact clean commit, GPU 2 at
0% busy, and zero selected KFD queues. Both authenticated postrun records
report the same clean commit, zero busy/queues, and runner exit 0.

Operator-observed facts after the retained archives had already been created
are not authenticated by those archives: the exact remote roots
`/tmp/fe2o3-r36-baseline-r26.8edX35h0` and
`/tmp/fe2o3-r36-current-r26.2lMISu2G`, runner roots
`/tmp/fe2o3-r26-inplace-gfx942.rBHeIw` and
`/tmp/fe2o3-r26-inplace-gfx942.JiG4fd`, and remote archive copies were deleted
with exact-root `find -depth -delete`; their absence was observed afterward.
GPU 2 at 0% busy and KFD GPU 29122 at zero queues were also observed after
cleanup. No `sudo`, kill, or foreign-process/root modification was performed;
a pre-existing foreign `/tmp/fe2o3-r26-8953f757.TwJZz1` was left untouched.

## Reproduction

Run from a checkout containing both exact commits while the two external
archives still exist. This validates gzip streams, archive/file hashes, the
checker materialized from exact R36 production and pinned by SHA-256, immutable
source shape, full raw tables, component and adjusted effects, residual ratios,
identities, all 18 monitors, and authenticated pre/post edges.

```bash
set -euo pipefail
baseline_archive=/tmp/fe2o3-r36-baseline-0a8555e-r26-20260905-evidence.tar.gz
r36_archive=/tmp/fe2o3-r36-d32aa6e-r26-20260905-evidence.tar.gz
root=$(mktemp -d /tmp/fe2o3-r36-evidence-repro.XXXXXXXX)
trap 'find "$root" -depth -delete' EXIT

printf '%s  %s\n' \
  ff8e3b795ce1f62b52afab5ebb8924269e79350fa0813fce00b85ce1692118c6 \
  "$baseline_archive" \
  1a3170f88db6235d1a1a7f28042d50a27f347a43c1f0f7971ec897f28bfed407 \
  "$r36_archive" | sha256sum --check --status
test "$(stat -c %s "$baseline_archive")" = 94477
test "$(stat -c %s "$r36_archive")" = 94850
gzip -t "$baseline_archive"
gzip -t "$r36_archive"
mkdir -m 700 "$root/baseline" "$root/r36"
tar -xzf "$baseline_archive" -C "$root/baseline"
tar -xzf "$r36_archive" -C "$root/r36"

baseline_set="$root/baseline/output/r26-inplace-102493e81644b4b4be6289b1a35ef5dfb2769662f81befbc24d5e6ecd2e51654"
r36_set="$root/r36/output/r26-inplace-a9f0281634f8b57499e537a02bf5d5b5ddab01ab3bb3b57b59dc983d269d27a0"
checker="$root/check-parity.py"
git show \
  d32aa6e61e49fb16e44ba3cd715563e9e452b23f:benchmarks/runtime_gfx942/check-parity.py \
  >"$checker"
printf '%s  %s\n' \
  46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f \
  "$checker" | sha256sum --check --status
python3 "$checker" --schema fe2o3.r26-inplace-benchmark.v4 \
  --r26-counterbalance-set "$baseline_set"/slot-*.log \
  >"$root/baseline-recomputed.txt"
python3 "$checker" --schema fe2o3.r26-inplace-benchmark.v4 \
  --r26-counterbalance-set "$r36_set"/slot-*.log \
  >"$root/r36-recomputed.txt"
cmp "$root/baseline-recomputed.txt" "$baseline_set/set-validation.txt"
cmp "$root/r36-recomputed.txt" "$r36_set/set-validation.txt"

printf '%s  %s\n' \
  1e4f1d04df9291be293de66be9dca4d321382990b4c375e6c67074c55d10720d "$baseline_set/set-validation.txt" \
  096c315f07f68bdbe1f88c3490764d926e711e42a9269edcaecdf5ee9cf70f3a "$root/baseline/run.log" \
  9cba9580b01322427018c90f5b9c6893951676f8e4f30d2f9b60ff14e9aea2b8 "$root/baseline/preflight.txt" \
  be66e212f50ba2e44939903d36c180ad13ec15e4d5beffcdae4b72b883e34943 "$root/baseline/postrun.txt" \
  ced6ef198cd9eb81a67167e1b1221846080f1d8b7dcbf85ffe04094a7f28f7f9 "$baseline_set/slot-0.log" \
  3a8bfccd592e400c69503fb3ae5d17ddb57b7e420ebb3dd80060d3e8321f0568 "$baseline_set/slot-1.log" \
  bac872d81495c3faaad9a35a68ce937f81b95c306cbe292cf18e0c8eb7d1c9e6 "$baseline_set/slot-2.log" \
  a6b164f1e91cfa733765237e81e2a8d8c5d941650c2c6605d9dfc05244a879b1 "$r36_set/set-validation.txt" \
  74b9a2dcf2b73421479f0afc45dd3e6c5d1cc6c583a51432eee641b2efd69317 "$root/r36/run.log" \
  5df70d22c332a58893d4f0908b63d75b0032bea354da2a338af2d0373491a6a2 "$root/r36/preflight.txt" \
  64cceb6a03deb2d66333488217895181b1be4ec952063c5f263c80500e1ab90c "$root/r36/postrun.txt" \
  8c02b1b00463e432d28f480fdde016796c6085a30edbd2dfa07baa82beaba1c7 "$r36_set/slot-0.log" \
  312fed966dad66c53cb4a9a1dc503e3e2f3ee743815ad8aef04d821ad39864e8 "$r36_set/slot-1.log" \
  0d6895bb18339d82a7458353e5a72f6063a309c6519783354ac5df7ed30efd5e "$r36_set/slot-2.log" | sha256sum --check --status
grep -Fq 'manifest_sha256=3787a126f37665054f2d4e773906b45ce8d1e5a4c3539661e98c872a0d5c6b74' "$baseline_set/set-validation.txt"
grep -Fq 'manifest_sha256=2c94cfb0b715ff5d2e59aee09e097cd3661d9fcf7f3ada8d63370336b579879c' "$r36_set/set-validation.txt"

git diff --quiet \
  4b324bbd53e4c6e767c5c5f2f18817c133edbe03 \
  0a8555e4fb029ff5317011f0063549d1a49ff541 -- \
  crates/fe2o3-kfd crates/fe2o3-runtime/src
test "$(git diff --name-only \
  0a8555e4fb029ff5317011f0063549d1a49ff541 \
  d32aa6e61e49fb16e44ba3cd715563e9e452b23f -- \
  crates/fe2o3-kfd crates/fe2o3-runtime/src)" = "$(printf '%s\n' \
  crates/fe2o3-kfd/src/persistent_compute.rs \
  crates/fe2o3-kfd/src/queue_completion.rs \
  crates/fe2o3-kfd/src/queue_live.rs \
  crates/fe2o3-kfd/src/shared_memory.rs \
  crates/fe2o3-runtime/src/kfd_backend.rs)"
git show d32aa6e61e49fb16e44ba3cd715563e9e452b23f:crates/fe2o3-runtime/src/kfd_backend.rs |
  sed -n '5107,5204p' | grep -F 'poll_and_recycle_directional_persistent_fixed_dispatch_v1' >/dev/null
git show d32aa6e61e49fb16e44ba3cd715563e9e452b23f:crates/fe2o3-kfd/src/queue_live.rs |
  sed -n '11449,11517p' | grep -F 'execute_persistent_compute_poll_and_recycle_v1' >/dev/null
git show d32aa6e61e49fb16e44ba3cd715563e9e452b23f:crates/fe2o3-kfd/src/queue_completion.rs |
  sed -n '1282,1370p' | grep -F 'recycle_current_handoff_retaining' >/dev/null

python3 - "$baseline_set" "$r36_set" "$root" <<'PY'
from pathlib import Path
from statistics import median
import sys

phases = ("h2d", "compute", "d2h", "e2e")
components = (
    "promotion", "preparation", "bound_snapshot", "authority",
    "native_binding", "publication", "publish_to_completion",
    "completed_readback", "completion_signal_recycle",
    "completion_detach_restore", "recycle_inclusive",
)

def fields(line):
    return dict(item.split("=", 1) for item in line.split() if "=" in item)

def read_set(path):
    slots = []
    for slot in range(3):
        backends = {}
        for line in (path / f"slot-{slot}.log").read_text().splitlines():
            if not line.startswith("backend="):
                continue
            row = fields(line)
            values = [int(row[f"{phase}_p50_ns"]) for phase in phases]
            if row["backend"] == "kfd":
                values += [int(row[f"{name}_p50_ns"]) for name in components]
            backends[row["backend"]] = values
        slots.append(backends)
    return slots

def record(path, prefix):
    line = next(line for line in path.read_text().splitlines() if line.startswith(prefix))
    return fields(line)

baseline_path, r36_path, extraction = map(Path, sys.argv[1:])
observed = {"baseline": read_set(baseline_path), "r36": read_set(r36_path)}
expected = {
    "baseline": [
        {"kfd": [144335,86533,48996,280280], "hsa": [44908,20667,25370,91189], "hip": [47275,25082,26494,98944]},
        {"kfd": [144335,86590,49589,281443], "hsa": [44841,20758,25319,91007], "hip": [47146,24970,26793,99132]},
        {"kfd": [144780,87370,49472,282064], "hsa": [44854,20836,25417,91226], "hip": [47010,25013,26656,98768]},
    ],
    "r36": [
        {"kfd": [141394,83228,50113,275358], "hsa": [44779,20810,25265,90945], "hip": [46766,25065,26798,98853]},
        {"kfd": [137266,83718,50155,271424], "hsa": [45118,20736,25333,91289], "hip": [47096,25003,26796,99015]},
        {"kfd": [144603,83448,49574,278431], "hsa": [45411,20791,25847,92303], "hip": [47061,25008,26685,98964]},
    ],
}
baseline_components = [
    [13218,4200,186,2860,16895,16233,30843,0,9800,4229,14074],
    [13258,4121,199,2833,16982,16340,30738,0,9757,4318,14128],
    [13215,4227,199,2837,17129,16375,30989,0,9793,4420,14264],
]
r36_components = [
    [13451,4296,211,2824,17574,16382,30817,0,5195,4502,9714],
    [13444,4312,259,2848,17561,16449,30931,0,5211,4524,9740],
    [13275,4421,231,2867,17407,16366,30976,0,5177,4543,9729],
]
for name in expected:
    for slot in range(3):
        for backend in ("kfd", "hsa", "hip"):
            assert observed[name][slot][backend][:4] == expected[name][slot][backend]
for slot in range(3):
    assert observed["baseline"][slot]["kfd"][4:] == baseline_components[slot]
    assert observed["r36"][slot]["kfd"][4:] == r36_components[slot]

common = {
    "runner_sha256": "8c8b59b77705072b83866076d38260f38951e1f66ef7dd6ffc384e5372c13c2f",
    "checker_sha256": "46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f",
    "host_guard_sha256": "877c2b9199c5594a23c681dccdc1e58c2bef7228a87b16e22150baebc21af8b6",
    "system_identity_collector_sha256": "6a80769bde37c41787a28c725d9c6eeb04ec0837a752924969bed952af8e036f",
    "rustc": "rustc_1.96.0-nightly_(55e86c996_2026-04-02)_",
    "cargo": "cargo_1.96.0-nightly_(888f67534_2026-03-30)_",
    "hsaco_sha256": "8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9",
    "kernel_source_sha256": "1185d4cd931c1bb43d113e66714af3d98bd96f7d036f5c610a909abf34ba87d5",
    "hsa_source_sha256": "a1470c846474dcb10354202a5abd028a7ef9f13e9f36271eedec557953ff523e",
    "hip_source_sha256": "da7839dbbf12b18421e01c32e35d3b33935846deeef6d0210dfa725179bed542",
    "topology_sha256": "43538be8d641b68ec9cfe545f0b64e42e0b1404de6678dce43752824a91c0c37",
}
revision = {
    "baseline": {
        "git_commit": "0a8555e4fb029ff5317011f0063549d1a49ff541",
        "kfd_binary_sha256": "c9a94d231472dd5b5ffa215f586a85cf6cf3e555204a02357f41f73c075e0111",
        "hsa_binary_sha256": "9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83",
        "hip_binary_sha256": "3a229bf9c70403a7d272204d234f3d3f509af7a5a71636ccd87cf7bc2d9a0238",
    },
    "r36": {
        "git_commit": "d32aa6e61e49fb16e44ba3cd715563e9e452b23f",
        "kfd_binary_sha256": "8bcb681fbcddede0566beeeedfd982c4b232d44a4f05d569f751bedd38dbc5fc",
        "hsa_binary_sha256": "9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83",
        "hip_binary_sha256": "75a5feb54c301559555869316996574a1a663df07043d31eb6983a9673d88acb",
    },
}
contexts = {}
systems = {}
for name, path in (("baseline", baseline_path), ("r36", r36_path)):
    contexts[name] = record(path / "slot-0.log", "context schema=fe2o3.r26-inplace-benchmark.v4")
    systems[name] = record(path / "slot-0.log", "context schema=fe2o3.r26-system-identity.v1")
    for key, value in common.items(): assert contexts[name][key] == value
    for key, value in revision[name].items(): assert contexts[name][key] == value
assert contexts["baseline"]["hsa_binary_sha256"] == contexts["r36"]["hsa_binary_sha256"]
assert contexts["baseline"]["hip_binary_sha256"] != contexts["r36"]["hip_binary_sha256"]

stable_system = {
    "boot_id": "317d0f9a-4f05-4ab0-8922-3ebfd7354c8b",
    "kernel_release": "6.8.0-124-generic",
    "amdgpu_version": "6.16.13",
    "amdgpu_build_id": "4cd22e1f91450b8d9da1fc7bbbc02ee412e202d9",
    "amdgpu_module_sha256": "e5a327a8f46459e07ee3f59cc991d16feee17103e199d39149823879b7fcff0b",
    "amdgpu_module_decompressed_sha256": "61317154cee502ea97a74818879dff4b20abf8f074a2f4d19a94288e25d4ac3a",
    "pci_bdf": "0000:46:00.0", "unique_id": "0xd2e26fef80cf5c33", "gpu_guid": "29122",
    "hsa_library_sha256": "b8cdfe93d343649a35c1daf73a0a3a6840f09379ebeee9be65670461ffea43f4",
    "hip_library_sha256": "f1043337461c8e54ee135e95fa979a7d0e4344676ad5b0554652f844f8f098ac",
    "rocm_smi_library_sha256": "cca245677e869de87b11b3f4c0358be63c60190f26a0821ed06f6801764125a5",
}
for key, value in stable_system.items():
    assert systems["baseline"][key] == systems["r36"][key] == value

def reductions(reference=None):
    answer = {}
    for index, phase in enumerate(phases):
        values = []
        for slot in range(3):
            ratio = observed["r36"][slot]["kfd"][index] / observed["baseline"][slot]["kfd"][index]
            if reference:
                ratio /= observed["r36"][slot][reference][index] / observed["baseline"][slot][reference][index]
            values.append(100 * (1 - ratio))
        answer[phase] = tuple(f"{value:.6f}" for value in values + [median(values)])
    return answer

assert reductions() == {
    "h2d": ("2.037621","4.897634","0.122254","2.037621"),
    "compute": ("3.819352","3.316780","4.488955","3.819352"),
    "d2h": ("-2.279778","-1.141382","-0.206177","-1.141382"),
    "e2e": ("1.756101","3.559868","1.288006","1.756101"),
}
assert reductions("hsa") == {
    "h2d": ("1.755409","5.481511","1.347330","1.755409"),
    "compute": ("4.480276","3.214204","4.282231","4.282231"),
    "d2h": ("-2.704847","-1.085488","1.460889","-1.085488"),
    "e2e": ("1.492519","3.857781","2.439786","2.439786"),
}
assert reductions("hip") == {
    "h2d": ("0.971401","4.796667","0.230492","0.971401"),
    "compute": ("3.754119","3.444387","4.469859","3.754119"),
    "d2h": ("-1.119503","-1.130059","-0.097278","-1.119503"),
    "e2e": ("1.665662","3.445911","1.483506","1.665662"),
}

component_expected = [
    ("-1.762748","-1.402927","-0.454030","-1.402927"),
    ("-2.285714","-4.634797","-4.589543","-4.589543"),
    ("-13.440860","-30.150754","-16.080402","-16.080402"),
    ("1.258741","-0.529474","-1.057455","-0.529474"),
    ("-4.018941","-3.409492","-1.622979","-3.409492"),
    ("-0.917883","-0.667075","0.054962","-0.667075"),
    ("0.084298","-0.627887","0.041950","0.041950"),
    ("0.000000","0.000000","0.000000","0.000000"),
    ("46.989796","46.592190","47.135709","46.989796"),
    ("-6.455427","-4.770727","-2.782805","-4.770727"),
    ("30.979110","31.058890","31.793326","31.058890"),
]
for index, expected_row in enumerate(component_expected):
    values = []
    for slot in range(3):
        before, after = baseline_components[slot][index], r36_components[slot][index]
        values.append(0.0 if before == after == 0 else 100 * (1 - after / before))
    assert tuple(f"{value:.6f}" for value in values + [median(values)]) == expected_row

residual = [
    ("3.157596","3.023436","3.999423","3.320487","1.983495","1.870028","3.027742","2.785530"),
    ("3.042378","2.914600","4.037326","3.348318","1.979829","1.871735","2.973239","2.741241"),
    ("3.184317","3.072672","4.013660","3.336852","1.917979","1.857748","3.016489","2.813457"),
]
for slot in range(3):
    values = []
    for index in range(4):
        kfd = observed["r36"][slot]["kfd"][index]
        values += [f'{kfd / observed["r36"][slot][ref][index]:.6f}' for ref in ("hsa", "hip")]
    assert tuple(values) == residual[slot]

for name, path, expected_max, commit in (
    ("baseline", baseline_path, 4071, revision["baseline"]["git_commit"]),
    ("r36", r36_path, 4513, revision["r36"]["git_commit"]),
):
    monitors = []
    for slot in range(3):
        monitors += [fields(line) for line in (path / f"slot-{slot}.log").read_text().splitlines() if line.startswith("monitor ")]
    assert len(monitors) == 9
    assert max(int(row["observed_maximum_gap_us"]) for row in monitors) == expected_max
    for row in monitors:
        assert row["status"] == "clean"
        assert row["foreign_selected_queues"] == row["terminal_selected_queues"] == "0"
        assert row["target_exit_code"] == "0" and row["target_reaped"] == row["process_group_absent"] == "1"
        assert int(row["observed_maximum_gap_us"]) <= int(row["maximum_gap_us"])
    pre = fields((extraction / name / "preflight.txt").read_text().replace("\n", " "))
    post = fields((extraction / name / "postrun.txt").read_text().replace("\n", " "))
    assert pre["commit"] == post["repo_commit"] == commit
    assert pre["repo_status_count"] == post["repo_status_count"] == "0"
    assert pre["gpu2_busy_percent"] == post["gpu2_busy_percent"] == "0"
    assert pre["selected_gpuid_29122_queues"] == post["selected_gpuid_29122_queues"] == "0"
    assert post["runner_exit_status"] == "0"

print("R36 evidence reproduction: pass")
PY
```

## Claim limits

The two revisions ran sequentially, baseline first, rather than in randomized
revision order. Each revision has only three Latin slots on one MI300X, one
fixed 1 MiB in-place kernel, and 30 samples per backend per slot. The p50s are
descriptive statistics, not confidence intervals. Host-monotonic measurements
include runtime polling and scheduling effects and do not identify device
dispatch boundaries.

HSA is byte-identical across runs; HIP is matched source/toolchain/library but
not byte-identical. The implementation delta contains related failure-custody,
one-packet, and timing-attribution changes, so even a narrow observed reduction
cannot be assigned uniquely to one source statement. No syscall trace,
currentness-call counter, hardware performance counter, power normalization,
thermal normalization, cross-host replication, broad size sweep, concurrent
workload, multi-device workload, or application benchmark was collected.

The R36 proof establishes a finite, premised projection of custody and logical
ordering plus a separate abstract four-to-three successful currentness-count
fact. The projection intentionally excludes that count and production/public
error identity, real time, and physical timing. It has no Rust-to-Verus
correspondence and proves no production Rust, native observation, syscall,
driver, firmware, hardware, coherence, progress, liveness, parity, or
performance property. The measured result and formal result are complementary
bounded evidence, not a combined proof of optimization correctness or speedup.
