# R108 Context Writer Issuance Evidence

## Scope

Local source/test acceptance for V1 / VER-1A.2a in
[`context_version_journal.rs`](../../../crates/fe2o3-runtime-model/src/context_version_journal.rs),
its nested tests and the model module export. These are the only three non-doc
changes relative to accepted R107 `589c6f6fd90048643a57cc4d0e250cb9698550a6`.
The publication parent is planning-only
`f767fad35d24f585e8ecb95558bce97a9d543cfe`.

The model implements construction, existing-ID registration, exact Reserved
lookup and explicit pre-effect Reserved abort. It preallocates W slots/free-stack
storage, retains independent A configuration, permits ID gaps and never rolls
back its registration watermark. References are inert Copy projections, not
authentic runtime authority. Discarding one does not release its retained slot.

This is not a production Context journal, membership/Begin or settlement,
authenticated formal correspondence, cross-run reuse, native execution,
aggregate-memory closure or a HIP/HSA performance result. No SSH, GPU or solver
run was performed for this acceptance.

## Accepted Checks

| Check | Result |
| --- | --- |
| Frozen source campaign | 17/17 gates, unchanged source |
| Selected auxiliary campaign | 10/10 gates; all 20 Linux-helper named results intact |
| GNU tests | 2,605 passed, 5 ignored, 0 failed; 48 harnesses |
| Musl tests | 2,605 passed, 5 ignored, 0 failed; 48 harnesses |
| Frozen and fresh-restored model suites | 11/11 each |
| Frozen and fresh-restored credit suites | 3/3 each |
| Frozen and fresh-restored batch suites | 6/6 each |
| Selected compiled negatives | 12/12 compile and fail their exact planned assertions |
| Evidence-clock contract tests | 16/16; exact seven-helper hashes captured before tests and unchanged afterward |
| Source restoration | All 5,669 non-doc path/hash identities match the freeze |
| Final collector | Exit 0; closed transcript binds the exact collector and summary |

The valid frozen `r108-final-*` source campaign and the fresh
`r108-accepted-auxiliary-*` campaign are reused without changing their source or
commands. Selected negative and restored-positive records use `r108-clock-*`.
Compiler/linter, production/strict Clippy, documentation, host/macros, Python,
format/whitespace, dependency/policy and lockfile gates are retained. Supplemental
Cargo rosters are compared by exact passing names and harness totals, not just
aggregate counts. Production metadata stdout and stderr are both retained.

The eleven model tests include exact state/pointer/capacity rejection snapshots,
independently exposed logical/backing abort-capacity guards and counted-work
controls at empty, half-full and nearly-full occupancy. Successful registration,
lookup and abort use 4/1/3 counted primitives. Stable Vec storage and these
counters are not allocator instrumentation or a general complexity proof.
Constructor allocation failure is source-reviewed, not dynamically injected.

An independent BTreeMap oracle covers nine actions at depth four for W=1 and W=3:
13,122 traces and 52,488 steps, not an exhaustive unbounded transition proof.
The twelve negatives cover lookup/abort/capacity watermark errors, partial-key
checks, free-slot retention, integer edges, linear lookup, registration growth
and both independent abort-capacity guards.

## Preserved Rejections

The first auxiliary log reports twenty passes but interleaves child output
inside one named result, leaving nineteen intact named pass lines. The original
collector rejects this exact-roster mismatch. The log is not normalized.

The original mutation cohort has next-start/restoration UTC inversions of
55 ms and 349 ms. Its repeat has a restoration timestamp 236 ms before that
run's recorded finish. Both mutation cohorts remain unaccepted; raw timestamps,
logs, source maps, restoration receipts and helper bytes are preserved.
The five-second repeat pauses were host orchestration, not a measured minimum
UTC interval, and did not protect the finish-to-restoration boundary.

The corrected, pretested/pinned contract gates each new run start on its exact
predecessor and each restoration receipt on its run's raw finish. It records
the actual qualifying UTC sample, never a clamped or rewritten timestamp.
Waits have a separate ten-second monotonic budget and 1,001-sample bound;
failure rejects and prevents subsequent execution. Raw child-finish regressions
still reject. Command duration and post-command source-hashing duration are
recorded separately. These records do not establish the clock anomaly's cause.

The accepted `admit-max` run exercised this gate: its first observation was
524 ms below the predecessor floor; after 51 samples and 527.851513 ms of
monotonic waiting, the qualifying observation was 4 ms above that floor.
No runtime test deadline, concurrency setting or oracle was relaxed.

## Provenance And Next Work

[`test-summary.json`](test-summary.json) indexes all 281 exact raw artifacts.
The new manifest preserves 210 historical artifacts and specifies all 28
run/restoration/validation links before execution. Predecessor checks bind the
planned event, source/manifest identity, exact bytes and expected outcome.
Archive creation verifies equality with the already closed successful collector
transcript. Failed and preliminary artifacts are retained, not acceptance inputs
silently substituted for successful ones.

V2 next adds allocation membership and whole-roster Begin; V3 adds settlement;
V4 supplies property-specific proofs and separate Rust correspondence; V5/V6
integrate the actual Context journal/hooks. Complete V7 write-family coverage,
ordered writers and Unknown recovery precede V8 cross-run leases. Native N3-L1,
Admission C1, memory-domain closure and hardware/performance qualification remain
open on the [current board](../../runtime-a1-a2-swarm-current.md).
