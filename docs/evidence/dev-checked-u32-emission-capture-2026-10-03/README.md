# Actual Checked-Add Emission Capture

This compiler-admission increment is based on `8eb0ebcb954c5cf795f08ba4265da54681bf40c9`.
It does not change native multi-GPU transport or enable ordinary application
kernels. Already-admitted multi-GPU workloads do not depend on this capture.

## Ownership And Scope

The real SSA adapter now optionally observes source events, their ordinals and
resolved values. It joins these to the original retained SSA plan, preserving
ordinary identities, summaries and the current event grammar. The source layer
and resource ledger are narrowly adapted from upstream `839861804`; later
projected-call/borrow changes and V12 emission-owner frameworks are not imported.

An opt-in current V8 lowerer captures one admitted `Copy(u32 local) + u32 literal`
checked-add from its actual SSA bindings and emitted operations. The same
move-only compiler owner retains the source occurrence and KIR operand, value
and overflow IDs. Replay reruns actual lowering and compares the capture.
The borrowed fact survives that owner's move into formal-memory admission;
callers cannot assemble it from detached IDs or substitute another owner.

The verifier joins exact MIR and typed V8 bytes and source/emission coordinates
to the existing conditional MOV-prefix/ADD obligation. Equal-content owners may
match the decoded bytes, but the returned capture continues to borrow its actual
owner. Neither unresolved entry-value equality is discharged.

Resource accounting covers added occurrence work/storage and retained capture
layout, with error/unwind rollback. Existing source/SSA/lowering allocations and
bounded lookup/replay work are explicitly outside this capture-only ledger; it
is not an end-to-end RSS limit or semantic proof.

## Qualification

Generation 02 runs all-feature library/integration tests for kernel-ir, PLIRON,
lower-MIR, kernel-analysis and verifier: 1,895 pass, with 14 existing ignores.
The 1,909-name roster spans 94 harnesses. All 56 new source-occurrence tests,
four lowerer tests and three public borrowed-join tests pass, as do the resource
ledger tests. All five doctest suites pass, totaling 49 tests, including capture
lifetime and private-construction compile failures.

Strict library/binary and selected public-test Clippy, five-package no-default
checks and the all-feature host-library check pass. This is not all-workspace
test coverage or all-target Clippy. Source controls, formatting and the final
independent receipt audit are retained in `qualification.tar.xz`.

The archive contains exact commands/environment, source inventories, executable
identities, test rosters/output, the source patch and independent auditor.
The auditor requires final-source receipts and checks that existing proof/shared
bodies are unchanged. No new solver execution or instruction theorem is claimed.

Earlier diagnostics are retained but excluded: one stale enum name, two invalid
synthetic tuple-layout fixtures, missing fixture imports and unused backport
wrappers. The final fixtures use the admitted aggregate layout; dead wrappers
are removed and direct test helpers are test-only. No admission check was relaxed.
Generation 01's passing tests preceded the final lint cleanup and are not the
selected final-source qualification.

## Remaining Boundary

Actual source/SSA/KIR capture is structural provenance, not a source-state value
invariant, machine-entry correspondence, protected compiler-origin proof or
launch authority. CFG/continuation, ABI/EXEC, addresses, memory effects,
completion and protected per-invocation compiler/verifier/finalizer custody
remain separate obligations. A3, issue #182 and the broad parity goal remain open.
