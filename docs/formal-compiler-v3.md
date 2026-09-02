# Formal Compiler V3

Status: one bounded target-neutral compiler fragment has a mechanically proved
composition theorem and production-wired, fail-closed evidence custody. A real
rustc kernel reaches the composed `Proved` status, and a source-level
wrong-store mutation is denied that status. fe2o3 is not a formally verified
compiler as a whole.

LLVM and every later machine boundary are excluded. V3 composes exact
CFG/value refinement, guarded byte-memory trace refinement, and dynamic affine
bounds for one closed kernel shape. None of this evidence grants compilation,
artifact, publication, load, or launch authority.

## Closed Production Fragment

The declarative contract is
[`formal/compiler-v3/guarded-u32-xor-helper-store-v1.json`](../formal/compiler-v3/guarded-u32-xor-helper-store-v1.json).
Generation rejects unknown or missing axes and emits constants used by the
runtime classifiers and Verus composition proof. The complete canonical JSON
contract has one SHA-256 identity; evidence commits to that identity rather
than a hand-selected subset of fields.

The exact fragment has:

- one one-dimensional kernel root and one internal helper, with no production
  loop;
- two immutable `u32` slice inputs and one disjoint mutable `u32` slice output;
- three distinct dynamic extent identities and three ordered short-circuit
  predicates, one per slice;
- one common `gid`, two ordered little-endian loads at `gid * 4`, and one store
  at `gid * 4`;
- a helper computing `left ^ right`, selecting the XOR value when it is zero
  and an arbitrary retained `u32` fallback otherwise; and
- an exact store of the helper call-result SSA value.

Runtime extent identities are distinct even when two concrete lengths happen
to have the same numeric value.

## Mechanical Claims

### Exact helper CFG and value

For related loaded `u32` inputs, the exact semantic-MIR and canonical-KIR
helper executions agree on the XOR expression, chosen branch, join value,
helper return, and caller call result. The production classifier replays the
two semantic load definitions, direct call, four-block helper diamond, KIR
block arguments, ordered operands, fallback, and result SSA values against one
live owner.

A broader non-authoritative model also proves bounded structured CFG behavior
for `u8`, `u16`, `u32`, and `u64`, checked and wrapping addition, truncation and
zero extension, two diamonds, call depth two, and loop trip counts one through
four. Those cases are model coverage, not production-connected compiler
coverage.

### Guarded byte-memory trace

For three related pairwise-disjoint byte allocations, the bounded source,
semantic-MIR, and KIR models produce equal final memory, equal helper result,
and the same ordered read/read/write trace. The enabled path checks allocation
identity, provenance, range, alignment, mutability, address overflow, and
little-endian `u32` encoding. A disabled lane terminates with unchanged memory
and an empty trace.

Here “source” means the bounded source-language model bound to live semantic
source locations. Those locations do not authenticate the model semantics; the
Rust-to-model transcription remains trusted. This is not a proof of general
Rust or HIR semantics.

The production classifier locates one exact three-guard short-circuit chain,
permits only a finite pure setup prefix, and proves every modeled operation is
dominated by all three accepted guard edges. Compiler-generated option and
bounds control is traversed only when it repeats an already dominating exact
`gid < extent` relation. Every rejected guard path must be finite,
effect-free, call-free, and normally returning. Pure cycles, `Unreachable`,
unmodeled calls or effects, re-entry, and bypasses are rejected.

### Dynamic affine bounds

For an accepted V3 certificate and every point satisfying its exact
constrained domain, Verus proves `0 <= index < runtime_extent`. The checker
composes independently checked lower-bound and slack
`runtime_extent - index - 1` certificates, verifies their exact affine
relation, retains a concrete satisfying witness, and rejects duplicate guard
rows.

Production custody retains exactly three access sites and all three ordered
guard sources. Each record binds the semantic root, access statement and
ordinal, access kind, ranked block/operation/dimension, common index identity,
its own dynamic extent identity, runtime-variable roster, semantic guard
locations and operands, ranked guard edges and operands, normalized rows, and
proof/tool closure.

### Composed observation

`fe2o3_guarded_u32_xor_helper_store_composes_v3` uses three accepted
dynamic certificates over one exact shared three-row domain. An explicit
pairwise-distinct row permutation binds producer coordinate order to the
source input/input/output order. The theorem derives each row from the
corresponding ordered `gid < extent[i]` predicate, obtains all three element
bounds, bridges them through
`allocation.bytes.len == 4 * extent`, and discharges each byte-range premise.
It then composes the exact helper result with the byte-memory theorem.

The final postcondition states source/MIR/KIR observation equality, the exact
selected result and output bytes on the enabled path, and unchanged memory
with an empty trace on a disabled path. It also exposes all three established
dynamic bounds. The byte-length relation is an explicit typed-slice premise;
concrete launch allocation state is not compiler-authenticated by this
target-neutral theorem.

## Live Evidence Custody

The lower compiler derives `NotApplicable`, `Incomplete`, or `Proved` from the
same move-only semantic/KIR/formal-memory owner. `Proved` requires exact
agreement on semantic and KIR identities, root/helper functions, load sites,
call and continuation blocks, helper locals and SSA values, call destination,
fallback, operation locations, and store value.

The rustc integration additionally requires one ranked root and exactly three
unique read/read/write dynamic sites. It joins each ranked guard to the same
semantic guard and access source retained by the lower classifier, checks the
ordered true-edge chain, distinct extent roster, common index and domain, and
current proof identities, then replays the status before target lowering.
Asymmetric or partial evidence cannot become `Proved`.

The status is private, inert, non-serializable compiler custody. Its evidence
is dropped before LLVM; only a non-authoritative status label is retained for
diagnostics. Neither the evidence nor the label enters compiler-module,
artifact, runtime, or worker handoffs.

The pinned-nightly real-rustc fixture uses explicit finite launch geometry,
two immutable `u32` slices, one disjoint mutable output slice, three nested
dynamic guards, two loads, the XOR/diamond helper, and the exact helper-result
store. It reaches one revalidated composed `Proved` status. A sibling fixture
stores the first input instead; it still compiles, but cannot claim `Proved`
and retains no artifact or launch authority.

## Trusted Computing Base

V3 still trusts:

- rustc HIR/raw-MIR extraction, monomorphization, source provenance, type and
  slice-layout interpretation, and exact-fragment eligibility enumeration;
- the Rust validator-to-Verus transcription for source, semantic MIR, KIR,
  byte memory, affine expressions, guard edges, and observations;
- semantic-MIR-to-ranked-PLIRON and semantic-MIR-to-KIR correspondence
  construction, plus canonical owner replay and private evidence custody;
- the typed-slice relation between element extent and byte allocation length;
- SHA-256 collision resistance for contract, source, model, and evidence
  identities; and
- the pinned Verus, vstd, Z3, Rust, and host execution closures.

Proof runners reject `assume`, `admit`, and external-body shortcuts in the V3
sources. Positive proofs are paired with hostile Verus and Rust mutations, but
testing does not remove the trusted transcription above.

## Explicitly Unproved

Formal Compiler V3 does not establish:

- whole-Rust, whole-MIR, whole-PLIRON, or whole-KIR semantic preservation;
- general pointers, aliasing, allocation, lifetimes, panics, unwind, unsafe
  Rust, atomics, concurrency, barriers, volatile access, or arbitrary memory;
- arbitrary CFGs, loops, recursion, indirect calls, expression trees, types,
  layouts, ABI lowering, or multi-kernel composition;
- soundness of every Presburger, sparse-index, initialization, convergence,
  race, alias, disjointness, or layout analysis;
- concrete runtime allocation provenance or GPU execution; or
- KIR-to-LLVM, LLVM optimization, code generation, linking, HSACO, ISA,
  driver, firmware, or hardware refinement.

The accurate community statement is: fe2o3 has a mechanically verified
target-neutral refinement theorem for one explicitly bounded guarded `u32`
kernel model, plus fail-closed production custody for the corresponding three
evidence tracks. A real rustc kernel demonstrates that composed production
path, and a wrong-store source mutation is rejected from it. Broader proved
models are not production-connected compiler coverage, and fe2o3 is not a
formally verified compiler.

## Reproduction

The compiler and runtime proof sets use two pinned Verus closures:

```sh
VERUS_COMPILER=/path/to/verus-0.2026.08.02.b677dd5 \
VERUS_RUNTIME=/path/to/verus-0.2026.08.09.92f466f \
  scripts/ci-local.sh verus

FE2O3_HIP_SYS_DISABLE=1 cargo test --locked -p rustc-codegen-fe2o3 \
  --test production_extraction_driver_v1 \
  real_rustc_guarded_xor_fixture_reaches_composed_formal_compiler_v3_proved_status \
  -- --ignored --exact --test-threads=1

FE2O3_HIP_SYS_DISABLE=1 cargo test --locked -p rustc-codegen-fe2o3 \
  --test production_extraction_driver_v1 \
  real_rustc_wrong_store_fixture_cannot_claim_formal_compiler_v3_proved_status \
  -- --ignored --exact --test-threads=1
```

The `Formal Compiler V3` workflow authenticates both release archives before
running the contract generator, positive proofs, negative proof fixtures,
focused Rust custody tests, and both real-rustc non-vacuity fixtures. The V1
and V2 ledgers remain historical records of their narrower, noncomposed
boundaries.
