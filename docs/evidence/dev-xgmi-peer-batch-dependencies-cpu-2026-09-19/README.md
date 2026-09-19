# XGMI Peer-Batch Dependency CPU Qualification

This packet qualifies single-pass reverse-dependency counting during native
XGMI peer-batch custody validation. It does not change submission, fence
waiting, currentness, mapping restoration, ordinary per-submission progress or
retirement. Full admission still inspects global scheduling and custody state.

## Implementation Boundary

One fallibly reserved vector holds the requested IDs and their dependencies.
After sorting and deduplicating the IDs, one active-table traversal counts
each dependent record once per relevant ID. A per-row ordinal marker preserves
the old `contains` semantics even when an unrelated malformed record repeats a
dependency. Existing waiter, retain-count and selected-dependency predicates
are unchanged.

Healthy input contains at most 16,191 raw relevant IDs. The iterator visits
each active record once for counting, but also traverses empty hash-table
buckets. The other custody passes and bounded allocation-owner checks remain.
There is no claim of backlog-independent or allocation-free admission.

Scratch failure is propagated separately from logical corruption. The earlier
completion-capacity, directional-count and stream-owner-cardinality checks
still precede allocation; later dependency, stream-depth and allocation checks
follow it. The public backend maps scratch failure to pre-effect nonterminal
`Rejected(Capacity)` and logical corruption to the existing terminal path.
This public routing is code-reviewed, not native allocation-fault-injected:
the CPU tests inject reservation failure in the pure validator and do not
construct fake native owners or tickets.

## Coverage

The tests freeze the prior dependency predicate from
`d29afddf7dc971ac58ccbad43585291f31ec29e8` and compare accepted and corrupted
fixtures against it. They cover the asymmetric retain-count requirements,
missing/failed completed dependencies, empty waiter-map keys, selected and
unselected duplicate dependencies, caller order, late waiter corruption,
empty-input allocation bypass and nonmutating scratch failure. Instrumented
counting checks one visit per source record, and separate correctness fixtures
cover the maximum dependency breadth and 65,536 active records.

The qualification selects Context, native-XGMI, diagnostic and peer-batch
runtime tests on GNU and musl, plus the six lower KFD batch-wait tests and
benchmark example tests. Strict Clippy, feature-off compilation, unsafe-source
policy, formatting and before/after source identity are also recorded. The
broader unrelated KFD creation/currentness fault matrices are not rerun here.

## Recorded Outcome

All 23 stages in the completed run passed. GNU and musl each passed the same
268 selected runtime/Context tests, six lower KFD batch-wait tests and six
example tests. The six feature-off example tests also passed. The verifier
requires the exact ten new dependency test names and eight previous admission
test names, not merely one test from each family.

All 20 verifier calibration tests passed. They include coordinated omission
and same-family substitution in both target rosters and passing outputs, with
updated receipt hashes and roster bindings, to exercise the explicit allowlist.

The identical before/after snapshot of 5,580 source files has SHA-256
`878af56d713f8b8f8f9bef9322c2e15a44dd13af0f039d3cfebde6a161595f18`.
It records the original Git base plus the exact qualified source hashes, not
a claim that the unchanged base commit already contains this patch.

An earlier attempt was deliberately stopped during the GNU test-roster build
to strengthen exact test-name acceptance. Its interrupted receipt and original
tools are retained locally in
`/home/harsh/.codex-tmp/fe2o3-dependency-qualification-attempt1.GNhazGyU`.
That attempt is not counted as passing qualification; the accepted raw records
in this packet come from the subsequent complete rerun.

## Measurement Boundary

The five profile cases vary selected-roster size, completed dependencies and
blocked successor count. Selected records are all in direction zero; blocked
successors are distributed round-robin across their predecessor IDs. Each
selected record has zero or four shared completed dependencies. These are
coherent dependency-index fixtures, not fully populated backend custody or
native GPU workloads.

Each matched row measures one old-predicate invocation followed by one new
invocation on the same immutable fixture. Both run in the same optimized test
binary (`opt-level=1`, assertions and overflow checks on). Fixture preparation
and surrounding admission/native work are excluded. The 65,536-record case
runs only the candidate. The separate maximum-breadth correctness test uses
256 distinct completed dependencies per selected record and is not timed.

These are descriptive single-shot CPU observations, without warmup, randomized
ordering, exclusive CPU reservation or a timing threshold. They are not
statistical performance acceptance, GPU throughput, formal refinement or a
HIP/HSA comparison. Earlier native measurements do not qualify this new source.

Observed validator-only milliseconds, rounded to three decimals:

| Active | Selected | Dependencies per selected | GNU reference | GNU candidate | musl reference | musl candidate |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 64 | 1 | 0 | 0.021 | 0.047 | 0.028 | 0.035 |
| 4,096 | 1 | 4 | 0.348 | 0.189 | 0.485 | 0.268 |
| 4,096 | 63 | 0 | 2.345 | 0.205 | 3.221 | 0.426 |
| 16,384 | 63 | 4 | 74.433 | 3.117 | 71.531 | 3.087 |
| 65,536 | 63 | 4 | not run | 29.663 | not run | 22.017 |

The candidate was slower in both 64-record observations. This packet does not
establish a small-input latency improvement or separate allocator, cache and
shared-host scheduling effects. The larger cases support the expected benefit
of removing repeated table scans, but are not statistical speedup acceptance.

## Replay

```sh
python3 -I -B docs/evidence/dev-xgmi-peer-batch-dependencies-cpu-2026-09-19/verify.py --live
python3 -I -B docs/evidence/dev-xgmi-peer-batch-dependencies-cpu-2026-09-19/test_verify.py
```

The recorder is non-overwriting. Exact command receipts, test rosters, tool
hashes, compiler identities, source snapshots and parsed profile rows are
bound by `binding.json`; `SHA256SUMS` seals the full archive inventory. Replay
checks those records, and `--live` additionally compares current source hashes.
Historical evidence packets are unchanged.
