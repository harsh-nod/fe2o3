# R56 gfx942 striped-SDMA diagnostic spin-budget experiment

R56 adds one benchmark-only experiment to the explicitly profiled striped-tail
wait. It does not change the ordinary
`wait_gfx942_striped_sdma_copy_batch_for_v1` policy. The diagnostic command
admits exactly these labels:

- `current`: the existing 64-spin, 16-yield, then 25 us-capped sleep policy;
- `250us`;
- `500us`;
- `1ms`;
- `1500us`; and
- `3ms`.

The five nonzero choices are elapsed active-spin floors. The floor starts after
tail binding and opening currentness, is clamped to the same caller deadline,
and counts attempts while repeatedly observing the exact prebound tail roster.
After the floor, the existing adaptive stage resumes at its current attempt
count with the same 25 us sleep-request ceiling. No arbitrary duration or
unbounded mode is represented by the public selector or accepted by the
benchmark parser. Omitting the diagnostic argument is identical to `current`.

The `3ms` candidate was added because the supplied R55 screen at exact commit
`8be0ab47c6d96664169d488dbafdf38ee3dbcd05` observed approximately
2.50--2.62 ms tail scans. It is intended to remain active through those
particular observed completion windows and expose the corresponding CPU cost.
It does not guarantee coverage on another run, host, workload, or software
stack.

## Preserved authority and custody

Every choice uses the same exact tail binding, shared monotonic deadline, tail
observation loop, final full ordered audit, closing operational-currentness
check, lifetime-bound all-ready witness, abort-only ordered retirement suffix,
timeout return, panic conversion, terminal poisoning, and move-only custody
machine. A pause action, configured budget, counter, timestamp, CPU clock, or
rusage value never supplies completion authority.

The ordinary wrapper passes `Current` to the compile-time `PROFILE=false`
instantiation. The lower layer rejects any non-current selector paired with
`PROFILE=false`. The existing profiled wrapper also continues to select
`Current`; only the explicitly named diagnostic-spin-budget profiled wrapper
can select a nonzero floor.

CPU observation failure remains diagnostic-only. Unavailable or invalid
thread-cost snapshots clear their three optional values. Deadline construction
failure remains terminal. Active-spin end-time overflow clamps spinning to the
already validated caller deadline; it cannot produce an all-ready result.
Profile duration conversion saturates only an output field. Completion still
requires the final full ordered audit followed by closing currentness.

## Schema

The diagnostic row uses
`fe2o3.kfd-striped-wait-spin-budget-diagnostics.v1`. Its workload ID ends in
`-spin<label>`, and these required run-wide fields distinguish policy:

- `wait_policy=current-adaptive-v1` with `diagnostic_spin_budget=current` and
  `diagnostic_spin_budget_ns=0`; or
- `wait_policy=diagnostic-active-spin-floor-v1` with the exact admitted label
  and one of `250000`, `500000`, `1000000`, `1500000`, or `3000000` ns.

The existing per-direction raw sample, p50, and p95 fields retain wait wall
time, tail-scan wall time, total spin/yield/sleep actions, requested sleep time,
thread CPU time, and voluntary/involuntary context switches. Each returned
diagnostic record carries the closed budget value and is rejected by the
benchmark accumulator if it differs from the run-wide selection. This permits
policy/latency/CPU comparisons without inferring policy from observed pause
counts.

## Interpretation limits

GPU work progresses while the host thread actually sleeps. Requested sleep
duration is not measured sleep duration, and
`wall - thread CPU - requested sleep` is not cumulative avoidable latency.
Replacing sleeps with spins changes host observation and scheduler-wakeup
behavior and can perturb CPU time and context switching. It cannot establish
that native scheduling or device execution time changed.

The R55 screen reported roughly 0.215--0.238 ms thread CPU time, 31--32
voluntary switches, zero involuntary switches, and 0.775--0.800 ms requested
sleeps around a 2.50--2.62 ms tail. The supplied nine-line evidence log has
SHA-256
`160e70ac9383a1b8a079770dd133a15531f619a0e9baa6f3685defe6e7d5f976`.
Those values motivate the closed experiment but do not attribute the whole
non-CPU interval to wakeup delay. R56 adds no GPU measurement, hardware
qualification, parity result, or speedup claim. In particular, it does not
establish that any spin budget can close the observed approximately 0.7 ms HIP
gap. That requires matched measurements of the exact committed candidates.
