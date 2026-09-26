# Conditional Execution Custody Checkpoint

This continues [conditional native handoff](conditional-native-handoff-20260926.md).
All [#272](https://github.com/harsh-nod/fe2o3/issues/272) milestones M0-M7 remain
open. No tutorial entry receives new protected-execution, machine-refinement or
GPU-launch credit from this checkpoint.

## Implemented

Conditional SubjectV3 policy and signing-key capabilities now retain the actual
sealed objects. They reuse the existing native descriptor and secret-custody
implementation. Transfer revalidation checks object identity, not just equal
bytes. Key use pins the complete policy and revalidates the sealed seed before
and after signing. Seed staging is wiped on success, refusal and unwind.
Each operation uses the supplied account and requires prepaid retained inputs;
the caller must preserve the original account across operations. These primitive
capabilities do not independently retain a work owner. Returned owner storage
must be reserved separately. These are not RSS or instruction bounds.

An external-anchor transaction V3 consumes actual policy, request and publication
V3 owners. Its version-separated 1,874-byte image binds the exact subject,
signed receipt and rollback position. Its digest feeds the existing anchor
protocol; it does not introduce another monotonic service or durable journal.

The existing identity-only current-record V3 wire is unchanged. New typed
`new_native_v3`, `issue_native_v3` and `verify_native_v3` operations join it
to the conditional carriage. They authenticate both pinned keys, the exact
carriage, signed commit transaction and fresh challenge-bound anchor observation.
The corresponding V2 methods retain their original behavior through shared
private bodies. No V2 admitted owner is relabeled as V3.

Sealed bytes do not establish independently administered policy, protected key
custody, a live compiler occurrence, durable Worker provenance or GPU authority.
The currentness result explicitly does not authenticate a protected current
Worker record on its own.

## Integration Dependencies

The backend and Cargo boundary still admit V1 execution custody. Conditional
family selection must come from an independently pinned profile and authenticated
launch, not a packet tag, source name, environment fallback or decoded receipt.

There is also a lifetime dependency: backend admission precedes creation of the
conditional target budget, whereas the current native client exclusively borrows
that budget. The target work owner must move into the outer synchronous codegen
scope before native admission. A private validated peer must retain its original
ledger identity and absolute deadline without preventing later compiler work.
Only the terminal receipt exchange should borrow the account exclusively.
The source account remains separate and must not be reset.

Remaining work includes nominal V3 profile/launch/readiness and service packets,
the existing issuer's actual V5 occurrence observation and durable Worker/anchor
joins, trusted parent recovery/preflight, and the consuming production publication
continuation. The same protected invocation must be revalidated immediately
before publication. Machine semantics and the complete target-matched kernel
matrix remain separate requirements under
[#214](https://github.com/harsh-nod/fe2o3/issues/214).

## Verification

The primary owns all serial builds and tests. The native worker implemented the
capability slice in a separate source-only worktree. Runs use pinned nightly
2026-04-03, locked offline dependencies, one Cargo job/test thread, hidden GPUs,
the existing bounded cache and private writable runtime directory.

Each result applies to its named snapshot. Counts overlap and must not be added.
Confirmed results:

- Working tree preceding `a842114bf`, before the final two tests: 188 library and
  integration tests, 68 compile-fail examples and five positive examples passed.
- At `6d09a242f`: all ten new anchor/currentness tests passed, including exact
  and one-short nested budgets, valid resealed coordinate substitutions,
  cross-family refusals and signed alternate-anchor observations.
- At `6d09a242f`: all 118 capability unit tests, 32 compile-fail examples and
  two positive examples passed.
- At `daafac70f`: all 122 capability unit tests and the same 34 documentation
  examples passed. Four added tests call the actual V3 key receipt/currentness
  operations with same-ledger retention, underpaid inputs, wrong generation and
  stale challenge cases.
- At `daafac70f`: all nine existing native Worker crash/restart/currentness
  regressions passed. These execute the ordinary test consumer, not protected
  process admission or a new V3 deployment.
- At `a1e48cc17`: locked offline `cargo check --all-targets` passed for the
  execution client, issuer, broker authority service, closure capabilities,
  Cargo driver and rustc backend. These are Cargo target kinds, not GPU targets.
  Existing warnings remain; this is not whole-workspace validation.

Independent source review found no actionable correctness or resource-accounting
issue in the protocol adapters and tests. An initial Worker regression filter
matched zero tests; that command supplies build evidence only, not test credit.

Two earlier focused runs failed because of new test-fixture construction errors:
the anchor transaction uses an unprefixed digest preimage, and an alternate
anchor key requires a challenge bound to that key. The fixtures were corrected;
production validation was not weakened. Failed runs remain recorded.

Log SHA256s for the first three passing runs above, in order:

```text
a43e2f6dd05cd2efcf723b06a3f8923f7cf9b56acac468f30e8dfe83ad140f66
5ae5cdcb996825f5e232b859041d7ecab2a2823522b3aef48d4fb01ccfe08d50
7ce820632735a6c8ff0ceb46842af115ecbc53a4e79d22796178927e4ab6d7e8
```

The final capability-run log has SHA256:

```text
68d1b8a687ae13797eb09307f90ce92f803ad81aa486be9a1ffe21c3197f3281
```

The Worker regression log has SHA256:

```text
2833b571afd687003082ddea22a4f0255b79ff8e24fc9c927beb569e27b8bc8e
```

The downstream check log has SHA256:

```text
7ea87cdefc6172f1fb73ee16406a17f8b9b06fe14979bb555fb84cfef725732c
```

Both public mains were most recently observed at `e934c1437`, ahead of the
previous public checkpoint. Normal Git fetch still fails DNS, so these local
commits are not yet merged with that concurrent work or pushed. GPU SSH name
resolution also fails. No new remote job or scratch directory was created.
