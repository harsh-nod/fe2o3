# Original-meter root CFG qualification — 2026-09-26

This prerequisite for #280/#282 retains the actual source control-flow graph
alongside the joined rich tables and complete-CFG induction report. It does not
complete root projection, grant ordinary compilation, or close a broad milestone.
Accepted broad exits remain M1/V1/V2/U1/U2/U3 (6/18).

## What changed

The existing source-loop CFG implementation now has a shared strict resource
mode. The legacy entry retains its target checks, refusal order, boolean
empty-unreachable fallback, sorted/unique adjacency, predecessor order and
reachability semantics. The strict mode prepays graph scans, sorting, adjacency
capacity and reachability work on the original ledger. Pre-counted incoming
edges and bounded pending capacity prevent unaccounted growth.

An owner-bound lexical scope retains that graph with the actual admitted source
and induction/rich tables. The source pointer, semantic hash, function and
identity are rejoined; equal but separately constructed source is rejected.
The true incoming owner floor and complete closure/result frames are checked
before nested reservations. Partial graph/report values and panic payloads drop
before own-only refunds; callback surplus and sticky denials survive.

These are analysis facts, not a ranked CFG, assertion certificate, or bypass of
canonical bounds/access checks. There is no assumed-proof fallback.

## Fresh qualification

The first new gate, `compiler-bf16-root-cfg-genuine-r1`, passed 331 model tests,
2,481 backend tests (189 intentionally ignored), backend/extractor build and
all five fresh genuine Rust-kernel sessions. The added 21 tests cover shared
graph behavior, exact/one-short budgets, source substitution, partial refusal,
callback error/panic/surplus, frame accounting and custody. Genuine hooks use
the original budget and actual owner/caller; only the negative F-1 probe uses
a separate ledger, prepaid on the original one.

R13 observation: 281,064 bytes, SHA-256
`ea56fb96e7021e211983c6f3ecc5d46b3a20882a664eb05f6cf6430659155be1`.
Genuine gate receipt: 178,673 bytes, SHA-256
`e9a4acf20a692682ef60fbb37525fd8dc7cf1ffbce8b6201faf1a86eef338903`.

All 12 lossless-checker controls passed again. The independently gated R12-to-R13
comparison rehashed both 459-file dependency trees (352,670,356 bytes each).
Numerical results, masks, refusals, admission, storage and peak remain exact;
no peak override exists. Of 132 changed fields, 96 are reviewed work increments,
25 are generation paths, six dependency digests and five artifact digests.

Comparison receipt: 42,240 bytes, SHA-256
`8fcbd9a63283c6188d1499ef89d1c8d23a8b3ae1d6acc13e34fb3ab036e6383d`.
Comparison result: 38,229 bytes, SHA-256
`3295566d610645d263e695e920ba1ad234c291a917adba3be2be1b99bdecac2e`.

Uniform extra work is 8,525,897 for Identity/error/panic and 8,526,067 for
Swap01. Source constants prepay 8,454,656 diagnostic work and 51 callback
surcharges. The residual across five preparation-plus-observer pairs implies
14,238 / 14,272 per pair; preparation and observer costs were not independently
instrumented. Graph-dependent scans are charged before traversal. The negative
probe is not charged twice, no cap was enlarged, and these logical work units
are not kernel timing or allocator/RSS measurements.

## Next boundary

The full existing assertion/range evaluator still needs original-meter support
and a same-source retained decision mask. Its legacy bounds-check convention
must not become a standalone safety certificate. Complete root memory/effect
projection, generated ranked-coordinate correspondence, attachment,
formal/target/LLVM continuation, edited-source promotion and hardware acceptance
remain open. Public debugger activation is unchanged and disabled.
