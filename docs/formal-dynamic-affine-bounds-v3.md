# Dynamic Affine Bounds V3

Status: one bounded path-sensitive bounds fragment is mechanically proved and
connected to production ranked analysis. This result does not prove the whole
bounds analyzer, race freedom, initialization, or the compiler as a whole.

## Exact claim

For an affine index and affine dynamic extent over one exact, finite launch box
and a bounded unsigned-runtime-symbol box, an accepted certificate proves

```text
0 <= index(point) < extent(point)
```

for every integer point satisfying the retained ordered guard rows. Acceptance
also provides one exact point satisfying every row, so an empty constrained
domain cannot prove the property vacuously. The certificate contains separate
bounded nonnegative Farkas witnesses for the index and for
`extent - index - 1`; both are independently checked using the V2 checker.

V3 requires at least two distinct guard rows. The generic Verus guard theorem
establishes all rows when each exact true CFG edge occurs on every path from the
declared entry to the access and the same immutable affine SSA operands satisfy
the strict comparison that normalizes to that row.

## Production subset

The producer covers an access dimension only when all of these conditions hold:

- the ranked access uses a supported affine index;
- its ranked view extent is one exact dynamic function argument or one uniquely
  defined production `IndexUnknown` value;
- admission depends on at least two active, distinct affine path facts;
- every fact has one exact branch origin and establishing true edge;
- launch dimensions are finite and runtime symbols have the unsigned `u64`
  box; and
- all affine construction, certificate generation, and replay arithmetic stays
  within checked resource and integer limits.

For this subset the report counts every required site. A proved roster exists
only when that count is nonzero and exactly one certificate was retained for
every site. Unsupported input, duplicate guards or symbols, missing or extra
sites, ambiguous origins, malformed evidence, overflow, and certificate
generation exhaustion fail closed. `RankedBoundsReportV1::is_clean()` remains
an ordinary analysis result and is not proof authority.

Production custody reconstructs the affine index, dynamic extent, symbol box,
guard operands and normalized rows from the ranked recipe. It independently
checks each true edge, deletes it to test access unreachability, verifies the
certificate, and repeats the checks at final ranked-root revalidation. Every
guard also retains the exact semantic comparison block/statement and
condition/lhs/rhs local identities. Revalidation reopens that statement and
requires the same `LessThan` definition, while ranked replay requires the same
branch terminator, operands, successors, row, and cut-edge dominance. Guard
sources are unique and canonically ordered. Each retained site exposes these
guard locators together with the semantic access root/block/statement/access
ordinal and the PLIRON block/operation/dimension, access kind, view/index SSA
identities, runtime symbols, certificate, and proof identities. This is
sufficient for a top-level composer to join the three representative
read/read/write sites to their KIR memory-operation locations without
reconstructing private proof evidence.

The representative fixture has one invocation id and three distinct dynamic
extent arguments. Its enabled path is guarded by `gid < input_a_len`,
`gid < input_b_len`, and `gid < output_len`; the corresponding exact extent SSA
is retained separately at each of two read-only input accesses and one output
store over distinct allocations. Existing race analysis reports that fixture
clean, but V3 grants no race-freedom or initialization theorem. Read-only input
initialization is a caller obligation, and output disjointness remains outside
this proof.

## Identity and TCB

The runner pins the V3 theorem, its imported V2 theorem, every hostile fixture,
the runtime Verus executable, and the complete Verus/vstd/Z3 closure manifest.
The retained proof binding carries both theorem-source hashes and the tool and
closure hashes. A domain-separated digest of the full ordered semantic/ranked
guard-source roster is included in the canonical ranked-kernel roster identity.

The remaining trusted computing base includes:

- correspondence between the executable Rust checker and the Verus acceptance
  predicate;
- rustc/ranked/PLIRON integer, SSA, affine-expression, and CFG semantics;
- the deterministic ranked value-to-PLIRON `vN` naming relation used for exact
  production replay;
- semantic access/guard-source extraction and its exact KIR locator relation;
- live-owner custody and the Rust compiler/platform executing the checker; and
- the pinned Verus, vstd, Z3, Rust, and host closure.

A concrete next reduction is a proof-contract-owned declarative certificate
schema. One canonical schema would generate the Rust field/row validators, the
Verus acceptance predicate, ordered conformance vectors, and a shared schema
digest. Generated files would be checked in and re-generated in CI. This would
remove hand-maintained equation and ordering duplication; the schema generator
and templates would become a small, explicit TCB. V3 does not implement this
generator, so it does not claim that correspondence reduction yet.

## Reproduction

```sh
VERUS=/path/to/verus-0.2026.08.09.92f466f \
  scripts/test-dynamic-constrained-affine-bounds-soundness-verus-v3.sh
```

The runner accepts only the pinned runtime proof closure, verifies the positive
theorem set, and requires all hostile mutations to fail.
