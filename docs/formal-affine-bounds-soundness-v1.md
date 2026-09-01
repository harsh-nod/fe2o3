# Formal Affine Bounds Soundness V1

Status: one mechanically checked theorem for a production-used static-analysis
decision. This is not a whole-analysis, compiler-correctness, LLVM, machine-code,
or GPU-execution proof.

## Exact Theorem

For an affine expression

```text
f(x) = constant + sum(coefficients[i] * x[i])
```

and the nonempty integer box

```text
lower[i] <= x[i] < upper_exclusive[i]
```

`accepted_affine_box_certificate_is_sound` proves:

```text
affine_bounds_checker_accepts(certificate)
and point_is_in_box(x)
imply 0 <= f(x) < extent
```

The certificate contains the exact query, the coefficient-directed minimizing
and maximizing endpoint in every dimension, and the claimed exact extrema. The
acceptance predicate checks equal ranks, nonempty dimensions, positive extent,
endpoint selection by coefficient sign, exact endpoint evaluation, a
nonnegative minimum, and a maximum strictly below the extent. The proof is an
induction over affine terms and uses the pinned `vstd` integer-multiplication
ordering lemmas. It contains no `assume`, `admit`, or external body.

## Production Connection

`fe2o3-kernel-analysis` derives a `PresburgerMapV1` from the sparse index facts
used by the ranked-memory bounds pass. When an access index is one exact affine
output over an unconstrained finite box and the ranked dimension has a static
extent, the analysis constructs `AffineBoundsCertificateV1`, runs the canonical
checker, and admits that affine access only after acceptance. The exact checked
certificate is retained in `RankedBoundsReportV1::affine_certificates` with its
block, operation, and dimension coordinates.

`fe2o3-verifier::verify_compiler_affine_bounds_certificate_v1` independently
reruns the canonical checker and retains the exact certificate in a move-only
result. Neither the report, certificate, checked result, nor verifier result
authenticates a producer or grants lowering, artifact, publication, load, or
launch authority.

The production-route test builds the ranked Pliron access `2 * invocation + 1`
for `0 <= invocation < 8`, checks the emitted minimum `1`, maximum `15`, and
extent `16`, and reruns the canonical checker. Contract tests exhaust a signed
two-dimensional example and reject hostile endpoint, claimed-extremum, extent,
shape, empty-domain, and arithmetic-overflow mutations.

## Closed Fragment

The certificate path supports only:

- one exact affine output;
- an unconstrained, nonempty rectangular integer domain;
- at most 16 dimensions;
- a nonzero static `u64` extent; and
- construction and checking whose intermediate arithmetic fits in `i128`.

Constrained Presburger sets, remainder outputs, empty domains, missing or
dynamic extents, rank mismatches, rank-limit violations, and arithmetic
overflow do not produce this certificate. The surrounding analysis may use a
separately established conservative decision for such inputs, but it cannot
attribute that decision to this theorem. Unsupported or exhausted analysis
still returns `Incomplete` or a concrete rejection through the existing
pre-lowering gate.

## Proof Closure

Run the checked proof and its expected-failing tightened-extent mutation with
the existing runtime-model Verus closure:

```sh
VERUS=/home/harsh/.local/opt/verus-0.2026.08.09.92f466f/verus \
  scripts/test-affine-bounds-soundness-verus.sh
```

The script verifies the existing `0.2026.08.09.92f466f` closure manifest and
the existing pinned executable SHA-256 (`d97501a8...`), pins both proof source
files, requires `5 verified, 0 errors`, and requires the tightened-extent
mutation to fail. This slice does not alter the separate
`0.2026.08.02.b677dd5` MIR/Pliron proof closure.

Focused Rust checks are:

```sh
cargo test -p fe2o3-proof-contracts --test affine_bounds_v1
cargo test -p fe2o3-kernel-analysis --features pliron-analysis \
  --test pliron_presburger --test pliron_ranked_bounds
cargo test -p fe2o3-verifier --test affine_bounds_certificate_v1
```

## Assumptions And TCB

The theorem is over Verus mathematical integers. The Rust checker uses checked
`i128` operations and rejects overflow in every endpoint-extrema operation it
performs, so accepted extrema embed into those mathematical integers. It does
not prove that a separate machine-integer evaluation of every interior point
avoids intermediate overflow. The correspondence between the Rust checker and
the Verus acceptance predicate is review- and test-backed, not a verified
compilation of the checker itself.

The trusted boundary also includes Rust and `alloc`, the production extraction
of sparse Pliron index facts into the exact query, Pliron structural
verification and ranked-view extent identity, the pinned Verus/vstd/Z3 closure,
and the build/test environment. The theorem does not prove sparse-analysis
extraction, constrained Presburger reasoning, other bounds paths, tensor-layout
analysis, race freedom, source-to-Pliron correspondence, LLVM lowering,
machine-code refinement, runtime launch identity, or hardware execution.
