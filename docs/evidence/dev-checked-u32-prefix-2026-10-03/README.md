# Checked U32 Entry-Prefix Qualification

Candidate baseline: `802cac8cb8ff8ae3ecf483fe8bd536fbd319caa8`.
This checkpoint adds the [owner-bound entry-prefix checker](../../runtime-checked-u32-prefix-v1.md)
and a shared-executable-fold proof. It does not alter native multi-GPU transport
or authorize ordinary application kernels.

## Accepted Scope

The checker independently normalizes the actual semantic MIR and current V8 KIR
retained by one genuine lowerer owner. It checks direct-u32 argument correspondence,
exact statement spans, mutable-local copies/redefinitions, constants and the
captured checked-add operand/value/overflow. The returned relation borrows that
same owner. It covers the first entry traversal and observed terminal value,
not every dead intermediate value or later visits.

Verus proves the shared executable fold's successful-result contract and its
denotation by an independent concrete fold for every valid common input vector.
Equal initialized terminal origins imply equal concrete values. The existing
shared widened-add body supplies the modulo-2^32 value/overflow contract.
Normalization adapters, real rustc extraction, physical machine entry,
continuation, memory and protected invocation/launch authority are not proved.

## Final Qualification

- Verifier library: 114 passed, four existing ignored tests; all seven new
  prefix tests pass, including boundary, argument-role, mutable-version,
  unsupported-effect and malformed-KIR cases.
- Existing public checked-add integration suite: seven passed.
- Verifier doctests: 30 passed, including two new owner-lifetime compile failures.
- Strict production-library Clippy, no-default library check and changed-module
  formatting passed. This is not all-target Clippy or a workspace-wide test run.
- The final source-bound proof campaign passed 13 stages: pinned tool closure
  before/after, six runner controls, two whole-proof runs with 11 verified
  obligations and zero errors, and eight logical negatives.
- Three negatives fail the exact successful-return postcondition; five fail the
  symbolic-prefix loop invariant. Each has one verified obligation and one
  intended error. Parser, bounds, overflow, timeout and unrelated diagnostics do
  not count as accepted logical failures.
- Local CI dispatch tests and the actual proof shell wrapper passed. The wrapper
  removed its own temporary directory. The workflow and local Verus lane include
  the new campaign; remote GitHub Actions execution is not claimed.

The source fixtures are constructed admitted semantic MIR run through the real
lowerer and capture APIs. They are not a rustc-source extraction witness.
The conditional evaluator's boundary inputs are tests, not evidence of actual
device argument values. No GPU execution or performance measurement was added.

`qualification.tar.xz` retains exact command/environment receipts, complete
source inventories, the unit-test executable identity, test rosters, positive
and negative solver diagnostics, original staged proof sources, the source patch,
and audit scripts. The final audit rechecks source and output hashes, complete
test rosters, staged mutations and exact solver rejection classifications.
Independent read-only review also confirmed the final 22-file proof closure,
13 accepted stages, process-group absence and parent receipt continuity.

Earlier failures remain as excluded diagnostics: incomplete synthetic ABI
attributes; lexically misordered fixture identities above 255 locals; a fixture
that reused a moved value; a CI run whose fixed PATH omitted ripgrep; and the
broad test-target Clippy run's existing unrelated warnings. No production
admission check was relaxed to resolve them. Earlier passing source cohorts are
not substituted for the final-source acceptance roster.

## Remaining Work

The next functional gate is an ordinary reachable Rust helper extracted within
the active compiler transaction into this same retained owner, followed by
verified normalization and semantic/machine entry, continuation/memory and
protected invocation-bound admission. General multi-GPU application compute,
A3, issue #182 and HIP/HSA parity remain incomplete.

No MI300X files or processes were created during this checkpoint. Local cleanup
removed only the owned generated `target/tests/trybuild` cache (632.3 MiB);
source, sealed evidence and current build outputs were preserved.
