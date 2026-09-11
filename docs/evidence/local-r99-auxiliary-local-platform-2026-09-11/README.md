# R99 Auxiliary Local Platform: Local Evidence

Source parent: signed R98 `7506596f805af49e432aaa4ef66ec9a586ca4734`.

This record accepts **NATIVE-2B.5B-2**, the named CPU/local Linux helper
composition. It does not close .5B-3 CREATE/install coverage, NATIVE-2B, A1/A2,
issue #182, formal adapter correspondence, live KFD qualification or HIP/HSA
performance parity.

## Source Scope

Seven test-fixture files change, including two new files and three new test
functions. Production mechanisms, runtime/model/accounting/completion crates and
Cargo inputs are unchanged. The existing shared auxiliary driver runs after
successful primary construction with the original engine, foundation, memory,
accounts, data, dispatch/completion/dependency ledgers and platform owners.

One fixture-local registration retains its exact gate, opener PID and runtime
phase. Admission uses existing `admit_runtime`/`commit_first_enabled`, rejects
before minting a fixture owner, and joins the actual local lease count. The
phase consumes `admit_runtime_transition` exactly once. Drop poisons only the
local gate; it does not simulate confirmed native runtime disable or touch the
process-global gate. Binding remains inspectable after poison, without inventing
a lease count for poisoned state.

The matrix runs seventeen scenarios through both original-runtime routes:
34 auxiliary constructions, two successes and 32 failures. It covers admission,
arm, event and shadow-install rejection; initialization/restore error and panic;
cleanup panic before/after disposal; doorbell/finalization error and panic;
actual local gate-finalization rejection; and cross-event substitution. The
cross-event case uses distinct retained file bindings despite equal numeric
event IDs. Two separate registration tests cover foreign binding, arm exclusion,
lease joining, duplicate phase rejection and retained inspection after poison.

Before deliberate fixture disposal, assertions check original-primary retention,
all minted platform owners, exact local mappings/files, auxiliary-only unpublished
cleanup, published-payload retention and terminal-parent-before-cleanup ordering.
The first panic survives cleanup panic. After fixture Drop, only fixture-owned
maps/files have zero live counts. This is local cleanup, not native teardown.

CREATE remains scripted successful. Synthetic events and independent VM
reservations are not KFD BO aliasing, real doorbells, runtime-enable ioctls or
native device execution. Step failures before helper invocation do not qualify
callback-internal unreturned-owner custody. Concurrent bootstrap, replacement,
generated adoption and new formal refinement remain open.

## Gates

Authoritative records are `raw/r99-final-source-gate.json`,
`raw/r99-final-complete.json`, `raw/r99-auxiliary-results.json`,
`raw/r99-auxiliary-complete.json`, the frozen/restored records and five
`r99-mut-*` records. [test-summary.json](test-summary.json) contains parsed totals.

| Check | Result |
| --- | --- |
| GNU / musl runtime all-target suites | Each 2,540 passed, 5 ignored, 48 harnesses |
| GNU runtime/host documentation | 108 passed |
| musl runtime / host documentation | 91 / 16 passed |
| GNU host / musl host | 258 passed, 4 ignored / 141 passed |
| Generated macro fixtures | 7 passed |
| Frozen integration / broader restored construction | 32 / 54 passed |
| New local platform matrix / registration tests | 1 function with 34 cases / 2 passed |
| Ordinary / Linux helper / initialization regressions | 1 / 20 / 4 passed |
| Transition / preparation / bind regressions | 20 / 23 / 4 passed |
| Final source gates / auxiliary checks | 17 / 10 passed |
| Compiled behavioral mutations | All 5 fail their intended dynamic assertion |
| Non-documentation source identities | 5,655 unchanged and exactly restored |

Filtered suites overlap the full suites; their counts are not additional unique
tests. The 32-test frozen filter and 54-test restored filter have different
rosters. The collector requires each exact roster and checks that every existing
R97 construction test and all three new tests pass in both full targets.

Gates include both Clippy profiles with warnings denied, Python harness tests,
format/whitespace, dependency policy/tests, CI test-gate and standalone lockfiles.
Auxiliary audits include expected-negative proof inventory, production metadata
and pure-Rust dependency closure. Inventory checking is not a solver run.

`raw/r99-environment.json` records local WSL2 and pinned nightly-2026-04-03,
including cargo/rustc binary hashes. Runs use locked/offline dependencies, four
build jobs, disabled incremental compilation and no `XDG_RUNTIME_DIR`. Test wall
times are campaign metadata, not runtime or GPU performance measurements.

## Mutations And Preflight

| Mutation | Behavioral failure |
| --- | --- |
| `lease-count` | Joining a second registration leaves one lease instead of two. |
| `queue-phase` | Shared transition fails the primary setup's actual queue-live assertion. |
| `payload-cleanup` | Unpublished payload remains mapped instead of disposed with retained metadata. |
| `event-binding` | Cross-event substitution unexpectedly succeeds. |
| `auxiliary-phase` | Omitting the actual auxiliary driver's phase handoff leaves its registration not queue-live. |

The shared phase mutant is not an independently tested auxiliary-call omission;
the fifth mutation supplies that separate check while leaving primary transition
intact. Every mutant compiles, runs its exact named test and exits 101 with zero
passed/one failed. These are behavioral failures, not source-text guards. The
collector reconstructs tested bytes, checks the sole changed path, exact command
and dynamic log markers, and requires all original source hashes restored.

The first attempted frozen runner name collided with the already-created frozen
source inventory. Exclusive file creation rejected before starting any test or
build. `raw/r99-preflight-note.json` records the correction to the distinct
`r99-frozen-construction` name; the baseline was preserved. The first platform,
registration and Clippy runs also passed with unchanged frozen source.

The collector checks exact source gates against R98's committed commands,
auxiliary commands/totals against R97 plus the two new registration tests,
complete source path sets/hashes, campaign completion markers, toolchain hashes
and the seven-file test-only source delta. Three read-only workers independently
reviewed source, evidence boundaries and the next work packets. Shared builds
and mutations were serialized.

## Next Swarm Wave

The [current board](../../runtime-a1-a2-swarm-current.md) assigns Native
.5B-3A/B/C CREATE/recovery/roster-slot coverage, Admission CO-2A identity and
Resources VER-1A.2 model/proofs. It records their dependency-ordered follow-ons,
ready independent work and cross-team Worker/device-language/release handoffs.
Primary owns edits, integration, proof pins, builds, hardware and signed pushes;
queued assignments are not unattended background implementation jobs.

No SSH sessions, MI300X processes or remote staging were created; no shared-GPU
cleanup was required. No new solver, live KFD or performance result is claimed.
