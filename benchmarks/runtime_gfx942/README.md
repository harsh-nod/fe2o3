# gfx942 Runtime Qualification and Timing

This harness executes the repository-owned VecAdd qualification fixture through
the public fe2o3 runtime context over the direct KFD and reviewed HSA backends,
then through HIP's module API. All three paths load the same SHA-pinned COV6
HSACO, use 1,048,576 `f32` elements, grid `[1048576, 1, 1]`, workgroup
`[256, 1, 1]`, and the exact input images defined by
`trusted-gfx942-vecadd-v1/policy-v1.txt`. The size and geometry are not
configurable because the qualification-only constructor admits only that
invocation.

Run on an MI300X host with ROCm 7.2.4 and Rust 1.94 or newer:

```sh
benchmarks/runtime_gfx942/run-mi300x.sh
```

`FE2O3_RUNTIME_GPU_INDEX` selects the HIP device and the script resolves the
same device's KFD unique ID. The defaults are 10 warmups, 30 samples, and 10
launches per sample. `FE2O3_RUNTIME_WARMUPS`, `FE2O3_RUNTIME_SAMPLES`, and
`FE2O3_RUNTIME_LAUNCHES_PER_SAMPLE` may change only those statistical controls.
Every temporary build artifact is placed under one unique directory and
removed by an exit and signal trap. The runner requires a clean checkout so the
printed Git commit identifies every host source and checked fixture byte.

## Measurement Scopes

The output is a line-oriented `key=value` stream. Percentiles are computed over
per-sample average microseconds.

| Metric | Included work | Comparable use |
| --- | --- | --- |
| KFD `qualified_materialize_submit_wait_readback` | Exact output reset, typed `RuntimeContextV1` launch admission, qualification-gate content scan, three-buffer GTT materialization, AQL submit/wait, submission release, native writeback, and facade readback | Direct-KFD end-to-end qualification cost only |
| HSA `host_visible_submit_wait_readback` | Exact output reset outside timing, typed `RuntimeContextV1` launch, persistent host-visible buffers, host wait, submission release, and facade readback | Reviewed-HSA end-to-end host-visible path; not equivalent to KFD or HIP staging |
| HSA `synchronized_launch_wait` | Persistent host-visible buffers and facade launch/wait/release; exact output reset occurs before timing | Compare cautiously with the HIP row: HSA creates and releases a kernarg allocation and completion signal per submission |
| HIP `staged_submit_wait_readback` | Exact output reset, three host-to-device copies, module launch, output device-to-host copy, and stream synchronization | Staged end-to-end oracle; not equivalent to KFD allocation policy |
| HIP `synchronized_launch_wait` | Persistent initialized buffers and host launch-to-stream-synchronize latency; exact output reset occurs before timing | Host launch/wait comparison with persistent-buffer runtimes |
| HIP `device_event_interval` | One exact module launch between HIP device events; reset and validation occur outside the event interval | GPU execution interval only |

One untimed KFD launch records, waits, and releases a runtime event to cover the
typed event lifecycle without charging event management to every timed launch.
The HSA lane additionally runs real six-event cross-stream fan-in and enough
sequential submissions to wrap its 64-slot queue.
The KFD path then checks every output byte after warmup and every sample. The
HSA and HIP paths check every output element after every sample; HIP also checks
the device-event launch.

One-time fixture admission, module resolution, allocation, deterministic host
input construction, compilation of the host harness, and final cleanup are not
timed. Normal KFD exit explicitly releases all submissions, allocations,
module and stream handles, consumes `RuntimeContextV1::shutdown`, and tears down
the native KFD queue.

Record GPU load and competing processes with the results. Measurements taken
under unrelated device load are diagnostic observations, not release numbers.
Only the HSA/HIP `synchronized_launch_wait` rows have similar timing boundaries,
and their allocation/signal policies still differ. Do not compute ratios across
the other metric names. These rows do not establish general HIP/HSA parity:
they cover one artifact, one geometry, one device, and the exact software stack
printed by the runner.
