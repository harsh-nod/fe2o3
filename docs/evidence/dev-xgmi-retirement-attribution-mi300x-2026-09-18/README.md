# XGMI Retirement And Host Attribution Experiment

Completed bounded native experiment. Production bytes equal the CPU-qualified
source in signed commit `54b75b774fbbb510a0480646535835b7431cda67` and the sibling
`dev-xgmi-retirement-cpu-2026-09-18` archive. The signed launch/tooling commit is
`84b61ee39817cee8bfe3cd68312f0a05543bc9af`. The earlier prepared host-attribution
packet remains unchanged and does not qualify this revision. Native execution
is validated; formal refinement and performance acceptance are not claimed.

One optimized `hardware-diagnostic` binary runs four separate processes in the
predeclared order off, on, on, off. Each uses the same two physical device UIDs,
1 MiB, depth 1, ten warmups and thirty samples, forward then reverse. The two
off processes have no diagnostic records; the two on processes must each have
exactly 162 submissions, backend IDs 7 through 168, with one submit and one
completed poll per ID and zero or more intervening Pending polls. All records
have contiguous ordinals, exact directional UIDs, valid bounded host durations,
and complete explicit shutdown. The final aggregate rows bind those controls
and their diagnostic mode. Healthy shutdown exercises both directional native
XGMI retirement paths added in the qualified source revision.

Physical GPUs are selected from the pinned eight-device roster, then both must
pass fresh strict endpoint admission before every workload. Both endpoints are
observed at least two seconds after process closure and again at least twenty
seconds after both settled checks. Observations are not a reservation or proof
of continuous isolation. Other host, device and interconnect activity, unpinned
CPU/NUMA placement, fixed phase order and thermal drift may affect the results.

The controller authenticates the qualified source, toolchain outputs, exact
commands, private marked directory, payload and unchanged binary. It uses two
build jobs, disables core dumps, bounds each process group, collects and rehashes
all remote results before deleting only its owned directory, then checks path
and process absence independently. A failed or incomplete run cannot pass
acceptance. Uncollected failure data is retained at the recorded exact path.
The original native exception wins later settlement/finalization errors, which
are recorded separately. The outer native-control timeout exceeds the sum of
the declared nested command bounds; individual workloads are capped at 120
seconds. Protocol/controller calibration comprises 21 synthetic CPU tests;
the existing host observer has nineteen tests.

All three launch/verification scripts and `.gitattributes` must match their
containing signed commit before any run directory is created, and acceptance
rechecks those commit-tree blobs. A recorded ancestry check connects that commit
to the qualified source commit. The verifier authenticates the campaign hash
before importing it; the campaign in turn pins its shared runner. Historical
local commands and cwd remain bound to the recorded execution root when the
verifier is run from another checkout. This README is descriptive, not an
execution input, and may be finalized after collection before sealing.

## Observed Result

The run used physical GPUs 1 and 2, UIDs `ab83d2ffef0d3cdf` and
`d2e26fef80cf5c33`, at BDFs `0000:26:00.0` and `0000:46:00.0`. The host
reported kernel `6.8.0-124-generic` and ROCm `7.2.4`. All 33 remote command
receipts passed, including 24 admitted endpoint observations. All four workload
processes passed byte/canary checks and explicit teardown. Each enabled process
had exactly 162 submit/completed pairs, no Pending records, and backend IDs
7 through 168. First-observed-poll completion is not a GPU latency measurement.

The one executed binary SHA-256 is
`add6984d829e2416537b95904317d9fa3c786b2c5d5ae6e64586911ebdfa4389`.
Remote results were collected and rehashed before the private build directory
was removed. Separate path/process-absence checks passed; the local payload was
also removed. Cleanup targeted only controller-owned resources.

Facade p50 latency in milliseconds (30 samples per direction/population):

| Phase | Remap forward | Remap reverse | Hot forward | Hot reverse |
| --- | ---: | ---: | ---: | ---: |
| off1 | 207.180 | 207.400 | 118.020 | 118.063 |
| on1 | 207.081 | 206.959 | 117.754 | 117.901 |
| on2 | 207.160 | 206.901 | 118.285 | 118.263 |
| off2 | 208.871 | 209.277 | 119.126 | 118.984 |

Measured lower-call populations, each 30 copies. Currentness share is the ratio
of summed currentness time to summed lower-call total time, not a ratio of
percentiles. The last column is the p50 of each copy's combined native-host
publication and fence-observation intervals, not device time.

| Phase | Population/direction | Lower-call p50 ms | Currentness share % | Native-host p50 us |
| --- | --- | ---: | ---: | ---: |
| on1 | remap-sample/forward | 117.968 | 99.99017 | 5.338 |
| on1 | remap-sample/reverse | 118.053 | 99.99025 | 5.348 |
| on1 | hot-sample/forward | 117.726 | 99.99079 | 5.338 |
| on1 | hot-sample/reverse | 117.873 | 99.99058 | 5.499 |
| on2 | remap-sample/forward | 118.041 | 99.99044 | 5.057 |
| on2 | remap-sample/reverse | 118.144 | 99.99015 | 5.178 |
| on2 | hot-sample/forward | 118.253 | 99.98994 | 5.268 |
| on2 | hot-sample/reverse | 118.230 | 99.99069 | 5.258 |

Currentness dominates these measured host intervals. It includes more than
just topology discovery, so this experiment does not isolate sysfs or discovery
cost. Source inspection identifies four full topology discoveries per Full
pair-validation boundary, eight per lower submit or poll. With no Pending
records, that is sixteen source-derived discoveries within each measured
submit/completed-poll pair, not a separately measured discovery counter. Other
lifecycle checks may perform additional discoveries outside those intervals.

A candidate follow-up is one fresh pair-level snapshot per validation boundary,
preserving both endpoints' full pre/post observations and retained comparisons.
That would reduce discovery calls, without reusing a snapshot across independent
calls. It still needs implementation, hostile tests, refinement work, and a new
matched experiment; no latency gain is established here.

## Interpretation

Summaries separate remap/hot, warmup/sample/prime, direction and call type. The
measured populations contain thirty copies per direction; primes are individual
values, not percentile populations. Per-copy sums and Pending counts avoid
overweighting copies that needed more polls. No per-sample facade durations are
emitted, so aggregate facade percentiles cannot be joined to individual
diagnostic records. Subtracting percentiles is not a valid stage decomposition.

Every interval is host time, including `native_call_ns`, which measures host
publication or fence observation, not GPU copy execution. Runtime mapping,
first queue creation, loop work, waiting/backoff and parts of settlement lie
outside these lower-call intervals. Timer overhead and delayed validation are
part of the enabled path. Off/on differences are descriptive perturbations,
not a causal overhead estimate or confidence interval.

The persistent-hot byte oracle checks unchanged data after the full sequence;
it cannot by itself detect a skipped intermediate copy. Remap checks rewritten
data each round. Healthy execution and settled cleanup do not validate native
retirement-failure paths, Rust/native formal refinement, high-depth/concurrent
behavior, matched HIP/HSA performance acceptance, or general parity.

## Commands

Recorded launch (one-shot; the existing packet is not overwritten):

```sh
python3 -I docs/evidence/dev-xgmi-retirement-attribution-mi300x-2026-09-18/test_campaign.py
python3 -I docs/evidence/dev-xgmi-retirement-attribution-mi300x-2026-09-18/campaign.py run 1 2
python3 -I docs/evidence/dev-xgmi-retirement-attribution-mi300x-2026-09-18/verify.py --allow-unsealed --summary
```

Launch-time tools stay frozen. The final seal includes this result report and
all collected artifacts. To check that seal, omit `--allow-unsealed` from the
verification command. Offline verification does not rerun workloads or grant
parity/profiling authority.
