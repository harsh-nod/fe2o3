# Ordinary Host Cache

R81 implements MEM-2B-HOST against signed R80
`a41719eb45540ac3f631e6989366b2dd3924a4c3` on
`codex/r65-runtime-drain-versions`. The
[contract](../../runtime-host-cache-limits-v1.md) defines optional ordinary
coherent cached-free limits, not aggregate residency or native generated execution.

## Scope And Review

The existing SDMA pool gains immutable Host byte/record ceilings, separate from
Device limits and native backing budgets. It scans all Host entries using exact
native records and padded CPU spans. Cache/checkout/recycle keeps the original
N1 debit; pressure takes the existing explicit disposal path. Both runtime
startup paths forward configuration before SDMA activity. Defaults are unchanged.

Three read-only agents reviewed native records, lifecycle/configuration and
accounting. Primary implemented and tested all changes. Review found that an
empty-roster domain check initially omitted account session identity; it now
uses the existing exact session/device/VM predicate, with a foreign-session
negative test. Review also requested a valid native-token candidate alias under
full-cache pressure, distinct from stale-token rejection. That assertion is part
of the final acceptance source.

Nineteen new CPU test functions cover policy, native record projection and
configuration. Existing foundation-loan and failed-retake tests now exercise
the Host policy; all fourteen failed SDMA ingress tests also reject late Host
configuration. Fake-backend tests cover actual N1 records and charges, padding,
all four-setting configuration permutations, aliases, malformed candidates,
cached/checked-out occupancy, partial trim, native/currentness failures and
panics. Linux facade/configuration forwarding includes source-only checks.

## Acceptance

The first frozen-source attempt, `r81-final`, passed all seventeen gates with
5,604 source identities unchanged. Review then added the valid-token candidate
alias assertion above. `r81-final2` also passes all seventeen gates against
5,604 unchanged source identities and is the publication acceptance attempt.
Both attempts are retained. Production code is identical between them.

Exact gate commands, logs, source identities and counts are retained in
`raw/` and `test-summary.json`. GNU/musl runtime, host, doctest, fixture, lint,
format, dependency, lockfile and runner gates pass against the final frozen
non-documentation snapshot. Focused Host-cache and Host-backing checks are
retained separately and are included in the wider suite totals.

| Gate | Result |
| --- | --- |
| Five runtime crates, GNU all features/targets | 2,329 passed, five existing ignores, 48 reported harnesses |
| Same runtime scope, musl | 2,329 passed, five existing ignores, 48 reported harnesses |
| GNU runtime plus host doctests | 101 passed |
| musl runtime / default host doctests | 84 / 16 passed |
| GNU all-feature host / musl default host | 246 passed, four existing ignores / 129 passed |
| Generated macro fixture harness | Seven passed |
| All-feature/all-target and production library Clippy | Both pass with warnings denied |
| Runner/checker Python suite | 151 passed |
| Formatting, whitespace, dependency policy/tests and local CI gate | Pass |
| Standalone lockfiles | All 32 pass |
| Focused Host pool / Host backing | 19 / 36 passed |

The unchanged negative-proof inventory passes 686 files without running Verus.
The production musl dependency audit passes 43 packages and eight permitted
build scripts; metadata SHA-256 is
`6c0cac53358224a31c356d91228b602ae8545f7c156a0c695f103533238054ca`.

Cargo uses `nightly-2026-04-03`, locked/offline resolution, four build jobs and
disabled incremental compilation, with `XDG_RUNTIME_DIR` removed. Model,
resource-accounting, completion and root manifest/lockfile inputs are unchanged
from signed R73 `fb1e27e66cee27bb11f0b7c08f2d5994c5d168da`.

## Open Boundaries

The existing R72 cost projection and native charge/disposal path are reused.
No new theorem, Verus solver rerun, Host-policy proof, native cache-adapter
refinement or aggregate-accounting closure is claimed. Negative-proof inventory
checking is not execution of the solver. Fake-native-record tests are not Linux
admission-pressure or GPU acceptance.

MEM-QUAL-HARNESS and both-startup Linux cache/reuse/disposal campaigns remain
open. Native generated adoption still needs its pristine never-published abort
transition and integrated active lane/Context cleanup ownership. HIP/HSA parity
and performance acceptance remain open. No SSH, remote staging or GPU process
was started for R81, so no shared-machine cleanup was needed.

## Retention

`source-files.sha256` is repository-relative; `retained-files.sha256` is relative
to this directory. Raw gate output is unmodified. Source/evidence hashes are
checked before signed dual publication. Preliminary focused command output is
not represented as retained final-gate evidence.
