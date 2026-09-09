# Origin Runtime Integration Qualification

This evidence qualifies signed source commit
`57839db530cedcebc4a7eaca4ee22398dcc6be8c`, not the later R60 integration tree.
It refreshes the fixed R57 N3 numerical check after merging origin's native
initialization and completed-readback changes. It is not HIP/HSA parity or
copy-throughput evidence.

## Source and Device

- Date: 2026-09-09, beginning at 13:38:56 UTC.
- Host: `sharkmi300x-1`; selected ROCm GPU 1 only.
- Device: `gfx942:xnack-`, unique ID `ab83d2ffef0d3cdf`, PCI `0000:26:00.0`.
- Source archive SHA256:
  `ac9e6b3d3c10326887c956af82aa86d2f88f7875c102fab275e0707bec20f4bd`.
- Complete local evidence archive SHA256:
  `936d7cb51d47b42a487c6c4f5c109c65d1daace7f3c015b74be3b42bde47e094`.

The selected-device topology determined CPU and NUMA placement. The existing
R26 process-tree queue monitor ran unchanged with a 2 ms interval and a 10 ms
maximum observation gap. Other users' devices and processes were not modified.
Release binaries were built from the exact archive with the locked dependencies.
The two R57 source-shape tests also passed.

## Results

Both standalone N3 runs passed: one expected prepublication rejection, two
authorized launches, four exact readbacks, zero repeated user-data
materializations, and explicit cleanup. Both selected-device monitors passed.
Their maximum observed gaps were 5,756 and 6,007 microseconds; neither observed
foreign selected-device queues or remaining terminal queues.

A third monitored process group ran N3 followed by the public-device-memory
example with exactly 67,108,864 bytes. Both commands passed. The memory record
reports the expected initialization manifest, allocation flags `a0000001`, and
successful map/unmap and release. The composite monitor's maximum observed gap
was 5,286 microseconds, with zero foreign or terminal selected-device queues.

N3 establishes GPU execution. The memory-only example establishes CPU-side
initialization and KFD map/unmap/release; it creates no compute queue and does
not establish GPU-side memory contents or copy-engine throughput. Its reported
1,304 ms initialization time is a single correctness-run observation, not a
matched performance result.

## Rejected Attempts and Cleanup

An earlier attempt was rejected before execution because DRM card numbering
was incorrectly used for the ROCm GPU index. The harness now obtains the PCI
address from ROCm's selected device and cross-checks topology.

A subsequent attempt passed both N3 runs but correctly failed the unchanged
compute monitor when running the queue-free memory example alone. Its memory
output was not accepted. The successful composite keeps monitoring active
through both children and does not weaken the monitor's queue requirement.

All three task-owned remote staging directories were removed. After the final
run, selected GPU 1 reported zero utilization and zero allocated VRAM percent.
The monitor confirmed target reaping, process-group absence, and no remaining
selected-device queues before accepting output. Selected raw records are
retained in [raw](raw/), including the exact PASS records and sealed monitors.
