# Actual/Historical Enrollment Correspondence

This packet connects the shared actual-type enrollment execution to the existing
logical journal and issued-producer custody model. It advances Gate 1 without
changing production runtime execution or enabling pending native admission.

## Relational Contract

Field-preserving enrollment/reference projections and the existing exhaustive
error embedding relate exact entry, header, replay, vacancy, alias and capacity
decisions. Reverse-stack plans, recursive allocation updates and unchanged-field
frames commute with the journal content view. Slot projections require all-Some
values; no correspondence is asserted for arbitrary `None.unwrap()` values.

The raw paired harness starts from represented actual/logical journals and
corresponding enrollment/output slices. It executes both independently verified
batch functions and derives equal mapped decisions/output and represented
post-state. It does not require canonical input, clean output, successful
admission, a valid journal, a hypothetical post-state or post-state representation.
The actual shared executor retains its unconditional contract. Error identity is
taken separately from each executable contract, never inferred from Vec views.

The issued paired harness adds the existing logical issued-producer precondition.
It executes the actual body and historical issued wrapper, preserving the
logical issued invariant, reader/reservation storage and every previously valid
producer status. Both harnesses exist only in the verifier: production neither
constructs nor executes a second journal.

The include graph nests the actual declarations/execution once under the
historical model. Existing value views require historical Begin's write type,
so that proof is inherited too. Five aliases are explicitly self-qualified for
module hygiene; runtime source and shared executable macros are unchanged.

## Concrete Witnesses

- A real vacant journal pair enrolls two entries through a permuted free stack,
  checking canonical refill after sorting. The same pair then exercises late
  retained-prefix alias rollback and dirty-output precedence over invalid input.
- A constructor-based issued pair enrolls one entry, then rejects its replay,
  retaining issued validity and exact unchanged state/output on rejection.
- A synthetic live-custody pair contains a Pending writer/member/allocation and
  one retained producer reservation with a nonzero producer-reservation count.
  Enrollment of another allocation preserves protected contents, reservation storage and
  Pending status. Direct fixture population is not production Begin/acquire
  reachability or production-constructor refinement.

## Qualification

Two whole-root positives bracket seventeen scoped controls at default solver
limits. Expected positives are 422 verified and zero errors. The count includes
historical and shared-body obligations already covered by earlier packets and
is not a count of new obligations.

Controls are intentionally distinguished: six projection changes, five
executable omissions/mutations, four contract-sensitivity changes and two
live-fixture changes. They cover identity/presence/error mapping, omitted actual
or logical execution, initialized epoch, missing writer/error frames, invalid
status preservation, and missing reservation/count custody. Every negative must
fail for its exact source-bound verification diagnostics, not a syntax error,
timeout, unrelated failure or arbitrary nonzero exit.

The source commit binds 27 proof inputs, the runtime adapters/public forwarder,
all model source and the recorder/auditor. The three shared macro families have
10, 13 and 8 bodies. Staged sources, include envelopes, typed diagnostics, input
brackets, terminal receipts and the 190-file pinned Verus/vstd/Z3 closure are
reconstructed offline. CPU qualification runs the complete model suite,
formatting, all-target Clippy with warnings denied and a release test build.
Solver work finishes before the serialized CPU campaign.

[RESULTS.md](RESULTS.md) records the qualified source and checked scope. Runtime
code is unchanged; no new benchmarks were run or performance gains claimed.
Previous shared-host measurements remain
mixed; full HIP/HSA behavior/performance parity is not established.

## Limits And Reproduction

The correspondence starts with an existing represented logical pre-state; it
does not prove that every raw actual journal has an issued-model witness.
Actual reader-wrapper refinement, public-forwarder compilation into the proof,
native producer authority, physical Vec capacity/address preservation,
allocation failure and unwind behavior remain separate. The pinned toolchain,
standard-library contracts, trusted recorder/host and review of adapters remain
trusted; receipts are not independent hardware execution attestation.

At the source commit named in `SOURCE`, with fresh owned scratch:

```sh
python3 docs/evidence/dev-journal-enrollment-correspondence-2026-09-21/check.py \
  --repo . --verus "$VERUS" --output "$SCRATCH/proof"
python3 docs/evidence/dev-journal-enrollment-correspondence-2026-09-21/cargo-checks.py \
  --repo . --output "$SCRATCH/cargo" --target "$SCRATCH/target"
python3 docs/evidence/dev-journal-enrollment-correspondence-2026-09-21/audit.py \
  --repo . --packet docs/evidence/dev-journal-enrollment-correspondence-2026-09-21 \
  --selftest
```

The offline auditor uses Git objects and retained artifacts, not the original
worktree, Cargo, a solver or GPU hardware. It rejects developer probes and
rehashed substitutions. Verify source/artifact commit signatures independently
against the owner's trusted SSH signer configuration.
