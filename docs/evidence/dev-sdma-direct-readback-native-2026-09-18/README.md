# Direct Destination SDMA Readback Native Validation

Both existing cold-allocation probes passed on MI300X GPU 4. The DeviceLocal
probe exercises transient staging readback; HostVisible exercises indexed
readback. All six complete strict observations admitted: fresh preflight,
immediate postflight and one fixed delayed postflight for each test. Collection,
exact-owned cleanup and separate path/PID/group absence checks also passed.

This validates two bounded correctness routes of the direct-destination change.
It does not establish HIP/HSA performance parity, native fault behavior, formal
machine-code correspondence or full R126 acceptance. Accepted checkpoints and
A1/A2/#182 remain unchanged.

## Bound Inputs

- Signed source: `fd1cf3dd05e691797533b1beab1f015c4b2d8fad`.
- Musl executable SHA-256:
  `dbafaf826cc18b2d785015a3b4bb809c8f329f66b6f605a364a6a0e41148b145`.
- Fresh [CPU packet](../dev-sdma-direct-readback-cpu-2026-09-18/README.md):
  manifest `d35435e8d78df345f32697c8468a16b1aa28a7c008b845b9383a3d9c9bd2f686`.
- Equal 5,553-entry source maps:
  `a4241421d6ce92b74849a0df45ea94d4fe7a2e2f13f7348b7ee7078b8c3d24f1`.
- Frozen 23-file payload:
  `395806274d56b787af9bdd43771ef6a0cf422e2afbf7be190ae7a68930f059b9`.

The CPU packet passed full GNU and musl runtime suites, 1,105 tests with twenty
ignored on each target. Source and executable identities were unchanged across
those runs. The exporter verified the containing commit signature, compared
every selected source identity to its Git blob and rehashed the tested ELF.
Native execution used that exported binary without a remote build.

## Native Scope

Each test asserts cold capacity rejection and refund, a successful 4-KiB retry,
native zero and index-derived pattern bytes, and a native-only XOR mutation
while the retained CPU shadow remains unchanged. Subsequent public and direct
adapter reads must return the mutated native bytes. The final full readback
hash is `2e7d5db52627eb273e9c3837f94bdc7bff796711c2e0960f3c0416d2cb9868d9`
in both cases. Allocation release, backing accounting, pool state, Context
cleanup, retained shutdown and inert retry are asserted in the pinned tests.

The transcript parser independently checks test identity, the single successful
harness, memory kind, rejected/retried byte counts, marker uniqueness and full
readback digest. It leaves the accounting/shutdown debug values opaque; those
guarantees come from assertions in the pinned source and executable. The
adapter-only mutation is a routing oracle, not an all-public-API workflow.

There is no compute dispatch in these tests. Native subrange and multi-packet
DeviceLocal readback, currentness/retake/recycle faults, allocation counters and
compute-dirty paths are not qualified here. The small correctness probes are
not benchmarks; their harness durations must not be used as throughput claims.
The existing DMA comparator excludes this host readback from its copy timers.

## Admission And Cleanup

The shared host was not reserved. GPU 4 was bound to UID
`0x54f88318ca05093d`, BDF `0000:85:00.0`, CPUs 48-95 and NUMA node 1.
The unchanged full guard required zero utilization, VRAM below 512 MiB, no
selected-device PID attachment and complete identity/metric observations before
each launch. The isolation environment acknowledgement is not exclusivity proof.

Both tests launched within one second of fresh admission. T0 followed parent
reap and complete owned process-group absence. Immediate observations started
at T0+0.033115084 s (device) and T0+0.032498764 s (host); delayed observations
started at T0+20.031632106 s and T0+20.030197851 s. All were strictly within the
prospective windows, with no retries or relaxed policy. Test limits remained
180 seconds, zero core output and 16-MiB output files, with bounded outer waits.

All 58 remote files were collected and byte-checked before removal of
`/tmp/fe2o3-sdma-readback-fd1cf3dd-20260918.2gdfkcmf`. All eleven recorded
native/observer/outer process groups were absent. A separate command confirmed
path absence and the same PID/group roster. Accessible same-user exe/cwd/fd/maps
references were absent; unreadable entries remain explicit visibility limits.
No foreign work or shared temporary directory was removed.

## Retained Audit

`retention.json` binds 116 original files to their source paths and hashes. Only
the 58,532,256-byte ELF is omitted from the committed packet; its hash remains
in the frozen payload and complete remote inventory. The original ELF remains
available locally. The portable audit cannot reconstruct or rehash omitted
bytes and does not treat archived signature output as a new signature check.

`python3 -B verify.py` verifies the sealed archive, exact payload and CPU-map
bindings, native transcripts, command limits, observation timing, full remote
inventory and cleanup. `--allow-unsealed` is only for pre-seal review. Optional
`--source-root PATH` and `--binary PATH` separately revalidate current inputs.
The checker never invokes SSH, a GPU command, Cargo or the executable. The
fifteen archive calibration tests reject missing/mutated identities, captures,
timing, inventory, cleanup and native transcript data. Review identified that
the frozen transcript parser did not reject all unconsumed text; the portable
audit adds an exact whole-harness check, serial native/controller chronology,
and exact cleanup-command/PID/inventory/helper binding. The actual records pass
these stronger checks; frozen protocol and raw bytes remain unchanged.
Historical rejected primary-envelope campaigns remain unchanged and rejected.

The first manifest candidate failed exact closure because its basename exclusion
also omitted the nested CPU `SHA256SUMS`. That rejected manifest and the exact
missing-path diagnosis are retained under `audit`. The corrected sealer excludes
only the root manifest. No frozen payload, native receipt or test result changed.
