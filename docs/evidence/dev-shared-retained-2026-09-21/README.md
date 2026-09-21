# Shared Retained Admission and Unknown Execution

Development qualification of an allocation-free production Rust/Verus shared
executable slice. This is not registered proof-runner acceptance, whole-runtime
refinement, native admission, HIP/HSA parity or performance acceptance.

## Scope

`retained_bodies.rs` is included unchanged by the private ordinary Rust adapter
and the Verus root. Seven executable bodies implement full writer-key equality,
strict allocation-key order, exact allocation lookup, retained-header admission,
member admission, bounded chain traversal and Pending-to-Unknown execution.
Production settlement preflight and Unknown disposal reuse the shared retained
header/chain path; public Unknown marking delegates to the shared transition.

The chain body takes a syntax adapter and loop annotations. Rust's pinned identity
adapter receives an empty annotation list. Verus's pinned `verus_exec_expr` receives
only the fixed invariant/decreases clauses. The complete executable loop, including
its bounds, early errors and dangling-tail check, is shared. Array accesses retain
the existing test instrumentation; the production non-test implementation and the
Verus stub are empty. Instrumentation does not decide admission or perform stores.
The shared Unknown operation validates first, then replaces exactly one Pending
writer slot. Repeated Unknown marking performs no store. No allocation, public
authority, fallback path or global-validity restriction is added.

Raw proofs have no journal-validity or global-idle premise. They establish exact
admission results, unchanged-on-error modeled contents, and content-identical
repeated Unknown marking. Invariant-conditional issued wrappers preserve writer
history, stable leases and producer reservations. Constructor/enroll/register/Begin
witnesses execute empty and two-member writers, Unknown/repeat, and both settlement
outcomes. These constructor witnesses have empty reader arenas; they do not prove
a mixed-reader constructor trace.

Settlement composition uses the shared retained admission and prior shared scalar
return guard. Its scratch scan, staging and commit remain separate model/production
bodies. Matching type aliases and the corresponding proof type declarations were
reviewed, not proved as a full type/view correspondence theorem.

## Retained Checks

- Two whole-crate Verus positives: **382 verified, 0 errors** each.
- Fifteen executable negative controls: **381 verified, 1 error** each. Thirteen
  identify exact postcondition failures for writer identity, strict order,
  allocation identity/error precedence, header identity/Unknown admission,
  member identity/backlink/epoch/lineage, and chain guard/tail. Two identify exact
  supporting-assertion failures for writing after rejection and storing Pending
  instead of Unknown. Only the intended included executable body is changed;
  contracts, annotations and supporting assertions are unchanged.
- **833 unit tests passed**, two ignored, and **27 doctests passed**. Formatting
  and Clippy (`--all-targets -- -D warnings`) passed.
- Seven new regression tests exercise synchronized foreign identities, equal
  keys at distinct slots, every member writer-identity coordinate and position,
  actual truncated vector boundaries, missing interior links, unrelated malformed
  state, empty arenas, repeat identity and the shared-source integration shape.
  Existing corrupt-state/mixed-reader tests remain in the full CPU suite. Full
  snapshots include all seven vector pointers/capacities; access counters check
  rejection precedence and touched-chain bounds. These are CPU evidence, not
  theorems about physical pointer/capacity identity.

The 367 inherited obligations overlap earlier packets. Counts are not coverage
ratios and must not be summed across archives.

## Evidence Custody

The solver compiles the actual shared macros in their nested include layout.
The checker pins the root, body, complete Rust adapter, prior source/checker chain
and Verus release closure. Both live qualification and offline staging authenticate
the identity adapter and its empty annotation invocation. Macro envelopes and
the proof-only annotation invocation are checked before the pinned source-policy
scan. Full CPU source brackets bind production delegates and tests too.

Negative diagnostics must identify the exact intended postcondition or supporting
assertion. Secondary exits must be complete return expressions or exact block
exits, never arbitrary source-consistent substrings. The checker authenticates
typed coordinates, excerpts, highlights, labels, expansion nesting, invocation,
macro name and definition site. Partial runs, compiler errors, timeouts and
extra diagnostics are not accepted. `selftest.py` includes source-consistent
non-exit relocation, assertion relocation, adapter substitution, source injection
and rehashed-packet controls; the pristine packet must pass first.

Before/after input hashes, frozen case receipts, exact owned-process completion
and before/after 190-file pinned Verus closure measurements accompany the campaign.
Default solver limits, four threads and a 180-second outer timeout are retained.
`SOURCE` names the signed source commit. The offline auditor reconstructs nested
sources and mutations from Git objects, rechecks diagnostics and process receipts,
and binds CPU inputs to that commit. Runs started in a development worktree;
afterward source binding is not a claim of execution from a clean signed checkout.
Hashes alone do not prove execution. Incomplete development attempts are excluded.

## Reproduce

From the repository root, using the pinned Verus release:

```sh
python3 -I -B docs/evidence/dev-shared-retained-2026-09-21/check.py \
  --repo "$PWD" --verus /path/to/pinned/verus --output /new/owned/proof
python3 -I -B docs/evidence/dev-shared-retained-2026-09-21/selftest.py \
  --repo "$PWD" --proof docs/evidence/dev-shared-retained-2026-09-21/proof \
  --packet docs/evidence/dev-shared-retained-2026-09-21
python3 -I -B docs/evidence/dev-shared-retained-2026-09-21/audit.py --repo "$PWD"
```

The live checker retains development-layout pins. The portable retained-packet
audit requires Git objects but not the original solver, case paths, build output
or GPU. A rerun at a new path needs its own campaign identity; it cannot replace
the retained receipts retroactively.

## Remaining Gates

This advances gate 1 of the [producer-read roadmap](../../runtime-producer-read-reservations-v1.md).
Construction/enrollment and sorting/search, Begin, settlement scratch/staging/commit,
reader admission/release and unread guards still need full production correspondence.
Standard-library indexing/length/capacity behavior, physical storage, allocator
failure, normal/unwind semantics, compiler/machine refinement and complete type/view
correspondence remain outside this proof. No native GPU or benchmark ran here.

Context event/member binding, independently retained producer results,
default-false success-gated backend capability, bounded producer-first progress,
native pending XGMI dataflow and matched HIP/HSA comparisons remain open.
Native pending consumers remain fail-closed.
