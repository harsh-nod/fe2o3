# Bounded debugger failure observations — 2026-09-26

The [preceding native attempt](physical-debugger-cmdline-refusal-20260926.md)
refused an empty command-line read before sending any debugger command.
Its process-family cleanup passed independently; its cause was not established.
This checkpoint adds failure-only observations, not a startup fix or a
successful physical capture. Public runtime bindings remain disabled.

## Implementation

After preserving the original first setup refusal and before the unchanged
teardown, the controller can make one immediate child-status observation and
bounded nonblocking reads from pipes it already owns. It does not reopen a PID,
start readers, send commands, retry, sleep or introduce a new observation clock.

Pipe observation is allowed only before reader/input ownership has transferred.
Stderr is sampled first. Each stream has a fixed 257-byte buffer including one
overflow probe, emits at most 256 bytes as hex, and permits at most four read
calls. The total is at most 14 pipe operations plus one immediate child-status
operation. Original descriptor flags are restored once, even if the observation
deadline expires; restoration errors are retained explicitly. EOF, EAGAIN,
interruption, other errors, truncation, unavailable ownership and expired time
remain distinct outcomes.

Observed exit status, signal/core status or OS errors are diagnostic only.
They do not relax argv, executable, custody, command or capture checks.
Diagnostic EOF does not become cleanup evidence. The original setup and
teardown order and predicates are unchanged. This bounded observation is not
a whole-process hard-time or RSS guarantee.

## Qualification

The public-disabled package passed **28 Node controls**, **58 library tests**
and **54 binary tests**, strict Clippy and build, without invoking the controller.
The fixed source roster now has 26 leaves; its verifier reports an unbound
runtime profile. The profile, config, custody, initial setup diagnostics,
all-null runtime bindings and Cargo lockfile retain their prior bytes.

Receipt: 53,544 bytes,
`fb0cc1e3acf8b6623a3dfd415c5e5083e2f5bb2ef8993af664eaa32b7c6ac9d8`.
Qualified source: 8,379 files, 119,765,189 bytes,
`90e1fdc5f5c3b94e4682eccc2fe34375af6853c6af6299d67c1a7147376f0e2a`.

Earlier failed proposals are retained. The source manifest was regenerated
after formatting in the verifier's exact path order. The first CPU request
omitted the verifier's required package argument; the next omitted the pinned
compiler library path. Fresh requests corrected only those invocation details,
and the final gate passed. Neither failure justified changing runtime checks.

## Next qualification

A fresh private source/profile and actual executable must be bound through the
family CPU/build/deployment and read-only prerequisite checks before a separately
coordinated native attempt. No previous receipt or successful cleanup proves
that successor can capture a stopped wave. Broad accepted exits remain
**M1/V1/V2/U1/U2/U3 (6/18)**.

