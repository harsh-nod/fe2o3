# Shared invocation-index preparation

Status: a qualified compiler prerequisite; the resource-accounted component
still produces unjoined data. Accepted broad exits remain M1/V1/V2/U1/U2/U3
(6/18).

## Implementation

Ordinary compilation and the bounded preparation component now share the same
invocation-index seed, propagation and assignment implementations. The ordinary
adapter borrows its existing operation stream, next value ID, tables and FIFO;
it does not restart IDs, move earlier operations or change the position of
GridLeader recovery. Existing diagnostic text and partial-emission/refusal
ordering are preserved.

The separate bounded component supports ThreadIndex1d seeds and the admitted
alias/authenticated-enum-payload edges only. It rejects unsupported GridLeader
and transform families before building its tables. It prepays work, vector
growth and callback/result frames and retains partial storage in its outer
owner through error and panic postflights. Occupied retries and foreign-ledger
reuse are refused. Credits are not reset or refunded early.

Its local value IDs are deliberately **not** IDs in an actual root recipe.
No public conversion, ready token, source-owner authority, guarded access or
ordinary nominal-helper admission is added. The next connector must borrow
the actual retained root inputs, launch contract and reference bindings, emit
the real existing prefix and continue its same allocator. A zero-based local
component cannot be spliced into that stream as authority.

## Qualification

The merged regression passed 331 model tests and 2,617 backend tests
(189 ignored), plus backend/extractor build. Twelve new controls cover:

- Existing nonzero value-ID prefixes, operation/table/FIFO parity and capacities.
- Original emission-before-refusal order, duplicates, conflicts and predicates.
- Broad ordinary behavior versus the narrow bounded profile.
- Exact and one-short work/storage limits, occupied state and foreign ledgers.
- Error/panic retention, enum availability, malformed shapes and ID overflow.

Three preexisting alias-cycle/conflict tests remain unchanged and now call a
test-only adapter into the shared implementation. The first regression's
missing-helper failure is retained; it was not accepted as qualification.

Five fresh actual Rust sessions passed Identity, Swap01, wrong-launch,
callback-error and callback-panic coverage. The S2 component is not newly
wired into that genuine observer; these sessions establish preservation, not
a completed root-namespace join.

The lossless R19-to-R20 comparison validates all 2,868 report leaves, both
complete 459-file dependency closures, 372 raw/typed graph diagnostic rows,
their positions and every derived sidecar. Work, numeric lexemes, masks,
refusals, storage, peaks and admission fields are exact. The 36 measured
report differences are 25 generation substitutions, six dependency digests
and five artifact digests; dependency-byte totals are unchanged. Full stderr
remains pinned; changing panic thread IDs is not a graph diagnostic change.
The comparison's 12 new controls and 55 retained controls passed.

Fresh ordinary ladders passed 36 composition and two direct-BF16 sessions.
All 38 observation bodies and 52 artifacts are byte-identical to the preceding
checkpoint. This is ordinary preservation, not nominal-helper admission.

| Retained evidence | Bytes | SHA-256 |
| --- | --- | --- |
| regression-r2 | 193,688 | `a2b923ac90c5bb14761a01c4a2897243bc3b2b2a3ddf8bc011284ba9516b4a69` |
| genuine-source-r1 | 486,135 | `1ba149efa0ee6a85af79591a897db79269977edca808f16929d49215ff6fb5eb` |
| normal-ladders-r1 | 297,425 | `d76b915291e3ee9c77892409c04757b14ca194a31cd4cd89fd222984cf34c351` |
| parity-controls-r1 | 45,913 | `9751733db5eb396d8aa0ee6638f04b3c762d1f34bb561fbdc556d432681a1cea` |
| lossless-parity-r1 | 574,535 | `38e20b32ce23cba4c11d9e351569a99a98ef7cc50d5b3439a7a4c3e78df85ce3` |
| R20 observation | 281,064 | `f57ce3f4518b6a1e863ea0b7c37f82c2ed335a7187f75d8c199ebfb10aa821f2` |
| Normal output readback | 16,576 | `e88651befb7c45b3aaa00bb28603ece1a81bb548b82d988c6ec2e0d7e3ab6131` |

## Remaining joins

Actual root-prefix/index ownership, source-ordered guarded-access appends,
reference origins and dereference sites must still be connected. Later guard,
CFG/assertion, Final-effect, bounds, reference-write and launch/placement
stages must construct one complete unverified root recipe before mandatory
verification can admit it. There is no new edited-source promotion, LLVM
continuation, GPU execution, physical capture or milestone closure here.
