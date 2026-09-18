# Striped SDMA Retained Teardown Native Protocol

Prospective correctness campaign, not a benchmark or full R126 acceptance.
Run the signed CPU-qualified static musl public example once for each count
2, 4, 6, 8, 10, 12, 14 and 16, in order, through its isolated parent/child route.
No packets, transfers, MMIO stores or cursor-advancing publication occur.
The cursor marker is source-qualified initial state, not a public observation.

GPU 4 is a candidate, not reserved. Its current topology was independently
checked before freezing this protocol. Before every native invocation require
the complete pinned observer to admit UID 0x54f88318ca05093d at BDF 0000:85:00.0,
zero GPU and memory busy in all sysfs snapshots, VRAM below 512 MiB, and no
reported selected-device process attachment. Bind CPUs 48-95 and NUMA node 1.
Fail closed on incomplete or conflicting data. Count 16 fills the admitted
eight ordinary SDMA queues on each engine; it does not authorize concurrency
with other work. No foreign process is signalled.

Each native case is bounded to 180 seconds with TERM then KILL after five
seconds, no core files, and 16-MiB output limits. Launch within one second of
preflight completion. After parent reap and owned-group absence, observe at
T0+[0,1] and T0+[20,21] seconds. No retries or relaxed gates. The whole campaign
has a 3600-second TERM/15-second KILL outer bound. Stop after the first failed
case, preserving both post-observation attempts.

Require exact complete two-line ASCII stdout and empty stderr: count N,
exactly N distinct nonzero u32 SDMA IDs (not necessarily contiguous), alternating
engines, host growth of 4096*N bytes/N records, 5+3*N returned resources,
configured-account refunds, inert rejected retry and completed public-root Drop.

Collect/hash-check the entire exact marked directory before deleting only it.
Verify all recorded PIDs/groups and accessible same-user exe/cwd/fd/maps
references absent, then independently verify path/PID/group absence after
cleanup. Preserve visibility limitations. If collection fails, retain the
directory for controlled recovery instead of deleting uncollected evidence.

Bind the complete signed source cohort, sealed CPU packet, reviewed public
signer file and exact executable. No remote build. This does not qualify
submitted work, nonzero native cursors, fault recovery, physical-residency
disposal, multi-device use, formal implementation correspondence, HSA engine
mask equivalence or HIP/HSA performance parity.
