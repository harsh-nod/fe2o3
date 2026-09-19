# XGMI Peer-Batch Admission CPU Qualification

This packet qualifies the replacement of quadratic ready-index scans by
fallible sorted scratch indexes. It does not change native submission, fence
waiting, currentness, mapping restoration, dependency validation or retirement.
The original ready queues retain FIFO order. Scratch failure rejects before
native effects and leaves all caller and backend state intact.

## Scope

- Differential tests compare the new admission function against its frozen
  predecessor from `ca7a30d7d1a9705a6299ea9f5c8410a336856da5`.
- Tests cover FIFO/request permutations, late index corruption, the precedence
  of corruption over subset/unknown errors, and either scratch reservation
  failing before metadata validation. Invalid request shape precedes allocation.
- Synthetic admission-only fixtures exercise 65,536 active records with either
  an opposite-direction ready backlog or dependency-blocked records. These are
  not fully populated backend custody fixtures or native GPU submissions.
- Successful published-ticket admission is not synthesized: the native ticket
  type has no public constructor. Existing inconsistent in-flight cases and
  lower batch-wait tests remain selected; this is not new native retry evidence.

The recorded qualification selects Context, native-XGMI, diagnostic and batch
runtime tests on GNU and musl, plus the six lower KFD batch-wait tests. It does
not rerun the broader lower-layer creation/currentness fault matrices from the
previous packet. It also checks strict Clippy, feature-off compilation and
example tests, formatting, unsafe-source policy and unchanged source identity.

## Recorded Outcome

All 23 recorded stages passed. GNU and musl each passed the same 258 selected
runtime/Context tests, six lower KFD batch-wait tests and six example tests.
The six feature-off example tests also passed. The runtime selection contains
eight new admission tests in addition to the previous 250-test selection.
All 17 verifier calibration tests passed, including malformed profile rows,
changed observations, Boolean-for-integer substitutions and inherited archive
tampering checks.

The before/after source snapshot SHA-256 is
`ee6f7293406ca4f32dca9bdba151572e4053b4c4bdb9425aba5bfc7c5b9d81cc`.
The snapshot records the original Git base plus 5,579 exact qualified source
file hashes; it does not imply that the unchanged base commit contains this
patch.

## Timing Boundaries

The dedicated GNU and musl profile stages run one test serially. Each matched
row measures one invocation of the old admission function followed by one
invocation of the new function on the same fixture. Fixture construction and
later custody validation are outside the timer. Both implementations use the
same optimized test binary (`opt-level=1`, assertions and overflow checks on).

These are single-shot CPU diagnostics, not statistical performance acceptance:
there is no warmup, randomized ordering, exclusive CPU reservation or latency
threshold. The 65,536-record row runs only the candidate and proves bounded
fixture correctness, not a speedup. No HIP/HSA baseline or native GPU execution
is included, and previous native measurements do not qualify this new source.

Observed admission-only elapsed time, milliseconds rounded to three decimals:

| Active records | GNU reference | GNU candidate | musl reference | musl candidate |
| ---: | ---: | ---: | ---: | ---: |
| 64 | 0.064 | 0.142 | 0.062 | 0.077 |
| 256 | 0.091 | 0.022 | 0.088 | 0.025 |
| 1,024 | 1.106 | 0.106 | 1.376 | 0.120 |
| 4,096 | 18.280 | 1.215 | 14.942 | 1.181 |
| 16,384 | 271.415 | 9.091 | 226.823 | 6.070 |
| 65,536 | not run | 32.142 | not run | 23.270 |

The candidate was slower in both 64-record observations. This packet does not
establish a small-backlog latency improvement or isolate allocator, cache and
shared-host scheduling effects. The algorithmic change removes quadratic
ready-index scans; the table alone is not a statistical speedup claim.

## Replay

```sh
python3 -I -B docs/evidence/dev-xgmi-peer-batch-admission-cpu-2026-09-19/verify.py --live
python3 -I -B docs/evidence/dev-xgmi-peer-batch-admission-cpu-2026-09-19/test_verify.py
```

The recorder is non-overwriting. Raw command receipts, test-name rosters,
before/after source snapshots, tool hashes and compiler identities are bound
by `binding.json`; `SHA256SUMS` seals the exact archive inventory. Replay checks
the recorded evidence, while `--live` also requires current source equality.
The older September 18 evidence packets are unchanged.
