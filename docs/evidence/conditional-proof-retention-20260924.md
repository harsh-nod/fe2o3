# Conditional Proof Retention and Original-Account Replay

This follows [source-bound conditional bounds](source-bound-conditional-bounds-20260924.md)
and advances #272. No milestone is closed and no entry in the 47-kernel
production-to-safe-GPU-launch qualification matrix changes.

## Integrated Changes

The verifier now returns a move-only `RetainedProductionConditionalFormulaV1`.
It retains the actual signed receipt, verifying key, accepted import policy and
source/graph subject. The borrowed and owned APIs use the same preparation,
execution and strict receipt-import path. Replay regenerates the expected
statement and strictly reimports the same receipt; it does not rerun Verus.
A public receipt or copied report alone cannot establish source ownership,
original-account continuity, ordinary lowering authority or launch authority.

The backend creates one boxed resource account before projection and retains it
with the materialized source and verified roots. Consumption borrows that same
account, without reconstructing counters or retaining numeric address tokens.
Replay checks the source and roster, obtains fresh authenticated CPU-MIR
correspondence, reruns the lower-MIR continuation and shared nine-stage analysis,
then checks the retained receipt against the current request. Work and first
denial histories are preserved. Budget replacement and storage-floor damage are
terminal failures; errors and unwinding never replenish accepted work.

All seven existing target-conversion entrypoints now invoke that replay for
conditional roots: target-neutral attachment and direct/erased checked-output
policies 4, 5 and 6. Successful replay still reaches
`FE2O3-COND-FINALIZER-001`. It does not convert a conditional root into ordinary
V5 authority or introduce a conditional finalizer.

Source and adjusted ABI coordinates remain distinct. Recipe-view origins use
source arguments; expression-load origins use adjusted arguments. Both come
from the same authenticated read row. The resolver uses the matched read's
checked coordinates instead of rescanning argument occurrences and incorrectly
rejecting repeated reads of one argument. Ordinary early-refusal paths also keep
the original account alive until the remaining source/root locals are destroyed.

The actual-source Fill and Vecadd fixtures now require passive observations of
receipt retention, replay callback, postcheck acceptance and phase destruction
before reporting retention success. These observations grant no authority.
Those protected positive fixtures were strengthened but **not executed** in
this checkpoint.

## Validation

Implementation: `81fb80c3c4888769ea38307854530f19c8ecd519`.
Two concurrent mainline updates were preserved by merges, not overwritten:

- A: `f67149660f3b5770609f24b3a0ae6cf767f03725`, including BF16 inspection work
  through `2af2a8d7dc75d8edac3da50325c91ea1911f8324`.
- B: `197921b05d966defd79e082a214cef2ae8c4cd46`, additionally including runtime
  work `807f0bef70da75c81e56de6eb4fd6e9c1f78e5e2`.

All passing runs below used nightly `2026-04-03`, locked offline dependencies,
one Cargo job/test thread, hidden GPUs, disabled HIP, a 12 GiB virtual-memory
cap and a 1200-second deadline. Source and tool inputs were stable in every run.
The source snapshots include tracked and nonignored untracked files, not Git
tree hashes. This evidence page and its incoming link were added afterward.

- A, 7988 files: `0dfba8464f95d49e1917fb523a3c8177996cacf46a484b243d594ffb7f2182b3`.
- B, 7997 files: `4b8093e968c1d6f2cc66ad63c717fbec31d3bca023c2c8c671f9df306a67aa6a`.

| Guard run | Snapshot | Result | Log SHA-256 |
| --- | --- | --- | --- |
| `conditional-retention-merged-backend-r1` | A | 1201 passed, 107 ignored | `c23914991689956497a4c1ae8ae03def81d943ff7697cfc17b4602cd3829cd23` |
| `conditional-retention-merged-verifier-r1` | A | Full verifier library: 238 passed, 15 ignored, none filtered | `96e8823b9ab6a4388c3ac567d60c0f641cdb63af163cc396a85abf3865050a91` |
| `conditional-retention-merged-api-r1` | A | One positive and four compile-fail doctests passed | `0a9fc760bd75f9111258dfbed8ff4ecfec0c6340f2cb7152235f33b15747b1ea` |
| `conditional-retention-runtime-merge-backend-r1` | B | 784 passed, 29 ignored | `09ecad9cc22938385be6a7e209bc12b1b9b3365e1c776b2a08b8d03aa3298956` |

Backend selections share `conditional_`, `production_ranked_projection_v1::`,
`input_guard_`, `consuming_continuation_`, `borrowed_continuation_`,
`ordered_composition`, `parameter` and `argument`. A additionally selects
`production_pipeline::` and `checked_output`, but skips
`production_pipeline::private_cell_native_v1::`. Both enable
`fe2o3-pliron/internal-proof-staging`; verifier and doctest runs use their
default feature set. Selections overlap and their counts must not be added.
The full verifier/doctest suites were not rerun after the runtime-only merge B.

Tests cover signature/policy and theorem substitution, reservation transfer,
original-account moves and replacement, exact resource denials, unwinding,
distinct origin coordinates, and actual ordinary-roster early-refusal cleanup.
Doctests reject cloning, conversion to ordinary formal-memory ownership and two
borrow escapes with the intended errors, not unresolved imports.

An earlier broad backend run, `conditional-retention-backend-r2`, timed out
after 1200 seconds during an existing loop-unroll resource test. It is incomplete,
not a passing suite; no failed test was reported before termination. Its source
snapshot was `de0e79c70c430be47e35170e12792b8cfd05e1e9688d50aadc6029396b95d6ce`
and log hash was `521ced775e1e324d5d7fd85c960d3d19c48b986e38e878d653ab7b6f2ca69284`.
The first verifier attempt also caught a moved-value error in a new test fixture;
signing before moving the receipt fixed it without changing admission policy.

Guard SHA-256:
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.
Formatting, source-size hygiene, whitespace and dependency policies passed.
Existing dead-code and duplicate-target fixture warnings remain.

## Remaining Gates

Coordinate tests are not actual shifted-ABI Rust fixtures. A full checked-request
case with two distinct reads of one parameter remains to be added. The ordinary
early-refusal test observes real account cleanup, but not independent source/root
destructor liveness; the drop-order fix also relies on Rust's lexical drop order.

The [prepared Vecadd exporter](prepared-conditional-vecadd-20260925.md) is now
integrated as test infrastructure. The [generated-field check and inert V4
transport](conditional-generated-fields-20260925.md) are also integrated;
host-premise patches remain separate, unintegrated work. Protected source replay must first
demonstrate retention and consumption in the real transaction. Conditional target
admission, applicable machine/numerical refinement, native issuance and generated
safe host launch are still required. This change retains an existing theorem; it
does not add an ISA-equivalence or floating-point error-bound theorem. No protected
proof execution, GPU execution or new kernel qualification is claimed here.
