# Aggregate XGMI Host Attribution

Status: all four native trials completed, collection replay passed, and owned
remote/local cleanup was confirmed. This packet follows the persistent-hot comparison in
`dev-xgmi-peer-hot-mi300x-2026-09-19`, which measured roughly 14.3 ms KFD versus
30 us HSA and 37 us HIP for a one-MiB peer copy.

## Results

Source commit: `b44409e2fe94c547c887b64db9dedd3d90b2c5f5`, signed and published
to both remotes before execution. The KFD ELF SHA-256 was
`8d0ab80661955fad30a5b1a8cbbb00197affc08ef81def53a19149482dfbaaa7`, unchanged
through all four trials. The CPU seal SHA-256 is
`e3e7cdded8dd13525430f7eaa2f3fff1025d278bfdda87580ea30ddac1e3199a`.

One MiB, depth one, ten warmups and thirty measured samples per direction:

| Trial | Diagnostic | Forward Facade p50 (ms) | Reverse Facade p50 (ms) |
| --- | --- | ---: | ---: |
| 1 | off | 14.325887 | 14.306048 |
| 2 | on | 14.265117 | 14.285578 |
| 3 | on | 14.287971 | 14.308881 |
| 4 | off | 14.321481 | 14.287489 |

Both instrumented trials contain the exact 82-record roster. The table below
uses only their 120 measured calls, with both directions equally represented.
Each mean is the arithmetic mean of raw nanoseconds; each share is the sum of
that phase divided by the sum of backend totals over the same calls.

| Host Phase | Mean (us) | Share of Backend Total |
| --- | ---: | ---: |
| Admission/validation | 2.134 | 0.015% |
| Preparation | 0.355 | 0.002% |
| Full opening currentness | 7067.333 | 49.423% |
| Submission | 84.137 | 0.588% |
| Waiting | 81.154 | 0.568% |
| Full closing currentness | 7062.030 | 49.386% |
| Settlement | 1.672 | 0.012% |
| Unattributed gaps/clock overhead | 0.932 | 0.007% |
| Backend total | 14299.747 | 100% |

Opening plus closing account for 98.8085% of measured backend time. Submission
plus waiting is about 165.291 us and includes its own validation; it is not an
SDMA/device-duration measurement. The four priming calls took 278.995-279.958 ms
and are excluded, as are all forty warmup calls. On/off facade medians remain
near 14.3 ms in this small shared-host sample; no causal instrumentation-speedup
or zero-overhead claim follows from that observation.

The next performance target is the full fresh-currentness implementation, not
additional depth-one admission tuning. Fixed-schema parsing can remove redundant
allocations without eliding observations, but these measurements do not attribute
time to individual syscalls or parsers and do not predict a specific speedup.

The accepted packet contains 35 native command receipts, 24 endpoint observations
(72 raw sysfs snapshots), and 15 local controller receipts. Complete collection
preceded removal of
`/home/harsh/fe2o3-xgmi-aggregate-attribution-20260919.d38d1d6699f4f3d9`;
owned process/path absence and local payload removal are recorded. The earlier
interrupted attempt is preserved separately in
`dev-xgmi-aggregate-attribution-mi300x-2026-09-19-interrupted-2026-09-20` and is not
included in these results.

## Protocol

- One signed source tree, qualified on GNU and musl, published to both remotes.
- One KFD ELF built with `hardware-diagnostic`; identical executable and inputs
  for diagnostic off, on, on, off. One MiB, depth one, ten warmups and thirty
  measured samples in both ordered directions.
- Adapt the prior controller and complete collection verifier, and reuse its
  authenticated lifecycle tests and endpoint observer. Historical packets remain
  unchanged. Source, ELF, payload, toolchain, command, and process receipts remain
  bound to the signed source. The new runner and parser validate the new mode.
- Shared MI300X: no exclusive reservation. Every trial requires fresh identity,
  zero GPU/memory activity, bounded VRAM, and no selected-device processes.
  Postflight checks run after two seconds and again after twenty seconds. Never
  reset GPUs, kill foreign work, or delete outside the marked owned directory.
- Complete byte-exact collection precedes cleanup. Verify owned path/process
  absence and remove the owned local transport directory. An incomplete or failed
  campaign is preserved and cannot certify native success.

## Interpretation

Enabled trials must contain exactly 82 successful call observations: two prime,
twenty warmup, and sixty measured calls. Ordinals 0..81 map to backend submission
IDs 7..88, alternating the two selected device identities. Each row contains all
seven nonoverlapping host phases and a checked total at least their sum. The
diagnostic capture is preallocated and extracted only after successful runtime
and native teardown. Rejected shapes, pending work, retries, errors, incomplete
timing, or terminal cleanup invalidate the entire capture without changing the
workload result.

The phase total covers backend aggregate progress, not the entire facade timer.
It excludes facade enqueue/settlement/release outside that call. Phase timings
are not device execution times, authenticated profiler events, or authority to
complete or release resources. Comparisons of diagnostic-on and off runs are
descriptive perturbation checks, not statistical speedup claims. Repeated payload
and final canary checks do not independently detect every skipped post-prime copy.

This packet makes no performance-acceptance, full HIP/HSA parity, exclusive-host,
or formal machine-code-refinement claim. Existing currentness and fail-closed
custody checks remain enabled. CPU equivalence tests cover submit/wait/close
outcomes, original deadlines, custody pointer order, and panic identity. Native
dual-recorder enable rejection and mixed ordinary/aggregate invalidation do not
yet have dedicated executable coverage.

## Commands

Run `python3 -I -B campaign.py` only from a clean, signed and dual-published source
tree with a completed live-matching CPU packet. Then run
`python3 -I -B verify.py --seal` followed by `python3 -I -B verify.py`. The verifier
authenticates the signed tooling and full CPU archive before importing their
code, rederives observations from raw output, checks exact artifact closure, and
checks the immutable SHA-256 manifest.
