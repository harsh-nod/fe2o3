# Explicit Peer-Copy Batch MI300X Comparison

Status: protocol preparation only. No native results have been accepted.

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
