# MI300X R34 asynchronous single-copy fusion, 2026-09-05

Status: `Measured` for the exact bounded R26 V4 run below. Against the retained
exact R33 archive, R34 reduced median slot-matched KFD E2E latency by **4.707%**
unadjusted, **4.558%** after HSA ratio-of-ratios adjustment, and **4.768%**
after HIP adjustment. Most of the change was in D2H: its median slotwise
reduction was 18.701% raw, 18.428% HSA-adjusted, and 18.716% HIP-adjusted.
Compute regressed by about 1%. This is one clean `n=3` descriptive run, not a
parity, causal, or workload-general result.

## Provenance

- R33 comparison implementation:
  `f25000bec19d45229a4b9ab531457d70f7977e3d`.
- R34 production implementation:
  `b015b81f862220d48671e1c4809b8ce858a317e7` (`perf(kfd): fuse
  asynchronous single-copy submission`). The production delta is confined to
  `persistent_directional_sdma.rs`, `queue_live.rs`, and the R26 structural
  assertion in `kfd_backend.rs`.
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm `7.2.4`.
- Device: host GPU 2, AMD Instinct MI300X, `gfx942:xnack-`, PCI
  `0000:46:00.0`, unique ID `0xd2e26fef80cf5c33`, KFD GPU ID `29122`, NUMA
  node 0.
- Workload: 1 MiB R26 V4 in-place transform, 10 warmups, 30 samples per
  backend per slot, and 10 iterations per sample. The cyclic Latin orders were
  KFD/HSA/HIP, HSA/HIP/KFD, and HIP/KFD/HSA.
- Runner SHA-256:
  `126c69a2193437d9da996b927eb8dd2af35f75b5583f1f6c961d012049e3a5fd`;
  host-guard SHA-256:
  `877c2b9199c5594a23c681dccdc1e58c2bef7228a87b16e22150baebc21af8b6`;
  checker SHA-256:
  `46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f`.
- HSACO SHA-256:
  `8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9`.
  Kernel source, policy, and fixture recipe SHA-256 values were respectively
  `1185d4cd931c1bb43d113e66714af3d98bd96f7d036f5c610a909abf34ba87d5`,
  `c060c3c4a96012fc6661b0585f4ff8ffe7b7f8483eb40262e4a018133c0ea585`,
  and `29c6db8ea2a86392eb980b78e42fa1c049a6f92ca8dd3dc8224f90cf66254ab5`.

The R34 external archive is
`/tmp/fe2o3-r34-b015b81f-r26-20260905-evidence.tar.gz`, SHA-256
`ba1820d1e27f33e789aeba0963089fcaeb2d3b0ee94adb5a8f0d6eba854d4bd7`.
Its retained extraction is `/tmp/fe2o3-r34-evidence-extract.yrUsIUr0`; its
counterbalance set ID is
`444a5c8d2bf6aba2e70212ede5eabaeca3d255f379c191ff654e65fb2cf5bcd0`.
The evidence-manifest SHA-256 recorded in the validated set is
`f784c533fb4c0105c9b3a5e516d75ccf3767d4b584cc93bbb3dd0ed24002acea`.
The set-validation report SHA-256 is
`d81727e862dc787b69ba87a22429e3ef996902498e92257b1e562beaef6de005`.

The retained exact R33 archive is
`/tmp/fe2o3-r33-f25000be-r26-20260905-evidence.tar.gz`, SHA-256
`0574ebc3a4a21cc5a0a21d412998378668e4087f0d77bfe8fdd0f718c60b5483`.
Its set ID is
`27a9f17a96a5cc879d8c95a5cf4b9cb6db469d216c109932c6ea145f29bc20b6`,
and its set-validation SHA-256 is
`87db3ed51535b570cabcd1f6fd99d55abf2a47ce0f867255a2cc674df9e02c5e`.
These `/tmp` artifacts are external and non-durable.

R34 executable SHA-256 values were KFD
`bafaecfe0601124be09361d71a5eb39e623a67bc608f20761f1a0ca80818ea60`,
HSA `9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83`,
and HIP `8ecacf62a6a4643b2f274e422d66fd7d53f83eebede41da315cee13b1d8c102c`.
R33 values were KFD
`5302ce84cc51ccc2a4caa37b05ec5b114bcd97409a2ce22aab43a49cc688ac85`,
the same HSA binary, and HIP
`f27354534a7d88b07308409419277fd2541f797b589d902ae3901abcb4290f1d`.
HSA source and executable bytes match. HIP source and `hipcc` version match,
but HIP executable bytes do not, so HIP is a matched-source rather than
byte-identical executable control.

### Toolchain-provenance correction

Neither archive's recorded `rustc` or `cargo` field is build identity. The V4
runner invoked those rustup shims from its caller working directory, but ran
the actual build from the private archived `source_tree`. R33 consequently
recorded stable 1.97.1 while R34 recorded 1.96.0 nightly solely because their
runner invocation directories selected different rustup contexts. Both source
commits contain the same `nightly-2026-04-03` `rust-toolchain.toml`, so the
builds were expected to resolve that pinned toolchain inside `source_tree`, but
the old archives do not independently authenticate that fact.

Commit `71cbe8e8cb2147aaad076fda84a44bd4875f08ec` (`bench(runtime): bind
toolchain provenance to source tree`) corrected future capture. It resolves and
validates both versions from the archived source-tree cwd with the same
`env -i` used for the build and fails closed on malformed output. Its corrected
runner SHA-256 is
`8c8b59b77705072b83866076d38260f38951e1f66ef7dd6ffc384e5372c13c2f`.
The field names and V4 schema did not change; existing archives remain valid,
but their old toolchain fields must not be used for compiler attribution.

## Measured path

The R26 source and the R34 structural test establish the following exact path:

1. `gfx942-runtime-r26-inplace-benchmark.rs:146-164` starts timing, calls
   `RuntimeContextV1::copy_async`, then waits separately. The iteration invokes
   it for H2D and D2H at lines 526-538.
2. `context.rs:2602-2710` validates the request and calls backend
   `copy_async_v1` at line 2685.
3. `kfd_backend.rs:14197-14502` admits the one-MiB copy and immediately calls
   `publish_sdma_copy_v1` when ready. Its directional branch reaches
   `DirectionalSdmaOpsV1::submit` through lines 3689-3789.
4. In `kfd_backend_sdma_seam.rs:798-860`, the single-request branch calls
   `submit_directional_persistent_sdma_copy_v1` at lines 830-839. One MiB is
   below `GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1`, so neither measured direction
   uses the windowed path.
5. In exact production `queue_live.rs:6392-6558`, that asynchronous method
   admits once, opens exactly one `with_sdma_owner_memory` loan at line 6417,
   performs opening currentness, persistent-use preparation/detach, lower
   preparation, prepublication currentness, publication at line 6515, and final
   currentness before retake. It returns published incomplete custody without
   observing completion; the public poll/wait path remains separate.
6. `kfd_backend.rs:15198-15255` is the structural regression test asserting
   this R26 route, exactly one owner-memory loan, publication, and no fused wait.

Thus R26 directly measures R34's fused asynchronous single-copy submit for both
directions. This differs from R33, whose fused synchronous helper R26 did not
execute.

## P50 observations

All values are untrimmed p50 nanoseconds from the raw slot logs. Each sample is
the integer average of ten host-monotonic iteration measurements. Every output
element was validated in all 310 iterations per backend and slot.

| Revision | Slot | Backend | H2D | Compute | D2H | E2E | Promotion |
|---|---:|---|---:|---:|---:|---:|---:|
| R33 | 0 | KFD | 146133 | 90176 | 61587 | 298188 | 13293 |
| R33 | 0 | HSA | 45582 | 20867 | 26002 | 92580 | n/a |
| R33 | 0 | HIP | 47084 | 25080 | 26955 | 99314 | n/a |
| R33 | 1 | KFD | 146418 | 90988 | 61665 | 299332 | 13379 |
| R33 | 1 | HSA | 44864 | 20782 | 25432 | 91261 | n/a |
| R33 | 1 | HIP | 47050 | 24984 | 26724 | 98857 | n/a |
| R33 | 2 | KFD | 147838 | 90564 | 61891 | 300517 | 13427 |
| R33 | 2 | HSA | 44849 | 20792 | 25429 | 91269 | n/a |
| R33 | 2 | HIP | 46990 | 25108 | 26905 | 99223 | n/a |
| R34 | 0 | KFD | 145804 | 91192 | 49924 | 287198 | 13331 |
| R34 | 0 | HSA | 45043 | 20845 | 25223 | 91282 | n/a |
| R34 | 0 | HIP | 47034 | 25115 | 26809 | 99049 | n/a |
| R34 | 1 | KFD | 143116 | 91333 | 50177 | 285152 | 13435 |
| R34 | 1 | HSA | 44676 | 20810 | 25369 | 91090 | n/a |
| R34 | 1 | HIP | 46812 | 25081 | 26769 | 98954 | n/a |
| R34 | 2 | KFD | 144489 | 91542 | 50317 | 286373 | 13405 |
| R34 | 2 | HSA | 45344 | 20814 | 25903 | 92250 | n/a |
| R34 | 2 | HIP | 47115 | 25056 | 26910 | 99287 | n/a |

The medians across the three KFD slot p50 values changed from
`146418/90564/61665/299332` ns to `144489/91333/50177/286373` ns for
H2D/compute/D2H/E2E respectively. These medians are descriptive summaries;
the percentage result below is the median of the three slotwise effects, not a
percentage calculated from these cross-slot medians.

### R34 KFD launch components

These components describe compute launch/completion, not the copy submission
interval. Preparation contains bound snapshot and authority; recycle inclusive
contains signal recycle and detach restore. Compute is inclusive, so the table
is nested and non-additive.

| Slot | Promotion | Preparation | Bound snapshot | Authority | Native binding | Publication | Publish to completion | Completed readback | Signal recycle | Detach restore | Recycle inclusive |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 13331 | 4287 | 239 | 2866 | 19801 | 16449 | 31392 | 0 | 10014 | 4365 | 14408 |
| 1 | 13435 | 4370 | 235 | 2890 | 20012 | 16471 | 31406 | 0 | 9954 | 4484 | 14461 |
| 2 | 13405 | 4374 | 223 | 2856 | 19949 | 16544 | 31435 | 0 | 9914 | 4543 | 14490 |

No compute component improved consistently. Median slotwise changes were
-0.286% for promotion, -1.584% for native binding, -1.119% for publication,
and -0.915% for publish-to-completion, consistent with the overall compute
regression and with R34 targeting copy submission rather than compute.

## Slotwise contrast

For phase `p`, matching Latin slot `s`, and reference `r`, the adjusted R34
latency reduction is the ratio of ratios:

```text
100 * (1 - (R34_KFD[p,s] / R34_r[p,s])
           / (R33_KFD[p,s] / R33_r[p,s]))
```

The raw value omits the reference ratio. Positive means lower R34 KFD latency.
Each cell gives `slot 0 / slot 1 / slot 2; median`, in percent.

| Phase | Raw KFD | Adjusted vs HSA | Adjusted vs HIP |
|---|---:|---:|---:|
| H2D | 0.225137 / 2.255187 / 2.265317; **2.255187** | -0.968803 / 1.843870 / 3.332243; **1.843870** | 0.119071 / 1.758236 / 2.524616; **1.758236** |
| Compute | -1.126686 / -0.379171 / -1.079899; **-1.079899** | -1.233416 / -0.244110 / -0.973060; **-0.973060** | -0.985757 / 0.009042 / -1.289676; **-0.985757** |
| D2H | 18.937438 / 18.629693 / 18.700619; **18.700619** | 16.433861 / 18.427622 / 20.188319; **18.427622** | 18.495977 / 18.766480 / 18.715725; **18.715725** |
| E2E | 3.685594 / 4.737215 / 4.706556; **4.706556** | 2.316035 / 4.558381 / 5.719920; **4.558381** | 3.427911 / 4.830597 / 4.767981; **4.767981** |

Rounded, the median E2E result is **4.707% raw, 4.558% HSA-adjusted, and
4.768% HIP-adjusted**. Exact R34-minus-R33 KFD slot deltas were
`-329/-3302/-3349` ns for H2D, `+1016/+345/+978` ns for compute,
`-11663/-11488/-11574` ns for D2H, and `-10990/-14180/-14144` ns for E2E.
The consistent approximately 11.5 us D2H reduction accounts for most of the
E2E movement; H2D improved less and compute moved higher.

## Remaining reference ratios

These are R34 KFD p50 divided by its same-slot reference p50. Every value above
one means KFD remained slower in this harness.

| Slot | Phase | KFD/HSA | KFD/HIP |
|---:|---|---:|---:|
| 0 | H2D | 3.236996 | 3.099970 |
| 0 | Compute | 4.374766 | 3.630978 |
| 0 | D2H | 1.979305 | 1.862210 |
| 0 | E2E | 3.146272 | 2.899555 |
| 1 | H2D | 3.203420 | 3.057250 |
| 1 | Compute | 4.388900 | 3.641521 |
| 1 | D2H | 1.977886 | 1.874444 |
| 1 | E2E | 3.130442 | 2.881662 |
| 2 | H2D | 3.186508 | 3.066730 |
| 2 | Compute | 4.398097 | 3.653496 |
| 2 | D2H | 1.942516 | 1.869825 |
| 2 | E2E | 3.104314 | 2.884295 |

R34 therefore remained 1.86x-1.87x slower than HIP D2H, 2.88x-2.90x slower
than HIP E2E, 3.06x-3.10x slower than HIP H2D, and 3.63x-3.65x slower than HIP
compute. HSA gaps were also material. No HIP/HSA parity is claimed.

## Validation and cleanup

An independent checker rerun reproduced the R34 retained set report byte for
byte with SHA-256
`d81727e862dc787b69ba87a22429e3ef996902498e92257b1e562beaef6de005`.
The archived `run.log` SHA-256 is
`4828fb987e1927d5d5587befbfd224abb8b9fbb159a281b2f6173786ffafc9d2`.
Slot-log SHA-256 values, in order, are
`aaa825068d8bc75fb883b7e148a4886dae8815b873310eb01737a885964f6902`,
`7badae9f486e1e1aa18f048a06c328d36bf085dca4286833dd8e8048fa3c2553`,
and `9b7a8c2abffbce9f0fb19150e1f7651d3e3c1e419bc9dd18f2f7be7988c721b6`.
The preflight and postrun record SHA-256 values are
`60ecd0c5a72d7d6b52ca0df93bc10ba2cd23234eb2c04ff3b64d5aed37c0f497`
and `d70ae3212289cd2beb6c6bae16b89780a603b709f110354ffddfb28d24f7dc2e`.

All nine phase monitors reported `status=clean`, zero foreign selected-device
queues, zero terminal selected-device queues, target exit zero, target reaped,
and process group absent. The maximum observed monitor gap was 4,056 us, below
the 10,000 us limit. Preflight and postrun both recorded GPU busy zero and KFD
GPU ID `29122` queue count zero; the checkout was clean at exact R34 production
commit `b015b81f862220d48671e1c4809b8ce858a317e7`.

The first unique root, `/tmp/fe2o3-r34-b015-r26.JDoNMevx`, failed closed before
measurement when the host guard rejected a negative process identity. GPU busy
and selected queues remained zero, and that exact root was deleted and its
absence confirmed. The successful root was
`/tmp/fe2o3-r34-b015-r26.TUJTMYHk`. The remote archive SHA-256 was rechecked
immediately before transfer cleanup and matched the retained local archive.
That exact root was deleted only with `/usr/bin/find "$root" -depth -delete`;
absence, GPU busy zero, and selected queues zero were reconfirmed. No foreign
process or state was modified.

## Reproduction

From the repository root, while both external archives remain available:

```bash
set -euo pipefail
r33_archive=/tmp/fe2o3-r33-f25000be-r26-20260905-evidence.tar.gz
r34_archive=/tmp/fe2o3-r34-b015b81f-r26-20260905-evidence.tar.gz
printf '%s  %s\n' \
  0574ebc3a4a21cc5a0a21d412998378668e4087f0d77bfe8fdd0f718c60b5483 \
  "$r33_archive" \
  ba1820d1e27f33e789aeba0963089fcaeb2d3b0ee94adb5a8f0d6eba854d4bd7 \
  "$r34_archive" | sha256sum --check --status

root="$(mktemp -d /tmp/fe2o3-r34-doc-check.XXXXXX)"
trap '/usr/bin/find "$root" -depth -delete' EXIT
mkdir "$root/r33" "$root/r34"
tar -xzf "$r33_archive" -C "$root/r33"
tar -xzf "$r34_archive" -C "$root/r34"
r33_set="$root/r33/output/r26-inplace-27a9f17a96a5cc879d8c95a5cf4b9cb6db469d216c109932c6ea145f29bc20b6"
r34_set="$root/r34/output/r26-inplace-444a5c8d2bf6aba2e70212ede5eabaeca3d255f379c191ff654e65fb2cf5bcd0"

python3 benchmarks/runtime_gfx942/check-parity.py \
  --schema fe2o3.r26-inplace-benchmark.v4 --r26-counterbalance-set \
  "$r33_set/slot-0.log" "$r33_set/slot-1.log" "$r33_set/slot-2.log" \
  >"$root/r33-recomputed.txt"
cmp "$root/r33-recomputed.txt" "$r33_set/set-validation.txt"
python3 benchmarks/runtime_gfx942/check-parity.py \
  --schema fe2o3.r26-inplace-benchmark.v4 --r26-counterbalance-set \
  "$r34_set/slot-0.log" "$r34_set/slot-1.log" "$r34_set/slot-2.log" \
  >"$root/r34-recomputed.txt"
cmp "$root/r34-recomputed.txt" "$r34_set/set-validation.txt"

printf '%s  %s\n' \
  87db3ed51535b570cabcd1f6fd99d55abf2a47ce0f867255a2cc674df9e02c5e \
  "$r33_set/set-validation.txt" \
  d81727e862dc787b69ba87a22429e3ef996902498e92257b1e562beaef6de005 \
  "$r34_set/set-validation.txt" \
  4828fb987e1927d5d5587befbfd224abb8b9fbb159a281b2f6173786ffafc9d2 \
  "$root/r34/run.log" \
  60ecd0c5a72d7d6b52ca0df93bc10ba2cd23234eb2c04ff3b64d5aed37c0f497 \
  "$root/r34/preflight.txt" \
  d70ae3212289cd2beb6c6bae16b89780a603b709f110354ffddfb28d24f7dc2e \
  "$root/r34/postrun.txt" \
  aaa825068d8bc75fb883b7e148a4886dae8815b873310eb01737a885964f6902 \
  "$r34_set/slot-0.log" \
  7badae9f486e1e1aa18f048a06c328d36bf085dca4286833dd8e8048fa3c2553 \
  "$r34_set/slot-1.log" \
  9b7a8c2abffbce9f0fb19150e1f7651d3e3c1e419bc9dd18f2f7be7988c721b6 \
  "$r34_set/slot-2.log" | sha256sum --check --status
grep -Fq \
  'manifest_sha256=f784c533fb4c0105c9b3a5e516d75ccf3767d4b584cc93bbb3dd0ed24002acea' \
  "$r34_set/set-validation.txt"

python3 - "$r33_set" "$r34_set" <<'PY'
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

def read_set(path):
    slots = []
    for slot in range(3):
        backends = {}
        for line in (path / f"slot-{slot}.log").read_text().splitlines():
            if not line.startswith("backend="):
                continue
            fields = dict(item.split("=", 1) for item in line.split() if "=" in item)
            backend = fields["backend"]
            if backend not in ("kfd", "hsa", "hip"):
                continue
            values = {phase: int(fields[f"{phase}_p50_ns"]) for phase in phases}
            if backend == "kfd":
                values.update({name: int(fields[f"{name}_p50_ns"])
                               for name in components})
            backends[backend] = values
        slots.append(backends)
    return slots

def context(path):
    line = next(line for line in (path / "slot-0.log").read_text().splitlines()
                if line.startswith("context schema=fe2o3.r26-inplace-benchmark.v4"))
    return dict(item.split("=", 1) for item in line.split() if "=" in item)

expected_backend = {
    "r33": [
        {"kfd": [146133, 90176, 61587, 298188], "hsa": [45582, 20867, 26002, 92580], "hip": [47084, 25080, 26955, 99314]},
        {"kfd": [146418, 90988, 61665, 299332], "hsa": [44864, 20782, 25432, 91261], "hip": [47050, 24984, 26724, 98857]},
        {"kfd": [147838, 90564, 61891, 300517], "hsa": [44849, 20792, 25429, 91269], "hip": [46990, 25108, 26905, 99223]},
    ],
    "r34": [
        {"kfd": [145804, 91192, 49924, 287198], "hsa": [45043, 20845, 25223, 91282], "hip": [47034, 25115, 26809, 99049]},
        {"kfd": [143116, 91333, 50177, 285152], "hsa": [44676, 20810, 25369, 91090], "hip": [46812, 25081, 26769, 98954]},
        {"kfd": [144489, 91542, 50317, 286373], "hsa": [45344, 20814, 25903, 92250], "hip": [47115, 25056, 26910, 99287]},
    ],
}
expected_components = [
    [13331, 4287, 239, 2866, 19801, 16449, 31392, 0, 10014, 4365, 14408],
    [13435, 4370, 235, 2890, 20012, 16471, 31406, 0, 9954, 4484, 14461],
    [13405, 4374, 223, 2856, 19949, 16544, 31435, 0, 9914, 4543, 14490],
]
r33, r34 = (read_set(Path(item)) for item in sys.argv[1:])
common_identity = {
    "runner_sha256": "126c69a2193437d9da996b927eb8dd2af35f75b5583f1f6c961d012049e3a5fd",
    "checker_sha256": "46175467358f6fda3e629c07aba330ef12608bf3b701ea06c680039add1c8a6f",
    "host_guard_sha256": "877c2b9199c5594a23c681dccdc1e58c2bef7228a87b16e22150baebc21af8b6",
    "hsaco_sha256": "8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9",
    "kernel_source_sha256": "1185d4cd931c1bb43d113e66714af3d98bd96f7d036f5c610a909abf34ba87d5",
    "kernel_policy_sha256": "c060c3c4a96012fc6661b0585f4ff8ffe7b7f8483eb40262e4a018133c0ea585",
    "fixture_recipe_sha256": "29c6db8ea2a86392eb980b78e42fa1c049a6f92ca8dd3dc8224f90cf66254ab5",
    "hsa_source_sha256": "a1470c846474dcb10354202a5abd028a7ef9f13e9f36271eedec557953ff523e",
    "hip_source_sha256": "da7839dbbf12b18421e01c32e35d3b33935846deeef6d0210dfa725179bed542",
}
revision_identity = {
    "r33": {
        "git_commit": "f25000bec19d45229a4b9ab531457d70f7977e3d",
        "kfd_binary_sha256": "5302ce84cc51ccc2a4caa37b05ec5b114bcd97409a2ce22aab43a49cc688ac85",
        "hsa_binary_sha256": "9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83",
        "hip_binary_sha256": "f27354534a7d88b07308409419277fd2541f797b589d902ae3901abcb4290f1d",
    },
    "r34": {
        "git_commit": "b015b81f862220d48671e1c4809b8ce858a317e7",
        "kfd_binary_sha256": "bafaecfe0601124be09361d71a5eb39e623a67bc608f20761f1a0ca80818ea60",
        "hsa_binary_sha256": "9eebbf232bf4afaeb662ebbc8c42b5f068e77f109f64d9ef127929bf1eaf3b83",
        "hip_binary_sha256": "8ecacf62a6a4643b2f274e422d66fd7d53f83eebede41da315cee13b1d8c102c",
    },
}
for name, path in (("r33", Path(sys.argv[1])), ("r34", Path(sys.argv[2]))):
    observed = context(path)
    for key, value in common_identity.items():
        assert observed[key] == value, (name, key, observed[key])
    for key, value in revision_identity[name].items():
        assert observed[key] == value, (name, key, observed[key])
for name, data in (("r33", r33), ("r34", r34)):
    for slot in range(3):
        for backend in ("kfd", "hsa", "hip"):
            observed = [data[slot][backend][phase] for phase in phases]
            assert observed == expected_backend[name][slot][backend]
for slot in range(3):
    observed = [r34[slot]["kfd"][name] for name in components]
    assert observed == expected_components[slot]
print("r33-kfd-medians", *(int(median(r33[s]["kfd"][phase]
                                         for s in range(3)))
                            for phase in phases))
print("r34-kfd-medians", *(int(median(r34[s]["kfd"][phase]
                                         for s in range(3)))
                            for phase in phases))

for phase in phases:
    raw = [100 * (1 - r34[s]["kfd"][phase] / r33[s]["kfd"][phase])
           for s in range(3)]
    print(phase, "raw", *(f"{v:.6f}" for v in raw),
          "median", f"{median(raw):.6f}")
    print(phase, "r34-minus-r33-ns",
          *(r34[s]["kfd"][phase] - r33[s]["kfd"][phase]
            for s in range(3)))
    for reference in ("hsa", "hip"):
        adjusted = [
            100 * (1 - (r34[s]["kfd"][phase] / r34[s][reference][phase])
                   / (r33[s]["kfd"][phase] / r33[s][reference][phase]))
            for s in range(3)
        ]
        print(phase, reference, *(f"{v:.6f}" for v in adjusted),
              "median", f"{median(adjusted):.6f}")
    print(phase, "r34-kfd/hsa",
          *(f'{r34[s]["kfd"][phase] / r34[s]["hsa"][phase]:.6f}'
            for s in range(3)))
    print(phase, "r34-kfd/hip",
          *(f'{r34[s]["kfd"][phase] / r34[s]["hip"][phase]:.6f}'
            for s in range(3)))
for component in components:
    changes = [
        100 * (1 - r34[s]["kfd"][component] / r33[s]["kfd"][component])
        if r33[s]["kfd"][component] else 0.0
        for s in range(3)
    ]
    print(component, "raw", *(f"{v:.6f}" for v in changes),
          "median", f"{median(changes):.6f}")
PY

/usr/bin/find "$root" -depth -delete
trap - EXIT
```

## Claim limits

The paired comparison unit is a matching Latin slot, so all aggregate effects
have only `n=3`. The R33 and R34 revisions were separate runs, not an
interleaved cross-revision experiment. The 30 raw samples within a backend/slot
are time ordered and may be autocorrelated; they are not ordinal pairs across
revisions. All three slots share one host and one GPU, so they are not three
independent machines. HSA/HIP reference phases ran sequentially rather than
simultaneously with KFD. Ratio-of-ratios adjustment can reduce common slot
drift but cannot remove all temporal, thermal, compiler, or executable drift.

H2D, compute, D2H, and E2E are measured in the same iteration and are
correlated; the E2E and D2H changes are not independent evidence. The compute
component intervals are nested and non-additive. The historical toolchain
fields do not authenticate build compilers, HIP executable bytes differ, and
only HSA provides a byte-identical control executable.

No confidence interval, significance test, long-run variance bound,
multi-host result, throughput result, energy result, or workload-general result
is claimed. The archive supports the narrow observation that this exact R34
run directly exercised the fused asynchronous single-copy path and showed a
consistent D2H-concentrated E2E reduction. It does not prove that fusion alone
caused the full difference, establish HIP/HSA parity, or support an
orders-of-magnitude performance claim.
