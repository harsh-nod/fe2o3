# Combined SDMA Retained Teardown Native Protocol

Prospective correctness campaign, not a benchmark or full R126 acceptance.
Run the signed CPU-qualified static musl public example once for each striped
count 2, 4, 6, 8, 10, 12 and 14, in order, alongside two directional queues,
through its isolated parent/child route. No packets, transfers, MMIO stores
or cursor-advancing publication occur. The cursor marker is source-qualified
initial striped state, not a public observation.

GPU 1 is a candidate, not reserved. Its topology was independently checked
before freezing this protocol. Before every native invocation require the
complete pinned observer to admit UID 0xab83d2ffef0d3cdf at BDF 0000:26:00.0,
zero GPU and memory busy in every sysfs snapshot, VRAM below 512 MiB, and no
reported selected-device attachment. Bind CPUs 0-47 and NUMA node 0. Fail
closed on incomplete/conflicting observations. Fourteen striped plus two
directional queues fill eight ordinary queues per engine, not permission to
share a GPU with other work. No foreign process is signalled.

Each case is bounded to 180 seconds with TERM then KILL after five seconds,
no core files and 16-MiB output limits. Launch within one second of preflight
completion. After parent reap and owned-group absence, observe at T0+[0,1]
and T0+[20,21] seconds. No retries or relaxed gates. The whole campaign has
a 3600-second TERM/15-second KILL outer bound. Stop after the first failed
case, preserving both post-observation attempts.

Require exact two-line ASCII stdout and empty stderr: striped count N,
total N+2 distinct nonzero u32 queue IDs (not necessarily contiguous), H2D/D2H
engines 1/0, alternating striped engines, capacity 2/8/14, host growth of
4096*(N+2) bytes/N+2 records, 11+3*N returned resources, configured-account
refunds, inert rejected retry and completed public-root Drop. The directional
engine marker is asserted against observations before being printed literally.

Collect/hash-check the entire exact marked directory before deleting only it.
Verify recorded PIDs/groups and accessible same-user exe/cwd/fd/maps references
absent, then independently verify path/PID/group absence after cleanup.
Preserve visibility limitations. If collection fails, retain the directory
for controlled recovery instead of deleting uncollected evidence.

Bind the complete signed source cohort, sealed CPU packet, reviewed public
signer and exact executable. No remote build. This qualifies neither submitted
work, nonzero native cursors, fault recovery, physical-residency disposal,
multi-device execution, formal implementation correspondence, HSA engine-mask
equivalence nor HIP/HSA performance parity.
