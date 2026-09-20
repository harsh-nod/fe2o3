# MI300X Full-Currentness Host Attribution

This packet records four KFD diagnostic off/on/on/off trials from signed
source `0ec05ee146e9c852727135e56f77d8b96168b857`, qualified by the
[runtime CPU packet](../dev-xgmi-currentness-capture-cpu-2026-09-20/PROTOCOL.md).
The [protocol](PROTOCOL.md) specifies the bounded workload and admission.

Physical GPU 1 (`0000:26:00.0`, `0xab83d2ffef0d3cdf`) and GPU 2
(`0000:46:00.0`, `0xd2e26fef80cf5c33`) were admitted independently before
every trial and passed settled/delayed postflight checks. The shared host
was not exclusively reserved. Other devices could remain active.

Each trial used the same diagnostic-enabled ELF, 1 MiB depth-one persistent
hot copies, one prime batch, 10 warmups, and 30 samples per direction. The
two enabled trials contain 164 diagnostic rows. Statistics use only their
120 `sample` rows, excluding four prime and 40 warmup rows. All 41 fields
are parsed, identity-bound, and checked for hierarchical containment.

## Results

The mean backend aggregate host interval was **14.241571 ms**. Opening plus
closing currentness accounted for **14.068853 ms (98.7872%)**. Within those
checks, fresh whole-host topology discovery accounted for **13.621740 ms
(95.6477% of the backend total)**.

Opening plus closing means per measured aggregate call:

| Host interval | Mean ms | Share of backend total |
| --- | ---: | ---: |
| Full topology discovery, parent interval | 13.621740 | 95.6477% |
| Topology-tree traversal | 9.800819 | 68.8184% |
| Render/PCI identity correlation | 3.352496 | 23.5402% |
| Initial boot/kernel/module identity | 0.218910 | 1.5371% |
| Closing generation/boot/kernel/module identity | 0.248095 | 1.7421% |

The last four rows are nested inside discovery, not additional costs to add
to it. Tree traversal includes every node and both link sets; correlation
includes every GPU, not just the selected route endpoints. These two phases
are the dominant observed host costs. This locates an optimization target;
it does not justify removing observations, narrowing snapshots, caching
between calls, or treating generation alone as currentness authority.

Aggregate submission plus wait averaged **0.167138 ms (1.1736%)**. That
includes host validation and is **not** SDMA device execution time or a
measure of XGMI link bandwidth.

Facade enqueue-through-aggregate-close observations, in milliseconds:

| Trial | Forward p50 | Forward p95 | Reverse p50 | Reverse p95 |
| --- | ---: | ---: | ---: | ---: |
| 1 off | 14.116034 | 14.232969 | 14.140400 | 14.346279 |
| 2 on | 14.146369 | 14.295502 | 14.177355 | 14.309203 |
| 3 on | 14.331165 | 14.436823 | 14.310804 | 14.433788 |
| 4 off | 17.336200 | 18.002177 | 17.462569 | 18.409457 |

The final off trial was slower than the earlier trials despite successful
endpoint checks. The observations do not establish a cause or a causal
instrumentation-overhead estimate. No cross-campaign HIP/HSA ratio is made.
Readback/canaries and explicit teardown passed. Final readback alone does
not independently prove every repeated hot copy executed.

To reproduce the attribution arithmetic, select `population == "sample"`
from the verifier's parsed records. For each nested field, add its `opening`
and `closing` values per record and divide the sum by 120. Shares divide the
same sum by the sum of the 120 aggregate `durations_ns.total_ns` values.
This is a ratio of sums, not an average of per-record percentages. Primes
and warmups remain in the raw evidence but never enter these statistics.

## Evidence And Cleanup

The packet contains 15 local command receipts, 35 native command receipts,
24 endpoint observations with 72 sysfs snapshots, and all four raw workload
transcripts. The one ELF SHA-256 is
`e4a3f3368ce0056927578dfadf191ea75db0563f99d97880ed4b94c9144b1617`.
The CPU packet seal SHA-256 is
`2c38eb0f384e51fe787a4507a632ced74af3d0475ab29b1742a28d422b1214b7`.

The controller completed byte-exact collection before cleanup. Remote
directory/process absence and local transport absence are recorded. It did
not reset GPUs, stop foreign jobs, or clean unrelated files.
Its private remote directory was
`/home/harsh/fe2o3-xgmi-currentness-attribution-20260920.d5615a8c865476a6`;
the recorded absence check reports both path and processes absent. The
terminal controller state has no primary or secondary failures.

Replay the complete archive from the source root with
`python3 -I -B docs/evidence/dev-xgmi-currentness-attribution-mi300x-2026-09-20/verify.py`.
This checks signed source and tooling, CPU qualification, exact command
receipts, endpoint admission/postflight, parsed observations, cleanup, and
the final seal. Native execution is observed; performance acceptance and
formal refinement remain false. This is not HIP/HSA parity, a speedup, or
a causal instrumentation-overhead claim.
