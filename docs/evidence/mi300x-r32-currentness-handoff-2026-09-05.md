# MI300X R32 fused directional currentness handoff, 2026-09-05

Status: `Measured` for the exact bounded R26 V4 run below. Against the exact
R31 raw archive, R32 reduced median slot-matched KFD E2E latency by 9.32%
unadjusted, 9.61% after HSA adjustment, and 9.54% after HIP adjustment. KFD
nevertheless remained approximately 3.01x slower than HIP E2E in every R32
slot. This result does not establish HIP/HSA parity or an orders-of-magnitude
performance claim.

## Provenance

- R31 comparison implementation:
  `2f95b4619a6ca95cd37159821429d9db196d5550`.
- R32 production implementation:
  `9f715189b8f35d4adb58be303900f937d88389ad` (`perf(kfd): fuse
  directional currentness handoff`). The intervening R31 commits were
  proof-only and evidence-only changes.
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm `7.2.4`.
- Device: host GPU 2, AMD Instinct MI300X, `gfx942:xnack-`, PCI
  `0000:46:00.0`, unique ID `0xd2e26fef80cf5c33`, KFD GPU ID `29122`, NUMA
  node 0.
- Rust: `1.96.0-nightly (55e86c996 2026-04-02)`; Cargo:
  `1.96.0-nightly (888f67534 2026-03-30)`.
- Runner SHA-256:
  `126c69a2193437d9da996b927eb8dd2af35f75b5583f1f6c961d012049e3a5fd`;
  host-guard SHA-256:
  `877c2b9199c5594a23c681dccdc1e58c2bef7228a87b16e22150baebc21af8b6`;
  checker SHA-256:
  `46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f`.
- HSACO SHA-256:
  `8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9`.
  R32 executable SHA-256 values were KFD
  `3ab81d26a58089e20013f6bb7e10a633c89bbed2856195e9431ba41d7ecc755d`,
  HSA `9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83`,
  and HIP `c5fcd86008b0b454e28b21aea707c7eb3623fa43d9e8cfb7987e9314ec76db27`.

The R32 local external archive is
`/tmp/fe2o3-r32-9f715189-r26-20260905-evidence.tar.gz`, SHA-256
`b78d9f37801d9e2b6e2391acdb2f532546d41cb88b4a65599a39ab98b91feb5c`.
Its retained extraction is
`/tmp/fe2o3-r32-evidence-extract-9KTSDxjp`; its counterbalance set ID is
`163fa3f48bf1247bb75b53cbe4f5caf18fc417ab54626af18eb94b28e699ef5a`.
The retained set-validation report has SHA-256
`2d8a59c2ce741894de7f5658754466cc9afa05b29ae2a39adc841296b6bfc7b8`.

The exact R31 comparison archive is
`/tmp/fe2o3-r31-2f95b461-r26-20260905-evidence.tar.gz`, SHA-256
`ff98158caaf0fdffe003eda78fb26e5892231ffd8b12d21300f21b77abaf0ae2`.
Its counterbalance set ID is
`1746de309702f89da09699b79ffcaeb52c26098f66c46544937deab62f01b80b`.
Both `/tmp` archives and the extraction are external, non-durable artifacts.

R32's HSA executable is byte-identical to R31's HSA executable. R32's HIP
executable is not byte-identical to R31's
`b8d63c85e727e1cdd0e7f4cd72f868999cd87ebe6dd097fb9da79323178768dd`,
although the pinned HIP source is identical in both archives, SHA-256
`da7839dbbf12b18421e01c32e35d3b33935846deeef6d0210dfa725179bed542`.
The HSA source is also identical, SHA-256
`a1470c846474dcb10354202a5abd028a7ef9f13e9f36271eedec557953ff523e`.
Consequently this is a matched-source comparison, not an entirely
byte-identical reference-binary comparison.

## Method

Each revision used the R26 V4 1 MiB in-place transform, 10 warmups, 30
samples per backend and slot, and 10 iterations per sample. A sample is the
integer average of its 10 host-monotonic iteration durations; the reported
statistic is the untrimmed p50. The cyclic Latin orders were KFD/HSA/HIP,
HSA/HIP/KFD, and HIP/KFD/HSA. Every output element was validated in all 310
iterations per backend and slot.

R32 fuses directional preparation and publication under one owner/memory loan
and performs the shared currentness close before the no-fail handoff to
publication. Its error paths still distinguish preparation failure, prepared
custody without a handoff, recoverable publication failure, retained
publication failure, and confirmed publication. The benchmark sources,
fixture, runner, timing boundaries, and 1 MiB workload were unchanged from
R31. Operational currentness checks and authenticated H2D Ready promotion
remain in the timed paths.

## P50 observations

All values are nanoseconds. Promotion is KFD-only and covers the full
authenticated post-H2D Ready transition, not an isolated lookup. R31 values
below were read from its raw archive rather than transcribed from its evidence
note.

| Revision | Slot | Backend | H2D | Compute | D2H | E2E | Promotion |
|---|---:|---|---:|---:|---:|---:|---:|
| R31 | 0 | KFD | 162444 | 89878 | 77405 | 330440 | 13304 |
| R31 | 0 | HSA | 44851 | 20824 | 25449 | 91362 | n/a |
| R31 | 0 | HIP | 46828 | 25167 | 26941 | 99204 | n/a |
| R31 | 1 | KFD | 162163 | 89764 | 77315 | 329298 | 13326 |
| R31 | 1 | HSA | 44986 | 20817 | 25395 | 91403 | n/a |
| R31 | 1 | HIP | 46931 | 25006 | 26829 | 98939 | n/a |
| R31 | 2 | KFD | 162025 | 90024 | 76771 | 328769 | 13331 |
| R31 | 2 | HSA | 44836 | 20726 | 25248 | 90935 | n/a |
| R31 | 2 | HIP | 47255 | 24979 | 26638 | 98989 | n/a |
| R32 | 0 | KFD | 146451 | 90768 | 61725 | 298941 | 13424 |
| R32 | 0 | HSA | 44673 | 20931 | 26013 | 91809 | n/a |
| R32 | 0 | HIP | 47046 | 25164 | 26829 | 99208 | n/a |
| R32 | 1 | KFD | 146466 | 90194 | 61318 | 298613 | 13264 |
| R32 | 1 | HSA | 44942 | 20863 | 25592 | 91574 | n/a |
| R32 | 1 | HIP | 47099 | 25134 | 26830 | 99198 | n/a |
| R32 | 2 | KFD | 146312 | 90481 | 61367 | 298457 | 13275 |
| R32 | 2 | HSA | 44919 | 20789 | 25468 | 91325 | n/a |
| R32 | 2 | HIP | 47241 | 24961 | 26938 | 99285 | n/a |

### R32 KFD launch components

These are p50 nanoseconds from the R32 raw component sample vectors. They are
nested intervals and are not additive; compute remains the inclusive latency.

| Slot | Preparation | Bound snapshot | Authority | Native binding | Publication | Publish to completion | Completed readback | Signal recycle | Detach restore | Recycle inclusive |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 4384 | 238 | 2848 | 19582 | 16185 | 31209 | 0 | 9802 | 4446 | 14275 |
| 1 | 4343 | 252 | 2852 | 19622 | 16131 | 30906 | 0 | 9861 | 4409 | 14300 |
| 2 | 4444 | 244 | 2879 | 19627 | 16072 | 31035 | 0 | 9816 | 4389 | 14247 |

## Slotwise contrast

For phase `p`, reference `r`, and matching Latin slot `s`, the
reference-adjusted R32 latency reduction is:

```text
100 * (1 - (R32_KFD[p,s] / R31_KFD[p,s])
           / (R32_r[p,s] / R31_r[p,s]))
```

The unadjusted value omits the reference ratio. Positive means lower R32 KFD
latency. Each cell gives `slot 0 / slot 1 / slot 2; median`, in percent.

| Phase | Unadjusted KFD | Adjusted vs HSA | Adjusted vs HIP |
|---|---:|---:|---:|
| H2D | 9.845239 / 9.679767 / 9.697886; **9.697886** | 9.486016 / 9.591340 / 9.864744; **9.591340** | 10.262995 / 10.001935 / 9.671125; **10.001935** |
| Compute | -0.990231 / -0.479034 / -0.507642; **-0.507642** | -0.473966 / -0.257492 / -0.203059; **-0.257492** | -1.002271 / 0.032676 / -0.580121; **-0.580121** |
| D2H | 20.257089 / 20.690681 / 20.064868; **20.257089** | 21.986033 / 21.301182 / 20.755371; **21.301182** | 19.924196 / 20.693637 / 20.955081; **20.693637** |
| E2E | 9.532442 / 9.318307 / 9.219847; **9.318307** | 9.972910 / 9.487641 / 9.607521; **9.607521** | 9.536089 / 9.555072 / 9.490492; **9.536089** |
| Promotion | -0.901984 / 0.465256 / 0.420074; **0.420074** | n/a | n/a |

Thus the rounded median E2E reductions are **9.32% unadjusted**, **9.61%
HSA-adjusted**, and **9.54% HIP-adjusted**. The exact R32-minus-R31 KFD slot
deltas were `-15993/-15697/-15713` ns for H2D, `+890/+430/+457` ns for
compute, `-15680/-15997/-15404` ns for D2H,
`-31499/-30685/-30312` ns for E2E, and `+120/-62/-56` ns for promotion.
Directional latency fell consistently; compute moved slightly higher.

## R32 reference ratios

These are R32 KFD p50 divided by the matching R32 reference p50. Lower is
better; every value above one means KFD was slower in this harness.

| Slot | Phase | KFD/HSA | KFD/HIP |
|---:|---|---:|---:|
| 0 | H2D | 3.278289 | 3.112932 |
| 0 | Compute | 4.336534 | 3.607058 |
| 0 | D2H | 2.372852 | 2.300682 |
| 0 | E2E | 3.256119 | 3.013275 |
| 1 | H2D | 3.259000 | 3.109748 |
| 1 | Compute | 4.323156 | 3.588526 |
| 1 | D2H | 2.395983 | 2.285427 |
| 1 | E2E | 3.260893 | 3.010272 |
| 2 | H2D | 3.257241 | 3.097140 |
| 2 | Compute | 4.352350 | 3.624895 |
| 2 | D2H | 2.409573 | 2.278083 |
| 2 | E2E | 3.268076 | 3.006063 |

Promotion/H2D p50 shares were `0.091662`, `0.090560`, and `0.090731` for
slots 0 through 2. The KFD/HIP E2E ratios of `3.013275`, `3.010272`, and
`3.006063` are the basis for the approximately 3.01x residual gap.

## Validation and cleanup

An independent checker rerun reproduced the R32 retained report byte for byte,
SHA-256
`2d8a59c2ce741894de7f5658754466cc9afa05b29ae2a39adc841296b6bfc7b8`.
The same rerun reproduced the R31 report byte for byte, SHA-256
`c98126f430424f6ecec5e31900de52b392bf3351d4bb3a88cc30821700141c74`.
An independent calculation directly from the raw p50 fields reproduced every
slotwise and median reduction above. R32 slot-log SHA-256 values were, in
order,
`1a9b46fbf0ba80f2afbd865e5bc45bdc3cf44d904e3630ff18c9516c9b29f575`,
`8d6b559548e1b86ad8b503b5adc55b55980676f015ba7ffa7072019c08832cc4`,
and `3655c628f23f53195268299b181c57e86c25848c9304be89a3d9e4a4f2db4ec6`.
The archived R32 `run.log` has SHA-256
`92a4b53406183b1839be2735856a708c630611a174a1ceba10f3a64ca58b0836`.

All nine phase monitors reported `status=clean`, zero foreign selected-device
queues, and zero terminal selected-device queues. The maximum observed gap was
4,183 us, below the 10,000 us limit. GPU 2 busy and KFD GPU ID `29122` queue
counts were both zero at the post-run, pre-cleanup, and post-cleanup checks.
The remote archive SHA-256 was rechecked immediately before deletion and
matched the local archive. The exact remote root
`/tmp/fe2o3-r32-9f715189-r26-tZZ3EPQ9` was deleted only with
`/usr/bin/find "$root" -depth -delete`, and its absence was confirmed.

## Reproduction

From the repository root, while the external archives remain available:

```bash
r32_archive=/tmp/fe2o3-r32-9f715189-r26-20260905-evidence.tar.gz
r31_archive=/tmp/fe2o3-r31-2f95b461-r26-20260905-evidence.tar.gz
sha256sum "$r32_archive" "$r31_archive"

root="$(mktemp -d /tmp/fe2o3-r32-doc-check.XXXXXX)"
trap '/usr/bin/find "$root" -depth -delete' EXIT
mkdir "$root/r32" "$root/r31"
tar -xzf "$r32_archive" -C "$root/r32"
tar -xzf "$r31_archive" -C "$root/r31"

r32_set="$root/r32/output/r26-inplace-163fa3f48bf1247bb75b53cbe4f5caf18fc417ab54626af18eb94b28e699ef5a"
r31_set="$root/r31/output/r26-inplace-1746de309702f89da09699b79ffcaeb52c26098f66c46544937deab62f01b80b"

python3 benchmarks/runtime_gfx942/check-parity.py \
  --schema fe2o3.r26-inplace-benchmark.v4 --r26-counterbalance-set \
  "$r32_set/slot-0.log" "$r32_set/slot-1.log" "$r32_set/slot-2.log" \
  >"$root/r32-recomputed.txt"
cmp "$root/r32-recomputed.txt" "$r32_set/set-validation.txt"

python3 benchmarks/runtime_gfx942/check-parity.py \
  --schema fe2o3.r26-inplace-benchmark.v4 --r26-counterbalance-set \
  "$r31_set/slot-0.log" "$r31_set/slot-1.log" "$r31_set/slot-2.log" \
  >"$root/r31-recomputed.txt"
cmp "$root/r31-recomputed.txt" "$r31_set/set-validation.txt"

sha256sum "$root/r32-recomputed.txt" "$root/r31-recomputed.txt"
/usr/bin/find "$root" -depth -delete
trap - EXIT
```

## Claim limits

The paired comparison unit is one matching Latin slot, so every aggregate has
only `n=3` descriptive effects. R31 and R32 were separate runs rather than an
interleaved cross-revision experiment, and their raw samples are not ordinal
pairs. Reference adjustment cannot remove all run-to-run drift or turn the
controls into identical executions. In particular, the HIP sources match but
the HIP executable bytes do not; only the HSA control executable is
byte-identical across the two archives.

No confidence interval, significance test, long-run variance bound,
multi-host result, throughput result, energy result, or workload-general result
is claimed. This evidence demonstrates functional validation and a consistent
directional/E2E reduction for this measured path. It does not establish
HIP/HSA parity, general application speedup, or orders-of-magnitude advantage;
KFD remained about 2.28x-4.35x slower than the matched references across the
reported R32 phases.
