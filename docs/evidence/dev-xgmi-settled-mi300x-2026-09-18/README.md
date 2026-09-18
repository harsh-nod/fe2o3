# Settled XGMI Diagnostic

Healthy execution and settled cleanup passed at signed source commit
`31ebb8fc4abea542055b85cdce707c36d94e758d`. This separately declared retry does
not reinterpret the rejected creation-root campaign. Its 5,562 source files
match the sealed CPU creation-root packet. All source files, local control tools,
uploaded payload and three optimized benchmark executables are hash-bound.
The earlier rejected archive is unchanged.

## Observed Results

All three processes exited successfully and produced the required four rows.
All 18 endpoint observations passed: both GPUs were idle, below the VRAM bound,
and had no mapped KFD process. Every settled observation began at least two
seconds after workload closure; every delayed observation began at least twenty
seconds after both settled endpoint checks completed. These are observations,
not an exclusive reservation or an instantaneous reclamation claim.

Descriptive p50/p95 host latency in microseconds (see interpretation limits below):

| Path | Forward p50 | Forward p95 | Reverse p50 | Reverse p95 |
| --- | ---: | ---: | ---: | ---: |
| KFD remap per round | 206606.280 | 207943.828 | 206723.846 | 207622.384 |
| KFD persistent hot | 117961.380 | 118927.523 | 117999.166 | 119113.961 |
| HSA | 29.995 | 31.547 | 29.944 | 30.406 |
| HIP | 42.904 | 43.695 | 42.794 | 43.756 |

The controller collected and rehashed all 92 remote result files, including
29 successful process receipts, before deleting its exact marked directory
`/home/harsh/fe2o3-xgmi-settled-20260918.f67e49c338f865df`. Independent verification
confirmed path/process absence; the local payload was also removed. The complete
offline verifier and all 12 controller calibrations passed independent review.
The required 19 host-observer CPU tests also passed before launch.

This packet does not contain the subsequently developed host-stage instrumentation.
Repeated topology validation is a source-level performance hypothesis, not a
measured attribution established by these four rows.

## Declared Workload

Use physical GPUs 1 and 2 only when fresh strict preflight admits both endpoints.
Authenticate unique IDs `ab83d2ffef0d3cdf` and `d2e26fef80cf5c33`, PCI addresses
`0000:26:00.0` and `0000:46:00.0`. KFD selects UIDs directly. HSA/HIP mask physical
indices 1 and 2 and authenticate their visible ordinals 0 and 1 against the UIDs.

Each backend runs 1 MiB copies at depth 1, ten warmups and thirty samples, forward
then reverse. Backend order is KFD, HSA, HIP. Require four complete output rows:
KFD remap-per-round, KFD persistent-hot, HSA and HIP. Each process has a 120-second
bound and must close its owned process group. Depth 16 and concurrent execution
are deliberately not qualified by this diagnostic. The prior KFD depth-1 process
took 43.13 seconds; that motivates a bounded first comparison, not a linear-scaling
claim or a reduction of the overall runtime parity objective.

## Observation Policy

Require a strict observer on both GPUs before each workload. After workload process
closure, wait at least two seconds before starting the first settled postflight
observer, then check the second GPU. After both settled observations complete,
wait at least twenty more seconds before starting the delayed observations.
Apply this same policy to all three backends. Both lower timing bounds are checked
against recorded timestamps within the remote clock domain. These are settled,
not immediate, postflight checks; instantaneous reclamation is not established.

Every observer must report zero GPU use, less than 512 MiB VRAM and no mapped KFD
process on the selected endpoint. Offline verification replays the underlying
sysfs and SMI captures through SHA-pinned parsers. The user permits observed-free
devices; there is no exclusive reservation or continuous isolation claim. CPU/NUMA
affinity is not pinned, and other host/device/interconnect work can affect timing.

Use a fresh private marked directory under `/home/harsh`, two Cargo build jobs,
core dumps disabled and bounded owned process groups. Preserve first failure while
attempting both settled and delayed endpoint observations. Collect and rehash the
complete result inventory before deleting only the owned directory, then check
path/process absence independently. Leave uncollected failures at their recorded
path for recovery; remove the local payload only after remote cleanup closure.

## Interpretation

This is healthy-path correctness plus diagnostic API timing, not performance
acceptance or formal Rust/native refinement. KFD uses guarded 32-byte-offset
buffers and one ordered SDMA engine. HSA/HIP use aligned exactly sized buffers
and unpinned engine scheduling. KFD's persistent-hot loop primes once and checks
unchanged data after the whole sequence; HSA/HIP rewrite and validate each round
outside timing. The final KFD byte oracle cannot detect a skipped intermediate
copy by itself. KFD remap-per-round is not matched to the HSA/HIP setup lifetime.
Timing ratios remain descriptive under these differences and fixed backend order.

The XGMI retirement failure-path custody gap remains open. Healthy shutdown does
not qualify native failure injection, full memory/stream/event behavior, broad
language support, high-depth behavior, multi-device parity, or general HIP/HSA
parity. Twelve controller calibrations and nineteen CPU host-observer tests are
required before launch. Full offline verification depends on the sibling CPU
archive and pinned observer parsers in the earlier LogicalMux native archive.

```sh
python3 -B docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/verify.py
```

Verification reads evidence; it never reruns native workloads or emits a parity
verdict. A successful packet must include the full four-row roster and cleanup.
