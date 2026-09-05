# MI300X R33 synchronous fusion regression control, 2026-09-05

Status: `Measured` as an asynchronous-path regression control for the exact
bounded R26 V4 run below. The benchmark did not execute R33's new fused
synchronous directional helper, so this run is not direct performance evidence
for that optimization. Against the exact R32 raw archive, R33 changed median
slot-matched KFD E2E latency by -0.24% unadjusted, -0.58% after HSA
adjustment, and -0.59% after HIP adjustment, where positive means faster.
These small, inconsistent changes do not establish an improvement. KFD remained
approximately 3.00x to 3.03x slower than HIP E2E.

## Provenance

- R32 comparison implementation:
  `9f715189b8f35d4adb58be303900f937d88389ad`.
- R33 production implementation:
  `f25000bec19d45229a4b9ab531457d70f7977e3d` (`perf(kfd): fuse synchronous
  directional execution`). The intervening commits were proof-only,
  evidence-only, and documentation-only changes.
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm `7.2.4`.
- Device: host GPU 2, AMD Instinct MI300X, `gfx942:xnack-`, PCI
  `0000:46:00.0`, unique ID `0xd2e26fef80cf5c33`, KFD GPU ID `29122`, NUMA
  node 0.
- Archived caller-cwd fields, not build identity: Rust
  `1.97.1 (8bab26f4f 2026-07-14)` and Cargo
  `1.97.1 (c980f4866 2026-06-30)`. See the correction below.
- Runner SHA-256:
  `126c69a2193437d9da996b927eb8dd2af35f75b5583f1f6c961d012049e3a5fd`;
  host-guard SHA-256:
  `877c2b9199c5594a23c681dccdc1e58c2bef7228a87b16e22150baebc21af8b6`;
  checker SHA-256:
  `46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f`.
- HSACO SHA-256:
  `8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9`.
  R33 executable SHA-256 values were KFD
  `5302ce84cc51ccc2a4caa37b05ec5b114bcd97409a2ce22aab43a49cc688ac85`,
  HSA `9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83`,
  and HIP
  `f27354534a7d88b07308409419277fd2541f797b589d902ae3901abcb4290f1d`.

The R33 local external archive is
`/tmp/fe2o3-r33-f25000be-r26-20260905-evidence.tar.gz`, SHA-256
`0574ebc3a4a21cc5a0a21d412998378668e4087f0d77bfe8fdd0f718c60b5483`.
Its retained extraction is
`/tmp/fe2o3-r33-evidence-extract-dzWQn1yN`; its counterbalance set ID is
`27a9f17a96a5cc879d8c95a5cf4b9cb6db469d216c109932c6ea145f29bc20b6`.
The retained set-validation report has SHA-256
`87db3ed51535b570cabcd1f6fd99d55abf2a47ce0f867255a2cc674df9e02c5e`.

The exact R32 comparison archive is
`/tmp/fe2o3-r32-9f715189-r26-20260905-evidence.tar.gz`, SHA-256
`b78d9f37801d9e2b6e2391acdb2f532546d41cb88b4a65599a39ab98b91feb5c`.
Its counterbalance set ID is
`163fa3f48bf1247bb75b53cbe4f5caf18fc417ab54626af18eb94b28e699ef5a`.
Both `/tmp` archives and the extraction are external, non-durable artifacts.

R33's HSA executable is byte-identical to R32's HSA executable. R33's HIP
executable is not byte-identical to R32's
`c5fcd86008b0b454e28b21aea707c7eb3623fa43d9e8cfb7987e9314ec76db27`,
although the pinned HIP source is identical in both archives, SHA-256
`da7839dbbf12b18421e01c32e35d3b33935846deeef6d0210dfa725179bed542`.
The HSA source is also identical, SHA-256
`a1470c846474dcb10354202a5abd028a7ef9f13e9f36271eedec557953ff523e`.
Toolchain-provenance correction: the runner used for R32 and R33 invoked the
rustup shims for the recorded `rustc` and `cargo` fields from the caller's
working directory, while the actual Cargo build ran from the private archived
source tree. The differing recorded values therefore describe caller-cwd
rustup resolution and do not authenticate either build compiler. Both measured
commits contain the same `nightly-2026-04-03` `rust-toolchain.toml`, so their
archived-tree builds were expected to select that pinned toolchain, but the old
archives do not independently prove it. Commit
`71cbe8e8cb2147aaad076fda84a44bd4875f08ec` corrects future capture by resolving
and validating both versions from the archived source-tree cwd under the same
clean environment used for the build. It retains the V4 fields and schema, so
this correction does not invalidate either archive or alter any numerical
result in this note.

## Method and path audit

Each revision used the R26 V4 1 MiB in-place transform, 10 warmups, 30
samples per backend and slot, and 10 iterations per sample. A sample is the
integer average of its 10 host-monotonic iteration durations; the reported
statistic is the untrimmed p50. The cyclic Latin orders were KFD/HSA/HIP,
HSA/HIP/KFD, and HIP/KFD/HSA. Every output element was validated in all 310
iterations per backend and slot.

R33 added `execute_synchronous_directional_sdma_v1`, which fuses synchronous
directional submission and completion handling under one owner/memory loan.
A source-level call-graph audit found that only the KFD host upload/download
helpers invoke it. R26 instead calls `RuntimeContext::copy_async`, waits on the
returned submission separately, and reaches
`submit_directional_persistent_sdma_copy_v1`. Its 1 MiB transfers use the
single-packet asynchronous plan. Therefore the unchanged benchmark remains
useful for detecting an accidental regression in the public asynchronous path,
but cannot measure R33's new synchronous path.

## P50 observations

All values are nanoseconds. Promotion is KFD-only and covers the full
authenticated post-H2D Ready transition.

| Revision | Slot | Backend | H2D | Compute | D2H | E2E | Promotion |
|---|---:|---|---:|---:|---:|---:|---:|
| R32 | 0 | KFD | 146451 | 90768 | 61725 | 298941 | 13424 |
| R32 | 0 | HSA | 44673 | 20931 | 26013 | 91809 | n/a |
| R32 | 0 | HIP | 47046 | 25164 | 26829 | 99208 | n/a |
| R32 | 1 | KFD | 146466 | 90194 | 61318 | 298613 | 13264 |
| R32 | 1 | HSA | 44942 | 20863 | 25592 | 91574 | n/a |
| R32 | 1 | HIP | 47099 | 25134 | 26830 | 99198 | n/a |
| R32 | 2 | KFD | 146312 | 90481 | 61367 | 298457 | 13275 |
| R32 | 2 | HSA | 44919 | 20789 | 25468 | 91325 | n/a |
| R32 | 2 | HIP | 47241 | 24961 | 26938 | 99285 | n/a |
| R33 | 0 | KFD | 146133 | 90176 | 61587 | 298188 | 13293 |
| R33 | 0 | HSA | 45582 | 20867 | 26002 | 92580 | n/a |
| R33 | 0 | HIP | 47084 | 25080 | 26955 | 99314 | n/a |
| R33 | 1 | KFD | 146418 | 90988 | 61665 | 299332 | 13379 |
| R33 | 1 | HSA | 44864 | 20782 | 25432 | 91261 | n/a |
| R33 | 1 | HIP | 47050 | 24984 | 26724 | 98857 | n/a |
| R33 | 2 | KFD | 147838 | 90564 | 61891 | 300517 | 13427 |
| R33 | 2 | HSA | 44849 | 20792 | 25429 | 91269 | n/a |
| R33 | 2 | HIP | 46990 | 25108 | 26905 | 99223 | n/a |

### R33 KFD launch components

Preparation contains the bound-snapshot and authority intervals. Native
binding, publication, publish-to-completion, and recycle are separate
sequential intervals; recycle inclusive contains signal recycle and detach
restore. Compute is the inclusive latency, so the table as a whole is not
additive.

| Slot | Preparation | Bound snapshot | Authority | Native binding | Publication | Publish to completion | Completed readback | Signal recycle | Detach restore | Recycle inclusive |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 4450 | 218 | 2859 | 19638 | 16221 | 30786 | 0 | 9804 | 4447 | 14314 |
| 1 | 4289 | 241 | 2863 | 19700 | 16344 | 31210 | 0 | 9952 | 4471 | 14425 |
| 2 | 4272 | 198 | 2861 | 19507 | 16361 | 31150 | 0 | 9927 | 4423 | 14371 |

## Slotwise contrast

For phase `p`, reference `r`, and matching Latin slot `s`, the
reference-adjusted R33 latency reduction is:

```text
100 * (1 - (R33_KFD[p,s] / R32_KFD[p,s])
           / (R33_r[p,s] / R32_r[p,s]))
```

The unadjusted value omits the reference ratio. Positive means lower R33 KFD
latency. Each cell gives `slot 0 / slot 1 / slot 2; median`, in percent.

| Phase | Unadjusted KFD | Adjusted vs HSA | Adjusted vs HIP |
|---|---:|---:|---:|
| H2D | 0.217137 / 0.032772 / -1.042977; **0.032772** | 2.207016 / -0.141030 / -1.200684; **-0.141030** | 0.297669 / -0.071338 / -1.582704; **-0.071338** |
| Compute | 0.652212 / -0.880325 / -0.091732; **-0.091732** | 0.347508 / -1.273516 / -0.077290; **-0.077290** | 0.319468 / -1.485994 / 0.494276; **0.319468** |
| D2H | 0.223572 / -0.565902 / -0.853879; **-0.565902** | 0.181362 / -1.198591 / -1.008557; **-1.008557** | 0.689973 / -0.964794 / -0.977580; **-0.964794** |
| E2E | 0.251889 / -0.240780 / -0.690217; **-0.240780** | 1.082585 / -0.584578 / -0.751997; **-0.584578** | 0.358353 / -0.586553 / -0.753133; **-0.586553** |
| Promotion | 0.975864 / -0.867008 / -1.145009; **-0.867008** | n/a | n/a |

The exact R33-minus-R32 KFD slot deltas were `-318/-48/+1526` ns for H2D,
`-592/+794/+83` ns for compute, `-138/+347/+524` ns for D2H,
`-753/+719/+2060` ns for E2E, and `-131/+115/+152` ns for promotion. The
mixed signs and sub-percent medians are consistent with a control path that
did not execute the R33 optimization.

## R33 reference ratios

These are R33 KFD p50 divided by the matching R33 reference p50. Lower is
better; every value above one means KFD was slower in this harness.

| Slot | Phase | KFD/HSA | KFD/HIP |
|---:|---|---:|---:|
| 0 | H2D | 3.205937 | 3.103666 |
| 0 | Compute | 4.321465 | 3.595534 |
| 0 | D2H | 2.368549 | 2.284808 |
| 0 | E2E | 3.220868 | 3.002477 |
| 1 | H2D | 3.263597 | 3.111966 |
| 1 | Compute | 4.378212 | 3.641851 |
| 1 | D2H | 2.424701 | 2.307476 |
| 1 | E2E | 3.279955 | 3.027929 |
| 2 | H2D | 3.296350 | 3.146159 |
| 2 | Compute | 4.355714 | 3.606978 |
| 2 | D2H | 2.433875 | 2.300353 |
| 2 | E2E | 3.292651 | 3.028703 |

## Validation and cleanup

An independent checker rerun reproduced the R33 retained report byte for byte,
SHA-256
`87db3ed51535b570cabcd1f6fd99d55abf2a47ce0f867255a2cc674df9e02c5e`.
An independent calculation directly from the raw p50 fields reproduced the
reported observations and reference ratios. R33 slot-log SHA-256 values were,
in order,
`ce6c1a3d82b6276cd0040646958523cbe2354ccd9c58c951faeaea74a30c1fb9`,
`4bcffb8fb7fcbb6570da27fe1e2ebf57d383f1e1c58d9812b1e5de03163abc04`,
and `9c25daa2a82f4f8cb95c5e5091e3d65c0fec7f3796bbafbc543cfebeada4ff73`.
The archived `run.log` has SHA-256
`55337480fad45e3ded2e39262e1c14f594350cc94f4bf635c9c22884992fa325`.

All nine phase monitors reported `status=clean`, zero foreign selected-device
queues, and zero terminal selected-device queues. The maximum observed gap was
6,766 us, below the 10,000 us limit. GPU 2 busy and KFD GPU ID `29122` queue
counts were both zero before and after the run and before and after cleanup.
The operator's cleanup record, which is not contained in the retained archive,
states that the remote archive SHA-256 was rechecked immediately before
deletion and matched the local archive. The exact remote root
`/tmp/fe2o3-r33-f25000be-r26-TaYKlaXP` was deleted only with
`/usr/bin/find "$root" -depth -delete`, and its absence was confirmed.

## Reproduction

From the repository root, while both external archives remain available:

```bash
r33_archive=/tmp/fe2o3-r33-f25000be-r26-20260905-evidence.tar.gz
r32_archive=/tmp/fe2o3-r32-9f715189-r26-20260905-evidence.tar.gz
sha256sum "$r33_archive" "$r32_archive"

root="$(mktemp -d /tmp/fe2o3-r33-doc-check.XXXXXX)"
trap '/usr/bin/find "$root" -depth -delete' EXIT
mkdir "$root/r33" "$root/r32"
tar -xzf "$r33_archive" -C "$root/r33"
tar -xzf "$r32_archive" -C "$root/r32"

r33_set="$root/r33/output/r26-inplace-27a9f17a96a5cc879d8c95a5cf4b9cb6db469d216c109932c6ea145f29bc20b6"
r32_set="$root/r32/output/r26-inplace-163fa3f48bf1247bb75b53cbe4f5caf18fc417ab54626af18eb94b28e699ef5a"
python3 benchmarks/runtime_gfx942/check-parity.py \
  --schema fe2o3.r26-inplace-benchmark.v4 --r26-counterbalance-set \
  "$r33_set/slot-0.log" "$r33_set/slot-1.log" "$r33_set/slot-2.log" \
  >"$root/r33-recomputed.txt"
cmp "$root/r33-recomputed.txt" "$r33_set/set-validation.txt"
python3 benchmarks/runtime_gfx942/check-parity.py \
  --schema fe2o3.r26-inplace-benchmark.v4 --r26-counterbalance-set \
  "$r32_set/slot-0.log" "$r32_set/slot-1.log" "$r32_set/slot-2.log" \
  >"$root/r32-recomputed.txt"
cmp "$root/r32-recomputed.txt" "$r32_set/set-validation.txt"
sha256sum "$root/r33-recomputed.txt" "$root/r32-recomputed.txt"

python3 - "$r32_set" "$r33_set" <<'PY'
from pathlib import Path
from statistics import median
import sys

phases = ("h2d", "compute", "d2h", "e2e")

def observations(directory):
    result = {}
    for slot in range(3):
        backends = {}
        for line in (directory / f"slot-{slot}.log").read_text().splitlines():
            if not line.startswith("backend="):
                continue
            fields = dict(field.split("=", 1) for field in line.split() if "=" in field)
            backend = fields.get("backend")
            if backend in ("kfd", "hsa", "hip"):
                backends[backend] = {
                    phase: int(fields[f"{phase}_p50_ns"]) for phase in phases
                }
                if backend == "kfd":
                    backends[backend]["promotion"] = int(fields["promotion_p50_ns"])
        result[slot] = backends
    return result

r32, r33 = (observations(Path(path)) for path in sys.argv[1:])
for phase in phases:
    raw = [
        100 * (1 - r33[slot]["kfd"][phase] / r32[slot]["kfd"][phase])
        for slot in range(3)
    ]
    print(phase, "raw", *(f"{value:.6f}" for value in raw),
          "median", f"{median(raw):.6f}")
    for reference in ("hsa", "hip"):
        adjusted = [
            100 * (1 - (r33[slot]["kfd"][phase] / r32[slot]["kfd"][phase])
                   / (r33[slot][reference][phase] / r32[slot][reference][phase]))
            for slot in range(3)
        ]
        print(phase, reference, *(f"{value:.6f}" for value in adjusted),
              "median", f"{median(adjusted):.6f}")
promotion = [
    100 * (1 - r33[slot]["kfd"]["promotion"] / r32[slot]["kfd"]["promotion"])
    for slot in range(3)
]
print("promotion raw", *(f"{value:.6f}" for value in promotion),
      "median", f"{median(promotion):.6f}")
PY
/usr/bin/find "$root" -depth -delete
trap - EXIT
```

## Claim limits

The paired comparison unit is one matching Latin slot, so every aggregate has
only `n=3` descriptive effects. R32 and R33 were separate runs rather than an
interleaved cross-revision experiment, their recorded caller-cwd Rust fields
differ without establishing actual build-toolchain identity, and the raw
samples are not ordinal pairs. The HIP sources match but the HIP executable
bytes do not; only the HSA control executable is byte-identical across the two
archives.

Most importantly, R26 did not execute R33's fused synchronous path. No R33
synchronous latency or throughput result is claimed. No confidence interval,
significance test, long-run variance bound, multi-host result, energy result,
or workload-general result is claimed. This evidence establishes functional
validation and no large regression in one unchanged asynchronous workload. It
does not establish HIP/HSA parity, general application speedup, or an
orders-of-magnitude advantage.
