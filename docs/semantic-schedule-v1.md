# Semantic CPU schedule V1

`fe2o3-simulation-schedule-v1` persists the bounded decision record already
produced by `SimulationScheduleRecordV1`. It is not another scheduler. Canonical
and seeded recording, in-process replay, command-line replay, and debugger
capture all use the same cooperative simulator implementation.

The canonical JSON document binds exact KIR V7 identity, direct-KIR versus
verified-bundle admission, the complete canonical bundle identity and its
semantic subject, exact request bytes, CPU target profile, every simulation
limit, schedule identity and seed, complete coverage, context/transcript/record
identities, and every workgroup, phase, and runnable local selection.

The decoder accepts only its unique whitespace-free encoding. Unknown,
duplicate, missing, null, trailing, alternate-case, oversized, over-limit,
overlong-string, overflowing, allocation-failed, invalid-target, invalid-limit,
invalid-coverage, and integrity-corrupt inputs fail closed. Linux CLIs add
`openat2` no-symlink snapshot admission and private durable no-replace output.

During recording the schedule is the primary durable artifact. It is published
only after successful execution and encoding, and before ordinary result
delivery. A later result sink failure cannot roll that schedule back; the CLI
reports `"schedule_published":true`. This is an explicit two-sink boundary, not
an atomic transaction across schedule and result outputs.

## Commands

```text
fe2o3-kir-sim (--kir-v7 KERNEL.kir | --bundle KERNEL.fe2sim) \
  --request REQUEST.json \
  --record-canonical-schedule SCHEDULE.json \
  --schedule-max-decisions 1048576

fe2o3-kir-sim (--kir-v7 KERNEL.kir | --bundle KERNEL.fe2sim) \
  --request REQUEST.json \
  --record-seeded-schedule SCHEDULE.json \
  --schedule-seed 42 \
  --schedule-max-decisions 1048576

fe2o3-kir-sim (--kir-v7 KERNEL.kir | --bundle KERNEL.fe2sim) \
  --request REQUEST.json --replay-schedule SCHEDULE.json

fe2o3-debug sim (--kir-v7 KERNEL.kir | --bundle KERNEL.fe2sim) \
  --request REQUEST.json --replay-schedule SCHEDULE.json --protocol jsonl
```

Replay first compares the persisted binding with already admitted artifact and
request custody. The simulator then rechecks semantic context and validates
every decision against currently runnable invocations before committing that
selection. Missing, duplicate, unavailable, wrong-workgroup, wrong-phase, or
trailing decisions and coverage/transcript drift fail closed.

The debugger captures under that exact semantic schedule, then creates its
ordinary immutable transcript. Its protocol revision and pagination semantics
remain independent. The session configuration identity adds the schedule
context, transcript, and record integrity so agent clients cannot mix sessions
captured under different exact decision orders.

Wave32 and Wave64 are debugger visualization partitions, not simulator
scheduling inputs, so one semantic schedule may be visualized with either.
The debugger session configuration separately binds that wave width, preventing
pagination cursors or agent state from being substituted across visualizations.

This record makes CPU simulator behavior reproducible. It does not describe or
predict hardware wave, workgroup, queue, or compute-unit scheduling; it provides
no timing, performance, race-freedom, GPU-equivalence, source-refinement,
compiler-execution, artifact, load, launch, or hardware authority.
