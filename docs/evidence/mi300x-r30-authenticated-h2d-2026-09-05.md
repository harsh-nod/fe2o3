# MI300X R30 authenticated H2D comparison, 2026-09-05

Status: `Measured` for the exact bounded R26 V4 runs below. This note compares
the authenticated H2D implementation at two exact commits; it is not a generic
HIP/HSA parity or application-speedup result.

## Provenance

- Baseline commit: `7970da879291292bcf02fd99a07b5f3c9a3b6427`.
- Current commit: `dc1887e9999135e450e53e99a8d3d99bf933c689`.
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm `7.2.4`.
- Device: host GPU 2, AMD Instinct MI300X, `gfx942:xnack-`, PCI
  `0000:46:00.0`, unique ID `0xd2e26fef80cf5c33`, KFD GPU ID `29122`, NUMA
  node 0.
- Rust: `1.96.0-nightly (55e86c996 2026-04-02)`; Cargo:
  `1.96.0-nightly (888f67534 2026-03-30)`. Both runs resolved the pinned
  `nightly-2026-04-03` toolchain through the same explicit
  `$HOME/.cargo/bin` path and clean build environment.
- Common runner SHA-256:
  `126c69a2193437d9da996b927eb8dd2af35f75b5583f1f6c961d012049e3a5fd`.
  Common host-guard SHA-256:
  `877c2b9199c5594a23c681dccdc1e58c2bef7228a87b16e22150baebc21af8b6`.
- Common HSACO SHA-256:
  `8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9`.
  HSA and HIP source identities also matched. The HSA executable was
  byte-identical; the run-local HIP executables were not byte-identical despite
  their matched source and native toolchain, so HIP is a matched-source rather
  than bit-identical control.

The following are **local external evidence archives**. They are not retained
repository artifacts, and their `/tmp` paths are not durable:

| Revision | Local external archive | Archive SHA-256 | Counterbalance set ID |
|---|---|---|---|
| Baseline | `/tmp/fe2o3-r30-7970da87-sametoolchain-r26-20260905T145113-1336306-18815-evidence.tar.gz` | `ee1ef8133d3b7b0f3400edba7481041744b491e73b9ed4c5e49daae259e779f6` | `140aedc0a225ff1120f4f3a179275d17630e8335cda02982d97912d0757ec29f` |
| Current | `/tmp/fe2o3-r30-dc1887e-r26-20260905T144200-29771-evidence.tar.gz` | `6fef2604c9e649c79972d9b76237ec4a40b634b589baf986bca774bb6c45fdd8` | `572deebf2e2eea38842b3d7fcf1932814e3d1782b754b2a9406902cfedf342d3` |

## Method

Each set used the R26 V4 1 MiB in-place transform, 10 warmups, 30 samples per
backend and slot, and 10 iterations per sample. A sample is the integer average
of its 10 host-monotonic iteration durations; the reported statistic is the
untrimmed p50. The three cyclic Latin orders were KFD/HSA/HIP, HSA/HIP/KFD,
and HIP/KFD/HSA. Every iteration validated every output element.

The timing boundaries were unchanged between revisions:

- the exact full host `write_allocation`, including the current revision's
  fused SHA-256 work, occurs before both the E2E and H2D timers;
- KFD H2D starts before `copy_async` and ends only after its wait succeeds, so
  it includes the successful full-H2D-to-compute-Ready transition;
- the nested promotion interval measures that full transition, including
  affiliation/preflight, operational currentness observations, certificate
  lookup and comparison, and frontier retirement. It is not an isolated hash
  or certificate-lookup duration;
- the launch critical path remains inclusive preparation, native binding,
  publication, publish-to-completion, and inclusive recycle. Nested component
  p50 values are not additive and are not substituted for the inclusive
  compute p50.

Consequently, this comparison measures moving the post-H2D payload reread and
SHA work out of the timed H2D/E2E path. It does not measure or establish removal
of the CPU cost from the preceding host write.

## P50 observations

All values are nanoseconds. Promotion is available only for KFD and is the full
Ready-promotion interval described above.

### Baseline `7970da87`

| Slot | Backend | H2D | Compute | D2H | E2E | Promotion |
|---:|---|---:|---:|---:|---:|---:|
| 0 | KFD | 672221 | 88603 | 77585 | 838232 | 579045 |
| 0 | HSA | 44750 | 20788 | 25357 | 91011 | n/a |
| 0 | HIP | 47025 | 25086 | 26798 | 99089 | n/a |
| 1 | KFD | 674626 | 88381 | 77325 | 840760 | 579063 |
| 1 | HSA | 45024 | 29769 | 25320 | 100176 | n/a |
| 1 | HIP | 47039 | 25104 | 26656 | 98942 | n/a |
| 2 | KFD | 672697 | 88469 | 76620 | 837975 | 579006 |
| 2 | HSA | 44742 | 20976 | 25320 | 91172 | n/a |
| 2 | HIP | 46868 | 25115 | 26814 | 98834 | n/a |

### Current `dc1887e9`

| Slot | Backend | H2D | Compute | D2H | E2E | Promotion |
|---:|---|---:|---:|---:|---:|---:|
| 0 | KFD | 160856 | 88721 | 77118 | 327397 | 13256 |
| 0 | HSA | 44832 | 20807 | 25468 | 91201 | n/a |
| 0 | HIP | 47091 | 24964 | 26678 | 98965 | n/a |
| 1 | KFD | 162297 | 90082 | 78048 | 331105 | 13455 |
| 1 | HSA | 44662 | 20808 | 25394 | 90996 | n/a |
| 1 | HIP | 47215 | 25104 | 26545 | 99072 | n/a |
| 2 | KFD | 161075 | 89116 | 76733 | 327499 | 13207 |
| 2 | HSA | 45060 | 20827 | 25334 | 91246 | n/a |
| 2 | HIP | 47100 | 24987 | 26819 | 99169 | n/a |

## Paired contrast

For each phase, reference, and matching Latin slot, the reference-adjusted
latency reduction is:

```text
100 * (1 - (current_KFD / baseline_KFD)
           / (current_reference / baseline_reference))
```

The table reports the median of the three slotwise effects. Positive values
mean a lower current KFD latency. The unadjusted column applies the same
slotwise calculation without a reference.

| Phase | Unadjusted KFD reduction | Adjusted vs HSA | Adjusted vs HIP |
|---|---:|---:|---:|
| H2D | 76.055341% | 76.114732% | 76.104502% |
| Compute | -0.731330% | -1.451979% | -1.247342% |
| D2H | -0.147481% | -0.092138% | -0.128810% |
| E2E | 60.917808% | 60.949503% | 60.893020% |
| Promotion | 97.710713% | n/a | n/a |

The exact current-minus-baseline KFD slot deltas were
`-511365/-512329/-511622` ns for H2D,
`+118/+1701/+647` ns for compute, `-467/+723/+113` ns for D2H,
`-510835/-509655/-510476` ns for E2E, and
`-565789/-565608/-565799` ns for promotion. HSA compute in slot 1 moved from
29,769 ns to 20,808 ns and its E2E moved from 100,176 ns to 90,996 ns; this
reference excursion is why both controls are shown rather than presenting HSA
alone.

## Current reference ratios

These are current KFD p50 divided by current reference p50. Lower is better;
values above one mean KFD remained slower than that reference in this harness.

| Slot | Phase | KFD/HSA | KFD/HIP |
|---:|---|---:|---:|
| 0 | H2D | 3.587973 | 3.415854 |
| 0 | Compute | 4.263998 | 3.553958 |
| 0 | D2H | 3.028035 | 2.890696 |
| 0 | E2E | 3.589840 | 3.308210 |
| 1 | H2D | 3.633895 | 3.437403 |
| 1 | Compute | 4.329200 | 3.588352 |
| 1 | D2H | 3.073482 | 2.940215 |
| 1 | E2E | 3.638676 | 3.342064 |
| 2 | H2D | 3.574678 | 3.419851 |
| 2 | Compute | 4.278869 | 3.566495 |
| 2 | D2H | 3.028855 | 2.861143 |
| 2 | E2E | 3.589187 | 3.302433 |

Current promotion/H2D p50 shares were `0.082409`, `0.082904`, and `0.081993`
for slots 0 through 2.

## Validation and cleanup

The baseline revision's checker and the current revision's checker were each
rerun independently over the copied baseline logs. Both passed all slots and
the set and reproduced the persisted report byte-for-byte, SHA-256
`0853c205aae075247c56ea506e33e22c5355e5d0aea10866ad38f842bd2b5bbb`.
The current checker independently reproduced the current persisted report
byte-for-byte, SHA-256
`c77c417333632b88dcb113c4ba718fc1c507c1c9077f77c08009a5b8dffe90e8`.

One preliminary baseline attempt was rejected during slot 1 because a guard
observation gap was 10,682,992 ns, above the 10,000,000 ns maximum. It emitted
no evidence set and none of its partial values are included here. A fresh full
run then passed the normal guard and checker.

All measurement roots were unique `/tmp` directories and were removed only
with `find "$root" -depth -delete`; their absence was confirmed. All eight GPUs
reported 0% busy after each successful cleanup, and GPU 2 had zero KFD queues.
Four queues for an unrelated, pre-existing ComfyUI process remained on GPU 0
(KFD GPU ID `28851`) before and after the runs. They were not modified, and
host-wide KFD queue count was therefore not zero.

## Claim limits

The paired unit is one matched Latin slot, so every aggregate above has only
`n=3` descriptive effects. The separately executed revisions were not
interleaved, and their raw samples are not valid ordinal pairs. No confidence
interval, significance test, long-run variance bound, multi-host result,
throughput result, energy result, or workload-general result is claimed.

The exact evidence supports a reduction in this harness's timed authenticated
KFD H2D, Ready-promotion, and E2E intervals. It does not support a generic
application speedup, hardware claim, HIP/HSA equivalence, or parity claim. The
current KFD/reference ratios above remain materially greater than one.
