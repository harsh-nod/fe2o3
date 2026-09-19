# Explicit Peer-Copy Batch MI300X Comparison

Status: bounded native execution passed. All 9 workload processes, 18 result
rows, 54 endpoint observations, 17 local commands and 68 remote commands passed
replay. All 18 controller/parser calibration tests passed. Results were collected
and hash-checked before exact-owned cleanup; remote path/process absence and
local payload removal were confirmed.

Signed source: `71b85d9e68d3f7f1d56af8e82cb946068547b85b`.
Signed launch tooling: `5319dd58d1b698bb6b23fc5bfbda86a693fec04e`.
The one release ELF had SHA-256
`e2552dfc83e1669a096761084860c0230ba2f000e6fc2254633480d030c555a5`.
The [CPU prerequisite](../dev-xgmi-peer-batch-cpu-2026-09-18/README.md)
seal is `1a22011c43756df691b7e8823bd529c75435cb0c2703e2c3c8bd44d0af384b8c`.

This packet compares ordinary and explicitly aggregated peer-copy waits using
one signed-source release binary on physical GPUs 1 and 2. Each timed mode uses
1 MiB per submission, 10 warmups and 30 samples. Depths 1 and 2 each run in
ordinary/aggregate/aggregate/ordinary order. A separate aggregate-only depth-63
run uses one sample and no warmups; it is correctness coverage, not a matched
performance comparison.

The CPU prerequisite binds exact source bytes, GNU/musl test rosters, compiler
reports and command receipts. The native controller requires signed source and
tooling, publication to both remotes, a clean matching source tree and the same
reported compiler toolchain. Source, payload and binary identities are checked
before each workload and at completion.

The user authorized available GPUs, not an exclusive reservation. Strict
admission runs on both endpoints before every workload and again after settled
and delayed postflight intervals. These observations are not continuous host
monitoring and cannot rule out all shared-host interference.

The controller uses a fresh private ownership-marked directory, two build jobs,
private build and temporary directories, bounded processes and disabled core
dumps. It collects the complete result inventory and verifies its local hashes
before removing only its owned remote directory. Path and process absence are
checked afterward; uncollected evidence is retained on failure.

```sh
python3 -I -B docs/evidence/dev-xgmi-peer-batch-mi300x-2026-09-18/test_campaign.py
python3 -I -B docs/evidence/dev-xgmi-peer-batch-mi300x-2026-09-18/campaign.py run 1 2
python3 -I -B docs/evidence/dev-xgmi-peer-batch-mi300x-2026-09-18/verify.py --allow-unsealed
python3 -I -B docs/evidence/dev-xgmi-peer-batch-mi300x-2026-09-18/verify.py --seal
```

The parser requires exact per-mode schemas, matching device/run controls,
payload and canary success, explicit teardown and consistent latency/bandwidth
arithmetic. Replay also requires complete command and endpoint rosters,
nonoverlapping process receipts, exact source and collection identities and
owned cleanup closure.

## Observed Comparison

Each cell below is the arithmetic mean of the two per-process p50 host latencies,
not a pooled-sample median. Speedup divides the ordinary mean by the aggregate
mean. The underlying per-process p50/p95 values remain in `remote/parsed.json`
and the original transcripts. Two repetitions on a shared host do not establish
statistical confidence or a general performance guarantee.

| Depth | Mapping | Direction | Ordinary ms | Aggregate ms | Ratio |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | Remap per round | Forward | 111.496 | 97.736 | 1.141x |
| 1 | Remap per round | Reverse | 111.556 | 98.045 | 1.138x |
| 1 | Persistent hot | Forward | 28.246 | 14.333 | 1.971x |
| 1 | Persistent hot | Reverse | 28.235 | 14.329 | 1.971x |
| 2 | Remap per round | Forward | 208.794 | 181.425 | 1.151x |
| 2 | Remap per round | Reverse | 208.744 | 181.431 | 1.151x |
| 2 | Persistent hot | Forward | 42.400 | 14.378 | 2.949x |
| 2 | Persistent hot | Reverse | 42.296 | 14.367 | 2.944x |

Persistent-hot host latency decreased about 49.3% at depth 1 and 66.0-66.1% at
depth 2. Remap-per-round latency decreased only 12.1-13.1%; the remap-per-round
end-to-end interval remains much larger. These are host intervals, not GPU copy-engine
latencies. The depth-63 run passed payload/canary checks in both mapping modes
and both directions, but its single sample has no matched ordinary baseline.

## Limits

This is workload-scoped native testing. It is not native driver fault injection,
formal refinement, a HIP/HSA comparison, general performance acceptance or an
orders-of-magnitude claim. See the
[API contract](../../runtime-xgmi-peer-batch-v1.md).

Aggregate admission currently checks the complete backend scheduling indexes.
Its duplicate and membership checks can be quadratic in a large unrelated
ready backlog, despite the requested batch being bounded at 63. This packet
does not test that backlog regime. Linearizing admission and adding scaling
regressions remain follow-up performance work; the deadline includes this
preparation and does not make its cost constant.
