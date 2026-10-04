# VecAdd fe2o3/HIP comparison

`run-mi300x.sh` compares the production typed fe2o3 VecAdd with a direct HIP
translation. Both implementations use one output element per work-item,
256-work-item blocks, identical FP32 inputs, the same allocation sizes, HIP
events, untimed warmups, repeated launch batches, and full result validation
after timing.

Run on a `gfx942` host with:

```sh
benchmarks/vecadd_hip/run-mi300x.sh
```

The defaults use 16,777,216 elements, 20 warmups, 30 samples, and 100 launches
per sample. Override them with `FE2O3_VECADD_N`, `FE2O3_VECADD_WARMUPS`,
`FE2O3_VECADD_SAMPLES`, and `FE2O3_VECADD_LAUNCHES_PER_SAMPLE`.

The event interval is an end-to-end dispatch-path measurement, not a pure GPU
kernel duration. The current safe fe2o3 launch consumes its prepared value and
waits for quiescence before releasing Rust borrows. The HIP loop queues launches
asynchronously. That host-policy difference is intentionally visible in this
number.

Use the kernel trace for an execution-only comparison:

```sh
benchmarks/vecadd_hip/profile-mi300x.sh
```

The profile script first generates the fe2o3 artifact through the production
compiler, then uses `rocprofv3` to report GPU dispatch durations for that exact
HSACO and the HIP translation. Temporary binaries and profiler databases are
removed on exit.

One MI300X observation on 2026-08-22, after enabling `-O2` and rejecting
register-spilling typed artifacts, measured:

| Measurement | fe2o3 | HIP | Interpretation |
| --- | ---: | ---: | --- |
| GPU dispatch average, 105 launches | 42.52 us | 45.42 us | Kernel execution was at parity within run-to-run noise. |
| Event interval median, 30 x 100 launches | 67.38 us | 47.34 us | fe2o3's synchronous safe launch path was 1.42x slower. |

The optimized fe2o3 ISA has zero SGPR/VGPR spills. It performs three independent
slice bounds checks where the HIP kernel checks one shared length; otherwise the
load/add/store path is equivalent.

Neither command measures compilation, allocation, or copies. These results do
not claim tiled-GEMM performance. A direct tiled-GEMM comparison must wait until
the general Rust GEMM source-to-HSACO route is production-connected rather than
fail-closed.
