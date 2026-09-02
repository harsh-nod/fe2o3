# fe2o3-proof-contracts

This crate is the solver-neutral, compiler-IR-neutral, and runtime-neutral
contract layer shared by the issue #134 compiler pipeline and the issue #135
persistent-execution work. It records what a producer says about a property
without executing a verifier or granting runtime authority.

## Authority model

Each property has its own identity, exact statement, status, evidence identity,
and obligation. The five statuses are independent:

- Proved records an exact proof artifact, input, model, tool, TCB, and
  correspondence reference.
- Validated records an exact validation artifact, input, model, tool, and TCB.
- Contracted records only an exact contract artifact.
- Checked records an exact check artifact, input, tool, and TCB.
- Unsupported records an explicit reason and rationale artifact.

There is intentionally no status ordering or conversion API. A Proved record
does not satisfy an obligation requiring Validated, Checked, or any other
status. A property about memory safety cannot satisfy a data-race-freedom
obligation: obligation satisfaction is bound to the exact property, statement,
evidence, and status.

## Validation

ContractSetV1::validate performs deterministic structural validation:

- all sections and per-evidence TCB references are bounded;
- records and references are in strict canonical identity order;
- absent all-zero identities and malformed extension identifiers are rejected;
- each status matches its status-specific evidence variant;
- evidence is bound to one exact property and statement;
- input, model, tool, artifact, TCB, and correspondence identities are present;
- tool-backed evidence explicitly cites a matching tool TCB entry;
- proof correspondence is nonvacuous and local to the same property, statement,
  and input;
- every property has a satisfied obligation at exactly its reported status;
- unused TCB and correspondence records are rejected.

Structurally valid sets may retain additional open obligations so gaps can be
represented honestly. ContractSetV1::validate_closed additionally rejects all
open obligations.

All vectors are untrusted input until validation. Their limits are checked
before ordering or cross-reference scans, and validation uses bounded
quadratic scans rather than hash-map iteration so results are deterministic.

## Trust boundary

Validation does not authenticate digests, establish that a tool ran, check a
proof, validate a model, prove erasure correctness, authorize publication, or
authorize GPU execution. Authority-bearing integration must authenticate the
exact external identities and revalidate the complete set immediately before
use.

The crate contains no process runner, solver adapter, Pliron type, LLVM type,
HSA handle, filesystem access, networking, unsafe code, or target-specific
logic. Process supervision remains owned by fe2o3-verifier. Proof production,
artifact authentication, and runtime admission remain outside this crate.

## Constrained affine bounds V2

The V2 certificate proves `0 <= f(x) < extent` for every integer point in one
exact, nonempty constrained launch box. The certificate binds the ordered
inequalities, affine map, box, extent, a satisfying-domain witness, and two
bounded nonnegative multiplier vectors. The canonical checker rejects empty,
malformed, unsupported, over-limit, or overflowing inputs.

For the production-connected subset, kernel analysis separately counts every
static-shape affine access dimension admitted by an affine path fact. A V2 site
roster exists only when that count is nonzero and exactly one certificate is
present per required site. `RankedBoundsReportV1::is_clean()` is a legacy
analysis decision and does not imply that this V2 roster exists. An empty V2
roster has no proof meaning.

Production custody re-derives the launch box and affine expressions from the
exact ranked recipe, matches the access and branch SSA identities, and checks
that deleting the retained true edge makes the access block unreachable. The
Verus proof establishes both the universal multiplier theorem and the generic
cut-edge dominance lemma. The correspondence between the executable Rust
checker and the Verus integer model, plus ranked-recipe-to-PLIRON SSA and CFG
semantics, remains explicitly in the trusted computing base; this work does
not prove all bounds analysis, race analysis, or compiler passes sound.

## Integration

Adapters belong in verifier, compiler, Pliron, and runtime crates rather than
in this contract layer. Integrators must authenticate identities outside this
crate and treat validation success as structural consistency only.
