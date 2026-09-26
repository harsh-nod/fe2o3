# Mixed-Duration Observer And Backpressure Development

This packet does **not** pass full CPU qualification and does not qualify native
execution, formal refinement, physical overlap, performance, SCALE-2, A1 or A2.
The source is signed commit `97b8d41fe` on `codex/r65-runtime-drain-versions`.

## Changes

- A recovered timed-out operation can register a new waker while still Pending,
  without resubmission or waking its former observer, and releases exactly once.
- Frozen-launch command rejection refunds reply and snapshot credits without
  issuing the rejected payload; later admission can reuse both budgets.
- Both 2,048-operation CPU admission loops now have explicit retry deadlines.
- Three ignored native tests cover exact-operation timeout recovery, autonomous
  progress after dropping the observer, and command rejection/refund/retry.
  Timeout and Drop execute inside one owner callback bracketed by the same
  native Pending receipt. They inspect complete output bytes and lifecycle and
  backing cleanup. They are not native-slot saturation tests.

## Evidence And Limits

`raw/` retains the signed-source cold GNU build and full suite, including all
failures. The run reports 1,422 passed, three failed and 27 hardware-only ignores.
The three failures are the existing `authorized_execution` telemetry tests:

- `cooperative_debug_telemetry_emits_only_bounded_logical_records`
- `failed_session_end_is_explicit_and_terminal`
- `pre_native_telemetry_failure_is_returned_and_poisoned`

The corrected musl run has the same counts and the same three failures. All
fail at socket admission with `InspectSocket(EPERM)`. The independent
`socket_probe.py` records that this environment denies `SO_DOMAIN`, `SO_TYPE`,
`getpeername`, `SO_PEERCRED` and credential configuration/inspection. The first
failed production check is `SO_DOMAIN`; socket validation is unchanged and
remains fail-closed. No test was skipped or changed to manufacture a passing
suite. Both new CPU regressions pass on GNU and musl.

`continuation/` preserves a separate runner configuration failure: the global
LLD `--threads=1` flag is rejected by musl's GNU linker. `validation/` records
the correction, removing only that flag from the musl build environment, and
the remaining independent checks. The original failures remain part of the
packet. The continuation runners always return failure overall; successful
independent checks cannot turn the original full-suite failure into a pass.
All 46 doctests, the default-feature check, all-target strict Clippy and the
format check pass. The replay classifier has seven passing regression groups.

The exact source archive, signed input/blob map, complete test rosters, command
receipts and compressed executed ELFs are retained. `verify.py` independently
joins compiler artifacts, source identities, ELF digests, commands and raw test
rows. A successful replay authenticates this **failed** qualification; its result
explicitly says `cpu_qualification_passed=false` and `native_executed=false`.

Provenance limits are explicit: the original GNU failure prevented the immediate
post-execution digest check. `validation/gnu-delayed-digest.json` records a later
matching digest, not an immediate check. The continuation helpers were measured
before and after execution but were not signed before execution. Preliminary
development terminal output is not retained and is not accepted evidence.

Replay commands:

```sh
python3 -I -B docs/evidence/dev-mixed-observers-2026-09-25/test_verify.py
python3 -I -B docs/evidence/dev-mixed-observers-2026-09-25/verify.py
```

`cleanup.py` only removes the two literal task-owned local build paths after
retained replay. Its receipts distinguish independently observed path absence
from controller-reported process-group absence in the original PID namespace.
It does not remove shared Cargo caches or claim cleanup of unavailable prior
mounts. No new remote resources were created: `mi300x` hostname resolution fails
in the current environment. None of the 27 native tests ran in this packet;
this does not negate earlier native checkpoints. Preliminary development build
and test handles were checked again and are no longer available; retained
controller process receipts cover only the recorded qualification commands.
The completed cleanup removed 1,946,607,616 inode-accounted allocated bytes from
those two paths, independently confirmed both absent, and replayed the retained
packet afterward. `packet-checks/` retains the runner test, seven classifier
groups, replay and cleanup command receipts.

## Next Gates

Rerun the full CPU suites where the socket prerequisites are available, then run
the native canaries under an external process-group deadline on an available
MI300X GPU. The opt-in 1,024-epoch-per-lane capacity profile, exact simultaneous
2,048-native-retained-epoch observation, generated execution/refinement, resource
gates, device timelines and matched HIP/HSA performance remain open. Default
64-slot limits and ReadWrite early-publication restrictions are unchanged.
