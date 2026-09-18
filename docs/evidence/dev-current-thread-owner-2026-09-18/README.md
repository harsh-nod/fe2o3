# Caller-Driven Owned Progress Development

CPU qualification above source base `992dc9d4cafda47b93d8a698b42ce84e86cb8fb0`,
including the exact uncommitted sources in `raw/source-before.log`.
This packet covers CPU runtime mechanics. It is not native execution, protected
application integration, formal Rust/native refinement, performance acceptance
or HIP/HSA parity. Accepted R125 Native CPU/test, R118B C1/C2/C3 and R116/V3
checkpoints are unchanged; A1/A2 and #182 remain incomplete.

## Change

`RuntimeAsyncCurrentThreadOwnedEngineV1` constructs, drives and shuts down its
Context on the caller thread. Its factory can borrow local non-Send values; the
engine is neither Send nor Sync. `tick` executes one configured work budget and
does not wait for a command. `drive_until_ready` polls a borrowed pinned future
between ticks, preserving that exact future on deadline expiry. Deadlines are
cooperative: backend/user callbacks must return. Owner-side scheduling and
observation require caller pumping; already-issued native work may continue.
A ready result wins over an expired deadline.

Background and caller-driven engines share one persistent scheduler, including
drain/graph state, fairness cursors, operations, waiters and stream progress.
Nonblocking commands are admitted between caller ticks; callback reentrancy is
rejected. Blocking owner-thread waits remain rejected. Synchronous event/progress
observer registration has no nonblocking counterpart and is not available on the
caller-owner. Generated/tracked operations maintain their own progress.

Shutdown is Stop, not drain: it discards queued commands without sending into
its own potentially full channel. Drive `begin_drain` first to finish an accepted
prefix. Shared cleanup and the unchanged four-fact R61 release predicate preserve
driver-before-backend disposal and process-lifetime retention on uncertain
custody. The active guard covers reply wakes, panic recovery and shutdown.
Scheduler finish becomes idempotent only after the cleanup suffix completes,
allowing retry after a graph destructor panic.

Twenty-one new tests cover ownership, command/poll budgets, two streams, deadline
identity/credits before and after submission, callback/waker reentrancy, foreign
thread admission, full-queue Stop/Drop, cleanup/finalizer failures and panics,
ordinary graphs, quiescent/exhausted drain, initialization and panic recovery.
The generated fixture follows actual prepare/reserve/activation and the original
reply through settlement, decode and receipt delivery. It proves original-cell
identity across rejected owner join and deadline, without a receipt constructor.
The backend and completion hooks are CPU fixtures, not protected/native authority.
The graph-destructor regression checks the unfinished suffix before scheduler
Drop could mask the omission. Public doctests cover caller-driven type linkage
and Send rejection; compile-fail execution is not an independent diagnostic-code
oracle.

The production protected verifier/refinement provider and corresponding proof
artifacts remain missing. This runtime API does not alter seccomp or establish
protected typed bundle execution. Formal scheduler refinement, synchronous
observer-registration ergonomics and matched HIP/HSA results remain separate.

## Qualification

`qualify.sh` records exact commands, UTC start/end timestamps, exit statuses and
complete stdout/stderr. Full all-feature library harnesses run serially:

| Harness | GNU | musl |
| --- | --- | --- |
| `fe2o3-host` | 282 passed, 4 ignored | 282 passed, 4 ignored |
| `fe2o3-runtime` | 1,088 passed, 17 ignored | 1,088 passed, 17 ignored |

Ignored tests are not passes. Musl sets `FE2O3_HIP_SYS_DISABLE=1`, excluding
optional legacy HIP linkage, not direct KFD. GNU doctests pass 69 cases: host
4 compile-only and 21 compile-fail; runtime 2 compile-only and 42 compile-fail.
No doctest executes a native kernel. Both crates pass no-default-feature checks
and strict all-feature/all-target Clippy. Workspace formatting and the unchanged
unsafe-source policy pass; the latter has five passes and one maintenance ignore.

`source.py` covers 5,538 Cargo source files, including new untracked modules.
Before/after source maps and four actual test-executable hashes are identical;
`verify.py` also checks current source and binary contents and exact build/run
paths. Hash-pinned C5 baseline transcripts preserve all 286 host and 1,084 prior
runtime outcomes, with exactly 21 passing runtime additions on both targets.
The verifier imports only the hash-pinned whole-harness parser from the earlier
native-wait CPU archive, including its known abort-child transcript handling.
It validates complete main qualification commands and closed receipts, including
recorded development failures. Source identities exclude documentation/evidence
scripts, which are finalized separately. Current-source/binary verification
requires this qualified workspace; manifest verification works in other checkouts.

Read-only production/API and evidence reviews found no remaining blocker in
their stated CPU scope. These reviews do not establish protected application,
hardware, formal refinement or performance acceptance.

## Development History

Recorded exploratory failures are retained: two test compilations used incorrect enum or
method names; Clippy identified a complex shared cleanup signature and test-module
placement. Both Clippy findings were corrected without suppressions. The focused
expanded retry passes 22 tests, including one pre-existing Tokio test and all 21
additions. It is not represented as a full crate run. Earlier scheduler extraction
syntax corrections and compile/format checks occurred before this archive was
created; no receipts for those preliminary commands are claimed here. Additional
unrecorded formatting checks identified the cleanup-alias layout, corrected
before the qualification source snapshot.

No MI300X job or remote scratch directory was created for this CPU-only packet.
