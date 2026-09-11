# Unpublished Lifecycle

R83 implements the private ADOPT-LIFE engine substrate against signed R82
`5184428b7bb9bbb0e9cc30c6929d3c77ee60cbdb` on
`codex/r65-runtime-drain-versions`. The
[contract](../../runtime-unpublished-operation-lifecycle-v1.md) records exact
ownership and acceptance boundaries. Native DATA hooks remain uninstalled;
production generated preparation has no hooks for the private activation
transition. No public activation API is added.

## Review And Scope

Three read-only workers reviewed lifecycle, native ingress and resource custody;
Primary implemented and tested the changes. The same reserved driver/ticket,
completion producer/consumer and host payload survive parked-to-active transfer.
Context holds block same-stream work, destruction and graph/cleanup admission.
Drain and owned shutdown explicitly retire unpublished custody without flushing.

Review found equal-budget progress and retirement rotations could starve each
other. Retirement now scans without rotating, with both initial-order unit-budget
regressions. Native review found older pending submissions could publish from
polling despite a hold. Hold admission now rejects exact-stream nonquiescent
work; a real Context/MockBackend launch/event regression checks rejection and
subsequent admission only after conclusive completion observation.

Fifteen new runtime CPU test functions cover scripted private hooks, actual
command/registry/Context transitions and owned shutdown. Pointer and reply-cell
checks do not establish R73 account integration or native refund behavior.
Retirement handles an empty prefix after Stop before first advancement.
Failed retirement retains the carrier and Context and does not retry effects.

## Acceptance

The frozen-source `r83-final` attempt passes all seventeen gates with 5,610
non-documentation source identities unchanged. Exact commands, raw logs and
counts are retained in `raw/` and `test-summary.json`. Focused checks are
included in the full suite totals, not additional independent test counts.

| Gate | Result |
| --- | --- |
| Five runtime crates, GNU all features/targets | 2,363 passed, five existing ignores, 48 reported harnesses |
| Same runtime scope, musl | 2,363 passed, five existing ignores, 48 reported harnesses |
| GNU runtime plus host doctests | 101 passed |
| musl runtime / default host doctests | 84 / 16 passed |
| GNU all-feature host / musl default host | 246 passed, four existing ignores / 129 passed |
| Generated macro fixture harness | Seven passed |
| All-feature/all-target and production library Clippy | Both pass with warnings denied |
| Runner/checker Python suite | 151 passed |
| Formatting, whitespace, dependency policy/tests and local CI gate | Pass |
| Standalone lockfiles | All 32 pass |
| Focused unpublished lifecycle / complete async suite | 15 / 233 passed |

The unchanged negative-proof inventory passes 686 files without running Verus.
The production musl metadata audit passes 43 packages and eight permitted build
scripts; metadata SHA-256 is
`6c0cac53358224a31c356d91228b602ae8545f7c156a0c695f103533238054ca`.
Local documentation links and anchors pass. Cargo uses `nightly-2026-04-03`,
locked/offline resolution, four build jobs and disabled incremental compilation,
with `XDG_RUNTIME_DIR` removed.

The immediate parent is planning-only commit
`32c1beff73f829e7ff5db78333acfdaa0a7f685c`; its non-documentation tree is unchanged
from signed R82. Preliminary tests exposed an unsupported `unwrap_err` Debug
bound and missing
test imports; both were fixed before final gates. Initial unused-field warnings
were removed with explicit diagnostic formatting and production hold access.
Callback type-complexity lint was fixed with named function-pointer aliases;
the preliminary runtime all-feature/all-target warning-denied lint then passed.
The review-found scheduling and pending-publication gaps were corrected before
the accepted source snapshot.

## Open Boundaries

No native materialization, protected production construction, actual R73 charged
carrier integration, Linux qualification, new Verus theorem, solver rerun or
whole-executor refinement is claimed. The unchanged negative inventory is a
source-policy check, not solver execution. Model/accounting/completion and root
Cargo inputs remain unchanged from signed R73
`fb1e27e66cee27bb11f0b7c08f2d5994c5d168da`.

Native allocation shells, closed packet transfer, complete prefix ownership,
publication/completion, Linux campaigns and matched HIP/HSA performance remain
open. No SSH, GPU workload or remote staging was started; shared-machine cleanup
was not needed.

## Retention

Raw commands/logs and the frozen non-documentation source inventory live in
`raw/`; aggregate counts live in `test-summary.json`. `source-files.sha256` is
repository-relative and `retained-files.sha256` is evidence-directory-relative.
Raw test logs are retained without whitespace normalization; staged authored
source checks exclude `raw/*.log`. Hashes are checked before signed dual push.
