# Local R88 Session Transition Evidence

Implementation baseline: signed R87
`29462c524104d9bb01052376a0ef79c9ffdc3d61`, published on both repositories'
topic branch `codex/r65-runtime-drain-versions`.
This packet implements the
[session transition custody contract](../../runtime-session-transition-custody-v1.md),
CONTROL-1 above R87 pending GTT allocation custody.

## Status

All seventeen frozen-source gates and the auxiliary checks pass, with 5,624
non-documentation source identities unchanged. All three expected-negative
mutations reject and restored source passes. Two read-only source reviews found
no blocking issue; a third reviewed the updated completion work orders.
No SSH, GPU workload, remote staging or Verus solver was started.

## Final Acceptance

| Gate | Accepted result |
| --- | --- |
| GNU and musl runtime crates, all features/targets | 2,435 passed and five ignored per target, across 48 harnesses each. |
| GNU all-feature host / musl default host | 258 passed, four ignored / 141 passed. |
| GNU runtime and host doctests | 108 passed. |
| Musl runtime / default host doctests | 91 / 16 passed. |
| Generated macro fixtures | Seven passed. |
| Focused transitions / pending allocation / shared memory | Twenty / fourteen / 195 passed; the first two selections are subsets of shared memory. |
| Existing retention / readback / charged / shell / storage / adoption / async selections | Thirteen / ten / fifty / 19 / nine / fifteen / 233 passed. These are subsets; storage and shell overlap by one runtime test. |
| Lint, formatting and policy | All-feature/all-target and production-library Clippy pass with warnings denied; formatting, whitespace, dependency policy/tests, local CI gate and 32 standalone lockfiles pass. |
| Runner/checker tests and production closure | 151 Python tests pass; dependency audit passes for 43 packages and eight build scripts. |
| Proof inventory | 686 negative files inventoried; no Verus solver rerun. |

R88 adds twenty KFD CPU test functions and updates the existing revision-budget
regression to check delegation plus actual production-helper behavior. It adds
no runtime/host test function, doctest or theorem. The first full source-gate
attempt, `r88-final`, passes; no failed full-suite attempt is omitted.

The [summary](test-summary.json), [changed-source hashes](source-files.sha256),
[retained-file hashes](retained-files.sha256),
[accepted gate record](raw/r88-final-source-gate.json) and
[complete source identities](raw/r88-final-source-inputs.json) retain the
accepted packet. Helpers, production metadata, focused/mutation/restoration
logs and full-gate logs are retained with their hashes.
Raw Cargo logs preserve their terminal blank lines. The staged whole-tree
whitespace check reports those log endings; source/document checks excluding
raw `.log` files pass. Raw evidence is not reformatted to suppress this warning.

## Intermediate Checks

Twenty new KFD CPU test functions exercise the production transition adapter
with actual fake-native records, optional backing accounts and a queue
foundation. They cover all supported allocation profiles, genuine native/model
aperture divergence, conflicting model mapping, exact revision budgets,
seal/map/currentness error and panic, returned partial map prefixes, exact
terminal precursor or successor custody, retain rejection and materialization.
Test-only stage injection is identified separately from naturally rejected model
transitions. The successful certified foundation-loan test checks full bytes
and exact token/mapping identities, not authenticated code or GPU execution.

The first focused compile stopped at two test-only fixture mistakes: a
nonexistent USERPTR constant and a usize/u64 count mismatch. Both were corrected
before preflight acceptance. That compiler output, an early production KFD check
and an early KFD Clippy pass are available only in the execution transcript,
not retained raw logs. They are not substituted for the full recorded gates.
Focused preflight, shared-memory and restored runs pass.

Three isolated mutations each fail one targeted regression with Cargo exit 101:

| Mutation | Rejected behavior |
| --- | --- |
| Discard terminal output | The real post-allocation projection rejection loses the returned token; exact terminal-output assertion fails. |
| Move input before native map | A native/currentness panic loses the precursor token; exact terminal-input assertion fails. |
| Allocation revision preflight 2 -> 1 | The complete-budget regression observes missing initial process-gate poisoning instead of rejecting before native effects. |

All mutations were restored before final acceptance. The restored production
helper, new tests and shared-memory integration match the accepted frozen-source
SHA-256 identities. The focused helper logs do not independently snapshot every
intermediate source; they record their selected tests and expected exit status.
No mutation remains.

## Open Boundaries

Admitted failures preserve the actual token fields in bounded opaque terminal
custody with no reconstruction, retag, extraction, cleanup or retry API. Raw
native observations remain untrusted. Existing records and N1/N2 charges are
retained without refund. The early active guard deliberately rejects a terminal
session without another currentness/revision observation; invalid foreign
consuming inputs retain legacy rejection semantics, not unbounded custody.

Control materialization borrows the caller's token and preserves the original
panic. It still needs CONTROL-2 to retain that caller-owned token and the full
preparation across outer unwind. CONTROL-3 must cover both bind cardinalities,
closing-retake/validation error and panic, and terminal-versus-retryable
classification. NATIVE-2 constructor custody, unreturned backend mappings,
unmap/release/teardown custody, native generated adoption and control/aggregate
budgets remain open.

Historical accounting/model proofs do not prove this adapter. Model,
resource-accounting, completion and manifest/lockfile inputs remain unchanged
from the R73 proof checkpoint; its 62 positive sources and 1,374 obligations
are historical evidence, not a new R88 proof run. No Linux execution, new
executable refinement, performance result, A1/A2 completion or HIP/HSA parity
is claimed.
