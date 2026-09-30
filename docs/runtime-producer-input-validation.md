# Runtime Producer Input Validation

This component connects the runtime's actual per-input validation body to a
typed observation specification. Fresh affected CPU tests, full unfiltered
validator verification, selector observation, and native static gates pass.
The complete logical-negative campaigns remain pending. This is not an
end-to-end proof of concrete Context, journal, or allocation-credit behavior.

## Implementation

`ProducerInputObservationsV1` borrows the existing runtime Context, version
storage, and retained producer-read root. Its `validate` method invokes
`producer_input_validate_body!`; `reconcile` invokes the existing
`producer_input_fold_body!`. Neither entry point gains a public API or changes
the ownership representation.

Three concrete iterator `any` calls previously prevented the pinned verifier
from establishing membership contracts. The concrete slice iterator overrides
`any`, while the relevant iterator specification supplies a default-method
contract. Explicit closure contracts did not establish the missing concrete-call
postcondition. Those diagnostic runs are retained as rejected evidence, not
qualification.

Two private helpers now use source-shared bounded scans:

- `producer_dependency_contains_v1` compares the complete dependency value.
- `producer_source_pair_contains_v1` compares both the region and record of the
  same source entry, in that order.

Each helper is allocation-free, terminates at the first match, and returns false
after exhaustion. Each scan is O(n) time and O(1) additional space, preserving
the previous membership operation's asymptotic behavior. This does not claim
that the entire multi-input validation workflow is linear or faster in measured
execution time.

Production and proof wrappers instantiate the same loop-body macros. The three
call sites remain at their original short-circuit positions: launch dependency,
launch source pair, and peer dependency. No diagnostic control-flow expansion,
additional semantic assumption, or `external_body` is introduced. The original
2003-byte fold prefix remains unchanged.

Primary source files:

- [Native adapter](../crates/fe2o3-runtime/src/context/versions/producer_readers.rs)
- [Shared bodies](../crates/fe2o3-runtime/src/context/versions/producer_input_fold_body.rs)
- [Actual helper and shared-body tests](../crates/fe2o3-runtime/src/context/versions/producer_input_fold_tests.rs)
- [Validator proof](../crates/fe2o3-runtime-model/verus/context_producer_input_validate_v1.rs)

## Proof Boundary

The validator proof expands the actual validation body and both actual scan
bodies. Its closure consists of the proof root and shared body file. Scan loop
invariants establish that the checked prefix contains no match; bounds and a
decreasing remaining length establish safe finite traversal. Concrete identity
and schema checks retain all relevant fields, including both coordinates of
runtime identities.

The existing validator contract establishes:

1. The local stored owner value is framed.
2. The supplied typed observation-return values are preserved.
3. The returned status or error equals the specified per-input outcome.
4. The active-family cursor equals the specified outcome cursor.
5. The queued-family cursor equals the specified outcome cursor.
6. The helper-call trace is extended by exactly the reached calls, with their
   arguments and order.

These are conditional contracts over the supplied typed observations and the
existing hash-key model obligations. The native helper forwarders and concrete
schemas are source-checked, but the implementations behind journal lookup,
journal status, live-allocation validation, and credit observation are not proved
by this packet. A byte-level source guard is not a theorem about those helpers.

Local owner equality does not establish immutability of an `Arc` pointee,
mutex-protected state, or allocation account. Each reached native credit predicate
is evaluated afresh; no coherent account snapshot is inferred. Native freshness,
lock recovery, allocation failure, unwind behavior, compiler correctness, ISA
execution, GPU behavior, and full-runtime parity remain outside this component's
proof claim.

## Accepted Evidence

The retained development records below are local evidence packets, not files
published in this repository. Packet names and SHA-256 digests identify the
measurements without presenting local paths as public artifact links.

| Measurement | Observed result | Scope |
| --- | --- | --- |
| Runtime test listing | 1930 tests listed | Listing, not execution of all tests |
| Affected CPU groups | 112 passed, 0 failed, 0 ignored | Four complete selected groups: 18, 62, 22, and 10 |
| Shared body and helper tests | 14 passed | Included within the 112, not additional passes |
| Full validator verification | 42 verified, 0 errors | Complete unfiltered proof root with no cheating |
| Selector observation | All 8 stages passed | Two full42 positives bracketed three selected mutation observations |
| Native static gate | All 5 stages passed | Scoped formatting, fresh no-default library build, strict Clippy, and whitespace checks, with base-signature and continuity checks |

The 42 count is the verifier-reported obligation total, including derived
equality, order, and helper obligations. It is not 42 independent runtime
properties or a count of tests.

Two direct native-helper tests cover empty inputs, mismatches, first and last
matches, complete identity-field differences, and source region/record pair
differences. They call the actual private helpers rather than a parallel test
implementation. Existing shared fold and validator tests remain included.

The CPU executable was freshly built with all five runtime feature flags,
opt-level 1, debug information disabled, and debug assertions and overflow checks
enabled. The retained executable is 43,514,320 bytes with SHA-256
`693f870199f0dbf0be4963f4501285de460d9681d65c024cc9e4ea503499e153`.

All three selector observations produced actual postcondition failures. Their
only previously unreviewed inert selector note was
`verifying root module (selected functions)`. Source controls now pin this exact
note separately for each permitted selector. The observation packet explicitly
does not qualify the logical-negative campaign.

The fresh no-default library build and strict no-deps, all-feature, all-target
Clippy both passed without warnings. Clippy retained `-D warnings`; no lint
allowance was added. One fresh runtime library artifact and 29 fresh Clippy
runtime artifacts were recorded. No historical-warning exception was exercised.
The static classifier requires one terminal Cargo `build-finished` record with
Boolean `true` and ordinary canonical output paths inside the dedicated target.

| Local record | Result SHA-256 |
| --- | --- |
| `cpu-discovery-attempt-4/results.json` | `61c2306cbf32b937a46179faa71678069a06776ade6e953b2117e8dbd79149ab` |
| `selection-capture-attempt-1/results.json` | `ca7404c668db666a2ce56090280782469150c48a53c734579a1a3afa707a1cf6` |
| `static-gate-attempt-2/results.json` | `f182f131bcb9cdddc90666520c41e7d12c32574b40f78b9fa570f39d69dddeb6` |

Each recorder retained source/tool/raw-input continuity and closed all process
groups it created in its unchanged PID namespace: ten CPU/discovery groups,
eight selector groups, and five static groups. This is a per-run ownership
statement, not a claim that the host had no other work. The accepted native
bodies, proof contracts, CPU tests, and retained CPU executable were unchanged
across the count/selector/checker metadata updates.

## Remaining Qualification

The validator campaign still requires full42 before, exact relocation full42,
all 38 logical-negative controls, full42 after, and release checks on the signed
candidate. The controls comprise 28 validation-body mutations, five dependency
scan mutations, and five source-pair scan mutations. Source-only controls check
the complete roster and enforce each mutation's intended macro region.
Constructing a mutant does not establish that verification rejects it.

The older fold checker now ends its mutation interval at the first scan helper,
with explicit guards on the current shared body and unchanged 2003-byte fold
prefix. Both scan helpers and the validator are excluded from fold mutations.
A guard refresh alone is not requalification: fresh full13 before, relocation
full13, all 22 logical-negative controls, full13 after, and release checks remain
required on the signed candidate.

Compiler errors, malformed diagnostics, unknown selection notes, warnings,
timeouts, and incomplete campaigns cannot count as qualified logical negatives.

## Next Composition Boundary

Fresh full13/all22 fold qualification preserves the separately proved
receipt-driven fold theorem. Combining its successful count with the validator's
42 obligations does not establish a composed concrete Context theorem.

The next proof boundary is the call from
`ProducerInputObservationsV1::reconcile` to `ProducerInputObservationsV1::validate`.
A composition proof must establish that every actually reached validator
execution produces exactly the family, cursor-advance, status/error, and reached
observation receipt expected by the fold theorem. It must preserve the reached
prefix, first-error propagation, final family-count checks, and local ownership
frame across all input iterations. Later inputs must not acquire receipts after
an early return.

The following native refinement boundary is the six `observe_*` forwarders.
Their typed return values must be related to the actual journal lookups/status,
`validate_live`, and fresh `has_expected_credit` calls at the time each call is
reached, under the real borrowing and interior-mutability rules. The present
typed observations must not be promoted into an assumed native snapshot.

These follow-on boundaries do not require expanding this candidate's runtime
semantics. They require additional composition and native-state evidence before
broader formal-verification claims can be made.
