# Single SDMA Retained Teardown Native Protocol

This is a prospective correctness campaign, not a benchmark or a full R126
acceptance test. Run the signed, CPU-qualified, non-test static musl example
for generic, engine 0 and engine 1 in that order, each through its public
parent/child route. No packets, copy transfers or MMIO stores are submitted.

GPU 4 is a candidate, not reserved. The user permits currently free GPUs. Before
each native invocation, require the unchanged complete observer to admit UID
0x54f88318ca05093d at BDF 0000:85:00.0, zero utilization, VRAM below 512 MiB,
and no selected-device process attachment. Verify the exact topology and bind
CPUs 48-95 with memory node 1. Fail closed on incomplete or conflicting data.

Each run is bounded to 180 seconds with TERM/KILL, no core files and 16-MiB
output limits; the recorder owns its process group. Launch within one second
of the completed preflight. After parent reap and group absence, take strict
observations at T0+[0,1] and T0+[20,21] seconds. No relaxed policy or retries.
Require exact complete two-line stdout, empty stderr and successful parent
status. The source asserts eight released resources, original queue identities,
exact configured backing refund and inert retry, then reports completed Drop.

Stop subsequent cases on any failure. Collect and hash-check the complete owned
directory, confirm all recorded PIDs/groups and accessible same-user references
are absent, remove only that marked private directory, and independently check
path/PID/group absence again. Preserve all rejected attempts and visibility
limits. The payload binds the signed source cohort, CPU archive and executable.

This does not qualify submitted work, native error/panic recovery, multi-device
behavior, machine-code refinement, formal implementation correspondence, HSA
engine-mask equivalence or HIP/HSA performance parity.
