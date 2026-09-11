# Generated Reservation

Local R80 evidence against signed dispatch baseline
`5c5133e631f186a35d9d649915e609a7ea00c2e2`, whose implementation baseline is
R79 `2d14fda36c91df9e776cf3044790af6f597f15f1`, on
`codex/r65-runtime-drain-versions`. The
[contract](../../runtime-generated-reservation-v1.md) defines host-only
reservation, not native adoption, launch completion or behavioral parity.

## Scope And Review

R80 composes the private generated-host carrier with a runtime-defined borrowed
source view, fixed complete-roster metadata and a typed reservation adapter.
Runtime checks actual HSACO identity, the existing Worker authority coordinates
and currentness, plus the original Context/native device binding. The immutable
R77 projection still supplies canonical packet/fixup/hidden-field invariants.
No source vector is converted to an independently retained Arc or cropped snapshot.

Readback storage is staged under one complete additional debit in the original
R73 account. The finite runtime command reserves acknowledgement and completion
reply cells before enqueue. Installation follows the closing checked scope;
ordinary failure or queued Stop returns the exact prepared ticket. Reserved
custody remains parked and explicitly disposable. Panic retains the owner under
the existing terminal Context cleanup policy, without returning a retry ticket.

Three read-only agents reviewed source/native binding, runtime lifecycle and
host accounting. Primary implemented every change and ran all checks. The review
required staging before installation to avoid a hidden debit after a late
currentness failure. It also found a new test race: after a drain reply, owner
cleanup can already have disposed the payload. The zero-drop assertion now runs
before drain, with exactly-one disposal checked after joining shutdown.

Twenty-one new runtime CPU tests cover source/roster rejection, reservation
identity and phase, reply/queue exhaustion, cutoff and queued Stop, wakeups,
observer loss, staged failure/install panic, and owner-thread drain/shutdown.
Nine new host tests exercise actual charged readback storage, including unused
read-only and zero-length entries, original-account overlap, retry, malformed
destinations, late allocation failure and unwind. Two new compile-fail doctests
reject reserved-ticket cloning and completion-consumer extraction.

The real protected constructor is wired to the typed bridge. Its successful
production construction and actual R73 storage through that constructor inside
the engine remain unqualified. Source joins reuse an existing data-only authority
fixture, never a native token. Stage/install tests inject a checked-stage error;
they do not qualify a genuine Linux closing-currentness failure.

## Final Gates

The first full attempt, `r80-release`, passed all seventeen gates against 5,601
unchanged non-documentation source identities. Review then moved the racy test
assertion described above. The final frozen-source attempt, `r80-final`, also
passes all seventeen gates with 5,601 source identities unchanged before/after.
Only this corrected snapshot is the publication acceptance source.

| Gate | Result |
| --- | --- |
| GNU five runtime crates, all features/targets | 2,310 passed, five existing ignores, 48 reported harnesses |
| musl runtime suite, same scope | 2,310 passed, five existing ignores, 48 reported harnesses |
| GNU runtime plus host doctests | 101 passed |
| musl runtime / default host doctests | 84 / 16 passed |
| GNU all-feature host / musl default host | 246 passed, four existing ignores / 129 passed |
| Generated macro fixture harness | Seven passed |
| All-feature/all-target and no-default library Clippy | Both pass with warnings denied |
| Runner/checker Python suite | 151 passed |
| Formatting, whitespace, dependency policy and regressions | Pass |
| Local CI gate regression / standalone lockfiles | Pass / all 32 pass |

Cargo used `nightly-2026-04-03`, locked/offline resolution, four build jobs and
disabled incremental compilation, with `XDG_RUNTIME_DIR` removed. New tests are
included in these totals. Exact commands and unmodified logs are retained.

The full focused async module passes 218 tests; its narrower `async_engine::tests`
filter passes 202. The charged host filter passes 38 tests. The local Markdown
check passes 79 links/anchors. Production musl metadata passes the direct-KFD
dependency audit with 43 packages and eight permitted build scripts; its SHA-256
is `6c0cac53358224a31c356d91228b602ae8545f7c156a0c695f103533238054ca`.
The expected-negative inventory check passes all 686 files, without running Verus.

## Proof And Hardware

Model, resource-accounting, completion, root manifest and lockfile inputs are
unchanged from signed R73 `fb1e27e66cee27bb11f0b7c08f2d5994c5d168da`.
Existing capacity, control and credit guards are reused. No new Verus theorem,
solver rerun or whole-adapter refinement is claimed. Rechecking the negative
inventory is distinct from executing the authenticated solver.

Native adoption, publication, complete generated result retirement, public launch
API, generated graph/drain, whole-executor refinement and matched HIP/HSA
performance remain open. R80 adds no GPU workload or speed measurement. No SSH,
remote staging or shared-machine process was started; no remote cleanup was needed.

## Retention

The final record retains both full attempts, exact commands and logs, source
snapshots, production metadata and reproduction helpers under `raw/`.
`source-files.sha256` is repository-relative; `retained-files.sha256` is relative
to this directory. Raw logs are unmodified. Initial development compiler/lint
errors were a test `unwrap_err` Debug bound, a shadowed test helper and a
collapsible conditional; those preliminary terminal outputs are not claimed as
retained gates. No manifest or lockfile changed.
