# Finite Async Preparation

Local R79 evidence against signed dispatch baseline
`aeec40eba081baff3b856ef676d970b11a40b69a`, whose implementation baseline is
signed R78 `042dcec1bc67f919e02ce4908a62ecc8d1e8b113`, on branch
`codex/r65-runtime-drain-versions`. The
[contract](../../runtime-async-generated-preparation-v1.md) distinguishes finite
host preparation from native adoption and generated launch completion.

## Scope And Review

An inert Send factory enters the existing reply/command/operation admission
path. Its owner-local driver invokes immutable Context preparation once, parks
the complete carrier before returning an opaque ticket, and remains retained
through explicit discard or owned shutdown. Active and parked entries share one
capacity ceiling; only active entries participate in progress, flush and graph
exclusion. Quiescent driver disposal is inside the owned unwind boundary.

The actual protected host constructor and persistent projection are wired to
this path. The carrier retains the original result account, storage, decoder and
authority without a second debit or generic extraction API. This is reviewed
source composition, not a successful protected constructor test.

Three read-only agents reviewed lifecycle, native admission, resource ownership,
and evidence boundaries. Review found and fixed a factory control-transfer bug:
cloning the control let the emptied factory stop the driver's observation on
Drop. The factory now transfers it with `Option::take`. A real command-to-park
regression checks callback execution and `ObservationFinished`.

Eighteen new runtime tests cover capacity, cancellation, errors, exact ticket
identity/retry, observer loss, no-flush parking, graph coexistence, Stop races,
shutdown failure and panic. Review added latest-waker replacement across the real
preparation/discard futures and a wrapper around the actual driver that injects
post-park completion panic before/after reply publication. One new host test
checks protected-constructor/account wiring. No worker ran builds or hardware;
Primary owns all edits and verification.

The tests use Rc payloads/drop counters and fake-backend Contexts. They do not
measure actual R73 charged storage through a successful production constructor.
The protected bridge's synthetic-device rejection test is negative evidence.
Four new runtime compile-fail doctests and one host compile-only example check
the stated ownership/type boundaries, not native execution.

## Final Gates

All seventeen gates in `r79-release` pass against the final source. The runner
checked 5,596 non-documentation source identities before and after. Exact
commands, exit codes and source hashes are retained under `raw/`.

| Gate | Result |
| --- | --- |
| GNU runtime, five crates/all features/all targets | 2,289 passed, five existing ignores, 48 harnesses |
| musl runtime, same scope | 2,289 passed, five existing ignores, 48 harnesses |
| GNU runtime plus host doctests | 99 passed |
| musl runtime doctests / default host doctests | 82 / 16 passed |
| GNU all-feature host / musl default host | 237 passed, four existing ignores / 120 passed |
| Generated macro fixture harness | Seven passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Both pass with warnings denied |
| Runner/checker Python suite | 151 passed |
| Formatting, whitespace, dependency policy and its regressions | Pass |
| Local CI gate regression / standalone lockfiles | Pass / all 32 pass |

Cargo used `nightly-2026-04-03`, locked/offline resolution, four build jobs and
disabled incremental compilation, with `XDG_RUNTIME_DIR` removed. The eighteen
new runtime cases and one host wiring case are included in the totals, not
additional runs. No new native negative fixture or compiler evidence is implied.

The local Markdown check passes 72 links/anchors. The production musl dependency
audit passes 43 packages and eight permitted build scripts. Retained metadata
SHA-256: `6c0cac53358224a31c356d91228b602ae8545f7c156a0c695f103533238054ca`.
The negative-proof inventory check passes 686 files; its solver boundary is below.

## Earlier Checks

An initial production check required widening the private `stop_reply` helper's
sibling-module visibility. Its terminal-only output is not release evidence.
The retained first preparation filter passes 16 runtime and nine host tests;
the expanded filter passes 22 runtime and ten host tests. These precede the two
final review-driven tests. The complete focused async suite passes 202 tests
before those additions and 204 afterward; both logs are retained separately.

## Proof And Hardware

Model, resource-accounting, completion, root manifest and lockfile inputs are
unchanged from signed R73. R61 capacity/reply and R62 control guards are reused;
no new theorem or Verus solver rerun is claimed. Negative-inventory checking is
not an authenticated rerun of the solver or a proof of the new adapter.

Positive protected construction, actual charged-result engine integration,
complete native adoption, readback reservations, publication/completion,
generated graph/drain, executable refinement and HIP/HSA performance remain open.
No Linux scope/queue acceptance follows from these CPU results. No SSH session,
remote stage or shared-machine workload was started; no remote cleanup was needed.

## Retention

`raw/` retains the exact gate commands, logs, source snapshot, helpers and
production metadata. `source-files.sha256` is repository-relative;
`retained-files.sha256` is relative to this directory. Raw logs are not reformatted
and post-retention whitespace checks exclude them. No manifest or lockfile changed.
