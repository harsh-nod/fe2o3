# Single-Packet KFD Copy Engine Diagnostic

All eight benchmark processes completed successfully on shared MI300X GPU 1,
UID `0xab83d2ffef0d3cdf`, PCI `0000:26:00.0`, on 2026-09-17. This is a small
engine-policy diagnostic, not HIP/HSA parity acceptance, a device-timeline
measurement, or reproduction of the earlier 256 MiB public-facade workload.

## Method

The unchanged `kfd-sdma-copy-benchmark` was built from committed
`04d9f3ca37cd17536901d4d7cab405bf06f54454` in a private remote checkout. The
post-build source manifest contains 5,434 selected files. The benchmark binary
and `/usr/bin/numactl` were hashed, and all captured source/binary identities
were checked before and after the campaign. Toolchain and build output are in
`raw/prepare.log`; this is not a pre-build source-identity interval claim.

Each process used 4,194,272 bytes (one maximum-sized native SDMA copy packet),
depth 1, three warmups and ten samples, first split submission/wait and then
combined submission/wait. Both directions used the selected engine. Four pairs
were ordered engine0/engine1, engine1/engine0, engine0/engine1, engine1/engine0.
All allocations used the same KFD coherent GTT host profile and the same
requested CPU affinity 0-47 and memory policy node 0. Physical page residency
and cache attributes were not measured.

The producer validates complete returned buffers before accepting each round.
Its 10,000 pool checkout/recycle pairs run afterward, outside copy timing.
Only per-process p50/p95 summaries are emitted; individual sample arrays were
not retained. Ratios below are medians of four paired process-p50 ratios, not
pooled sample statistics or confidence intervals.

Before and after every process, admission required the exact UID/BDF, zero
reported utilization, less than 512 MiB VRAM and no mapped GPU 1 PID. These are
point-in-time observations, not continuous isolation or an exclusive reservation.
GPU 0's existing allocation was left alone. The user authorized use of free
devices; no reservation was requested.

## Results

Ratios greater than one mean engine1 had higher host-observed latency.

| Measurement | Median Engine1/Engine0 | Four Paired Ratios |
| --- | ---: | --- |
| Split H2D p50 | 1.0084 | 1.0160, 0.9946, 1.0199, 1.0009 |
| Combined H2D p50 | 1.0069 | 1.0229, 0.9979, 1.0159, 0.9973 |
| Split D2H p50 | 1.4551 | 0.5187, 1.9291, 0.9951, 1.9152 |
| Combined D2H p50 | 0.9996 | 0.5061, 1.0059, 1.0000, 0.9993 |

Switching the requested engine did not demonstrate an upload improvement in
this diagnostic. Download ordering is unstable: split process medians vary
between approximately 114 and 220 microseconds. This does not establish a
stable engine winner or justify changing production directional placement.

Source review identifies a plausible confound, not a measured cause. Both
lower split and combined waits use the ordinary batch wait's 64 spins, 16
yields and requested 25/50/100/... microsecond sleeps, capped at 1 ms:
`crates/fe2o3-kfd/src/wait.rs` and `sdma.rs`. No pause counts, actual sleep
durations or GPU timestamps were recorded. Split and combined also have
different request/owner timing boundaries and currentness envelopes, and run
in separate loops; their difference is not a pure wait-policy measurement.

The earlier facade benchmark is materially different. It requests at most
50 microseconds per native wait, and the directional window wait has a
50-microsecond active-spin floor clamped to that deadline. That path cannot
request a positive backoff sleep within the slice. Consequently this lower
benchmark observation does **not** explain the earlier 256 MiB facade gap.
The next controlled experiment should measure pause behavior under fixed wait
policy; explicit HSA engine/pool selection remains a separate investigation.

## Validation And Cleanup

`summarize.py` authenticates and executes the exact bytes of the repository's
existing parser (SHA `03f798a9f94b88359a5e9ec309d1b74abb495afdc1a728d82303da465019c60b`).
It checks exact context, engine identity, all eight ordered phases, 17 admission
markers, successful completion/postflight records, positive metrics, the final
identity/occupancy result and a closed successful command record. The corrected
self-test accepts the real record and rejects twelve altered records.

The initial `summary-self-test` exit 1 is preserved along with
`summarize-preliminary.py`. Its duplicate-field negative was correctly rejected
by the shared parser, but the self-test did not catch that parser's `CheckError`
class. The correction only adds that expected exception to the self-test's
catch list; it does not relax production parsing or edit benchmark data.
`summary-self-test-corrected` passes, and the initial/final summary outputs are
byte-identical. No GPU rerun or discarded measurement resulted from this issue.

The owned directory `/home/harsh/fe2o3-engine-diagnostic-20260917.vNTF2iSX`
was removed at 19:28:42 UTC after evidence collection and live executable/cwd
checks. It occupied 236 MiB. No foreign job was signaled and no foreign files
were removed. The later observation records GPU 1 back at its original
298,647,552-byte VRAM baseline, zero utilization and no mapped PID.

`SHA256SUMS` seals this archive. The separate
[reader commit qualification](../dev-v4j3-reader-commits-2026-09-17/README.md)
does not derive native correctness or performance from these measurements.
