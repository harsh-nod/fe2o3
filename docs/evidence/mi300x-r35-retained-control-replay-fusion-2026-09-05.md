# MI300X R35 retained-control replay fusion, 2026-09-05

Status: `Measured` for the exact bounded R26 V4 runs below. Against an exact
corrected-runner baseline carrying the unchanged R34 runtime, R35 reduced the
directly targeted KFD native-binding p50 in all three matched slots by
**6.767%**, **7.028%**, and **10.191%**; the median slotwise reduction was
**7.028%**. Median slotwise KFD E2E latency changed by **0.683%** unadjusted,
**0.784%** after HSA ratio-of-ratios adjustment, and **0.816%** after HIP
adjustment. This is one clean `n=3` descriptive comparison. It is not a causal,
parity, orders-of-magnitude, or workload-general result.

## Provenance

- Exact baseline:
  [`82dffff4543d6d5ca052730fafa1e98b18e04ec1`](https://github.com/harsh-nod/fe2o3/commit/82dffff4543d6d5ca052730fafa1e98b18e04ec1).
  It includes the corrected runner and retains the R34 production runtime from
  `b015b81f862220d48671e1c4809b8ce858a317e7` unchanged.
- Exact R35 production implementation:
  [`4b324bbd53e4c6e767c5c5f2f18817c133edbe03`](https://github.com/harsh-nod/fe2o3/commit/4b324bbd53e4c6e767c5c5f2f18817c133edbe03)
  (`perf(kfd): fuse retained-control replay binding`). The complete baseline to
  R35 source delta is only `crates/fe2o3-kfd/src/queue_live.rs`.
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm `7.2.4`, amdgpu
  version `6.16.13`, build ID
  `4cd22e1f91450b8d9da1fc7bbbc02ee412e202d9`.
- Device: host GPU 2, AMD Instinct MI300X, `gfx942:xnack-`, PCI
  `0000:46:00.0`, unique ID `0xd2e26fef80cf5c33`, KFD GPU ID `29122`, NUMA
  node 0.
- Workload: one 1 MiB in-place `u32` transform, 10 warmups, 30 samples per
  backend per slot, and 10 iterations averaged into each sample. The cyclic
  Latin orders were KFD/HSA/HIP, HSA/HIP/KFD, and HIP/KFD/HSA.
- Corrected runner SHA-256:
  `8c8b59b77705072b83866076d38260f38951e1f66ef7dd6ffc384e5372c13c2f`;
  checker:
  `46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f`;
  host guard:
  `877c2b9199c5594a23c681dccdc1e58c2bef7228a87b16e22150baebc21af8b6`;
  system-identity collector:
  `6a80769bde37c41787a28c725d9c6eeb04ec0837a752924969bed952af8e036f`.
- Both archived builds record Rust
  `1.96.0-nightly (55e86c996 2026-04-02)` and Cargo
  `1.96.0-nightly (888f67534 2026-03-30)` from their private archived source
  trees. Both record HIP `7.2.53211-97f5574fe2` and GCC `13.3.0`.
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
`/tmp/fe2o3-r35-baseline-82dffff-r26-20260905-evidence.tar.gz`, SHA-256
`ba1ae56a1e347550299eb90842613c6a249646e55e9e272a137df1b20c28a056`,
size 102,063 bytes. Its retained extraction is
`/tmp/fe2o3-r35-baseline-evidence-extract.ZguRhBLa`; its counterbalance set ID
is `e19173aae3831d2fbe85203b70ec59e1ea4b3d002b81f4cbde7bbd9caa45a20f`.
Its evidence-manifest and set-validation SHA-256 values are respectively
`5123f6fb79bfbc28631881d7e827fb0a961ef3ef42b0671aacdacebee16ef3d0` and
`1611a9e3536732b00598de219cada57754cfa80f8e2df1b8719ede7e223d5802`.

The R35 external archive is
`/tmp/fe2o3-r35-4b324bbd-r26-20260905-evidence.tar.gz`, SHA-256
`7afb4a699712108dedd77c636d2cb62d1cbf6dc0de044e3cda9a6b275a685e65`,
size 102,580 bytes. Its retained extraction is
`/tmp/fe2o3-r35-current-evidence-extract.JRwiSKmh`; its counterbalance set ID
is `d9ee088e31391eaa2ec07257ec4366d803bd9f20ba4f120b1e2e4889ee9a872c`.
Its evidence-manifest and set-validation SHA-256 values are respectively
`ca04b11b82a9e74a67c098224325b4485a249022849805b45532f8157c752e90` and
`42be040dfd639b43f68183e1cc6334d675c87702aa4f2ddd62a3d499592963c5`.
These `/tmp` artifacts are external and non-durable.

Baseline executable SHA-256 values were KFD
`9cd0ba0245bf7e6beb8910fab952900152fbec42bbc7df4d9f50bd7c90b31b17`,
HSA `9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83`,
and HIP `2e9cf660d28d9df278e0be8404b55c5094490f108e6f6200ba11a2fdeb9771d2`.
R35 values were KFD
`d63715827781e082b42c11f3e3f3046a2f31a9ce33727ce814420538dd11ce03`,
the same byte-identical HSA executable, and HIP
`acea364a1e27c1784b1e8bedaf50ea8882b0fc34547d3eb86b5f0795378461a9`.
The HIP source and compiler identity match but the HIP executable bytes do not,
so HIP is a matched-source rather than byte-identical executable control.

Stable recorded system identities also match: boot ID
`317d0f9a-4f05-4ab0-8922-3ebfd7354c8b`, topology SHA-256
`43538be8d641b68ec9cfe545f0b64e42e0b1404de6678dce43752824a91c0c37`,
amdgpu module SHA-256
`e5a327a8f46459e07ee3f59cc991d16feee17103e199d39149823879b7fcff0b`,
decompressed module SHA-256
`61317154cee502ea97a74818879dff4b20abf8f074a2f4d19a94288e25d4ac3a`,
HSA library SHA-256
`b8cdfe93d343649a35c1daf73a0a3a6840f09379ebeee9be65670461ffea43f4`,
and HIP library SHA-256
`f1043337461c8e54ee135e95fa979a7d0e4344676ad5b0554652f844f8f098ac`.

## Measured path

The exact R35 sources establish the path measured by R26:

1. The [R26 compute interval](https://github.com/harsh-nod/fe2o3/blob/4b324bbd53e4c6e767c5c5f2f18817c133edbe03/crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs#L405-L452)
   calls the public typed launch, flushes the stream, waits separately, and
   stops its host-monotonic timer after completion.
2. The benchmark [requires every measured KFD iteration to report persistent
   HBM and retained-control reuse](https://github.com/harsh-nod/fe2o3/blob/4b324bbd53e4c6e767c5c5f2f18817c133edbe03/crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs#L455-L473).
   Its [constants select 10 warmups, 30 samples, and 10 iterations per
   sample](https://github.com/harsh-nod/fe2o3/blob/4b324bbd53e4c6e767c5c5f2f18817c133edbe03/crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs#L32-L37),
   and its [execution loop runs every warmup before the nested sample
   iterations](https://github.com/harsh-nod/fe2o3/blob/4b324bbd53e4c6e767c5c5f2f18817c133edbe03/crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs#L647-L687).
   Thus the first-control construction is outside the 300 measured iterations,
   and each measured launch confirms the retained replay path.
3. The backend's [`native_binding` interval](https://github.com/harsh-nod/fe2o3/blob/4b324bbd53e4c6e767c5c5f2f18817c133edbe03/crates/fe2o3-runtime/src/kfd_backend.rs#L6435-L6470)
   directly encloses `bind_directional_persistent_fixed_dispatch_v1` and ends
   before the separately measured publication interval.
4. Once use reservation and preparation succeed, the queue's
   [retained-dispatch branch](https://github.com/harsh-nod/fe2o3/blob/4b324bbd53e4c6e767c5c5f2f18817c133edbe03/crates/fe2o3-kfd/src/queue_live.rs#L10436-L10477)
   sends exact input, prepared-use, dispatch-control, storage, effect, and
   predecessor-generation custody to the R35 helper.
5. The [R35 fused helper](https://github.com/harsh-nod/fe2o3/blob/4b324bbd53e4c6e767c5c5f2f18817c133edbe03/crates/fe2o3-kfd/src/queue_live.rs#L9991-L10243)
   executes mapped-fact validation, detach, authenticated data construction,
   replay retention, and the final complete-authority audit inside one
   `with_live_queue_memory_model` call. It resolves loan/open/retake and
   stage-specific failure custody before committing the prepared attachment.
6. The exact baseline-to-R35 diff contains no H2D or D2H implementation change.
   Those phases remain useful within-run controls but are not direct R35 work.

The hardware run exercises the successful retained-control branch. It does not
inject native failures into the helper's failure-custody branches and is not a
Rust-to-formal-model refinement test.

## P50 observations

All values are untrimmed p50 nanoseconds from raw slot logs. Each sample is the
integer average of ten host-monotonic iteration measurements. Every output
element was validated in all 310 iterations per backend and slot.

| Revision | Slot | Backend | H2D | Compute | D2H | E2E | Promotion |
|---|---:|---|---:|---:|---:|---:|---:|
| Baseline | 0 | KFD | 145689 | 90425 | 49720 | 285976 | 13169 |
| Baseline | 0 | HSA | 44887 | 20779 | 25584 | 91473 | n/a |
| Baseline | 0 | HIP | 46951 | 25069 | 26763 | 99077 | n/a |
| Baseline | 1 | KFD | 141552 | 89871 | 49847 | 282030 | 13273 |
| Baseline | 1 | HSA | 45620 | 20790 | 25799 | 92294 | n/a |
| Baseline | 1 | HIP | 47325 | 25027 | 26763 | 99321 | n/a |
| Baseline | 2 | KFD | 144955 | 99318 | 49807 | 293702 | 13365 |
| Baseline | 2 | HSA | 44929 | 20824 | 25458 | 91383 | n/a |
| Baseline | 2 | HIP | 47108 | 24923 | 26433 | 98716 | n/a |
| R35 | 0 | KFD | 145289 | 88274 | 49645 | 284023 | 13263 |
| R35 | 0 | HSA | 44828 | 20708 | 25883 | 91566 | n/a |
| R35 | 0 | HIP | 47185 | 25086 | 26799 | 99210 | n/a |
| R35 | 1 | KFD | 142664 | 89126 | 50201 | 282379 | 13394 |
| R35 | 1 | HSA | 45278 | 20675 | 25361 | 91412 | n/a |
| R35 | 1 | HIP | 47264 | 25128 | 26783 | 99266 | n/a |
| R35 | 2 | KFD | 145691 | 88232 | 49382 | 283526 | 13388 |
| R35 | 2 | HSA | 44772 | 20810 | 25473 | 91277 | n/a |
| R35 | 2 | HIP | 46891 | 25064 | 26503 | 98599 | n/a |

The medians across the three KFD slot p50 values changed from
`144955/90425/49807/285976` ns to `145289/88274/49645/283526` ns for
H2D/compute/D2H/E2E. The percentages below are medians of the three slotwise
effects, not percentages calculated from these cross-slot medians.

### KFD launch components

These are nested compute-launch/completion intervals. Preparation contains
bound snapshot and authority; native binding is separate from publication;
recycle inclusive contains signal recycle and detach restore. They are not
additive.

| Revision | Slot | Promotion | Preparation | Bound snapshot | Authority | Native binding | Publication | Publish to completion | Completed readback | Signal recycle | Detach restore | Recycle inclusive |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Baseline | 0 | 13169 | 4162 | 191 | 2855 | 19167 | 16350 | 31423 | 0 | 9954 | 4351 | 14309 |
| Baseline | 1 | 13273 | 4207 | 209 | 2854 | 19323 | 16187 | 31432 | 0 | 9902 | 4259 | 14181 |
| Baseline | 2 | 13365 | 4213 | 212 | 2841 | 19693 | 16363 | 40014 | 0 | 9931 | 4342 | 14295 |
| R35 | 0 | 13263 | 4313 | 280 | 2859 | 17870 | 16367 | 30822 | 0 | 9807 | 4431 | 14248 |
| R35 | 1 | 13394 | 4441 | 197 | 2879 | 17965 | 16584 | 30913 | 0 | 9816 | 4498 | 14322 |
| R35 | 2 | 13388 | 4374 | 232 | 2876 | 17686 | 16417 | 30962 | 0 | 9861 | 4348 | 14242 |

Positive means lower R35 latency. Each cell gives
`slot 0 / slot 1 / slot 2; median`, in percent.

| Component | Slotwise R35 reduction |
|---|---:|
| Promotion | -0.713798 / -0.911625 / -0.172091; **-0.713798** |
| Preparation | -3.628063 / -5.562158 / -3.821505; **-3.821505** |
| Bound snapshot | -46.596859 / 5.741627 / -9.433962; **-9.433962** |
| Authority | -0.140105 / -0.875964 / -1.231961; **-0.875964** |
| Native binding | 6.766839 / 7.027894 / 10.191439; **7.027894** |
| Publication | -0.103976 / -2.452585 / -0.330013; **-0.330013** |
| Publish to completion | 1.912612 / 1.651184 / 22.622082; **1.912612** |
| Completed readback | 0 / 0 / 0; **0** |
| Signal recycle | 1.476793 / 0.868511 / 0.704864; **0.868511** |
| Detach restore | -1.838658 / -5.611646 / -0.138185; **-1.838658** |
| Recycle inclusive | 0.426305 / -0.994288 / 0.370759; **0.370759** |

The directly enclosed native-binding interval improved in all slots. The
baseline slot-2 compute and publish-to-completion values were elevated relative
to its other slots despite a clean interference record, so the large slot-2
compute effect is not sufficient evidence of a corresponding R35 causal gain.

## Slotwise contrast

For phase `p`, matching Latin slot `s`, and reference `r`, the adjusted R35
latency reduction is:

```text
100 * (1 - (R35_KFD[p,s] / R35_r[p,s])
           / (baseline_KFD[p,s] / baseline_r[p,s]))
```

The raw value omits the reference ratio. Positive means lower R35 KFD latency.
Each cell gives `slot 0 / slot 1 / slot 2; median`, in percent.

| Phase | Raw KFD | Adjusted vs HSA | Adjusted vs HIP |
|---|---:|---:|---:|
| H2D | 0.274557 / -0.785577 / -0.507744; **-0.507744** | 0.143305 / -1.546844 / -0.860190; **-0.860190** | 0.769116 / -0.915653 / -0.972869; **-0.915653** |
| Compute | 2.378767 / 0.828966 / 11.162126; **2.378767** | 2.044060 / 0.277350 / 11.102360; **2.044060** | 2.444922 / 1.227576 / 11.661892; **2.444922** |
| D2H | 0.150845 / -0.710173 / 0.853294; **0.150845** | 1.304301 / -2.449499 / 0.911677; **0.911677** | 0.284975 / -0.634969 / 1.115161; **0.284975** |
| E2E | 0.682924 / -0.123746 / 3.464736; **0.682924** | 0.783797 / -1.089802 / 3.352630; **0.783797** | 0.816068 / -0.179221 / 3.350185; **0.816068** |

Exact R35-minus-baseline KFD slot deltas were `-400/+1112/+736` ns for H2D,
`-2151/-745/-11086` ns for compute, `-75/+354/-425` ns for D2H, and
`-1953/+349/-10176` ns for E2E. E2E improved in two slots and regressed in one;
H2D and D2H movement is mixed and those paths were unchanged by R35.

## Remaining reference ratios

These are R35 KFD p50 divided by its same-slot reference p50. Every value above
one means KFD remained slower in this harness.

| Slot | Phase | KFD/HSA | KFD/HIP |
|---:|---|---:|---:|
| 0 | H2D | 3.241032 | 3.079135 |
| 0 | Compute | 4.262797 | 3.518855 |
| 0 | D2H | 1.918054 | 1.852494 |
| 0 | E2E | 3.101839 | 2.862846 |
| 1 | H2D | 3.150846 | 3.018450 |
| 1 | Compute | 4.310810 | 3.546880 |
| 1 | D2H | 1.979457 | 1.874361 |
| 1 | E2E | 3.089080 | 2.844670 |
| 2 | H2D | 3.254065 | 3.107014 |
| 2 | Compute | 4.239885 | 3.520268 |
| 2 | D2H | 1.938602 | 1.863261 |
| 2 | E2E | 3.106215 | 2.875546 |

R35 therefore remained 1.85x-1.87x slower than HIP D2H, 2.84x-2.88x slower
than HIP E2E, 3.02x-3.11x slower than HIP H2D, and 3.52x-3.55x slower than HIP
compute. The HSA gaps were also material. No HIP/HSA parity is claimed.

## Validation and cleanup

Independent checker reruns reproduced both retained set reports byte for byte.
The archived baseline and R35 `run.log` SHA-256 values are respectively
`b0958fb4ac2bf34a59eb0c49cbdb564bfa314dc7a76241e04cd8e1d51fa27845` and
`6db55be7bf43b9cb795f9268f72636a8c1d191874fe6a9a888b3255148a2d86b`.
Baseline slot-log SHA-256 values, in order, are
`6e673ea6e2e39c240ee4040cfa3949505a8667fd436ada9d62e98eb783ee71e7`,
`fd27e9f2acddbf9ed96dbd869ee0046230df06221cebe7b30b285e41222406c1`,
and `75287dc264dd1cd2031350d3f7ddcb5185264aa0d03573e6fa6a3536fcb39633`.
R35 slot-log SHA-256 values are
`2bb0874d96d2e70d1a4a4058694a2a6a26cc1ba42c2ac9a1765f04059c5029f7`,
`9f0cff1695ed66780b830cba94098e77f937e24bf8409f832164bcaa0f827c5c`,
and `8476822310589b24bcb804f8c8551cfab8c11e63788988e8a8ee56ac8378be04`.
Baseline preflight/postrun SHA-256 values are
`7df9d6d36a6941c4b0ac90600d6dbf81225c1a542ee17d2f10a82e3ca92ffea7` and
`908aa8a6bd8f66d6241b63d6f9d158ff937ce3005e96825cb73ce6adbce7f34b`;
R35 values are
`ded69a2d1a61b1804ba81ac0a4899ed77a822c1f7e92010b487ca067f34aa08e` and
`cec2a809dc4b3d7750982dd70fe66aaa4c03cfe8840e10e5c7bb24e6c99c56b2`.

All 18 phase monitors reported `status=clean`, zero foreign selected-device
queues, zero terminal selected-device queues, target exit zero, target reaped,
and process group absent. Maximum observed gaps were 4,752 us for the baseline
and 3,965 us for R35, below the 10,000 us limit. Every preflight and postrun
record reported GPU 2 busy zero and KFD GPU ID `29122` queue count zero. Both
checkouts were clean at their exact commits.

The operator recorded the baseline root as
`/tmp/fe2o3-r35-baseline-r26.yH3BwsMe` and the R35 root as
`/tmp/fe2o3-r35-current-r26.3Lyo2huZ`. The operator rechecked each remote
archive SHA-256 immediately before cleanup and observed that it matched its
retained local archive. The operator then deleted only those exact roots and
archives, using `find -depth -delete` for the roots, and reconfirmed their
absence, GPU busy zero, and selected queue count zero without modifying foreign
processes or state. These post-archive deletion, absence, and final-idle
observations are operator-reported; the retained archives do not authenticate
them.

## Reproduction

From the repository root, while both external archives remain available:

```bash
set -euo pipefail
baseline_archive=/tmp/fe2o3-r35-baseline-82dffff-r26-20260905-evidence.tar.gz
r35_archive=/tmp/fe2o3-r35-4b324bbd-r26-20260905-evidence.tar.gz
printf '%s  %s\n' \
  ba1ae56a1e347550299eb90842613c6a249646e55e9e272a137df1b20c28a056 \
  "$baseline_archive" \
  7afb4a699712108dedd77c636d2cb62d1cbf6dc0de044e3cda9a6b275a685e65 \
  "$r35_archive" | sha256sum --check --status
test "$(stat -c %s "$baseline_archive")" = 102063
test "$(stat -c %s "$r35_archive")" = 102580
gzip -t "$baseline_archive" "$r35_archive"

root="$(mktemp -d /tmp/fe2o3-r35-doc-check.XXXXXX)"
trap '/usr/bin/find "$root" -depth -delete' EXIT
mkdir "$root/baseline" "$root/r35"
tar -xzf "$baseline_archive" -C "$root/baseline"
tar -xzf "$r35_archive" -C "$root/r35"
baseline_set="$root/baseline/output/r26-inplace-e19173aae3831d2fbe85203b70ec59e1ea4b3d002b81f4cbde7bbd9caa45a20f"
r35_set="$root/r35/output/r26-inplace-d9ee088e31391eaa2ec07257ec4366d803bd9f20ba4f120b1e2e4889ee9a872c"
checker="$root/check-parity.py"
git show \
  4b324bbd53e4c6e767c5c5f2f18817c133edbe03:benchmarks/runtime_gfx942/check-parity.py \
  >"$checker"
printf '%s  %s\n' \
  46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f \
  "$checker" | sha256sum --check --status

python3 "$checker" \
  --schema fe2o3.r26-inplace-benchmark.v4 --r26-counterbalance-set \
  "$baseline_set/slot-0.log" "$baseline_set/slot-1.log" \
  "$baseline_set/slot-2.log" >"$root/baseline-recomputed.txt"
cmp "$root/baseline-recomputed.txt" "$baseline_set/set-validation.txt"
python3 "$checker" \
  --schema fe2o3.r26-inplace-benchmark.v4 --r26-counterbalance-set \
  "$r35_set/slot-0.log" "$r35_set/slot-1.log" \
  "$r35_set/slot-2.log" >"$root/r35-recomputed.txt"
cmp "$root/r35-recomputed.txt" "$r35_set/set-validation.txt"

printf '%s  %s\n' \
  1611a9e3536732b00598de219cada57754cfa80f8e2df1b8719ede7e223d5802 \
  "$baseline_set/set-validation.txt" \
  b0958fb4ac2bf34a59eb0c49cbdb564bfa314dc7a76241e04cd8e1d51fa27845 \
  "$root/baseline/run.log" \
  7df9d6d36a6941c4b0ac90600d6dbf81225c1a542ee17d2f10a82e3ca92ffea7 \
  "$root/baseline/preflight.txt" \
  908aa8a6bd8f66d6241b63d6f9d158ff937ce3005e96825cb73ce6adbce7f34b \
  "$root/baseline/postrun.txt" \
  6e673ea6e2e39c240ee4040cfa3949505a8667fd436ada9d62e98eb783ee71e7 \
  "$baseline_set/slot-0.log" \
  fd27e9f2acddbf9ed96dbd869ee0046230df06221cebe7b30b285e41222406c1 \
  "$baseline_set/slot-1.log" \
  75287dc264dd1cd2031350d3f7ddcb5185264aa0d03573e6fa6a3536fcb39633 \
  "$baseline_set/slot-2.log" \
  42be040dfd639b43f68183e1cc6334d675c87702aa4f2ddd62a3d499592963c5 \
  "$r35_set/set-validation.txt" \
  6db55be7bf43b9cb795f9268f72636a8c1d191874fe6a9a888b3255148a2d86b \
  "$root/r35/run.log" \
  ded69a2d1a61b1804ba81ac0a4899ed77a822c1f7e92010b487ca067f34aa08e \
  "$root/r35/preflight.txt" \
  cec2a809dc4b3d7750982dd70fe66aaa4c03cfe8840e10e5c7bb24e6c99c56b2 \
  "$root/r35/postrun.txt" \
  2bb0874d96d2e70d1a4a4058694a2a6a26cc1ba42c2ac9a1765f04059c5029f7 \
  "$r35_set/slot-0.log" \
  9f0cff1695ed66780b830cba94098e77f937e24bf8409f832164bcaa0f827c5c \
  "$r35_set/slot-1.log" \
  8476822310589b24bcb804f8c8551cfab8c11e63788988e8a8ee56ac8378be04 \
  "$r35_set/slot-2.log" | sha256sum --check --status
grep -Fq \
  'manifest_sha256=5123f6fb79bfbc28631881d7e827fb0a961ef3ef42b0671aacdacebee16ef3d0' \
  "$baseline_set/set-validation.txt"
grep -Fq \
  'manifest_sha256=ca04b11b82a9e74a67c098224325b4485a249022849805b45532f8157c752e90' \
  "$r35_set/set-validation.txt"

test "$(git diff --name-only \
  82dffff4543d6d5ca052730fafa1e98b18e04ec1 \
  4b324bbd53e4c6e767c5c5f2f18817c133edbe03)" = \
  crates/fe2o3-kfd/src/queue_live.rs
git diff --quiet \
  b015b81f862220d48671e1c4809b8ce858a317e7 \
  82dffff4543d6d5ca052730fafa1e98b18e04ec1 -- \
  crates/fe2o3-kfd crates/fe2o3-runtime/src
git show 4b324bbd53e4c6e767c5c5f2f18817c133edbe03:crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs |
  sed -n '405,452p' | grep -F '.launch(' >/dev/null
git show 4b324bbd53e4c6e767c5c5f2f18817c133edbe03:crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs |
  sed -n '455,473p' | grep -F 'persistent_control_reused' >/dev/null
git show 4b324bbd53e4c6e767c5c5f2f18817c133edbe03:crates/fe2o3-runtime/examples/gfx942-runtime-r26-inplace-benchmark.rs |
  sed -n '647,725p' | grep -F 'ITERATIONS_PER_SAMPLE' >/dev/null
git show 4b324bbd53e4c6e767c5c5f2f18817c133edbe03:crates/fe2o3-runtime/src/kfd_backend.rs |
  sed -n '6435,6470p' | grep -F 'native_binding_started' >/dev/null
git show 4b324bbd53e4c6e767c5c5f2f18817c133edbe03:crates/fe2o3-kfd/src/queue_live.rs |
  sed -n '9991,10243p' | grep -F 'with_live_queue_memory_model' >/dev/null
git show 4b324bbd53e4c6e767c5c5f2f18817c133edbe03:crates/fe2o3-kfd/src/queue_live.rs |
  sed -n '10436,10477p' | grep -F \
    'bind_retained_persistent_fixed_dispatch_control_replay_v1' >/dev/null

python3 - "$baseline_set" "$r35_set" "$root" <<'PY'
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

def context(path):
    line = next(
        line for line in (path / "slot-0.log").read_text().splitlines()
        if line.startswith("context schema=fe2o3.r26-inplace-benchmark.v4")
    )
    return fields(line)

def system_identity(path):
    line = next(
        line for line in (path / "slot-0.log").read_text().splitlines()
        if line.startswith("context schema=fe2o3.r26-system-identity.v1")
    )
    return fields(line)

baseline_path, r35_path, extraction = map(Path, sys.argv[1:])
observed = {
    "baseline": read_set(baseline_path),
    "r35": read_set(r35_path),
}
expected = {
    "baseline": [
        {"kfd": [145689, 90425, 49720, 285976], "hsa": [44887, 20779, 25584, 91473], "hip": [46951, 25069, 26763, 99077]},
        {"kfd": [141552, 89871, 49847, 282030], "hsa": [45620, 20790, 25799, 92294], "hip": [47325, 25027, 26763, 99321]},
        {"kfd": [144955, 99318, 49807, 293702], "hsa": [44929, 20824, 25458, 91383], "hip": [47108, 24923, 26433, 98716]},
    ],
    "r35": [
        {"kfd": [145289, 88274, 49645, 284023], "hsa": [44828, 20708, 25883, 91566], "hip": [47185, 25086, 26799, 99210]},
        {"kfd": [142664, 89126, 50201, 282379], "hsa": [45278, 20675, 25361, 91412], "hip": [47264, 25128, 26783, 99266]},
        {"kfd": [145691, 88232, 49382, 283526], "hsa": [44772, 20810, 25473, 91277], "hip": [46891, 25064, 26503, 98599]},
    ],
}
baseline_components = [
    [13169, 4162, 191, 2855, 19167, 16350, 31423, 0, 9954, 4351, 14309],
    [13273, 4207, 209, 2854, 19323, 16187, 31432, 0, 9902, 4259, 14181],
    [13365, 4213, 212, 2841, 19693, 16363, 40014, 0, 9931, 4342, 14295],
]
r35_components = [
    [13263, 4313, 280, 2859, 17870, 16367, 30822, 0, 9807, 4431, 14248],
    [13394, 4441, 197, 2879, 17965, 16584, 30913, 0, 9816, 4498, 14322],
    [13388, 4374, 232, 2876, 17686, 16417, 30962, 0, 9861, 4348, 14242],
]
for name in expected:
    for slot in range(3):
        for backend in ("kfd", "hsa", "hip"):
            count = 4 if backend != "kfd" else 4 + len(components)
            assert observed[name][slot][backend][:4] == expected[name][slot][backend]
            assert len(observed[name][slot][backend]) == count
for slot in range(3):
    assert observed["baseline"][slot]["kfd"][4:] == baseline_components[slot]
    assert observed["r35"][slot]["kfd"][4:] == r35_components[slot]

common_context = {
    "runner_sha256": "8c8b59b77705072b83866076d38260f38951e1f66ef7dd6ffc384e5372c13c2f",
    "checker_sha256": "46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f",
    "host_guard_sha256": "877c2b9199c5594a23c681dccdc1e58c2bef7228a87b16e22150baebc21af8b6",
    "system_identity_collector_sha256": "6a80769bde37c41787a28c725d9c6eeb04ec0837a752924969bed952af8e036f",
    "rustc": "rustc_1.96.0-nightly_(55e86c996_2026-04-02)_",
    "cargo": "cargo_1.96.0-nightly_(888f67534_2026-03-30)_",
    "hsaco_sha256": "8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9",
    "kernel_source_sha256": "1185d4cd931c1bb43d113e66714af3d98bd96f7d036f5c610a909abf34ba87d5",
    "kernel_policy_sha256": "c060c3c4a96012fc6661b0585f4ff8ffe7b7f8483eb40262e4a018133c0ea585",
    "fixture_recipe_sha256": "29c6db8ea2a86392eb980b78e42fa1c049a6f92ca8dd3dc8224f90cf66254ab5",
    "hsa_source_sha256": "a1470c846474dcb10354202a5abd028a7ef9f13e9f36271eedec557953ff523e",
    "hip_source_sha256": "da7839dbbf12b18421e01c32e35d3b33935846deeef6d0210dfa725179bed542",
    "topology_sha256": "43538be8d641b68ec9cfe545f0b64e42e0b1404de6678dce43752824a91c0c37",
}
revision_context = {
    "baseline": {
        "git_commit": "82dffff4543d6d5ca052730fafa1e98b18e04ec1",
        "kfd_binary_sha256": "9cd0ba0245bf7e6beb8910fab952900152fbec42bbc7df4d9f50bd7c90b31b17",
        "hsa_binary_sha256": "9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83",
        "hip_binary_sha256": "2e9cf660d28d9df278e0be8404b55c5094490f108e6f6200ba11a2fdeb9771d2",
    },
    "r35": {
        "git_commit": "4b324bbd53e4c6e767c5c5f2f18817c133edbe03",
        "kfd_binary_sha256": "d63715827781e082b42c11f3e3f3046a2f31a9ce33727ce814420538dd11ce03",
        "hsa_binary_sha256": "9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83",
        "hip_binary_sha256": "acea364a1e27c1784b1e8bedaf50ea8882b0fc34547d3eb86b5f0795378461a9",
    },
}
contexts = {"baseline": context(baseline_path), "r35": context(r35_path)}
for name, row in contexts.items():
    for key, value in common_context.items():
        assert row[key] == value
    for key, value in revision_context[name].items():
        assert row[key] == value
assert contexts["baseline"]["hsa_binary_sha256"] == contexts["r35"]["hsa_binary_sha256"]
assert contexts["baseline"]["hip_binary_sha256"] != contexts["r35"]["hip_binary_sha256"]

stable_system = (
    "boot_id", "kernel_release", "amdgpu_version", "amdgpu_build_id",
    "amdgpu_module_sha256", "amdgpu_module_decompressed_sha256", "pci_bdf",
    "unique_id", "gpu_guid", "hsa_library_sha256", "hsa_library_build_id",
    "hip_library_sha256", "hip_library_build_id", "rocm_smi_library_sha256",
)
expected_system = {
    "boot_id": "317d0f9a-4f05-4ab0-8922-3ebfd7354c8b",
    "kernel_release": "6.8.0-124-generic",
    "amdgpu_version": "6.16.13",
    "amdgpu_build_id": "4cd22e1f91450b8d9da1fc7bbbc02ee412e202d9",
    "amdgpu_module_sha256": "e5a327a8f46459e07ee3f59cc991d16feee17103e199d39149823879b7fcff0b",
    "amdgpu_module_decompressed_sha256": "61317154cee502ea97a74818879dff4b20abf8f074a2f4d19a94288e25d4ac3a",
    "pci_bdf": "0000:46:00.0",
    "unique_id": "0xd2e26fef80cf5c33",
    "gpu_guid": "29122",
    "hsa_library_sha256": "b8cdfe93d343649a35c1daf73a0a3a6840f09379ebeee9be65670461ffea43f4",
    "hsa_library_build_id": "cbe2c420f8c65e4710580d19cfd7950db722ea9f",
    "hip_library_sha256": "f1043337461c8e54ee135e95fa979a7d0e4344676ad5b0554652f844f8f098ac",
    "hip_library_build_id": "db1aaf11568a2d99249b8c24ff700694ff6857dd",
    "rocm_smi_library_sha256": "cca245677e869de87b11b3f4c0358be63c60190f26a0821ed06f6801764125a5",
}
baseline_system = system_identity(baseline_path)
r35_system = system_identity(r35_path)
for key in stable_system:
    assert baseline_system[key] == expected_system[key]
    assert baseline_system[key] == r35_system[key]

def reductions(reference=None):
    answer = {}
    for phase_index, phase in enumerate(phases):
        values = []
        for slot in range(3):
            before = observed["baseline"][slot]["kfd"][phase_index]
            after = observed["r35"][slot]["kfd"][phase_index]
            ratio = after / before
            if reference is not None:
                before_ref = observed["baseline"][slot][reference][phase_index]
                after_ref = observed["r35"][slot][reference][phase_index]
                ratio = (after / after_ref) / (before / before_ref)
            values.append(100 * (1 - ratio))
        answer[phase] = tuple(f"{value:.6f}" for value in values + [median(values)])
    return answer

assert reductions() == {
    "h2d": ("0.274557", "-0.785577", "-0.507744", "-0.507744"),
    "compute": ("2.378767", "0.828966", "11.162126", "2.378767"),
    "d2h": ("0.150845", "-0.710173", "0.853294", "0.150845"),
    "e2e": ("0.682924", "-0.123746", "3.464736", "0.682924"),
}
assert reductions("hsa") == {
    "h2d": ("0.143305", "-1.546844", "-0.860190", "-0.860190"),
    "compute": ("2.044060", "0.277350", "11.102360", "2.044060"),
    "d2h": ("1.304301", "-2.449499", "0.911677", "0.911677"),
    "e2e": ("0.783797", "-1.089802", "3.352630", "0.783797"),
}
assert reductions("hip") == {
    "h2d": ("0.769116", "-0.915653", "-0.972869", "-0.915653"),
    "compute": ("2.444922", "1.227576", "11.661892", "2.444922"),
    "d2h": ("0.284975", "-0.634969", "1.115161", "0.284975"),
    "e2e": ("0.816068", "-0.179221", "3.350185", "0.816068"),
}

component_effects = []
for index in range(len(components)):
    values = []
    for slot in range(3):
        before = baseline_components[slot][index]
        after = r35_components[slot][index]
        values.append(0.0 if before == after == 0 else 100 * (1 - after / before))
    component_effects.append(tuple(f"{value:.6f}" for value in values + [median(values)]))
assert component_effects == [
    ("-0.713798", "-0.911625", "-0.172091", "-0.713798"),
    ("-3.628063", "-5.562158", "-3.821505", "-3.821505"),
    ("-46.596859", "5.741627", "-9.433962", "-9.433962"),
    ("-0.140105", "-0.875964", "-1.231961", "-0.875964"),
    ("6.766839", "7.027894", "10.191439", "7.027894"),
    ("-0.103976", "-2.452585", "-0.330013", "-0.330013"),
    ("1.912612", "1.651184", "22.622082", "1.912612"),
    ("0.000000", "0.000000", "0.000000", "0.000000"),
    ("1.476793", "0.868511", "0.704864", "0.868511"),
    ("-1.838658", "-5.611646", "-0.138185", "-1.838658"),
    ("0.426305", "-0.994288", "0.370759", "0.370759"),
]

expected_residual = [
    ("3.241032", "3.079135", "4.262797", "3.518855", "1.918054", "1.852494", "3.101839", "2.862846"),
    ("3.150846", "3.018450", "4.310810", "3.546880", "1.979457", "1.874361", "3.089080", "2.844670"),
    ("3.254065", "3.107014", "4.239885", "3.520268", "1.938602", "1.863261", "3.106215", "2.875546"),
]
for slot in range(3):
    values = []
    for phase_index in range(4):
        kfd = observed["r35"][slot]["kfd"][phase_index]
        values += [
            f'{kfd / observed["r35"][slot]["hsa"][phase_index]:.6f}',
            f'{kfd / observed["r35"][slot]["hip"][phase_index]:.6f}',
        ]
    assert tuple(values) == expected_residual[slot]

for name, path, expected_max, expected_commit in (
    ("baseline", baseline_path, 4752, revision_context["baseline"]["git_commit"]),
    ("r35", r35_path, 3965, revision_context["r35"]["git_commit"]),
):
    monitors = []
    for slot in range(3):
        for line in (path / f"slot-{slot}.log").read_text().splitlines():
            if line.startswith("monitor "):
                monitors.append(fields(line))
    assert len(monitors) == 9
    assert max(int(row["observed_maximum_gap_us"]) for row in monitors) == expected_max
    for row in monitors:
        assert row["status"] == "clean"
        assert row["foreign_selected_queues"] == "0"
        assert row["terminal_selected_queues"] == "0"
        assert row["target_exit_code"] == "0"
        assert row["target_reaped"] == "1"
        assert row["process_group_absent"] == "1"
    preflight = fields((extraction / name / "preflight.txt").read_text().replace("\n", " "))
    postrun = fields((extraction / name / "postrun.txt").read_text().replace("\n", " "))
    assert preflight["commit"] == expected_commit
    assert postrun["repo_commit"] == expected_commit
    assert preflight["repo_status_count"] == postrun["repo_status_count"] == "0"
    assert preflight["gpu2_busy_percent"] == postrun["gpu2_busy_percent"] == "0"
    assert preflight["selected_gpuid_29122_queues"] == "0"
    assert postrun["selected_gpuid_29122_queues"] == "0"

print("R35 evidence reproduction: pass")
PY
```

## Claim limits

The two revisions ran sequentially, baseline first, rather than in randomized
revision order. Each revision has only three Latin slots on one device and one
fixed one-MiB workload. Timings are host-monotonic and do not identify GPU
dispatch start/end. HIP executable bytes differ despite matched source and
compiler identity. The elevated clean-monitor baseline slot-2 compute interval
shows why the large individual-slot effect must not be generalized.

The archive supports the narrow observation that the exact R35 production path
reduced its directly enclosed native-binding p50 in all three slots. It does
not establish that the source change alone caused the observation, that other
workloads or devices improve, or that KFD reached HIP/HSA parity. R35 remained
materially slower than both references in every measured phase and slot. No
orders-of-magnitude speedup is claimed.
