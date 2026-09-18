# R126 Primary Envelope Native Preparation

PREPARATION ONLY. This document and runner do not authorize native execution.
Root must review the frozen runner/checker and authorize the exact marked remote
directory after HIP reports its run, observations, collection, cleanup and
independent owned-process absence closed. Endpoint observations do not reserve a
GPU and cannot establish continuous absence of unrelated work.

## Bound Inputs

- Signed containing commit: `8b0021ba740c337b2641ecd26ed6c9375613ed32`.
- Final scoped-musl runtime test binary:
  `target/x86_64-unknown-linux-musl/debug/deps/fe2o3_runtime-afaad0775e07e3e7`.
- Binary SHA-256:
  `7e75d4a11011cf63705020d3b814aa25a610fe56371c1b8c5d3b5671357376f8`.
- CPU packet: `docs/evidence/dev-primary-envelope-cpu-2026-09-18`.
  Compare every one of its 5,553 final before/after source hashes to the signed
  commit, ignoring only the older recorded `base` field. No Cargo rebuild.
- GPU 4, UID `0x54f88318ca05093d`, PCI `0000:85:00.0`, CPU 48-95, NUMA 1.
  This is a selected identity, not a reservation or current availability claim.

`prepare.py` will create a fresh local bundle after signed-commit and CPU-packet
authentication. It keeps the final ELF, raw cohort maps, commit signature
receipt, exact observer/topology helper and relevant fixture snapshots. The
complete 5,553-entry cohort is checked against Git blobs, not inferred from a
clean worktree or a few selected files. It hashes every exported payload.
It performs no SSH, device observation or native execution.

Only after preparation/review may root create one fresh remote directory with
`mktemp -d /tmp/fe2o3-r126-primary-8b0021ba-20260918.XXXXXXXX`, record its exact
returned path, and bind the root-approval document to that path and payload
manifest hash. No pre-existing directory may be adopted or overwritten.
Directory ownership and a commit/path-bound `owner.json` marker are required.
Remote payload hashes are checked before any observation or test and again
afterward. Results are created exclusively; a prior result directory refuses
the run. The runner never builds or invokes Cargo.

## Exact Tests

Run separately and serially, each with `--exact`, `--ignored`, `--nocapture`,
`--test-threads=1`, and `--color=never`:

1. `kfd_backend::retained_release_tests::native_runtime_typed_dispatch_shutdown_refunds_and_profiles_retained_primary`
2. `kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_error_retains_installed_root`
3. `kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_panic_retains_installed_root_and_payload`

The positive must report one passing harness test, its exact completion marker,
three full readbacks with output hash
`79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`,
normal accounting refund, completed root/backend Drop, and the complete
profiler JSON. Check the single queue's created/published/completed/destroyed
ordering and identities, three allocation releases and three 4-MiB reads.

Each failure-envelope parent self-spawns its exact child. The runner must NOT
set `FE2O3_TEST_PRIMARY_RELEASE_ENVELOPE_CHILD`: doing so bypasses the parent.
Retain parent stdout/stderr in full, including its complete replay of child
stdout/stderr. Require two one-test harness summaries, two entries bearing the
same exact test name, exactly one matching child marker, normal parent exit and
owned process-group absence. Do not manufacture separate child-exit receipts:
the parent asserts the real child's successful status and replays its output.

The Error/Panic injections happen after installation of the genuine original
primary Box. They intentionally use `ManuallyDrop` and retain the backend until
child process exit. They are runtime-envelope fault evidence, NOT a failed KFD
ioctl, successful destructive cleanup of the retained root, or full R126.

## Resource And Observation Bounds

Each case uses external `timeout --signal=TERM --kill-after=5s 180s`, `prlimit`
with core size zero and a 16-MiB per-output-file limit, and the fixed CPU/NUMA
placement. The recorder has a 200-second outer test wait and bounded owned
process-group TERM/KILL cleanup. Never kill by executable name, UID, GPU index
or an unowned PID. Child processes inherit the same process group and limits.
The fixture itself waits at most ten seconds for its one vecadd launch.

Each case has three 4-MiB host-visible vecadd buffers, one compute packet, a
64-MiB/128-record host-backing budget, and a 128-event profiler. Failure fixtures
also configure a 1-MiB/eight-record device budget; they do not allocate a device
payload. The positive uses the existing host-visible single-stream profile.
This is correctness qualification, not a latency or throughput benchmark.

Before EACH test, require the complete pinned observer to admit UID/BDF/index,
zero GPU use in all sysfs/SMI snapshots, VRAM strictly below 512 MiB and no
selected-GPU PID attachment in a complete parsed process transcript. Missing
metrics, unreadable/incomplete observations and visibility filters fail closed.
The test must start within one second of preflight completion. A failed fresh
admission stops the campaign without running that or later tests.

Root prospectively approved these post-observation semantics:

- T0 is after the top-level test parent is reaped AND its entire owned process
  group is confirmed absent. It is not the HIP runner's parent-reap timestamp.
- Immediate full observer actual start must fall within T0+[0,1] seconds.
- Exactly one delayed full observer actual start must fall within T0+[20,21]
  seconds. Sleep targets T0+20; no polling, retries or substitute later sample.
- BOTH complete endpoints must strictly admit. Preserve both observations even
  if the immediate observation rejects or the test transcript fails. A missed
  window is rejection; any late observation is diagnostic only. Stop before
  another test after any failed test, observation, timing or cleanup condition.
- If the test process group cannot be closed, stop and preserve that failure;
  do not define a successful T0 or proceed with the next test.

All observer commands are bounded; their own full CLI outputs are retained in
the observer JSON. The runner is additionally launched in an owned outer group
under a 1,200-second external timeout with a 15-second kill-after grace. Root
must retain its full outer transcript and ensure inner group cleanup before
removing files if the outer guard expires or transport is interrupted.

## Collection And Cleanup

After any outcome, collect the complete exact owned directory locally before
deletion, verify file hashes, and retain command/status/timestamps plus all
successful, refused or failed observations. Enumerate recorded test/observer
process groups; confirm they are absent. Read-only `/proc` inspection checks
visible references to the exact owned path, while reporting inaccessible
processes as visibility limits, not claiming host-wide isolation.

Only root's separately reviewed cleanup may delete the one exact marked remote
directory, after verifying its owner, marker, real path and no live owned
processes. Do not delete shared `/tmp` contents, signal foreign processes, or
reuse earlier benchmark directories. Verify the remote path's absence in a
separate command and release the GPU handoff only after that receipt closes.

## Current State

Signed source identity received. CPU cohort and final binary identity received.
No SSH, GPU admission, remote export, native launch or cleanup has been performed
for this plan. Runner/checker freeze, local preparation, root READY review,
HIP handoff, fresh remote path and root authorization remain prerequisites.
