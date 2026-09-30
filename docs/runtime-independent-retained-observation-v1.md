# Independent Retained Credit Observation

The independent-account branch of `ResourceCreditAccountV1::matches_retained_charge_v1`
now shares its actual post-lock observation body with Verus. It borrows the
locked record slice and forwards the stored poison flag, token slot and owner,
and complete expected resource vector. Public APIs and resource lifetimes are
unchanged; token presence, account-kind dispatch, Arc identity and raw Mutex
acquisition remain in the caller.

## Theorem

The result is true exactly when the stored state is not poisoned, the requested
slot is in bounds and occupied, and that record has the requested nonzero owner,
the `Retained` phase and all nineteen expected charge coordinates. Poison and
bounds rejection precede indexing. The actual borrowed record predicate is
called only for an occupied slot; no cached validity receipt replaces it.

The five-file executable closure includes the existing retained-record theorem,
its production body and the vector declaration, plus the independent observer
root and production body. The source checker also binds the complete accounting
Rust roster and exact native forwarding expression. There is no new local
`assume`, `admit` or external proof body. Pinned vstd slice/array/equality
specifications and Rust derive/compiler lowering remain trusted boundaries.

This proves a single observation over supplied locked-state fields, not Arc or
Mutex semantics, cross-call freshness, conservation, token custody, Context
allocation identity, native execution or machine-code correctness. It is not a
coherent snapshot of multiple accounts. The separate
[domain observer](runtime-domain-retained-observation-v1.md) additionally checks
the actual bounded ancestry and leaf identity for domain accounts.

## Qualification

The campaign on signed candidate `4e340d5f24f04d7e1f7f3040eec00fd4e6b0aa2c`
passes three full eight-obligation `--no-cheating` runs, including the exact
relocated five-file closure. Integration at `f8994fdb8` preserves all ten changed
paths exactly; it is not a new merged-tree solver run. These counts overlap and
include the existing record proof; they are not twenty-four distinct theorems. All ten
actual-body logical negatives are rejected. They cover poison, slot bounds,
record selection, empty records, and owner/charge substitution. The two removed
or weakened slot guards produce the exact index-bounds proof failure; other
mutants produce postcondition failures. Compiler errors, warnings and timeouts
are not accepted as logical negatives.

Both 190-file verifier-release checks and all twenty-one fresh campaign process
groups close successfully, with unchanged signed source and tools. A separate
five-stage signed-source run passes strict all-feature/all-target accounting
Clippy, scoped Rust formatting and commit whitespace checks, with five fresh
process-group closures. These observations are local to each recorder's PID
namespace, not claims about historical PIDs or the whole host.

Earlier CPU qualification passes all 79 accounting library tests without
failures, ignores or filters. All sixteen accounting Rust files, the executable
proof closure and build inputs remain byte-identical to the signed candidate.
Six checker/control/manifest changes are listed separately in the byte-binding
receipt. This reuses pre-signing CPU evidence; it is not a fresh signed CPU run.

The older record/domain source guards and R75 source manifest include only the
reviewed accounting changes. Their executable closures and campaign counts are
unchanged. Lightweight calibrations pass without new standalone solver claims.
Raw qualification remains local under
`fe2o3-independent-retained-observer-qualification-20260930-retained`.
No combined full-runtime, GPU, performance, HIP/HSA parity or A0-A7 milestone
acceptance follows from this component qualification.
