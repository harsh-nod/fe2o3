# Multi-Device Runtime Qualification

## Scope

The `hardware-qualification` feature exposes
`KfdMultiDeviceRuntimeBackendV1::open_gfx942_vecadd_qualification_v1` for two
to eight distinct, nonzero device unique IDs. It admits only the existing
repository-owned exact gfx942 vecadd fixture, not arbitrary application kernels.
All selected devices are admitted before a child creates a VM or queue.

The `gfx942-runtime-multi-device-smoke` example:

1. Allocates and initializes the exact 4 MiB vecadd fixture on each device.
2. Publishes and flushes every producer before waiting for any producer.
3. Checks every output byte, then explicitly releases producer submissions.
4. Copies the first selected device's result to separate HostVisible
   destinations on the other selected devices using **host-staged** transfers.
5. Checks every copied byte, releases copy submissions, drains the completed
   group and explicitly shuts down both logical and native resources.

The final `PASS` line is emitted only after successful explicit cleanup.
This is replicated compute, not sharding. Publication before waiting does not
prove physical overlap. The native witness drains an already-completed group;
it does not qualify draining outstanding heterogeneous GPU work.

## Running

Use the hexadecimal device **unique IDs**, not GPU ordinals or PCI addresses:

```sh
cargo run --locked -p fe2o3-runtime --features hardware-qualification \
  --example gfx942-runtime-multi-device-smoke -- "$GPU0_UID" "$GPU1_UID"
```

Each argument must have the form `0x0123456789abcdef`. The host must expose
compatible `gfx942:xnack-` devices and permit KFD access. On a shared host,
inspect process attachments, engine activity and memory use immediately before
execution and again after cleanup. An idle percentage alone is insufficient:
another process may retain allocations or be between dispatches. A point-idle
check is not an exclusive reservation. Never reset devices or terminate other
users' processes to run this example.

## Qualification Status

CPU qualification of the unsigned 18-path overlay on `dd5e61712` passed
1,928 runtime tests, with 32 existing hardware tests ignored, no failures and
no filtering. Strict Clippy, the no-default library check, the single CLI test
and the normal example build also passed. The result composes seventeen
accepted original gates with five completion commands. The original campaign
remains rejected for its singular-test-list parser defect; archive collection
defects did not rerun or alter the test commands. This is not a new single-pass
campaign or a hardware result. The [CPU evidence packet](evidence/dev-multi-device-cpu-2026-10-01/README.md)
includes complete command records and a standalone readback auditor. The tested
source tree was recovered byte-for-byte after a local session restart; only
documentation and the evidence packet were added or updated afterward.

The new CPU regressions exercise roster validation, routed copies on two,
three and eight mock devices, opposite-direction group drain, retained results,
failed/cancelled work, deadline expiry, and contradictory-tail rejection.
The stream cleanup fix permits destruction of a completed cooperative tail
without releasing its result or event early. Pending work remains busy;
contradictory custody terminalizes the backend without destroying the child.

Native validation of this example remains pending. Earlier standalone XGMI
copy measurements do not qualify this combined compute-and-transfer path.
The implementation is not a machine-code-refined or fully formally verified
multi-GPU runtime, and no HIP/HSA parity or performance claim follows.

## Next Dependencies

The next implementation slice shares each device's existing compute VM with
native XGMI queue ownership. It must retain both foundations and any new native
owner until both session restorations succeed, including partial failure and
unwind. Subsequent work must integrate peer-mapped allocation ownership,
compute-to-copy dependencies, copy-to-compute readiness and ordered cleanup.

Only after full-byte native compute/transfer pipelines pass should qualification
expand to real workload partitioning, all admitted devices, partial failures,
physical-overlap measurement and matched HIP/HSA scaling benchmarks. The
[current milestone tracker](runtime-a1-a2-swarm-current.md) retains those exit
gates explicitly.
