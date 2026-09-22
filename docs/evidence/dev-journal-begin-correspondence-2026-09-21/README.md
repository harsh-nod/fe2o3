# Actual/Historical Begin Correspondence

This packet connects shared actual-type raw Begin execution to the existing
historical journal. An explicitly hybrid issued harness composes it with the
historical reader guards and custody model. Production runtime code is unchanged;
native pending-consumer admission remains closed.

## Relational Contract

Field-preserving roster, allocation, writer, plan and error projections connect
reserved lookup, exact allocation lookup, canonicality, destination, free-slot
and complete preflight decisions. Storage-ready equivalence retains its original
domain: it does not add unique member slots, distinct destinations, canonicality,
globally clean scratch, valid custody, lineage ordering or constructor metadata bounds.
Recursive member/allocation prefixes preserve sequential aliasing semantics.

The raw paired harness requires only represented pre-state and corresponding
writer/roster inputs. It independently executes actual and historical raw Begin,
then derives equal mapped results and represented post-state. It does not assume
successful admission or the desired post-state. Each executable contract supplies
its own exact rejection identity; equality of Vec views cannot manufacture it.

Raw Begin cannot unconditionally be paired with historical issued Begin: issued
Begin runs unread guards first, which can reject an otherwise successful raw
operation and outrank raw writer/header errors. The guarded harness therefore
executes the unchanged historical combined/stable unread guards before deciding
whether to execute actual Begin. Its result is `None` when a guard rejects and
`Some(actual_result)` when guards pass, including `Some(Err(...))`. It requires
the existing logical issued invariant but no extra caller unread-ready premise.
The historical issued relation preserves its invariant and reader/reservation
storage. This is not a proof of actual reader-guard or full-wrapper execution.

Paired execution is verification-only. Production constructs no second journal
and performs no additional execution or allocation. The offline audit compares
production source against published commit `30bc6fdbd493a82bc699ef1af99adc907e99109b`.

## Concrete Witnesses

- A raw pair exercises distinct and aliased free stacks, last-write behavior,
  captured epochs, a dirty scratch tail and lineage greater than epoch. Replay
  rejects with both entire objects unchanged. Noncanonical input outranks busy
  destinations. An empty roster succeeds outside constructor metadata bounds.
- A synthetic live pair holds a Pending producer with a retained reservation
  and nonzero count at allocation zero, plus a distinct Reserved writer and
  destination. Successful Begin preserves protected allocation/member/writer
  contents, reservation storage and Pending status while updating the destination.
- The live pair then checks empty-roster replay as `Some(Err(InvalidReference))`
  and combined-unread `AllocationBusy` before an invalid writer as `None`.
  Both rejections preserve both entire objects.

The live fixture is populated directly. It is not production constructor/Begin/
acquisition reachability, and it does not exercise a live stable-reader lease.
Concrete setup proves the private transition witness's admission premises; those
premises are not added to either paired relational harness. Protected status is
proved from unchanged concrete coordinates, not claimed for every producer here.

## Qualification

Two whole-root positives bracket twenty-four controls at default solver limits.
Expected positives are 413 verified and zero errors, including inherited
historical and actual-body obligations, not 413 new obligations.

Controls comprise six projections, ten executable omissions/mutations, six
contract sensitivities and two live-fixture sensitivities. They cover mapped
values/results, omitted paired execution, guard order and rejection mutation,
optional-result presence, error identity, scratch/free frames, recursive prefix
updates, stage epoch, commit backlink and retained reservation/count storage.
Each negative must have its exact source-bound verification diagnostics, not a
syntax error, timeout, unrelated failure or arbitrary nonzero exit.

Only case paths and top-level diagnostic emission order are normalized. Complete
diagnostics, nested ordering, JSON types and duplicate counts remain exact.
The offline auditor checks source identities, all 26 staged proof inputs, two
shared macro projections (eight Begin and seven retained bodies), input brackets,
terminal receipts and the pinned 190-file Verus/vstd/Z3 closure. Solver work
finishes before serialized CPU formatting, full model tests, all-target Clippy
with warnings denied and a release test build. Audit self-tests reject altered
solver results, missing/duplicated diagnostics and rehashed packet substitutions.

[RESULTS.md](RESULTS.md) records the qualified source and checked scope. No GPU
tests or new performance measurements are part of this proof-only change.
Earlier shared-host timings remain mixed; HIP/HSA parity is not established.

## Limits And Reproduction

The harness starts with an existing represented historical pre-state; it does
not construct an issued model for every actual journal. Actual construction,
stable/producer reader admission/release and unread guards, public forwarding
wrapper refinement, native producer authority, physical Vec storage/capacity,
allocation failure and unwind remain separate. The pinned toolchain, standard
library contracts, recorder/host and adapter review remain trusted; receipts
are not independent hardware execution attestation.

At the source commit named in `SOURCE`, with fresh owned scratch:

```sh
python3 docs/evidence/dev-journal-begin-correspondence-2026-09-21/check.py \
  --repo . --verus "$VERUS" --output "$SCRATCH/proof"
python3 docs/evidence/dev-journal-begin-correspondence-2026-09-21/cargo-checks.py \
  --repo . --output "$SCRATCH/cargo" --target "$SCRATCH/target"
python3 docs/evidence/dev-journal-begin-correspondence-2026-09-21/audit.py \
  --repo . --packet docs/evidence/dev-journal-begin-correspondence-2026-09-21 \
  --selftest
```

The auditor uses Git objects and retained artifacts, not the original worktree,
Cargo, solver or GPU. It rejects development probes. Verify source/artifact
commit signatures separately against the owner's trusted signer configuration.
