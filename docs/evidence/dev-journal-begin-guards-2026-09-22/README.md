# Actual Owner Begin Guards

This packet closes the earlier historical-guard-gated hybrid limitation for
allocation lookup, reader counts, ordered Begin exclusion and the two actual
reader-owner Begin wrappers. Native pending-consumer admission remains closed.

## Shared Execution And Correspondence

Rust and Verus compile the same private stable/producer owner declarations and
shared executable bodies. Public allocation lookup validates exact allocation
identity and the pending member's full allocation backlink. It returns that
member's writer verbatim: writer phase/identity, epoch, lineage and member-chain
validation are deliberately not added to this API.

Stable lookup precedes reader-count indexing. Combined count preserves stable
errors before producer indexing/addition. Each scan follows caller order and
returns the first allocation error or busy result. Producer Begin runs combined
exclusion, then stable exclusion, then unchanged raw Begin. Retirement/disposal
retain their existing generic iterator scans; these are not claimed as new
shared executable scan proofs.

Individual count contracts use conditional index and sum-fit premises: failed
lookup needs no count storage. Whole scans have sufficient storage-shape/sum-fit
premises. The paired issued harness derives these from full owner representation
and the existing historical issued invariant; it adds no caller admission or
post-state premise. Actual and historical Begin execute independently and return
ordinary Results. Actual execution is not skipped on a historical guard error.

Field-preserving views cover all stable leases, producer reservations, count and
free vectors, incarnation metadata and nested journal contents. The transition
relates exact mapped results and post-state while each side supplies its own
object-identity rejection guarantee. Successful Begin preserves owner custody.
The proof-only paired harness creates no second runtime journal or runtime work.

The synthetic live witness has a Pending producer reservation and count plus a
distinct Reserved writer/destination. It exercises successful Begin with
protected custody and Pending status, raw replay rejection, and guard-first
busy rejection with an invalid writer. This is not constructor/Begin/acquisition
reachability and is not a live stable-lease Verus witness.

## Qualification

Two unselected whole-root positives at default solver limits bracket twenty-two
scoped negatives. The expected positive is 444 verified, zero errors, including
inherited obligations. Thirteen executable mutations cover backlink/payload/
bounds, stable and combined counts, scan rejection/order, omitted guards,
rejection-state mutation and omitted actual/historical execution. Three contract
controls cover owner frames and sum safety; four projection controls cover
lookup extent, consumer identity, read offset and represented counts; two
live-fixture controls remove actual custody or the historical count.

Every negative must produce its source-bound logical verification diagnostic,
not a syntax/VIR error, resource limit or arbitrary failing exit. Only case paths
and top-level diagnostic emission order are normalized; types, duplicate counts,
nested order and complete diagnostic records remain exact. The auditor checks
the 33-file staged closure, three include envelopes, four macro projections
(Begin 8, retained 7, lookup 1, guards 5), exact owner-declaration movement and
reviewed production adapters. It authenticates the pinned 190-file Verus closure.

After solver work, serialized CPU checks cover formatting, all model unit tests
and privacy doctests, all-target Clippy with warnings denied, a release build and
two ignored release microbenchmarks. Nine new functional tests cover lookup,
both owners, simultaneous faults, complete state, pointer/capacity identity,
exact reset and missing count storage after lookup rejection.

The independent frozen comparison is authenticated against
e8134e770a30d12aa4abd190bd29c5083ea5b3b5: all changed lookup/count/guard/wrapper
methods plus their unchanged raw Begin/retained/helper dependencies. The complete
44-case, two-owner, two-scope, seven-round, two-variant roster has 2,464 rows.
Seven-round medians retain all patterns, including regressions. Indexed-access
counters, per-call timing and dispatch are inside the measurement; fixture reset
is outside. These are non-exclusive instrumented CPU regression observations,
not uninstrumented production latency or HIP/HSA performance measurements.

[RESULTS.md](RESULTS.md) is reconstructed by the auditor. Source/input brackets,
normal terminal receipts, solver-before-CPU ordering and the exact artifact
manifest are checked offline against Git objects. Self-tests reject changed
diagnostics, malformed benchmark rows and rehashed artifact substitutions.

## Limits And Reproduction

Actual construction, stable/producer reader admission/release, universal
represented-model existence, other wrapper operations, physical Vec capacity/
address preservation, allocation failure, unwind and native producer authority
remain outside this result. Physical storage is tested on these CPU fixtures,
not proved by Vec sequence equality. The raw journal Begin forwarder is
source-bound and reviewed; its Verus-local forwarding adapter calls the verified
raw body. The toolchain, standard-library contracts, recorder/host and adapter
review remain trusted. Full HIP/HSA parity and broad speedup remain unestablished.

Fresh execution at the source commit in SOURCE, using owned scratch:

```sh
python3 docs/evidence/dev-journal-begin-guards-2026-09-22/check.py \
  --repo . --verus "$VERUS" --output "$SCRATCH/proof"
python3 docs/evidence/dev-journal-begin-guards-2026-09-22/cargo-checks.py \
  --repo . --output "$SCRATCH/cargo" --target "$SCRATCH/target"
```

Separately, replay the retained packet from the artifact checkout. This checks
the published receipts, not the new receipts in the scratch directory above:

```sh
python3 docs/evidence/dev-journal-begin-guards-2026-09-22/audit.py \
  --repo . --packet docs/evidence/dev-journal-begin-guards-2026-09-22 --selftest
```

The auditor needs Git objects and retained packet files, not the original
worktree, solver, Cargo or GPUs. Probes are never accepted as qualification.
Verify source and artifact signatures separately against a trusted signer.
