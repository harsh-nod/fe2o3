# Conditional Durable Publication Adapter

Date: 2026-09-27. Base: `cbd0ab6241f86dc492b66e92bd1e08a4e4a992af`.
Follow-up to [conditional Worker recovery](conditional-worker-recovery-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**Conditional publication preparation, persistence and recovery are implemented.
Component tests pass, but a complete positive conditional source, journal and
artifact round trip has not been executed. The native Cargo continuation retains
the prepared publication; its consuming persistence transition is still open.
This is not selected production activation, protected execution, a completed
M0-M7 milestone, or 47/47 GPU completion.**

## Implementation

The adapter uses the existing opaque Worker journal and its unchanged storage
caps. It adds no journal format, alternate compiler frontend, IR, or proof route.
The shared durable-plan hashing helper accepts explicit domain families; the
conditional request, plan, intent, measurement and derived storage coordinates
use V5 domains. Frozen V1 request/plan/intent vectors protect the existing native
format during this extraction. Shared shape checks and bounded attachment copies
also remain single implementations.

Preparation consumes the actual finalized conditional artifact and its existing
transcript. It requires fresh `ConsumedPublication` custody, checks transcript
coordinates and storage bounds, and retains both owners without recreating the
transcript. Cargo's native continuation retains this prepared publication beside
its original parent invocation, readiness/budget borrow and execution carriage.
There is no detached-parts conversion or production selector change.

Persistence commits through the existing journal, then independently recovers
the returned record. Recovery strictly decodes V5 source bytes, recovers the
actual source/final graph/catalog under independently supplied roots, history
limits and target policy, decodes the conditional transcript, and invokes the
existing conditional Worker/finalizer replay. It requires `RecoveredTranscript`
custody and rederives the exact durable plan. Fresh persistence additionally
compares the retained source/binding, full outer bytes, transcript, finalized
bytes and identities against that independently recovered result.

The policy view is an inert borrowed input, not policy admission. Its provenance
and complete backing must be independently admitted/prepaid by the caller,
never inferred from the stored handoff. Neither returned owner authenticates
protected compiler origin or grants publication, load or launch authority.
Recovered custody cannot be promoted into a fresh consumed publication.

## Resource Contract

Conditional source recovery is outside all ordinary refund scopes. The terminal
adapter preserves partial reservations on error and unwind, exposes no nested
refundable error through its public error source chain, and releases only the
exact successful transfer after checking the original ledger and storage total.
A later semantic refusal does not erase an already committed inert record.
Callers must treat failure as terminal, not retry the attempt on a fresh meter;
this API does not itself own or enforce the surrounding attempt lifecycle.

Outer backing capacity/decode metadata, source recovery, transcript storage,
Worker replay and returned headers are charged on the original account. Temporary
wire/finalized-output backing is retired explicitly. Variable output/descriptor/
measurement hashing and exact byte comparisons are charged. Success returns an
additional unreserved storage charge; after consuming a prepared owner, retiring
its original reservation is still the enclosing caller's responsibility.

Journal I/O, bounded attachment copies/hashes, producer-package hashing and
artifact payload processing retain their existing bounded artifact domains.
This is not whole-process allocation, filesystem or RSS accounting. The only
dependency change declares the already-present workspace kernel optimizer as a
direct finalizer dependency for the exact history-limit type.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded
Cargo command at a time, with compiled source frozen during each command:

| Command / Filter | Result |
| --- | --- |
| Finalizer library: `native_worker_publication` | 26 passed |
| Complete finalizer library, including final assertion changes | 211 passed |
| Complete finalizer doctests | 68 compile-fail and 4 positive doctests passed |
| Cargo binary: `continuation_account_floor` | 1 passed |
| Five-package `--all-targets` check | Passed; warnings remain |

The focused selection is included in the complete suite; counts are not
additive. Two new positive doctests are compile-only borrowed API examples.
No positive semantic source/proof owner is fabricated by these tests.
The build check covers `cargo-fe2o3`, `rustc-codegen-fe2o3`,
`fe2o3-hsaco-finalize`, `fe2o3-compiler-execution-client` and
`fe2o3-artifact-transaction`.

New tests cover domain separation, all durable-plan identity axes, frozen native
hashes, custody refusal, exact and short work/storage limits, transfer overflow,
hidden/missing charges, replaced work ledgers, preserved denial history, error
opacity and unwind. Real opaque journal writes are recovered as bytes, then
refused for malformed or other-family source framing without deleting their
committed records. These negative journal cases stop before semantic recovery;
they are not positive source-admission or GPU tests.

The first build failed on a missing direct dependency and the V5 decoder's
Debug-only error type. Both were corrected before the passing runs. Rustfmt and
whitespace checks passed. No unsafe code or inventory allowance was added. The
preexisting unsafe-inventory failure recorded in
[the earlier checkpoint](shared-compiler-slot-admission-20260927.md) remains open
and was not rerun here. All build/test processes terminated, and private scratch
`/tmp/fe272publication.MaeflY1h` was removed after validation.

Logs under `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
f07dfd82ce05a3df41a8a227948bc6fba7b44fc51c2d9036452e146cae73bf6a  conditional-publication-focused-20260927-r1.log
4396c0ab400f23c306b13710365d2b665de9201dd40ce5f7d61ab327f78c646c  conditional-publication-focused-20260927-r2.log
3fb727937958cd28215e78547140db7544612f3614a9f794543e53dacfbdd6f3  conditional-publication-finalizer-lib-20260927.log
e1f137185784c942192e8dc662fbfda6889054e4bc8087aa2d993d9566f3a114  conditional-publication-finalizer-lib-20260927-r2.log
168c61655c35b977d154a58f8ae01da8836e8f2b1183c87c0b6a766b3592ac54  conditional-publication-doctests-20260927.log
1bd779c7b61fc565b486bb4fb647af7b1da649b31a120937d6ff6342016bfde3  conditional-publication-cargo-20260927.log
20d668a31850c7892190f6ccbacf7acd8f3a651a2eb22e00166507317512c443  conditional-publication-all-target-check-20260927.log
```

## Remaining Work

Implement Cargo's consuming durable transition while retaining the actual parent,
readiness, execution carriage and original budget, with exact success-only
retirement of replaced owners. Exercise positive conditional source/Worker/
artifact persistence and restart, then policy, source, transcript, provider and
output substitution refusals on that same positive fixture.

Independent root-policy admission, parent/profile/broker/driver transition,
protected compiler receipt continuity, sealed verifier/generated host integration,
machine/numerical refinement, target-matched 47/47 runs, selector retirement and
release gates remain open. Storage reproducibility cannot substitute for any of
these boundaries.

Native agent spawning hit the thread limit; implementation and review were local.
All three SSH aliases failed DNS resolution from this execution environment;
no remote job or file was created. GitHub fetch also failed DNS. Push and issue
publication outcomes are reported separately, not implied by these local tests.
