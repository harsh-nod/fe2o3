# Matched Persistent-Hot Peer Copies

Development comparison of the KFD runtime facade, HIP, and HSA on MI300X.
This packet is not a HIP/HSA parity certificate, formal refinement, an exclusive
GPU reservation, or evidence of equivalent physical copy-engine scheduling.

## Results

All six processes completed successfully against signed source
`2db05385c9f558cb56a37bccad5b8ce5dd07ef00`, published to both remotes before
execution. The verifier replays 55 remote commands, 36 endpoint observations,
all source/toolchain/ELF identities, six result rows, collection, and cleanup.
CPU and native Rust/Cargo version outputs match exactly.

Each entry below is one process's median of thirty directional samples, not a
pooled statistic or a confidence interval. Times are microseconds.

| Trial | Forward p50 (us) | Reverse p50 (us) |
| --- | ---: | ---: |
| 1 KFD | 14327.907 | 14345.553 |
| 2 HSA | 29.904 | 29.955 |
| 3 HIP | 37.796 | 36.464 |
| 4 HIP | 37.776 | 36.374 |
| 5 HSA | 30.065 | 29.985 |
| 6 KFD | 14321.477 | 14297.872 |

This workload is **not at copy-performance parity**. Matching preparation,
guarded payloads, priming, depth, and hot repetition does not remove the large
end-to-end gap. These observations are not physical copy-engine throughput or a
statistical performance acceptance result. Raw p95 and rounded GB/s values are
retained in each process transcript and `remote/validated-results.json`.

The owned remote directory
`/home/harsh/fe2o3-xgmi-peer-hot-20260919.833aa05ff0e3f703` was removed only after
byte-exact collection; path and process absence were confirmed. The local source
transport was also removed. No foreign files or processes were changed.

## Next Attribution

Source inspection identifies two full pair-currentness observations per timed
KFD batch, each including fresh topology discovery. Additional publication
boundaries and bounded roster allocations also execute inside the timer. This is
a source-level explanation to investigate, not a measured breakdown of 14 ms.

The next step is phase-level host attribution, followed by measured scratch reuse
or an explicitly bounded multi-operation currentness scope. Silently caching
topology observations would weaken the existing freshness contract. The existing
whole-active-record scans are constant-sized at this benchmark's depth one and
must not be credited as the cause without attribution.

## Workload

- Physical GPUs 1 and 2, correlated by PCI address and unique ID before every
  process. HIP/HSA see them as ordinals 0 and 1 under explicit visibility masks.
- Process order: KFD, HSA, HIP, HIP, HSA, KFD. One MiB per direction, depth one,
  ten warmup pairs, thirty measured forward/reverse pairs per process.
- Separate persistent source/destination allocations for both directions,
  32-byte prefix/suffix canaries, and a payload starting at offset 32.
- Prepare both directions once, prime each direction once, then perform all
  warmups and samples without host buffer access. Validate full source and
  destination payloads and guards after the sequence; explicitly tear down.
- KFD: `--aggregate-peer-batch-hot-only`; HIP/HSA: `--persistent-hot`.
  Existing default modes and their output schemas are preserved.
- Timers cover each API's enqueue through observed completion. KFD includes
  its aggregate close; HIP synchronizes a stream; HSA waits and resets a signal.
  Setup, priming, final readback, and teardown are outside the measurements.
  Buffer preparation is matched, but driver mapping and engine policies are not
  asserted to be identical.
- Final readback checks the resulting buffers. It does not independently detect
  a skipped repeated copy after the priming copy has populated the destination.

## Qualification

The sealed companion CPU packet is
`../dev-xgmi-peer-hot-controls-cpu-2026-09-19`. It covers nine Rust example tests
under GNU, musl, and feature-off configurations; three shared C++ control tests
(including UBSAN); and two legacy argument tests. This benchmark-only change
does not renew qualification of the unchanged runtime library.

The native campaign binds the signed, published commit, exact CPU-tested source
map, payload hashes, three executable hashes, build commands, toolchains, raw
results, and collection receipts. Native builds use two Cargo jobs and a private
owned directory. No GPU reset, clock change, or foreign-process action is used.

Admission requires zero GPU and memory busy percentages in all three sysfs
observations, less than 512 MiB VRAM use, matching SMI identities, and no selected
GPU process attachments. Both endpoints are checked before each process, again
after two seconds, and again twenty seconds after the settled checks complete.
These are sampled shared-host observations, not an exclusive reservation.
Receipt chronology is checked separately on each host; their wall clocks are not
assumed synchronized. Benchmark durations use each process's monotonic clock.

All remote evidence must be collected and hash-checked before exact-owned cleanup.
Cleanup and process/path absence are recorded. Failed attempts are retained, not
overwritten or relabeled as successful.

## Reproduction

The first attempt stopped locally at source packaging, before remote creation.
Its complete failed receipts and original tooling are preserved in
`../dev-xgmi-peer-hot-mi300x-2026-09-19-preflight-failure`. The corrected packer
omits absent optional selectors and still checks exact signed source-map equality.

Run from a clean signed commit pushed to both configured remotes:

```sh
python3 -I -B docs/evidence/dev-xgmi-peer-hot-controls-cpu-2026-09-19/cpu.py --live
python3 -I -B docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/test_native.py
python3 -I -B docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/test_results.py
python3 -I -B docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/test_campaign.py
python3 -I -B docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/campaign.py
python3 -I -B docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/verify.py --seal
```

`campaign.py` refuses to overwrite an existing attempt. A new hardware campaign
requires a new packet location and reviewed protocol, not deletion of prior raw
evidence. `verify.py` without `--seal` replays this recorded archive and seal;
it does not rerun the GPU workload. Any later results must retain the API-boundary
and shared-host caveats.
