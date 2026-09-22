# Actual-Type Begin Execution

This packet connects production Begin to the executable bodies verified over
the actual journal declarations. It advances Gate 1; native pending-consumer
admission remains closed.

## Execution And Contracts

Eight shared macros cover Reserved identity, canonical order, destinations,
selected slots, full preflight, staging, commit and composition. The public
method forwards through a small Rust adapter. Three unchanged retained lookup
and comparison macros are reused. No second logical journal runs in production.

Preflight and full execution have unconditional raw-content contracts: exact
error precedence, unchanged journal on rejection, and sequential successful
updates. Stage and commit retain the historical storage-ready domain. They do
not assume free-slot uniqueness, canonical destinations, valid custody, clean
untouched tails, ordered lineage/epoch, or constructor-consistent metadata.
Sequential update prefixes preserve alias overwrite behavior. Staging captures
all original epochs/lineages before commit; commit retains Option::take,
in-place allocation writes and final writer publication.

The proof is for normal contents. Opaque untouched vectors retain identity;
modified vectors have content relations, not invented physical identity facts.
Physical addresses/capacity and allocation failure are not proved. The runtime
adapter and public forwarder are source-bound, tested and reviewed, but the
public forwarder itself is not compiled into the Verus root.

Concrete actual-type executions cover duplicate selected/retained member slots,
dirty scratch tails, lineage greater than epoch, exact repeated rejection, and
empty success with deliberately inconsistent constructor metadata. A repeated
destination roster is rejected through full Begin without mutation, then
separately executed through the weaker stage/commit helper contracts.

## Qualification

Two whole-root positives bracket 33 source-scoped controls at default solver
limits. Each positive must verify 45 obligations with zero errors and clean
diagnostics. The controls comprise 31 executable mutations and two contract
sensitivities. They test identity, context, canonicality, destination errors,
error precedence, capacities, slots, raw alias acceptance, plans, scratch
consumption/tails, member links, allocation writes, free-stack consumption,
publication counts, stage/commit omissions, rejection mutation and frame/identity
contracts. Each must fail with its exact intended proof diagnostics, not a
syntax error, resource limit, timeout or unrelated nonzero exit.

The recorder retains staged sources, exact commands and terminal process-group
receipts. The auditor reconstructs seven proof inputs and the two macro
projections (eight Begin and seven retained templates), complete diagnostics,
input brackets, CPU source identities and the pinned 190-file Verus/vstd/Z3
closure from Git objects and retained artifacts. Only case paths and top-level
diagnostic emission order are normalized. Nested order, fields, JSON types and
diagnostic multiplicity remain exact.

CPU qualification includes the complete model unit/doctest suite, formatting,
all-target Clippy with warnings denied, a release test build, and the separately
selected ignored benchmark. The independent 4,356-case malformed-state oracle
is retained. New tests compare the frozen original against full state,
storage identity, exact access counts, dirty tails, aliases and resets.
Access counts are tested, not formally verified.

## Matched CPU Comparison

The frozen preflight and Begin methods are authenticated byte-for-byte against
commit 535763f018e1bf2236d4e8abe7790daf3c9df242, allowing only their renamed
methods and visibility change. All eight called journal helper bodies are also
checked unchanged against that Git object.

The benchmark covers 44 cases and seven alternating-order rounds, producing
616 rows. Cases include empty, normal, permuted and aliased slots, dirty tails,
early/late device failures, late member/scratch failures, rosters through 4,096
entries, and small rosters in a 65,536-capacity arena. Both variants use the
same journal and warm reset outside timing; complete state/storage/counter
equivalence qualifies every fixture and reset. Black-box inputs/results and
post-timing checks prevent discarded execution. A focused inline hint on
selected-slot validation allows the compiler to reuse the preceding bounds
guard without changing shared execution or proof contracts.

[RESULTS.md](RESULTS.md) retains every case, including slower results.
Measurements include timer and test-only access-counter overhead on a shared,
non-exclusive CPU host. They do not establish no-regression performance,
HIP/HSA parity or any native GPU speedup. No MI300X work is required for this
CPU journal operation.

## Limits And Reproduction

Historical Begin/issued-custody correspondence, production construction,
actual reader admission/release, physical storage, fallible allocation, unwind,
machine refinement and native producer authority remain separate work.
The compiler, pinned library contracts, recorder/host and adapter review remain
trusted. Receipts are not independent hardware execution attestation.

At the source commit named in SOURCE, using fresh owned scratch:

```sh
python3 docs/evidence/dev-journal-begin-execution-2026-09-21/check.py \
  --repo . --verus "$VERUS" --output "$SCRATCH/proof"
python3 docs/evidence/dev-journal-begin-execution-2026-09-21/cargo-checks.py \
  --repo . --output "$SCRATCH/cargo" --target "$SCRATCH/target"
python3 docs/evidence/dev-journal-begin-execution-2026-09-21/audit.py \
  --repo . --packet docs/evidence/dev-journal-begin-execution-2026-09-21 --selftest
```

The offline auditor needs Git history and retained artifacts, not the original
worktree, scratch, Cargo, a solver or GPUs. Verify source and artifact signatures
separately against the owner's trusted signer configuration.
