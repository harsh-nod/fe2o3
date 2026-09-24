# Source-Bound Conditional Memory Bounds

This checkpoint follows [shared source argument checking](source-argument-live-ledger-20260924.md)
and advances #272. It does not complete a milestone or change the 47-kernel
production-to-safe-GPU-launch qualification matrix.

Follow-up: [conditional proof retention and original-account replay](conditional-proof-retention-20260924.md)
integrates receipt retention and target-entry replay while preserving the typed
conditional-finalizer refusal.

## Integrated Changes

The conditional aggregate now requires the checked source-argument relation,
not a caller-supplied source digest. It retains the actual source owner and root
association, replays the canonical binding, and keeps canonical, source, and
adjusted ABI coordinates separate. Whole-slice recipe origins use the source
argument. The canonical output-store location is available without treating a
ranked operation ordinal as a canonical coordinate.

For the admitted dynamic D1/global-X case, the aggregate joins the complete
canonical read inventory to readonly global recipe views, their own extents,
indices and access domains. Materialization replay then joins those rows to
actual live operations and values. Recipe ordinals never stand in for live
operation identity. Both copies are charged to the original canonical account
and the production analysis resource contract.

The shared nine-stage pipeline now produces a nominal conditional MemoryBounds
report in its fixed slot 1. Typed read obligations capture exact operations,
views, indices and extents alongside the original bounds findings. Conditional
premises can discharge only matching read obligations. Static out-of-bounds,
machine overflow, structural errors, solver counterexamples and terminal solver
failures remain failures; diagnostic text is not a proof classifier.

Report validation retains that exact result. Ownership in slot 4 and semantic
refinement in slot 8 borrow its private same-invocation dependency, checking the
manager, subject, graph epoch, preservation checkpoint and complete read roster.
They do not rerun or reserve the bounds producer again. Ordinary pipeline
composition is unchanged. Foreign managers are refused before resource admission;
valid borrows prepay their ReportValidation work. `None` remains the legacy
diagnostic mode; `Some([])` means a source-checked complete empty read inventory.

Lower-MIR creates a fresh source relation inside each original-ledger callback
and still replays the full source-body translation. The production request has
private fields. The public Pliron aggregate alone does **not** attest continuity
of the original work account: a fresh checked relation on another funded account
can replay it, but cannot manufacture that production request. Tests explicitly
preserve this distinction rather than storing an address token as authority.

These are conditional analysis facts, not a new CPU-equivalence theorem, machine
refinement receipt, completed host precondition check, or launch capability.

## Validation

Implementation commit: `eee0e2e9398ca3e2cc5c8036f95d1ea666e1228b`.
Runs used nightly `2026-04-03`, locked offline dependencies, one Cargo job and
test thread, hidden GPUs, disabled HIP, a 12 GiB virtual-memory cap and a
1200-second deadline. Every listed run completed with stable source/tool inputs.
The source snapshot includes tracked and nonignored untracked files, not a Git
tree hash.

- A, 7876 files: `7ec2ba00009aed024b4c6598b3e37f8099695b2d2cdc5490e48085d80b0f1cc5`.
- B, 7876 files: `34b44b3228b33d8c44432a5d60b0bf2f05535260feb2d349de3ca8a409134f24`.

Only doctest comments in `conditional_pipeline_v1.rs` changed between A and B.
The implementation, unit tests and public exports did not change. The unit and
backend results below are for A, not claimed reruns on B. This evidence page and
its incoming link were added after validation.

| Guard run | Snapshot | Result | Log SHA-256 |
| --- | --- | --- | --- |
| `conditional-bounds-integration-r6` | A | Lower-MIR: 101 passed; Pliron: 123 passed | `68081a83b9da4d77bf0891cfcbfe913d3ba4b4686a39bc1d05939b4bdadc2e53` |
| `conditional-bounds-pliron-full-r1` | A | Full Pliron library: 1625 passed, none ignored or filtered | `d6dd591a327244048ab1a1fba7a1d4b9592f7d30f49f3ff8a3e6e2d1aaf151a3` |
| `conditional-bounds-lower-regressions-r1` | A | Lower-MIR: 630 passed; Pliron: 233 passed | `83ef277ee10900c59f032c3a3b19741c2e031d47771d01ba43b093642e257c3d` |
| `conditional-bounds-backend-regressions-r1` | A | Backend: 755 passed, 28 ignored | `018325fb71494dd614680bddd3c19eaa1ccc9a57d701a654653354a513909553` |
| `conditional-bounds-api-doctests-r2` | B | 27 lower-MIR and 11 Pliron doctests passed | `d196034f52ef338368a3d99a319888f7a467f1bf39ddcd53d97d724f0b798c61` |

Selections overlap and are not additive totals. The focused run selects
`conditional`, `source259`, `source_read` and `original_guard`. The wider run
selects `argument`, `call`, `parameter`, `conditional`, `physical`, `complete_body`,
`representation`, `shared_relation`, `shared_view`, `public_shared` and
`original_guard`. Both select `fe2o3-pliron` and `fe2o3-lower-mir-kernel` with
`fe2o3-pliron/internal-proof-staging`. The full Pliron run uses the same package
selection but filters all lower-MIR root modules. The full lower-MIR suite was
not run in this checkpoint.

Backend filters are `conditional_`, `production_ranked_projection_v1::`,
`input_guard_`, `consuming_continuation_`, `borrowed_continuation_`,
`ordered_composition`, `parameter` and `argument`. The ignored tests are not
counted as execution evidence. No protected proof or GPU execution is claimed.
The guard SHA-256 is
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.

The aggregate positive read fixture has independent input/output lengths: its
ordinary bounds report fails, while the source-bound conditional aggregate and
fresh replay succeed. This synthetic ABI fixture does not claim executable
Rust-body equivalence. Other tests cover missing/duplicate/substituted reads,
terminal failures, report omission and foreign custody, stale epochs, exact and
one-short resource limits, source-owner substitution and original-ledger reset.

Earlier integration attempts exposed a callback lifetime error, a typed-value
fixture error, and invalid fixture layout, ABI and root-closure data. Those were
corrected without relaxing admission. The first API run also exposed three old
compile-fail examples that stopped at unresolved imports. They now use the public
constructor, have a positive compilation control, and fail with the intended
E0277, E0599 and E0616 errors. The production-request construction negative fails
with E0451. The final 38 doctests include two positive and 36 negative examples.

Formatting, source-size hygiene and both dependency policies passed. Existing
lowering/backend dead-code and duplicate-target fixture warnings remain.

## Native Permission Probe

A separate bounded MI350 diagnostic executed two protected roles under the
existing container security policy. Same-namespace `pidfd_getfd` attempts,
including the self control and subject FDs 197/198/199, returned EPERM. Private
process and artifact accesses also denied. The owned-child-namespace case stopped
at `unshare(CLONE_NEWUSER)` with EPERM, before role execution in that namespace.
The observations do not identify which security-policy layer caused each denial.

This is not actual compiler observation, native Prepare/Issue, publication,
currentness or GPU evidence. Administrator access is available; native readiness
still needs an admitted observation and artifact-custody contract. No policy was
weakened. Both diagnostic containers, cgroups and scratch inputs were removed;
raw reports were retained and matched locally and remotely. The R2 report hash is
`9c402d0609be4813299a19afc05941b902043af75f59e5f911bea1aa9e5cad7c`.

## Remaining Work

The prepared owned-proof retention, descriptor transport and host-runtime patches
are not integrated or validated by this checkpoint. The normal transaction still
needs retained-proof consumption, generated-field attribution, conditional target
admission, applicable machine/numerical refinement, protected native issuance and
the generated safe host launch path. Actual source, simulator and target-matched
GPU acceptance must pass before any milestone or manifest qualification changes.
