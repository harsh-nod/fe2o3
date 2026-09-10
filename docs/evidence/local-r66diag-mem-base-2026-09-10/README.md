# R66 Diagnostics And Shared Credit Engine

Local implementation and regression evidence for OVL-DIAG-1 and MEM-BASE.
Baseline: `ca7476497ecf0be75e731ba01664f097bb1eed23`, branch
`codex/r65-runtime-drain-versions`. Changed inputs are identified relative to
the repository root in `source-files.sha256`; raw logs have a separate manifest.

## Scope

Native/runtime qualification observers now return fixed, address-free rejection
stages while preserving the Option interfaces, success predicates and digest
bytes. The example records cell, direction, order and observation phase. The
runner retains the actual bounded binary after a successful build and before
qualification; later rejected captures retain it too. Earlier build failures may
still lack a binary. These changes do not grant publication or retirement
authority, poll completion, or establish physical GPU overlap.

MEM-BASE moves the existing credit engine into `fe2o3-resource-accounting`, with
a thin runtime device-branded wrapper. It adds no native physical accounting,
parent budgets, batch/split transactions or aggregate quarantine ceiling.
Dependency policy and CPU/release-test rosters include the shared crate. All
22 affected lockfiles preserve registry package records; older fixtures also
reconcile pre-existing local completion/arrayvec dependency edges.

## Final Local Gates

| Gate | Result |
| --- | --- |
| GNU runtime all-feature/all-target tests, five crates | 2,034 passed; zero failed; five existing ignores; 47 harnesses |
| musl same runtime gate | 2,034 passed; zero failed; five existing ignores; 47 harnesses |
| GNU runtime plus host doctests | 77 passed |
| musl runtime doctests | 66 passed |
| musl default direct-KFD host doctests | Ten passed |
| GNU all-feature host library | 207 passed; four existing ignores |
| Generated macro fixtures | Seven passed |
| Runner/checker Python suites | 134 passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Passed, warnings denied |
| Formatting and source whitespace | Passed |
| Production musl metadata audit | 43 packages; eight permitted build scripts |
| Workspace dependency policy and its tests | 141 members, eight layers, 475 declarations; eight tests passed |
| CI-local shell regression and standalone lockfiles | Passed; 32 standalone manifests checked |

The five runtime crates are completion, runtime-model, resource-accounting,
KFD and runtime; lint also covers host and macros. Focused tests include eight
runtime diagnostic cases, six native diagnostic cases and the exhaustive stale
epoch matrix. Nine credit-engine tests moved with the engine; three runtime
wrapper tests and two compile-fail ownership examples supplement them.

Exact commands, elapsed times and exit statuses are in
`raw/r66diag-final-source-gate.json`. The serial harness uses the pinned Rust
toolchain, offline locked dependencies and four Cargo build jobs. It unsets
the stale `XDG_RUNTIME_DIR` for host tests, as established in the prior R67
record. The first lint attempt found two collapsible-match warnings, corrected
without suppression; all final gates reran afterward. Earlier passing tests,
the lint failure, an initial lockfile reconciliation assertion and a checker
usage error are retained separately, not promoted to final passes.

## Proof And Hardware Boundary

Model/proof sources, pins and the authenticated runner are unchanged from the
[R67 proof evidence](../local-r67-owned-credits-2026-09-10/README.md).
The 640-file negative inventory was rechecked. The full Verus solver run was
not repeated for this extraction; its earlier property-level results are not
a new proof of mutex/arena ownership, native extraction or the whole executor.
Independent source reviews found no predicate, digest or authority regression.

This record contains no accepted hardware or performance result. A new signed
MI300X campaign, actual ELF audit and owned-process/stage cleanup are separate
from local validation. A1/A2 and runtime-wide HIP/HSA parity remain open.
