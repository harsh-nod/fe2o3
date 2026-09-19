# XGMI Retirement And Host Attribution Experiment

Prepared experiment; no native result is claimed until the complete verifier
accepts a collected run. Production bytes must equal the CPU-qualified source
in signed commit `54b75b774fbbb510a0480646535835b7431cda67` and the sibling
`dev-xgmi-retirement-cpu-2026-09-18` archive. The signed containing commit at
launch is recorded separately. The earlier prepared host-attribution packet
remains unchanged and does not qualify this revision.

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

Launch once from a signed commit with exact qualified bytes and a newly
observed-free pair:

```sh
python3 -I docs/evidence/dev-xgmi-retirement-attribution-mi300x-2026-09-18/test_campaign.py
python3 -I docs/evidence/dev-xgmi-retirement-attribution-mi300x-2026-09-18/campaign.py run GPU0 GPU1
python3 -I docs/evidence/dev-xgmi-retirement-attribution-mi300x-2026-09-18/verify.py --allow-unsealed --summary
```

Launch-time tools stay frozen. After independent artifact review, finalize this
summary, create the non-overwriting seal, and verify it. Offline verification
does not rerun workloads or grant parity/profiling authority.
