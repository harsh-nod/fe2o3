# Retained Shared reads and observer qualification — 2026-09-29

This private component extends the retained alias owner with original read-place,
operand and scalar-operand processing, and retains the original SharedReads
observer rows. It does not activate ordinary analysis or prove a source/SSA
association.

## Implemented retention

Read operations retain their current alias, unvisited drain, selected aliases,
field vectors, output and borrowed source through fallible work. A candidate's
read flag is set at the original point, before observer work. Moves deactivate
only after the original observation and selection sequence. Scalar processing
attaches selected aliases before discarding them; zero-length vectors retain
their actual capacity.

Observer preparation attaches the actual original Reads owner before admission.
Typed frame, scratch and row storage are reserved in order, followed by attached
allocation, observed excess-capacity admission and exact binding checks. The
original physical Budget, Work lifetime and owned-credit counter remain paired;
there is one first-failure slot and no reconstructed ledger or refund escape.

Recording prepays the original unit of work before the original row-cap refusal.
A successful push uses admitted attached capacity without allocation. Refusal
keeps original partial rows and becomes terminal; ignored errors cannot restart
recording or alias work. Preparation is not source membership, promotion or
canonical-facts authority.

These envelopes do not prove a hard RSS or allocator-OOM bound. A refused
excess-capacity charge leaves the actual allocation attached; it does not mean
the excess was funded. Recoverable allocator failure and excess-capacity
injection were not forced by these tests.

## Qualification

Root reviewed implementation, controls, original donor transformations and the
independent review. Formatting was twice idempotent. The first build exposed
test-only type/privacy errors and remains archived as failed. Its narrow
successor gives one result an explicit type, uses existing test-only coordinate
accessors, and constructs inert fixture sites inside the original private
owner. Production visibility and algorithms are unchanged.

The corrected broad CPU regression on base
`1a5999f6e1c5f2363bc2d525af65e84c46502ce6` passed:

- 12 read and nine observer controls within 1,743 pliron tests;
- 370 model and 3,390 backend tests (197 backend tests remained ignored);
- 270 authority/capability tests (four ignored) and 20 policy/runtime controls;
- 391 broker, 253 coordinator and 258 protected-spawn tests (20, four and seven
  separately gated tests remained ignored);
- backend/extractor builds and whitespace checks.

The tests compare original data and work prefixes at work/storage refusal cuts.
They cover all operand forms, partial read state, original zero-cap side effects,
row-cap fallback, exact source and physical-account binding, terminal
continuation, unwind custody and drop-before-refund. None supplies an actual
whole-function source/SSA acceptance proof.

Full corrected regression receipt SHA-256: `908ecb7efcf2b455a8e111bbe81ce5317f4e7977f8676a826066c38967665438`.
Failed first-build receipt SHA-256:
`698e9d1c1338d1d79269bc725a7f44df0c0dc8e4048181106c51be71fb994cbe`.
No ignored native/GPU test is counted as executed.

## Remaining integration

Rvalue production, installation, statement/terminator processing and the
complete original analyze-observed loop must join these components. Real
SharedReads source/SSA/promotion construction, the canonical-facts backend
bridge and the original Shared-first whole-root preparation remain necessary.
Ordinary compilation, argument writers, genuine nonempty Fixed qualification,
native debugging and physical capture are not activated here.

Broad accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).
