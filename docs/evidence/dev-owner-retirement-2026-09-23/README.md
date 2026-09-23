# Public Allocation Retirement Correspondence

This developer checkpoint connects allocation retirement across the journal,
stable-read owner and producer-read owner to an independent logical executor.
It extends the [scalar enrollment checkpoint](../dev-owner-scalar-enrollment-2026-09-23/README.md).

## Scope

- Ordinary Rust and actual-owner Verus methods instantiate the same five bodies:
  journal preflight and execution, the slice unread guard, outer validation and
  outer execution. Frozen Rust baselines come from
  `3d686c5ca4bbf8a3a58cb9de357f00aab2db7116`, with authenticated method
  name/visibility changes, redirected baseline calls and prescribed formatting.
- Journal admission preserves roster bounds, exact identity before canonical
  order, canonical order before pending membership, checked return counts and
  logical before physical return headroom. Physical capacity is observed lazily,
  only after earlier admission succeeds. Empty rosters still check headroom.
- Outer owners preserve unread-before-child precedence and repeated validation.
  Raw journal contracts are total over represented normal contents. Outer
  execution needs only reached-prefix reader-count safety: it does not require
  well-formed unrelated count storage, free prefixes or global custody.
- Paired execution requires represented owners, mapped rosters and reached-prefix
  reader-count safety. Both actual and logical methods execute independently;
  the logical executor does not use the shared Rust bodies. Capacity is an
  explicit proof observation, not a
  refinement of actual Vec capacity or the ghost storage-capacity label.
- Rejection preserves the complete owner. Success clears exactly the selected
  allocation slots and appends their slot indices in caller order, preserving
  the other fields. Canonical keys need not have monotone physical slot indices.
- Producer invariants and issued custody are preserved conditionally on their
  initial validity. Under the initial producer invariant, stable-lease and
  producer-reservation lookup results are preserved for all reference arguments,
  including invalid references. Producer-status results are preserved for
  requests held by pre-existing live reservations, not arbitrary unretained
  requests to retired allocations.
- Synthetic-start witnesses cover live stable/producer reads, busy rejection,
  nonmonotone slot retirement, fresh-identity LIFO reuse, short-circuit rejection
  before malformed reader storage and empty-roster headroom rejection.

The fourteen added CPU tests include frozen-versus-candidate comparisons on the
same physical owner, with touched-only restoration and state/storage-identity
checks. They cover simultaneous faults, every roster fault position, both reader
kinds, all four producer statuses, malformed private state, nonempty success
with short unrelated count storage, raw identity extremes and free-stack order.
Counted successful journal lookups for a roster of k entries are k/k for
journal validation/retirement, 2k/3k for the stable owner and 3k/6k for the producer
owner. Successful retirement observes return capacity 1/2/3 times respectively.
These are selected work counts, not timings, total-work measurements or HIP/HSA
comparisons.

## Qualification

Source: `adae0c1fc2eba6ad96a24bf1970af1d5bfb3e752` (also recorded in `SOURCE`).
The source-bound campaign completed with 389 authenticated input files:

- Paired whole-root proofs: 999 verified, zero errors, before and after controls.
- All 25 scoped negative controls failed logically as intended, without frontend
  errors, timeouts or solver resource exhaustion.
- Raw-owner regression: 331 verified, zero errors.
- Scalar-enrollment regression: 929 verified, zero errors.
- CPU suite: 980 unit tests and 27 doctests passed; 18 unit tests intentionally ignored.
- Formatting, warnings-denied all-target Clippy and release test build passed.
- Recorder tests passed: 29 malformed or conflicting operation rejections and
  eight malformed negative-diagnostic cases rejected.
- Both pinned Verus distribution checks matched: 190 files, 129019839 bytes.

All 37 serial commands have terminal receipts and controller acceptance. Bounded
resume retained completed records; a final complete-campaign replay left all 115
record files byte-identical. Raw stdout/stderr are packaged unchanged. The packet
contains 119 files overall; `SHA256SUMS` covers the other 118. No exploratory or
interrupted command is counted as qualification.

## Replay

Run `crates/fe2o3-runtime-model/verus/check-owner-retirement.py` from the `SOURCE`
checkout with `--repo`, `--verus`, `--output` and `--target`; use a fresh output
directory and owned Cargo target directory. The recorder authenticates frozen
methods, exact runtime adapters, include envelopes and the closed source
discovery roster, and brackets source hashes and the pinned Verus distribution.

Long campaigns may use `--stop-after <case>` and then `--resume` with the same
arguments and source checkout. An incomplete checkpoint is not qualification.
The recorder holds an exclusive controller lock, validates the saved prefix
before new work, and reuses only exact-source, controller-accepted records.
`accepted.json` binds each completed command's three raw files after normal
process cleanup and semantic validation. Interrupted or unaccepted records are
not automatically retried or discarded. Controller acceptance is not independent
execution attestation. All 37 commands must finish before final summaries exist.

The packet auditor accepts `--repo <git-repository> --selftest` and reads source
from Git objects without needing a source checkout. It checks integrity, source
identity, exact command scopes/results, diagnostic policy and terminal receipts.
It does not rerun Verus, independently attest execution or verify commit
signatures. Its 26 rehashed-corruption selftests include source, scope, result,
receipt, acceptance and diagnostic-policy mutations. Arithmetic-overflow errors
are allowed only for the unchecked-addition negative control; frontend errors,
resource exhaustion and abort-only diagnostics do not qualify. Mutations rehash
both packet and controller records where applicable, exercising semantic checks.

## Limits

This is normal-state content correspondence, not physical Vec refinement.
Actual construction in witnesses is synthetic. Fallible construction, allocator
failure and unwind behavior, complete trait/wrapper coverage, Unknown disposal,
native disposal authority and native pending-consumer admission remain open.
No GPU, XGMI, HIP/HSA timing or parity claim follows from these records. The CPU
environment is not a hermetic build attestation. Negative diagnostics are
recorded observations, not precommitted diagnostic replay.
