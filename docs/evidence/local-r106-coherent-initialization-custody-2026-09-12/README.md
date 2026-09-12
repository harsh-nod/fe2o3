# R106 Coherent Initialization Custody: Local Evidence

Locally accepted CPU/shared-sequence checkpoint above signed R105
`3ae84f8542ef82d4615f5e5ff377e2a2b076e29a`, with documentation-only publication
parent `42d614d352f39fb13b4026b6d2c2b9af94a41200`. All frozen gates and mutations
ran above that parent. This is N3-C acceptance, not completion of N3, A1/A2,
issue #182 or HIP/HSA parity.

See the [summary](test-summary.json), [source gates](raw/r106-final-source-gate.json),
[auxiliary checks](raw/r106-auxiliary-results.json),
[environment](raw/r106-environment.json), [mutation recipes](raw/r106-mutations.json),
[collector](raw/r106-retain-local.js) and
[production contract](../../runtime-borrowed-coherent-initialization-v1.md#owning-copy-extension).

## Production Change

The private coherent initializer now consumes/returns its CPU token through
the existing transition owner during copying. Exact borrowed preflight occurs
before backend currentness. The original allocation remains rooted across copy
errors and panics; successful copy returns it before the separate map transition
begins. Successful native mapping promotes the token before projection, so late
projection/commit failure retains the actual mapped successor.

The private owning-access policy preserves a backend/callback panic without
running another currentness check that could replace it. This applies with and
without configured backing accounting. Public borrowed byte-access behavior,
allocation preflight/pending ownership, public signatures, accounting and the
fixed complete copy are unchanged. No extra source buffer, native copy, loan
engine or public initialization assertion is added.

## CPU Matrix

Ten new functions exercise 135 cases plus one successful pre-effect retry
control. Setup anchor initializations are not counted. The 130 initializer
outcomes are eight successes, 60 returned errors and 62 caught panics. Five
additional direct private-copy calls reject invalid input.

| Matrix | Cases |
| --- | ---: |
| Complete source/charge success at lengths 1, 4096, 4097 | 6 |
| Empty, capacity and revision preflight | 6 |
| Copy opening/closing currentness error or panic | 8 |
| Access/partial-write panic with closing fault combinations | 24 |
| Allocation/map projection and commit faults | 32 |
| Native allocation-prefix error or panic | 16 |
| Native map prefix, errno and currentness outcomes | 22 |
| Exact revision headroom and map exhaustion | 4 |
| Allocation currentness error or panic | 12 |
| Invalid private-copy input coordinates | 5 |

The same fixture keeps the original foundation, VM/device, allocation records,
source identity, unrelated mapped anchor and N1/N2 account observations. It
calls the real allocation/copy/map adapters over scripted native leaves. Exact
success equals the expected `project_map` result, not merely a changed model.
Pending-prefix checks compare actual reservation/allocation outputs, layout,
address, mapping bytes and charges; they do not exhaustively assert the pending
diagnostic stage enum. Failures grant no speculative cleanup or duplicate debit.

Partial-copy panics use an internal test callback over the same owning access
helper. The public initializer has no such callback and performs a full copy.
The five invalid private inputs are rejection-before-currentness tests, not
claims of arbitrary foreign-input custody. The unconfigured first allocation
currentness panic before any native attempt is followed by an actual successful
retry after clearing the fault; it is distinct from post-allocation failure.

## Gates

| Check | Result |
| --- | --- |
| Source gates / auxiliary checks | 17 / 10 pass |
| GNU / musl all-target runtime suites | Each 2,583 pass, five ignored, 48 harnesses |
| Frozen / restored coherent module | 10 / 10 pass |
| Frozen / restored borrowed regressions | 8 / 8 pass |
| Frozen / restored transition module | 30 / 30 pass |
| Compiled behavioral negatives | Seven intended rejections |
| Exactly restored source identities | 5,665 after every mutation |

The collector checks exact GNU/musl passing-name multisets against R105 plus
the ten new names. Auxiliary transitions contain the old 20 plus those ten;
other auxiliary rosters are unchanged. Focused suites overlap full-suite tests
and are not additional unique tests. GNU/musl each include 997 KFD tests.

The frozen toolchain is `nightly-2026-04-03`, with locked/offline resolution,
four Cargo jobs, four test-harness threads, incremental compilation disabled
and `XDG_RUNTIME_DIR` unset. Version outputs and Rust binary identities match
the R105 capture. Metadata stdout/stderr and every declared gate log are
required before archive creation. Deadlines are unchanged. The proof-inventory
check does not run a solver. R103's earlier default-concurrency musl watchdog
failures remain unresolved; this four-thread pass does not diagnose them.

## Compiled Negatives

All seven change only `shared_memory/transitions.rs`, compile, and fail exactly
one selected behavioral test. Each run is followed by exact restoration of all
source identities before the next mutation; restored focused suites follow the
last restoration. The collector retains strict observed timestamp ordering,
without importing R105's historical chronology exception. Per-command duration
uses a monotonic clock; cross-process timestamps are not authenticated time.

| Mutation | Observed Oracle |
| --- | --- |
| Drop Copy input | Missing original CPU token in terminal custody |
| Skip copy inside the normal access callback | Complete-byte mismatch at length 4096 |
| Disable owning panic policy | Unexpected closing-currentness count after access panic |
| Skip borrowed Copy preflight | Invalid input reaches backend currentness before rejection |
| Skip model map commit | Exact foundation differs from expected mapping success |
| Drop mapped successor at projection/commit | Missing mapped output after native success |
| Omit Copy quarantine | Existing borrowed regression sees Active before retry |

The panic-policy negative stops at the extra closing check; it does not observe
the later payload-replacement cell. Positive matrix tests cover original payload
preservation. The immediate-quarantine negative deliberately uses the existing
borrowed regression: the new initializer retry helper can let a later terminal
guard quarantine the session and mask an omitted initial phase assignment.

## Retained Attempts

The first [collector rejection](raw/r106-collector-rejection.json) happened
before archive creation. Its expected borrowed roster incorrectly included two
runtime materializer tests from the workspace log alongside the eight KFD
tests. The [rejected collector](raw/r106-retain-local-before-roster-fix.js) is
retained byte-for-byte; the corrected collector restricts that focused roster
to KFD namespaces. No gate, mutation, restoration record or source changed.
The original collector stderr has no separate retained file; the rejection
record identifies the command, assertion, counts and disposition.

Preliminary focused-01 used a nine-test filter before strengthened oracles.
The recorded formatter changed source as expected; focused-02, borrowed-01
and clippy-01 passed before freezing. They remain historical, not the accepted
campaign. The first formatter's empty output was not separately archived.
Raw logs are not reformatted to remove trailing blank lines.

## Remaining Boundary

This fixture does not establish original Linux-engine composition, live KFD
execution, authenticated executable refinement, protected Worker V3 authority,
or matched HIP/HSA performance. No SSH, GPU, solver or benchmark was run for
this checkpoint. Device initialization N3-D, live insertion/replacement N3-L,
cleanup N4, adoption/issue/completion, versioning and aggregate-memory closure
remain on the [current board](../../runtime-a1-a2-swarm-current.md).
