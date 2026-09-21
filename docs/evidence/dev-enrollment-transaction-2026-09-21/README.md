# No-Sort Enrollment Development Experiment

Production enrollment is unchanged. This packet retains a candidate, a separately
scoped logical proof, independent differential tests and CPU measurements. It
does not advance a native milestone or establish HIP/HSA parity.

`SOURCE` identifies the signed Git commit containing the exact source files.
The recorded input hashes are checked against those Git objects; the development
runs preceded that source commit, not a clean-commit qualification campaign.
`SHA256SUMS` covers the complete packet except itself. No scratch paths, compiler
installation or GPU are needed for offline validation:

```sh
python3 -B docs/evidence/dev-enrollment-transaction-2026-09-21/audit.py --repo .
python3 -B docs/evidence/dev-enrollment-transaction-2026-09-21/selftest.py \
  --repo . --proof docs/evidence/dev-enrollment-transaction-2026-09-21/proof
```

## Proof Boundary

The raw executor needs no journal invariant. It retains the exact admission/error
decision and historical success relation. Installation detects selected-slot
duplicates through originally vacant slots; retained overlap follows from the
unchanged full-key replay preflight. Reverse undo restores exact logical contents.
The issued wrapper preserves custody, stable readers, producer reservations,
issuance and all previously valid producer statuses without a globally idle state.

The separately named error relation proves equality of the allocation vector's
sequence view, **not its opaque Vec object identity**. Untouched journal vectors
retain object equality. The old contract implies the new relation, not conversely.
No axiom was added and the historical pinned contract was not weakened. Physical
storage/capacity, allocation and unwind behavior, Rust correspondence, and native
effects remain unproved. The candidate is not promoted or registered in the
shared proof runner.

`proof/` records two whole-crate runs at **275 verified, 0 errors**, including
263 inherited and 12 local obligations. Six body-only controls each report
**274 verified, one exact intended postcondition failure**: duplicate undo,
overlap undo, output restoration, retained free prefix, success result and
retained-overlap detection. Counts overlap the inherited campaigns, not additional
global obligations. Both distribution closure checks cover 190 files and
129,019,839 bytes. Input identities, source mutations, exact diagnostics and owned
process-group absence are rechecked by the offline auditor.

The diagnostic self-test rejects 28 adverse results. An earlier incomplete run
failed classification because a mutation tripped a proof assertion rather than
the intended postcondition. That run is excluded; the corrected control changes
only executable behavior after the restoration assertion.

## CPU Qualification

`cargo/` records **817 passing production unit tests**, two intentionally ignored
benchmark-style tests, **27 passing doctests**, formatting and Clippy with warnings
denied. The new immutable quadratic oracle checks 199,936 small malformed arena
states and another 1,000 capacity/shape/header cases. Comparisons include all
journal fields, exact output, returned errors and vector storage identities.
Replay-scan instrumentation is checked separately.

`cpu/` builds fresh standalone baseline and candidate packages from the same
source snapshot, compiler, dependency lock and explicit release profile. Baseline
is `ee6489242a5f9976847193fada2d5fd966b25353`; candidate changes only the archived
allocation-lifecycle implementation. Both receive the same new tests. One runtime
source file is also captured for an existing unit test's `include_str!` assertion.
Both variants pass the same 817 unit tests. Benchmarks use ordinary release
libraries, without `cfg(test)` counters. Executables and sources are hashed before
and after their four runs.

The healthy-success matrix has 69 cases: capacities 64/1024/4096, empty/half-full
arenas, batches 1/16/63/all remaining where admitted, and ascending/descending/
deterministically shuffled selected slots. Five timed blocks per case per run,
in baseline/candidate/candidate/baseline order, yield 1,380 blocks. Each block has
16 to 256 independently prepared journals. Construction, retirement, output
allocation and exact result validation occur outside timing. CPU affinity is
fixed, but this shared WSL CPU has no exclusive reservation. Short blocks and
host activity limit inference. The separately recorded loop floor is not
subtracted. Raw per-run medians, min/max ranges and paired ratios are retained;
the parser rejects 13 adverse CSV matrices.

Observed candidate medians were lower in 58 of 69 cases and higher in 11.
Baseline/candidate ratios ranged from 0.749 to 4.868. For a 4,096-slot empty arena,
a shuffled 63-item batch measured 65.53 us baseline versus 13.46 us candidate;
the 64-slot descending single-item case measured 156.20 ns versus 208.48 ns.
These are scoped observations on this host, not guaranteed speedups or confidence
intervals. Performance is mixed. Consult `cpu/summary.json` and the
raw CSV streams for every case, including regressions. This is only enrollment
metadata CPU cost, not copy bandwidth, kernel execution, rollback timing, latency
distribution, or comparison with HIP/HSA. An earlier exploratory comparison and
an incomplete extraction missing that unit-test source dependency are excluded.

## Reproduction

Both drivers require a fresh, nonexistent output directory:

```sh
python3 -B docs/evidence/dev-enrollment-transaction-2026-09-21/check.py \
  --repo . --verus /path/to/pinned/verus --output /path/to/fresh-proof-output
python3 -B docs/evidence/dev-enrollment-transaction-2026-09-21/compare.py \
  --repo . --output /path/to/fresh-cpu-output
```

The measurement drivers use the development host's explicit Cargo/Rustup paths;
adjust those when reproducing elsewhere and treat that as a new environment.
No MI300X work ran and no files or processes on the shared SSH host were touched.
