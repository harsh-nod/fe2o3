# Borrowed Coherent Initialization

Local R78 evidence against signed R77
`0193b8fa0c26ecef072acafd72979484d74d44e9`, branch
`codex/r65-runtime-drain-versions`. The
[contract](../../runtime-borrowed-coherent-initialization-v1.md) separates native
borrowed hooks and ordinary runtime copy removal from complete generated adoption.

## Scope And Review

Owned and borrowed coherent initializers share one private fixed-token
allocate/copy/map sequence. Queue insertion/replacement preserve existing guards,
model loan/retake, identity recording and terminal custody. Both runtime
HostVisible materializers now borrow the existing DataSpec range directly;
DeviceLocal's owned-content route is unchanged.

Three read-only agents reviewed native transitions, runtime source lifetime and
accounting/fault coverage. No production blocker was found. Review strengthened
the exact rebound source-argument check, partial-map call counts, copy-panic
currentness and record-held versus quarantined charge assertions. Workers ran no
builds or hardware jobs. Primary owns all edits and verification.

Ten new CPU tests comprise six shared-memory, two queue and two runtime cases.
The production sequencer runs with actual engine/accounting records over a fake
backend, not a successful Linux session. Source-wiring checks remain distinct
from execution. Allocation counting covers DataSpec views versus owned copies,
not whole native calls.

## Final Gates

All seventeen gates in `r78-release` passed against the final source. The runner
checked 5,594 non-documentation source identities before and after; the retained
snapshot and per-gate commands/results are under `raw/`.

| Gate | Result |
| --- | --- |
| GNU runtime, five crates/all features/all targets | 2,271 passed, five existing ignores, 48 harnesses |
| musl runtime, same scope | 2,271 passed, five existing ignores, 48 harnesses |
| GNU runtime plus host doctests | 94 passed |
| musl runtime doctests / default host doctests | 78 / 15 passed |
| GNU all-feature host / musl default host | 236 passed, four existing ignores / 119 passed |
| Generated macro fixture harness | Seven passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Both pass with warnings denied |
| Runner/checker Python suite | 151 passed |
| Formatting, whitespace, dependency policy and its regressions | Pass |
| Local CI gate regression / standalone lockfiles | Pass / all 32 pass |

Cargo used `nightly-2026-04-03`, locked/offline resolution, four build jobs and
disabled incremental compilation, with `XDG_RUNTIME_DIR` removed. The local
Markdown check passes all 59 links/anchors. Ten new CPU test functions are
included in the totals, not additional runs. No new doctest was added.

The current production musl dependency audit passes 43 packages and eight
permitted build scripts. Retained metadata SHA-256:
`6c0cac53358224a31c356d91228b602ae8545f7c156a0c695f103533238054ca`.
The negative-proof inventory check passes 686 files, with the solver boundary
described below. Partial/failed runs are not final acceptance evidence.

## Earlier Attempts

The initial production cargo check passed; its terminal-only output is not
substituted for retained source gates. The retained first focused build rejected
three test errors: an ambiguous integer, reading a GPU-mapped token through the
CPU-writable API, and a shared-allocation identity where a fixed-dispatch identity
was required. Tests now unmap before CPU reads and use correct typed identities.
Production implementation was unchanged by these corrections.
The corrected focused run passes all eight KFD and two runtime cases; its log
is retained separately from final frozen-source gates.

The first full sequence (`r78-final`) passed its eleven test/lint gates, then
stopped at rustfmt's layout of a nested test-fixture expression. The fixture now
names its storage identity before constructing the repeated vector; production
code and assertions are unchanged. The final acceptance sequence reruns all
seventeen gates against the corrected source, rather than combining partial runs.

## Proof And Hardware

Model, resource-accounting, completion, root manifest and lockfile inputs remain
unchanged from signed R73. No theorem is added or Verus solver rerun claimed.
The negative-proof inventory and production dependency audit are separate local
checks; neither proves native initialization, disposal or whole-executor refinement.

No complete generated adapter, new publication/timeout/readback behavior,
production compiler evidence, GPU qualification or HIP/HSA performance comparison
is established. No SSH session or remote stage was started; no shared-machine
cleanup was required.

## Retention

Commands, raw logs, failed attempts and final source identities are under `raw/`.
`source-files.sha256` is repository-relative; `retained-files.sha256` is relative
to this directory. Verbatim logs are not reformatted; post-retention whitespace
checks exclude `raw/`. No manifest or lockfile changed.
