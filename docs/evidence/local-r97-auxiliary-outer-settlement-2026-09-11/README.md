# R97 Auxiliary Outer Settlement: Local Evidence

Implementation parent: signed R96
`369f99835cfb2af5df9fda45cc828d462ef6b156`. Final validation ran above signed
planning checkpoint `82c8cd854bc6cb8b300a4f5a6b9a2467827dcc6f`.

This record accepts **NATIVE-2B.5B-1**, the production-used outer driver and
named CPU/fake-native prefix matrix, under the
[construction contract](../../runtime-auxiliary-queue-construction-custody-v1.md).
It does not close .5B-2/.5B-3, NATIVE-2B, A1/A2, issue #182, formal adapter
refinement, live KFD qualification or matched HIP/HSA performance.

## Accepted Source Scope

- Private `AuxiliaryParentV1` forwards actual currentness, model custody,
  original target ownership and nonallocating terminal-parent transport.
- Production and fixture use one owning scope and shared auxiliary driver.
  Planning stays inside the loan and before the data callback. Retake precedes
  operation-result gating; both precede CREATE/install. Complete parent storage
  precedes cleanup. The existing root unwinder is unchanged.
- The fixture starts after successful original primary construction and uses
  its actual foundation, engine, memory session, accounts, nonempty ledgers and
  platform owners. The live-model custody envelope and loan/reclaim are real
  model operations, including issuer/generation checks and certificate rejection.
- A separate early-prefix oracle preserves R96's strict late paired oracle.
  It joins raw/prepared/authority/completed owners, exact native records,
  account charges and layouts without counting custody markers as owners.
  Actual terminal ledger poison and live/terminal parent slots are observed.
- Native allocation/map/seal/write failures exercise existing fake-native
  memory transitions and exact pending/returned/in-session prefixes. The
  native-control pending-slot scope is append-only, not released-slot reuse.

The public concrete Linux session, existing two-compute-lane profile and
lifetime/auto-trait boundaries are unchanged. There is no new allocator,
public completion API, queue implementation or production fallback.

## Matrix

Fourteen integrated functions, twelve new over R96, are named in
`test-summary.json`. Source-derived loop counts are independently reviewed:

| Family | Shared auxiliary-driver runs |
| --- | --- |
| Existing R96 success / late failures | 1 / 8 |
| Operation crossed with reclaim outcomes | 15 |
| Opening/loan error and panic | 4 |
| Genuine loan exhaustion / reclaim certificate rejection | 2 |
| Preparation-stage error and panic | 62 |
| Cleanup panic before/after disposal | 4 |
| Control/platform boundary error/panic sweep / successful discovery | 72 / 1 |
| Data callback error/panic before allocation | 2 |
| Native preparation allocation failures | 56 |
| Native preparation map/seal/write failures | 50 |
| Control projection failures | 70 |
| Native control failures | 22 |
| Total | 369: 366 failures, 3 successes |

Separately, one primary-only fixture calls the actual borrowed capacity
preflight twice after model-only update history. It does not enter the shared
auxiliary driver or the public Linux entrypoint. The control sweep pins all 36
boundary occurrences, including exact per-name multiplicities. Seal tests assert
actual attempt/return progress, not only the existence of an earlier token.

Callback cases fail before allocating or returning data. They do not qualify
callback-internal unreturned ownership. Platform leaves remain scripted; their
counts are not the local Linux platform composition required by .5B-2. Nonempty
primary ledger snapshots are not an active GPU workload or physical overlap.

## Gates

Authoritative records are `raw/r97-final-source-gate.json`,
`raw/r97-final-complete.json`, `raw/r97-auxiliary-results.json`,
`raw/r97-auxiliary-complete.json`, `raw/r97-frozen.json`, `raw/r97-restored.json`
and the four `r97-mut-*` records. `test-summary.json` contains parsed totals.

| Check | Result |
| --- | --- |
| GNU runtime all-target suites | 2,531 passed, 5 ignored, 48 harnesses |
| musl runtime all-target suites | 2,531 passed, 5 ignored, 48 harnesses |
| KFD / Runtime library harnesses within each full suite | 951 / 709 passed |
| GNU runtime/host documentation | 108 passed |
| musl runtime / host documentation | 91 / 16 passed |
| GNU host / musl host | 258 passed, 4 ignored / 141 passed |
| Generated macro fixtures | 7 passed |
| Frozen / restored construction suite | 53 / 53 passed |
| Focused auxiliary / primary construction | 8 / 39 passed |
| Python runner/checker tests | 151 passed |
| Final source gates / auxiliary checks | 17 / 12 passed |
| Compiled behavioral negative mutations | 4 exact tests rejected after compilation |
| Non-documentation source identities | 5,651 unchanged and exactly restored |

Filtered suites overlap full suites and are not additional unique tests. Gates
include both Clippy profiles with warnings denied, format/whitespace, dependency
policy/tests, CI test-gate checks and standalone lockfiles. Auxiliary checks also
cover ordinary/lower construction, existing Linux helpers, initialization,
preparation, transitions and bind regressions, production metadata and the
pure-Rust closure audit.

`raw/r97-environment.json` records the local WSL2 environment and pinned
nightly-2026-04-03 toolchain, including cargo/rustc binary hashes. Runs use locked,
offline dependencies, four build jobs, disabled incremental compilation and no
`XDG_RUNTIME_DIR`. Wall times are test-campaign metadata, not GPU performance.

The expected-negative proof inventory check is not a Verus solver run.
Runtime-model, resource-accounting, completion and Cargo input path sets/hashes
are unchanged from R96. No new theorem or executable-adapter proof is added.

## Mutations And Earlier Attempts

The final `raw/r97-mutations.json` requires exactly four named mutations and
the pinned nightly, locked/offline KFD test command with `--exact`:

| Mutation | Observed behavioral rejection |
| --- | --- |
| `opening` | Missing opening observation fails the expected currentness-error assertion. |
| `retake` | Ignored reclaim error fails exact reclaim-error precedence. No additional CREATE is observed. |
| `operation-result` | Ignored operation error reaches unexpected success: `operation or reclaim must fail: ()`. |
| `parent-transport` | Missing stored terminal parent fails the exact parent-slot ownership assertion. |

Each changes only `queue_live/construction_auxiliary.rs`, compiles, runs one
exact test and exits 101 with zero passed/one failed. These are dynamic fixture
failures, not source-text guard failures. The parent-transport mutant has an
unused-code warning but compiles. All source is exactly restored before the
restored positive suite and final gates. The retainer reconstructs each mutation
from its manifest and checks its source hash, command and expected log markers.
Production forwarding source guards are retained separately; they do not execute
the concrete Linux adapter or demonstrate an extra CREATE for the retake mutant.

Preliminary runs remain in `raw/`, including failures:

- `r97-shared-driver`: 41 construction tests passed before the new matrix.
- `r97-prefix-first`: two passed/five failed; fixture corrections distinguish
  whole N2 totals from primary-only totals and actual poison from ProbeActive.
- `r97-prefix-second`: seven focused tests passed after those corrections.
- `r97-native-prefix-first`: compilation failed on a raw digest versus the
  typed identity digest; the fixture now uses its typed constructor.
- `r97-native-prefix-second`: thirteen focused tests passed before final
  sweep/seal/callback/production-guard refinements.
- `r97-clippy-preflight`: failed the new helper's type-complexity lint;
  a named `PrefixCaseResult` alias fixes it without suppressing the lint.

None of those preliminary runs substitutes for final-source acceptance.
`r97-clippy-final`, frozen/restored tests and every final gate pass after the
corrections. Runner/retainer scripts preserve commands, complete source maps,
before/after campaign identities and explicit successful completion records.
The retainer pins gate/test key sets, totals, all fourteen integrated names,
toolchain identities and required preliminary files; it rejects incomplete or
wrong-source campaigns. Three read-only workers reviewed source, accounting,
mutation sensitivity, evidence boundaries and the remaining work orders.

## Remaining Acceptance

.5B-2 must compose original primary and auxiliary ownership through actual
local runtime registration/gate/shadow helpers. .5B-3 must complete CREATE
uncertainty/malformed/panic, output/ID recovery, retained auxiliary/SDMA roster
collisions, both late currentness failures and occupied/reused installation.
The [current work orders](../../runtime-a1-a2-swarm-current.md) assign these
separately from the independently ready CO-1 and VER-1A.1 packets.

Replacement/insertion, native generated adoption, callback/installer-internal
unreturned owners, concurrent bootstrap, aggregate bounds, protected compiler
integration, new formal refinement, live KFD and performance remain open.
No SSH sessions, MI300X processes or remote staging were created by this
campaign; no shared-machine cleanup was required.
