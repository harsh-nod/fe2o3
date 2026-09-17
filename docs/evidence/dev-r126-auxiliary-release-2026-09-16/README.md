# R126 Auxiliary Queue Release: Development Receipt

This packet extends `be52bb33a23465a3836325175b0ac178fe225e26`.
R125 remains the accepted Native CPU/test checkpoint. R126, N5 DATA-ADOPT,
A1/A2, issue #182, formal implementation correspondence and HIP/HSA parity
remain incomplete.

`source-files.sha256` identifies all eighteen changed Rust files, with full
copies under `source/`. `source.patch` contains the complete staged source/test
delta, SHA-256
`2ee518283e0be2a629b1fa730e07ecb578cbb481d6bc62b9583e4d086a80c876`.
`runner.sha256` separately identifies the frozen campaign script.

## Implementation

The new auxiliary release driver borrows the original lane slot and installs
retained teardown custody before destructive entry. It retains native destroy
progress, platform owners, queue-resource authority or cleanup receipts,
dispatch/data cleanup, completion signals and the teardown gate. The original
slot becomes vacant only after memory cleanup, temporary model retake, closing
currentness and gate confirmation succeed. The primary and any remaining
neighbors keep their shared foundation; this is not permanent primary release.

Errors and panics after entry preserve the original unfinished suffix or its
completed disposal receipts. They poison the parent/process, reject reentry and
block primary fallback. Dropping an unfinished auxiliary release aborts rather
than running ordinary destruction on partially disposed owners. Preflight
rejections leave the original owners live and do not install release custody.
The new driver forbids unsafe code and reuses existing cleanup primitives.

Runtime shutdown verifies and clears the exact successfully destroyed auxiliary
handle before recording its profiling event. A later primary-custody allocation
rejection can therefore return quiescently and retry without destroying a stale
auxiliary handle twice. Destructive lower errors and panics remain terminal;
panic handling preserves the original payload even if diagnostics also panic.

## CPU Coverage

Seventeen new KFD tests include two root/Drop guards and fifteen tests using
original completed primary/auxiliary constructor owners. The constructed tests
exercise successful cleanup, fresh native generation after actual slot reuse,
primary/stale/busy/poisoned-dependency rejection, destroy outcomes, platform
cleanup, temporary model loan/retake and exact lower cleanup prefixes.

Fault matrices cover selected native and currentness boundaries in queue
resources, dispatch controls, Host/Device data and completion signals, plus
queue-resource model commits and closing currentness. DATA index three tests
failure after an earlier Host refund. Ordinary rows require the exact injected
error and successful model retake; combined-fault rows separately check retained
loans and error/panic precedence. Retry checks compare exact retained owner,
mapping, accounting, model and trace snapshots. Success compares the unaffected
primary's original owners and finally releases that primary normally.

These are selected matrices, not an exhaustive cross-product of every native
operation, failure mode, attachment profile and populated SDMA neighbor. The
empty-root guard fixtures prove rejection and Drop behavior only; they are not
native construction evidence. Existing primary-only ownership checks remain
strict. Active-foundation snapshots are used only where temporary auxiliary
model borrowing requires them.

The new runtime CPU test covers terminal auxiliary errors/panics, blocked
reentry, unchanged destroyed-event observations and original panic ownership.
A test-only one-shot primary-custody allocation rejection also supports the new
ignored native retry probe. Runtime layout remains covered by the existing
64 KiB ceiling test.

## Final CPU Results

| Target | KFD Library | Runtime Library | Failed | Runtime Ignored | KFD / Runtime Libtest Duration |
| --- | ---: | ---: | ---: | ---: | --- |
| GNU | 1,389 passed | 842 passed | 0 | 11 | 955.65 s / 24.88 s |
| musl | 1,389 passed | 842 passed | 0 | 11 | 2,599.31 s / 36.39 s |

All seventeen new KFD tests pass on both targets. KFD has no ignored tests;
the eleven ignored runtime tests require isolated native hardware. Strict
all-feature/all-target KFD/runtime Clippy, workspace formatting and runtime
no-default-feature compilation pass. Unsafe-source policy passes five tests,
with its explicit maintenance test ignored. Before/after source checks and the
campaign exit are zero. `results-audit.log` records the successful separate
post-run artifact, roster, count and command-record audit for all four test
executables and all twenty-one campaign commands. Read-only source and audit
reviews found no remaining blocker within this development scope; they are not
formal proofs or native execution.

The first full GNU attempt passed 1,388 tests and failed one source-shape gate
after 1,177.88 seconds. Its original source snapshots, patch, hashes and raw
records are retained under `preliminary/first-full-run/`. The gate still counted
both dependency-idle guards in `queue_live.rs`; one now resides in the new
auxiliary driver. The corrected gate requires one in each location, verifies the
public teardown delegates to that driver, and checks that auxiliary idle and
owner validation precede retained custody and destructive entry. Production
code did not change in this correction. All seventeen new auxiliary behavioral
tests passed in that failed overall run; those results do not replace a fresh
final campaign. The old binary receipt is historical and need not match a later
rebuild at the same output path.

## Native And Qualification Boundaries

The new ignored two-stream native probe executes the existing real vector-add
workflow, destroys the auxiliary queue, injects a quiescent primary-custody
allocation rejection, and checks exact lower/runtime auxiliary retirement and
profiling before retrying primary shutdown. It also checks a later inert
shutdown. This probe is compiled but unexecuted; compilation does not validate
its native assertions.

Two read-only MI300X availability captures found other jobs on all eight GPUs.
The refreshed check ran from `2026-09-16T21:24:59.652877301Z` to
`2026-09-16T21:25:05.932025053Z`; utilization alone was not used as an isolation
test. The raw captures are under `isolation/`. No GPU workload, upload, reset or
remote test scratch was created for this packet. Prior native results do not
qualify this source. Any separately authorized temporary-file cleanup is not a
native validation run.

Native failure/accounting evidence, broader attachment/profile composition,
full R126 qualification, generated adoption/ISSUE/COMPLETE, Stop/drain/graph
integration, formal correspondence, aggregate-memory bounds and matched HIP/HSA
benchmarks remain open. No new formal proof or performance result is claimed.
Test durations are regression timings, not benchmarks.

## Reproduction

Run `bash verify-auxiliary-release.sh CHECKOUT OUTPUT_DIRECTORY` with locked
offline dependencies, installed GNU/musl Rust targets, Bash, `jq`, GNU `prlimit`,
`sha256sum` and Python 3.11 or newer available. The output directory must already
exist. The runner records commands, timestamps and exit statuses; authenticates
changed source before/after; and selects exact library-test executables from Cargo JSON
before hashing and executing them. Tests use four threads with core dumps
disabled. The target scope is the full all-feature KFD/runtime library suites,
not every workspace harness or a formal qualification campaign.

After the final campaign exits successfully, `bash verify-results.sh` checks
the frozen runner and archived source, recorded exit statuses, all four retained
executable hashes, exact roster/result equality, all seventeen new KFD tests and
runtime ignored rosters. Its Python auditor also matches all twenty-one command
records, the source-check outputs, Cargo artifact identities/features/targets,
executed command paths and exact summary counts. This post-run binary audit
requires the original local executables still to exist; binaries are not
included in the archive. A fresh reproduction obtains new binary hashes rather
than promising identical hashes across different build environments.
`SHA256SUMS` covers every other file in this archive, including the failed
preliminary cohort. Raw command/log whitespace and patch context are preserved;
source/documentation whitespace checks exclude those unmodified evidence files.
