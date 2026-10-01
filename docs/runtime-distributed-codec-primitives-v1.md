# Distributed Codec Primitives V1

## Qualified Component Scope

This development component covers the private byte/cursor layer of the
model-only distributed publication description. The current Writer body passed
a fresh full debug model run (1118 passed, 19 existing ignored, zero failed or
filtered) and a fresh release-profile invalid-Writer differential. Complete
unfiltered proof discovery accepted 54 obligations with zero errors and empty
stderr, including generic fixed arrays and inherited construction/classifier
units. Signed candidate `e02b7229ab6b85c2c1697d239d6f82e96ff04d45`
now passes the complete 38-stage qualification: three fresh unfiltered 54/0
positives, including relocation, 31 actual-body logical negatives and both
release brackets. All 38 owned process groups close. Independent agent and root
readbacks agree. These counts include inherited obligations, not 54 independent
runtime properties or a milestone exit. The
[raw development packet](evidence/dev-distributed-codec-primitives-2026-10-01/README.md)
preserves all three signed attempts and the original signed source.

The first signed mutation campaign was rejected when a high-byte encoding
mutation produced a range recommendation at the exact proof macro argument
instead of in the shared macro body. Its real postcondition failure did not
override the stricter diagnostic-source policy. The rejected packet remains
unchanged; an argument-location replay fixture does not retroactively qualify it.
The second attempt was interrupted after three accepted stages; it also remains
incomplete. Only the fresh third attempt supplies the complete qualification.

Earlier frontend and 51/3 logical failures remain preserved. A later discovery
reported aggregate 54/0 but was rejected for five unexpected failure-worded
notes. Inspection of the pinned verifier source showed that error-enumeration
threshold zero emits context notes even after successful queries. A fresh
release/full-proof/release observation changed only that threshold to one and
passed the unchanged strict classifier; the rejected run was not relabeled.
CPU evidence is reused only from the fresh run of the exact unchanged current
native bytes, with both original and retained debug/release ELFs bound. Existing
construction, outcome-trailer and classifier bodies and proofs remain unchanged.

The real `Reader` and `Writer` keep their existing types and forward to eight
module-private helpers. Each helper expands the same executable body in native
Rust and in `distributed_codec_primitives_v1.rs`. The implementation uses fixed
arrays and borrowed slices, with no allocation, unsafe code or public API change.
The wrapper call sites are source-bound, not independently verified object or
whole-codec methods.

## Contracts

- `take` accepts exactly a representable range inside the input slice. Overflow
  and unavailable ranges return `WrongLength` without changing the cursor.
  Success returns exactly that input subrange and advances by its exact length.
  Zero-length reads succeed at every position through the exact end, and fail
  beyond it. No initially valid cursor is assumed.
- `fixed<N>` has the same decision and cursor contract, with exact byte content
  in a fixed array. Its stack copy replaces the old, unreachable-on-success
  slice-to-array conversion error, not any malformed-input check.
- `put` requires `offset <= bytes.len()` and
  `value.len() <= bytes.len() - offset`. Under those internal capacity obligations
  it advances exactly, writes exactly the requested bytes, and leaves every
  other byte and the total length unchanged. Invalid Writer calls still panic;
  differential CPU tests check overflow and out-of-capacity cases without partial
  byte or cursor mutation. The revised body retains the original end arithmetic
  first, checks both slice splits before copying, and updates the cursor last.
  The standard-library panic text may differ; byte-identical generated code or
  performance equivalence is not claimed. This proof does not yet discharge each
  encoder call's capacity obligations, and release-disabled debug assertions are
  not evidence that they hold.
- `finish` accepts exactly the end cursor. It does not advance the cursor.
- Fixed-array `u16` and `u64` little-endian helpers preserve every bit. The proof
  additionally composes each encode/decode pair in both directions. Native CPU
  tests use the standard Rust endian functions as an independent oracle.

Header validation order and partial progress remain unchanged: a consumed wrong
domain leaves the cursor after the domain; a consumed wrong schema leaves it two
bytes later; a consumed nonzero reserved field leaves it another two bytes later.
Failed reads retain the most recent successful cursor. CPU tests bind these
checkpoints and compound-error precedence; header refinement itself remains open.

## Qualification Evidence

The new CPU tests cover all small input ranges, invalid and maximal cursors,
sequential reads, zero/fixed-size arrays, exact Writer frames and canaries,
invalid Writer panic equivalence, exhaustive `u16` values, `u64` bit/boundary and
nonuniform patterns, and actual Reader/Writer header/digest wrappers. The entire
model test suite and scoped formatting have passed for the current native bytes.
Strict all-feature/all-target Clippy and no-default library compilation also
passed for these same native bytes. Those CPU/static results are explicitly reused
across diagnostic-policy metadata changes; the final signed proof campaign does
not rerun CPU tests or static compilation.

The source guard binds all model Rust files and the exact seven-file proof
closure. Light controls construct 31 distinct, focused actual-body mutations
across all eight primitive functions. The accepted signed campaign records a
fresh genuine logical failure for each of these 31 mutations, with the exact
selected function diagnostics. Constructing cases alone is not qualification.
Parse, borrow-check, unsupported-feature, resource or
controller failures cannot substitute for logical failures. The complete count
is pinned to 54 and the error-enumeration threshold to one. A separate fresh
selector observation measured all eight families between two full 54/0 positives.
Those observations are not retroactively accepted mutation qualifications.

The primitive-specific negative wrapper requires a genuine postcondition error
in the selected function's unchanged `ensures` region, exactly zero verified and
one failed selected obligation, and exact source-coordinate, text and macro
expansion readback. It then delegates to the unchanged underlying logical-error
classifier. Each family has exactly the observed root-selection and function-body
enumeration notes; only the two encoding families also require exactly two
distinct, source-bound range recommendations. Auxiliary notes alone, frontend
errors, unknown or duplicate diagnostics, and notes on a full positive are
rejected. This is a bounded primitive policy, not a global diagnostic allowlist.
An encoder recommendation may originate only in its authenticated macro
expression or the exact final `value` argument of its unique proof invocation.
The latter requires the precise interval, source text and coordinates with no
expansion; arbitrary proof-body, declaration or other-argument spans are rejected.
At most one of the two distinct recommendations can use that argument interval.

Portable fixtures retain eleven actual diagnostic streams with source-root
placeholders and original provenance hashes: the eight initial family captures,
the rejected u16 high-byte stream, and two observation-only u64 shifted-byte
captures. Both u64 captures measured the same direct argument and macro-expression
recommendation origins, between two additional full 54/0 positives. Their original
observation-only status and the signed campaign rejection remain unchanged.
Source-only controls replay those
fixtures and reject malformed results, wrong families, coordinates, expansions
and multiplicities. Replay is classifier calibration, not fresh verifier
evidence. Signed qualification requires three new unfiltered 54/0 positives
(original, relocated seven-file closure, original again), 31 fresh actual-body
logical negatives, pinned release brackets and owned-process closure. No mutation
kill is claimed by constructing the cases or replaying the fixtures.

Integration at `a8a908878` and `8fd3b7ddb` preserves all nine original candidate
paths. The queued-query, publication-classifier and construction source guards
are explicitly rebound from 299 to 302 model Rust files, with their complete
36-, three- and five-file executable proof closures unchanged. The primitive
and construction helper pins follow that metadata-only classifier guard update.
No extra theorem follows from refreshing these guards. The eighteen-command
local component CI replay, including the new primitive source controls, passes;
it does not execute a new solver or Rust suite, or establish a hosted CI result.

## Remaining Boundary

This unit adds no new trusted executable adapter or identity-validity premise.
It inherits the existing source-checked identity/declaration representation and
structural-equality bridges. Rust/compiler, pinned Verus/vstd, standard slice
index/copy and arithmetic specifications remain explicit trust boundaries.

Complete binding and receipt decode/encode refinement is still open: actual
header decisions, all field positions and wrappers, encoder capacity discharge,
validation precedence, exact failure cursors, accepted-input canonicality and
round trips must be composed. Arbitrarily constructed invalid model values do
not get an unconditional successful round-trip theorem. Nothing here establishes
authenticated transport, publication authority, native execution, HIP/HSA parity,
copy performance, distributed participant behavior or an A0 milestone exit.
