# KFD Copy Progress: Interrupted MI300X Diagnostic

**The planned campaign did not complete and is not a performance qualification.**
The source was signed commit `3e12ef82bbb41fb116afbb7ddf7cffdad735ec7d`, built
against the actual ROCm 7.2.4 HSA runtime and native KFD on shared `mi300x`.
The [CPU qualification](../dev-kfd-copy-progress-cpu-2026-09-18/README.md) and
[diagnostic contract](../../../benchmarks/runtime_gfx942/KFD-COPY-PROGRESS-DIAGNOSTIC.md)
describe the implementation and its limits. Production runtime behavior and
proofs are unchanged. Native R125 CPU/test, Admission R118B C1-C3 and Resources
R116/V3 remain the accepted checkpoints; A1/A2, #182 and HIP/HSA parity remain open.

## Plan And Stop

The intended four blocks were `ABDC`, `BCAD`, `CDBA`, `DACB`. Each process used
256 MiB, depth one, three warmups and ten measured rounds, CPUs48-95 and
`membind=1` on physical GPU4, UID `0x54f88318ca05093d`, BDF `0000:85:00.0`.

| Cell | Policy |
| --- | --- |
| A | KFD diagnostic, public waits capped at 50 microseconds |
| B | KFD diagnostic, full remaining deadline per public wait |
| C | HSA fine pool, engine mask2, CPU agent index0 |
| D | HSA fine pool, engine mask2, CPU agent index1, confirmed nearest |

Each process required a fresh UID/BDF/utilization/VRAM/mapped-PID guard, zero
exit, a twenty-second cooldown, and another guard. These observations are not
a reservation and cannot exclude activity between snapshots or shared CPU,
memory and interconnect contention. Other host/GPU work was active throughout.

The first **eight processes completed**, each with full-buffer validation,
explicit teardown and successful pre/post guards: **104 round trips**, including
80 measured and 24 warmup rounds. The first two blocks completed in order.
The third block's first process, cell C, **never launched**:

- At `2026-09-18T01:57:24.571326439Z`, GPU4 had **1% utilization** and
  **918,585,344 bytes VRAM**, violating both admission criteria.
- The runner's final guard observed **0% utilization** but **648,622,080 bytes
  VRAM**, still above the 512 MiB ceiling. Neither failed guard reached PID
  collection, so no specific process is blamed for that occupancy.
- The runner exited **1**. Source, binary and clean-checkout postchecks passed;
  the final occupancy check did not. No policy fallback, partial-block
  acceptance, subsequent workload or hidden retry occurred.

## Partial Observations Only

These are ranges of the **two observed process p50s** in each cell, not results
from the planned four-block experiment. Nearest-rank percentiles exclude each
process's warmups; rounds are never pooled across processes.

| Cell | H2D process p50 range, ms | D2H process p50 range, ms |
| --- | ---: | ---: |
| A | 6.020-6.035 | 5.065-5.067 |
| B | 6.540-6.549 | 5.430-5.436 |
| C | 4.833-4.869 | 4.839-4.842 |
| D | 4.981-5.023 | 4.821-4.837 |

In the measured KFD rounds, A used 108-111 public waits per H2D and 92-93 per
D2H; B used exactly two in each direction. Public flush counts matched waits.
Reducing those counts did not reduce observed latency in this prefix. The
initial KFD submit p50s were only about 14-16 microseconds for H2D and
1.6-2.8 microseconds for D2H; most elapsed time was after initial submission.
This does **not** attribute that time to physical DMA or prove a root cause.

HSA emitted CPU-driver node0/non-nearest for C and node1/nearest for D, with
GPU driver node6 in both. CPU-agent equality is not physical-page residency or
cache-policy equivalence. HSA mask2 is not established to equal KFD engine1.
There was no HIP cell, no isolated-host result, no new formal refinement, no
accepted speedup, and no parity conclusion from this interrupted attempt.

## Next Diagnostic

The current persistent wait uses a 50-microsecond active-spin floor followed by
adaptive yield/sleep; requested sleeps can grow to 1 ms. Each sliced retry
starts a new cursor. A full-remaining wait can instead enter that backoff.
See [wait.rs](../../../crates/fe2o3-kfd/src/wait.rs) and the full-roster completion
scan in [sdma.rs](../../../crates/fe2o3-kfd/src/sdma.rs).

This makes host completion-observation policy a hypothesis, not a measured
cause. A bounded follow-up should retain one wait per window and identical
instrumentation, comparing the existing 1 ms ceiling with a 25-microsecond
ceiling while recording scan/pause counts, requested sleep time, thread CPU
time and context switches. Keep production defaults, ownership checks and
completion authority unchanged. The remaining H2D gap still needs separate
copy-path/cache/engine investigation; fewer public calls alone did not explain it.

## Evidence And Cleanup

The archive retains both nonzero records: `benchmark` and the intentionally
refused complete-campaign `summary-refusal`. `interrupted.py` checks only the
closed eight-process prefix and the exact failed-admission tail, and emits
`accepted_campaign: false`. Its output contains individual process observations,
not aggregate paired performance claims. Twelve parser tests check valid prefix
consumption, wrong policies/geometry, counts, nearest-agent identity, pool access,
round order/decomposition, warmup exclusion, guards, cleanup and truncation.
They do not fabricate a successful campaign or run GPU workloads.

`audit.py` binds 5,527 source-file hashes to the source commit, source checks
before/after, both binary hashes, before/after harness hashes, exact commands,
closed receipt intervals, explicit failure statuses, and the complete-checker's
refusal. It is evidence-consistency checking, not full toolchain attestation.

Read-only review caught that the original multi-file `bash -n` receipt checked
only its first script; later paths were positional arguments. That receipt is
preserved. `shell-lint-final` checks all nine shell scripts in separate Bash
invocations, and `audit-final` includes that correction. Both audits are retained.

The supplemental `audit-cpu.py` also pins and checks the prior immutable CPU
archive, consumes all six complete serial test harnesses and rejects thirteen
corrupt harness mutations. It closes the prior verifier's permissive row-search
gap without modifying that sealed archive or rerunning its tests.

The only remote directory created was
`/tmp/fe2o3-kfd-copy-progress-20260918.rrF6He9N`. After collection, guarded cleanup
removed **408 MiB** and recorded that exact directory as absent. The subsequent
occupancy check passed. No other user's files or processes were removed or
terminated. No remote build or benchmark process remains from this attempt.

```sh
python3 -I docs/evidence/dev-kfd-copy-progress-mi300x-2026-09-18/interrupted.py
python3 -I docs/evidence/dev-kfd-copy-progress-mi300x-2026-09-18/test-interrupted.py -v
python3 -I docs/evidence/dev-kfd-copy-progress-mi300x-2026-09-18/audit.py
```

Running `summarize.py` against this archive must fail: a successful archive audit
does not turn the interrupted benchmark into a successful campaign.
