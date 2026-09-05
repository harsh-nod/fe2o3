# MI300X R31 single-packet directional comparison, 2026-09-05

Status: `Measured` for the exact bounded R26 V4 run below. The one-packet
specialization did **not** demonstrate a meaningful speedup in this run, and
KFD remained materially slower than both HSA and HIP. This is not a generic
parity or application-performance result.

## Provenance

- Baseline implementation: `dc1887e9999135e450e53e99a8d3d99bf933c689`.
- Measured R31 implementation: `2f95b4619a6ca95cd37159821429d9db196d5550`.
  Its parent is the R30 evidence-only commit
  `cc4fe70268ed3ae4723258c66fac28e07cb03e93`; the production change between
  the measured implementations is therefore the R31 one-packet
  specialization. The later proof-only commit
  `ce54ae7b06d51b5c2cd5844103def858ec93d6b7` was not in the measured tree.
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
  R31 executable SHA-256 values were KFD
  `257279747d9156250212f4c05e4c11a79f731551ffa037c374b569df5e23fe28`,
  HSA `9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83`,
  and HIP `b8d63c85e727e1cdd0e7f4cd72f868999cd87ebe6dd097fb9da79323178768dd`.

The R31 local external archive is
`/tmp/fe2o3-r31-2f95b461-r26-20260905-evidence.tar.gz`, SHA-256
`ff98158caaf0fdffe003eda78fb26e5892231ffd8b12d21300f21b77abaf0ae2`.
Its counterbalance set ID is
`1746de309702f89da09699b79ffcaeb52c26098f66c46544937deab62f01b80b`.
This `/tmp` archive is not a durable repository artifact.

The R30 archive is no longer present. Baseline values below are transcribed
from the committed [R30 evidence note](mi300x-r30-authenticated-h2d-2026-09-05.md)
at `cc4fe702`, whose blob is `a7805258883e388cc085d0e6313088d8253d8e83`
and whose rendered bytes have SHA-256
`7fb0b394ec4b288261ca9d616cb0b899cb043533e5b2f894ff063eb5ce889a95`.
The R30 raw samples could not be independently rechecked for this note.

## Method

Each set used the R26 V4 1 MiB in-place transform, 10 warmups, 30 samples per
backend and slot, and 10 iterations per sample. A sample is the integer average
of its 10 host-monotonic iteration durations; the reported statistic is the
untrimmed p50. The cyclic Latin orders were KFD/HSA/HIP, HSA/HIP/KFD, and
HIP/KFD/HSA. Every output element was validated in all 310 iterations per
backend and slot.

The 1 MiB H2D and D2H operations are below the gfx942 linear-packet maximum of
4,194,272 bytes. R31 routes each through the existing owned single-copy path
instead of materializing a one-element window request and ticket roster.
Larger directional copies retain the window path. The benchmark sources,
fixture, runner, and timing boundaries did not change between the two
implementation commits. Operational currentness checks and authenticated H2D
Ready promotion remain in the timed paths.

## P50 observations

All values are nanoseconds. Promotion is KFD-only and covers the full
authenticated post-H2D Ready transition, not an isolated lookup.

| Revision | Slot | Backend | H2D | Compute | D2H | E2E | Promotion |
|---|---:|---|---:|---:|---:|---:|---:|
| R30 | 0 | KFD | 160856 | 88721 | 77118 | 327397 | 13256 |
| R30 | 0 | HSA | 44832 | 20807 | 25468 | 91201 | n/a |
| R30 | 0 | HIP | 47091 | 24964 | 26678 | 98965 | n/a |
| R30 | 1 | KFD | 162297 | 90082 | 78048 | 331105 | 13455 |
| R30 | 1 | HSA | 44662 | 20808 | 25394 | 90996 | n/a |
| R30 | 1 | HIP | 47215 | 25104 | 26545 | 99072 | n/a |
| R30 | 2 | KFD | 161075 | 89116 | 76733 | 327499 | 13207 |
| R30 | 2 | HSA | 45060 | 20827 | 25334 | 91246 | n/a |
| R30 | 2 | HIP | 47100 | 24987 | 26819 | 99169 | n/a |
| R31 | 0 | KFD | 162444 | 89878 | 77405 | 330440 | 13304 |
| R31 | 0 | HSA | 44851 | 20824 | 25449 | 91362 | n/a |
| R31 | 0 | HIP | 46828 | 25167 | 26941 | 99204 | n/a |
| R31 | 1 | KFD | 162163 | 89764 | 77315 | 329298 | 13326 |
| R31 | 1 | HSA | 44986 | 20817 | 25395 | 91403 | n/a |
| R31 | 1 | HIP | 46931 | 25006 | 26829 | 98939 | n/a |
| R31 | 2 | KFD | 162025 | 90024 | 76771 | 328769 | 13331 |
| R31 | 2 | HSA | 44836 | 20726 | 25248 | 90935 | n/a |
| R31 | 2 | HIP | 47255 | 24979 | 26638 | 98989 | n/a |

### R31 KFD launch components

These are p50 nanoseconds from the R31 raw component sample vectors. They are
nested intervals and are not additive; compute remains the inclusive latency.

| Slot | Preparation | Bound snapshot | Authority | Native binding | Publication | Publish to completion | Completed readback | Signal recycle | Detach restore | Recycle inclusive |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 4252 | 207 | 2858 | 19297 | 16226 | 31284 | 0 | 9921 | 4297 | 14315 |
| 1 | 4246 | 198 | 2835 | 19443 | 16257 | 31210 | 0 | 9912 | 4268 | 14237 |
| 2 | 4315 | 240 | 2850 | 19523 | 16438 | 31073 | 0 | 9908 | 4181 | 14099 |

## Slotwise contrast

For phase `p`, reference `r`, and matching Latin slot `s`, the
reference-adjusted R31 latency reduction is:

```text
100 * (1 - (R31_KFD[p,s] / R30_KFD[p,s])
           / (R31_r[p,s] / R30_r[p,s]))
```

The unadjusted value omits the reference ratio. Positive means lower R31 KFD
latency. Each cell gives `slot 0 / slot 1 / slot 2; median`, in percent.

| Phase | Unadjusted KFD | Adjusted vs HSA | Adjusted vs HIP |
|---|---:|---:|---:|
| H2D | -0.987218 / 0.082565 / -0.589787; **-0.589787** | -0.944438 / 0.802194 / -1.092332; **-0.944438** | -1.554393 / -0.522079 / -0.259845; **-0.522079** |
| Compute | -1.304088 / 0.353012 / -1.018897; **-1.018897** | -1.221387 / 0.396093 / -1.511173; **-1.221387** | -0.486957 / -0.037511 / -1.051250; **-0.486957** |
| D2H | -0.372157 / 0.939166 / -0.049522; **-0.049522** | -0.447094 / 0.943066 / -0.390312; **-0.390312** | 0.607683 / 1.987780 / -0.729339; **0.607683** |
| E2E | -0.929453 / 0.545748 / -0.387787; **-0.387787** | -0.751593 / 0.988599 / -0.731116; **-0.731116** | -0.686296 / 0.412056 / -0.570331; **-0.570331** |
| Promotion | -0.362100 / 0.958751 / -0.938896; **-0.362100** | n/a | n/a |

The exact R31-minus-R30 KFD slot deltas were `+1588/-134/+950` ns for H2D,
`+1157/-318/+908` ns for compute, `+287/-733/+38` ns for D2H,
`+3043/-1807/+1270` ns for E2E, and `+48/-129/+124` ns for promotion.
Movements were mixed across slots. Even the positive 0.607683% median D2H
contrast against HIP is too small and inconsistent to establish a speedup with
three separately executed slot pairs.

## R31 reference ratios

These are R31 KFD p50 divided by the matching R31 reference p50. Lower is
better; every value above one means KFD was slower in this harness.

| Slot | Phase | KFD/HSA | KFD/HIP |
|---:|---|---:|---:|
| 0 | H2D | 3.621859 | 3.468950 |
| 0 | Compute | 4.316078 | 3.571264 |
| 0 | D2H | 3.041573 | 2.873130 |
| 0 | E2E | 3.616821 | 3.330914 |
| 1 | H2D | 3.604744 | 3.455349 |
| 1 | Compute | 4.312053 | 3.589698 |
| 1 | D2H | 3.044497 | 2.881770 |
| 1 | E2E | 3.602705 | 3.328293 |
| 2 | H2D | 3.613726 | 3.428738 |
| 2 | Compute | 4.343530 | 3.603987 |
| 2 | D2H | 3.040676 | 2.882011 |
| 2 | E2E | 3.615429 | 3.321268 |

Promotion/H2D p50 shares were `0.081899`, `0.082177`, and `0.082277` for
slots 0 through 2.

## Reproduction and validation

While the external archive remains available, its raw set can be checked with:

```bash
archive=/tmp/fe2o3-r31-2f95b461-r26-20260905-evidence.tar.gz
root="$(mktemp -d /tmp/fe2o3-r31-doc-check.XXXXXX)"
sha256sum "$archive"
tar -xzf "$archive" -C "$root"
set="$root/output/r26-inplace-1746de309702f89da09699b79ffcaeb52c26098f66c46544937deab62f01b80b"
python3 benchmarks/runtime_gfx942/check-parity.py \
  --schema fe2o3.r26-inplace-benchmark.v4 --r26-counterbalance-set \
  "$set/slot-0.log" "$set/slot-1.log" "$set/slot-2.log" \
  >"$root/recomputed.txt"
cmp "$root/recomputed.txt" "$set/set-validation.txt"
sha256sum "$root/recomputed.txt" "$set/set-validation.txt"
find "$root" -depth -delete
```

This independent rerun passed and reproduced the retained report byte for
byte, SHA-256
`c98126f430424f6ecec5e31900de52b392bf3351d4bb3a88cc30821700141c74`.
The checker recomputed every summary from the raw vectors. The slot log
SHA-256 values were, in order,
`6c4b13b45d88ff89aa1bff6f23606f0c1535730c99b939e1d84c14c5e65e91c6`,
`2d72c5b0bda1327499e09f1735a815896c3f4aba5123a4026cb5e9c69b29ec72`,
and `f5f675386d57a5b5c290bfbdb3a67a38de6d083098d1f8c2a882ebe701c1b632`.

All nine phase monitors reported `status=clean`, zero foreign selected-device
queues, zero terminal selected-device queues, and observation gaps no greater
than 5,773 us against the 10,000 us limit. The archived run ended with GPU 2
at 0% busy. As an external post-run observation, not archive-contained proof,
the exact remote root `/tmp/fe2o3-r31-2f95b461-r26-Igqd14TO` was deleted with
`/usr/bin/find "$root" -depth -delete`, its absence check passed, GPU 2 still
reported 0% busy, and KFD GPU ID `29122` had zero queues.

## Claim limits

The paired unit is one matching Latin slot, so every aggregate has only `n=3`
descriptive effects. The revisions were run separately rather than interleaved,
and raw samples are not ordinal pairs. The missing R30 archive prevents a new
raw-baseline audit and a new cross-revision executable-byte comparison; the
committed R30 evidence table is the baseline authority here. Reference
adjustment does not turn the HSA or HIP controls into bit-identical controls.

No confidence interval, significance test, long-run variance bound,
multi-host result, throughput result, energy result, or workload-general result
is claimed. This evidence demonstrates functional validation of the measured
path, but no meaningful performance gain from the one-packet specialization.
It does not support HIP/HSA parity, a generic application speedup, or an
orders-of-magnitude performance claim. KFD remained about 2.87x-4.34x slower
than the matched references across the reported phases.
