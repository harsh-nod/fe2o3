# Debugger and profiler qualification V1

Status: caller-bound baseline and candidate policy contract for GitHub issue
#215. This record does not satisfy the T0 or T6 exits by itself.

The production schema is `fe2o3-debug-qualification-manifest-v1` in
`fe2o3-debug-protocol`. The checked-in MI300X record is
`crates/fe2o3-debug-protocol/tests/fixtures/mi300x-qualification-v1.json`.
It is intentionally separate from the frozen simulator and live-debugger wire
schemas.

## Authority boundary

The manifest is caller supplied and inert. It can retain exact content,
version, configuration, environment, and evidence identities, but decoding it
does not authenticate the process that collected those identities. Its local
statuses therefore say `caller_bound_observed`, never `observed`. Documentation
claims and unavailable states are separate variants. The APIs report that the
manifest grants neither observation nor qualification authority.

No typed field interprets text as an executable path, argv, PID, native
address, descriptor, queue token, or state-changing operation. Free text may
mention any of those things, but it grants no authority. ROCgdb and rocprof
tools remain collectors outside this admission crate. Production runtime
authority remains pure KFD.

The protocol library does not launch tools or parse terminal output. The
checked-in record is an archived caller-supplied projection of separately run
version/probe commands; strict admission checks its shape and identities, not
the identity of the process that produced those bytes.

## MI300X baseline

The 2026-08-30 record binds Linux `6.8.0-124-generic`, one gfx942/wave64 KFD
topology record, and exact content/configuration identities for:

| Component | Caller-bound local installation record | Capability boundary |
| --- | --- | --- |
| fe2o3 native KFD debugger | usable executable identity | KFD device/queue/publication only; no native wave/register/source/memory claim |
| fe2o3 ROCgdb/KFD debugger | usable workflow identity | exact run reached KFD publication; ROCgdb stopped state was unavailable |
| ROCgdb | `rocm-rel-7.2-93`, GDB 16.3 | native debugging is documentation-only in the task matrix unless an exact fe2o3 stop is admitted |
| rocprofv3 | 1.1.0, git `97f5574f`, ROCm 7.2.4 | counter/PC/ATT collection is documented; no new complete capture is claimed |
| ROCprof Compute Viewer/ATT | entrypoint content observed, initialization unusable | version probe failed because required Python dependencies are absent; decoded ATT remains unavailable locally |
| HIP | HIP 7.2.53211, clang 22, ROCm 7.2.4 | representative compiler driver only; no comparative debug study was run |
| Mojo | unavailable | documentation describes a CUDA-GDB/NVIDIA path; no AMD or local parity claim |

The configuration identities bind these canonical labels, in matrix order:

```text
fe2o3-native-kfd-debugger-v4:mi300x:gfx942:wave64:structured-jsonl
fe2o3-rocgdb-kfd-debugger-v4:mi300x:gfx942:wave64:structured-mi
rocgdb:rocm-7.2.4:structured-mi
rocprofv3:rocm-7.2.4:structured-output
rocprof-compute:rocm-7.2.4:version-probe
hipcc:rocm-7.2.4:gfx942
mojo:unavailable
```

These labels aid reproduction; their hashes are identities, not signatures or
proof of who ran a probe.

The candidate comparator separately binds `raw-production-baseline:v1` and
`no-capture:v1`. Capture-mode configuration identities must be pairwise
distinct and cannot collide with the raw baseline identity.

## Candidate budgets

The six required modes have candidate, not approved, policies. Relative
duration policies use median workload duration; debugger-stop uses p95
stop/resume control latency so intentional stopped time is not mislabeled as
profiler overhead. All require 5 warmups and 30 measured repetitions.

| Mode | Candidate ceiling | Storage ceiling | Current result |
| --- | ---: | ---: | --- |
| no capture | 1% duration | 0 | not measured |
| counters | 10% duration | 1 GiB | not measured |
| PC sampling | 20% duration | 2 GiB | not measured |
| ATT | 100% duration | 4 GiB | not measured |
| debugger stop | 500 ms control latency | 64 MiB | stopped state unavailable |
| instrumented | 100% duration | 1 GiB | not measured |

These are declared candidate ceilings, not evidence that any mode meets them.
A supplied measurement must bind exact workload, input, artifact, environment,
device, collector content, baseline configuration, and captured configuration
identities plus the domain-separated canonical baseline-comparator identity.
That comparator fixes distinct raw and no-capture configurations and exact
caller-bound workload, input, artifact, environment, device, collector,
evidence, clock, repetition, and duration records. The no-capture measurement
binds raw execution as its baseline; every other mode binds the admitted
no-capture record, preventing arbitrary or swapped baselines. Warmups,
repetitions, clock domain, duration statistic, storage, collection time, loss,
and truncation are mandatory. Only manifest-level evaluation is public, and it
revalidates the complete current manifest before checking policy satisfaction.
It never upgrades caller-supplied bytes into an authenticated qualification.

Comparator availability is consistent only when the no-capture row is itself
measured and exactly matches the comparator axes, configurations, raw and
no-capture evidence, statistic, clock, warmups, repetitions, and durations.
The canonical no-capture baseline must be loss-free and non-truncated. All
other measured modes must use treatment evidence distinct from both the raw
and no-capture evidence identities. An unavailable comparator with unavailable
modes remains a valid, explicit qualification state.

T0 still needs an authenticated producer and archived query acceptance without
a source checkout. T6 still needs approved budgets, real measurements, broader
target/reset/loss qualification, and committed task-based usability studies.
CPU performance prediction remains outside scope.
