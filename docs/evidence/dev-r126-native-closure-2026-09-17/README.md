# R126 Native Development Validation

Eight native probes pass against source commit
`967a62dfffb94b4ca679ba3d4ae5ed60fc82eba1`. The campaign then stops before its
ninth probe because the per-probe guard detects a new workload on the selected
GPU. This is a successful eight-probe prefix, not completion of all thirteen
planned cases. R125 remains the accepted Native CPU/test checkpoint; R126,
N5 DATA-ADOPT, A1/A2, issue #182, formal correspondence and HIP/HSA parity remain
incomplete. No Rust source or test code changes in this packet.

## Source And Device

The exact musl runtime library-test executable comes from the preceding
[auxiliary-release CPU campaign](../dev-r126-auxiliary-release-2026-09-16/README.md).
Its SHA-256 is
`9204ac3f18b35d6ce4bcc29492c7c971f9187a647dd03ee0f936c73ba33a1f00`.
That receipt records its Cargo artifact identity, features, source delta and
full CPU regression results. Local checks and matching remote hashes bracket
this native run. Source commit checks and clean Rust worktree/index checks
establish that no source changed between those builds and native execution.
No executable is included in this archive.

The selected physical device is MI300X GPU 1, unique ID
`0xab83d2ffef0d3cdf`, PCI `0000:26:00.0`. GPU 0 remains occupied by another
workload and is not selected. Logical compute ordinals `[0, 1]` in test output
refer to primary and auxiliary queues on this single physical GPU; they do not
represent a multi-device test.

Each pre-probe check authenticates the unique ID and PCI address, requires zero
reported GPU utilization, at most 512 MiB VRAM usage and no mapped process on
GPU 1. The observations are point-in-time availability checks, not an exclusive
reservation. Explicit `FE2O3_TEST_NATIVE_ISOLATED=1` is an operator
acknowledgement, not a stronger exclusivity claim.

## Native Results

Every executed case runs one exact ignored test in its own process, with core
dumps disabled and an owned-process timeout. Each result has one pass, zero
failures and zero ignored tests. The 852 filtered tests are not part of that
native invocation.

| Record | Native Case | Libtest Duration |
| --- | --- | ---: |
| `probe-00` | DeviceLocal cold/warm capacity rejection, refund and retry | 2.97 s |
| `probe-01` | HostVisible cold/warm capacity rejection, refund and retry | 2.12 s |
| `probe-02` | Two-stream auxiliary shutdown followed by primary-capacity retry | 6.98 s |
| `probe-03` | Allocation while ordinary primary compute remains logically pending | 4.97 s |
| `probe-04` | Allocation while ordinary primary and auxiliary compute remain logically pending | 7.18 s |
| `probe-05` | Public allocation/shutdown selects retained directional release | 1.95 s |
| `probe-06` | Native Device promotion/readback and retained shutdown with caching | 2.23 s |
| `probe-07` | Zero-cache recycle disposes backing before pool trim | 3.28 s |

The first five cases were previously compiled but unexecuted. The last three
also validate successful allocation, synchronous-copy/readback, pool and teardown
paths on this exact source, not merely a preceding lower-driver revision.

Cold-capacity probes use actual native owners with deterministic session budgets:
4097 requested Device bytes exceed a 4096-byte backing budget; 16 MiB plus 4096
requested Host bytes exceed a 16 MiB backing budget. They assert cold quiescent
and warm rejected outcomes, exact Context credit refunds, a successful 4096-byte
retry, divergent native/shadow readback and zero-cache disposal. The observed
Host control baseline is 532480 bytes/three records, then zero after retained
primary cleanup. This is not global device exhaustion or an aggregate native
memory bound. Adapter-only writes intentionally participate in these probes;
they are not wholly public-API application workflows.

The auxiliary retry probe runs real vector addition on primary and auxiliary
queues, injects the test-only primary-custody shell-allocation rejection, checks
the quiescent retry state and absence of the destroyed auxiliary handle, then
finishes primary shutdown. Profiling records exactly the two queue destructions
in order. This is deterministic allocation-fault injection, not physical host
allocator exhaustion or a native DESTROY-ioctl failure.

Pending-allocation probes preserve original published receipt identities and
check Host/Device allocation, zeroed bytes, original vector-add output and
eventual account refunds. They establish logically pending native custody, not
physical GPU execution overlap. Their shared vector-add output SHA-256 is
`79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.

## Guard Stop And Cleanup

`probe-08-before-admit.exit` is intentionally retained as `1`, with diagnostic
`another process owns GPU 1`. The PID observation maps new process `1043324`
to GPU 1 and additional workloads to GPUs 2-7. The utilization/VRAM observation
alone would have missed these newly attached workloads. No ninth test is
started, no guard is weakened, and the runner is not restarted.

The unexecuted suffix consists of the standalone typed-dispatch regression,
standalone two-stream regression and AUX budget-retention prefixes 2, 0 and 1.
Their earlier receipts do not qualify them on this source. The successful retry
and pending-work probes exercise related vector-add workflows but do not replace
those exact missing invocations.

The owned upload directory was
`/tmp/fe2o3-r126-native-967a62df.CoT8qwY4`. Closure rechecks the remote hash,
owner/mode and exact single-file membership. `fuser` returns its expected
no-user status `1` with empty output after all test SSH commands have returned;
this is a process-closure check, not a failed native test. Only `runtime-tests`
is removed, followed by `rmdir` and explicit absence checks for the directory
and any symlink. Final GPU observations retain the unrelated workloads.
No GPU resets, unrelated process signals or unrelated file deletions occur.

## Evidence And Remaining Work

`native/` holds 54 command records: eight actual test invocations, their guards,
initial source/binary/device checks and the final rejected guard. `closure/`
holds 15 records for hash/process/source closure and exact cleanup. Every record
contains the command, raw output, UTC endpoints and actual exit code; the two
nonzero records are the guard stop and expected no-user `fuser` result described
above. Raw whitespace is preserved.

`audit-native.py` checks the exact record inventory, command vectors, serial
ordering, test identities/results, replayed availability decisions, source and
binary endpoints, and owned cleanup. `audit.log` records its successful result.
Its local-binary check requires the original retained executable and preceding
CPU receipt at their recorded paths; it does not launch GPU work or contact the
remote machine. `SHA256SUMS` covers all 351 other files, including the auditor,
scripts and raw records, for 352 files in this archive. Hash integrity is
distinct from the behavioral scope of the tests.

The runner and guard are frozen campaign artifacts, not a generic automatic
hardware scheduler. Reproduction requires a freshly qualified matching binary,
a fresh output directory and private remote scratch, explicit device selection,
and new per-probe availability checks. Do not rerun these scripts against their
already populated output directories or reuse the removed scratch pathname.
An SSH error or timeout requires inspecting owned process state before cleanup
or retry; it must not be interpreted as GPU isolation or completed teardown.

These debug/test timings and host profile timestamps are not benchmarks. This
packet adds no HIP/HSA comparison, physical-overlap proof, multi-device result,
new formal proof or comprehensive native fault coverage. Integrated R126 CPU
qualification, the unexecuted native suffix, additional attachment/queue
profiles and native failure matrices remain open. Generated DATA-ADOPT,
ISSUE/COMPLETE, Stop/drain/graph integration and resource/protocol work remain
subsequent implementation requirements.
