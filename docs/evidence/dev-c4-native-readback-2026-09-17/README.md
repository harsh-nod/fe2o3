# C4 Native Initialized Readback Prerequisite

Development above `13c5e5b8126dd461c5d7eb8972613d3ff6f4019b`.
R125 Native CPU/test, R118B C1/C2/C3 and R116/V3 remain the accepted checkpoints.
This packet does not complete C4, A1/A2, #182, protected Worker integration or
HIP/HSA parity. No performance comparison or formal correspondence is claimed.

## Implementation

A distinct lower read-into operation observes the original retained coherent
allocation after exact recycled generation and all epoch slots becoming vacant.
It requires sealed full initialization, checked nonempty bounds and exact
destination length. Read-only, unused and partially written initialized bytes
are observable; this is not inferred inspected write coverage. Existing writable
and declared-snapshot APIs keep their prior restrictions. Persistent attachments
and terminal queue state reject. The original mapped-copy implementation retains
its pre/post currentness checks and native owner; no borrowed native address or
new readback allocation is exposed.

Runtime joins the original saved source roster to plan member counts/extents,
the exact indexed submission, native recycled DATA cardinality and original lane.
Complete destination validation precedes copying. Destination buffers and their
capacities do not change. A failed copy preserves its partially copied prefix
and untouched suffix; the existing native-call envelope terminalizes failures
after entry, including closing-currentness rejection.

All N5 construction routes already initialize complete DATA from original host
buffers, so full-roster observation includes genuine native RO bytes and an
initialized output tail without fabricating a prefix or suffix from source.

## Validation

The corrected cohort below is qualified at its CPU and lower-native-fixture
boundaries. The initial full GNU and musl KFD regressions each
finished with 1,401 passes and two failures in the frozen dispatch-binding and
session manifest digests (`raw/gnu.exit` and `raw/musl.exit` record 101). The
policy text changed without refreshing its pinned hashes. The dependent
production manifests have since been corrected in the separate cohort below;
these are not merely test-local expected values. Both failed runs and their
original source identities are retained. Neither initial full run qualifies
this packet.
The final GNU runtime harness passes 896 tests with seventeen opt-in native tests
ignored. Strict all-features/all-targets Clippy for both crates, workspace
formatting, the no-default runtime check, 27 KFD and 33 runtime doctests, and the
unsafe-source policy check (five passes, one maintenance ignore) pass.

The initial ten Rust source files are frozen against the base above. The complete crates
patch has SHA-256
`24eaf1c649fd78a08633517455632449224d1167c0091f18b986d4feffc7ff92`;
the source manifest has SHA-256
`3fe9ea3b2c02afd8d341bdca21cc5330a466730125d342447163f0b803ed7a02`.
The first GNU combined run began before the runtime fixture was corrected to
explicitly exercise mixed RO/WO/RW access. Its unchanged KFD source remains
applicable; its runtime portion is not used for final-source qualification.
`gnu-final-runtime` supplies the corrected runtime result. Earlier exploratory
helper results are not substituted for final-source regression.

### Corrected Source Cohort

The dispatch-binding digest is now refreshed before the session digest, followed
by the native queue foundation, semantic observation and service-host ownership
digests. These are changed policy/evidence identities, not Rust API changes.
`manifest-digests.rs` derives string constants with the Rust syntax parser; the
existing unit tests independently check both manifest bytes and nested pins.
The corrected GNU focused checks pass 24 KFD and two service-host manifest tests.

`corrected-source-*` freezes thirteen Rust files against the same base. Its patch
SHA-256 is `fd3f82ee6330210ec91157acfdbd9815ef913e1e0378fcbef7337a850be14c3b`;
its source manifest SHA-256 is
`c774232047cdc11634474d871f030afb031f7fef89ea5ca30eb8be73de9cbd21`.
All initial artifacts remain unchanged. Their successful runtime and native
observations do not qualify these corrected identities.

The corrected GNU and musl regressions each pass all 1,403 KFD tests, 896 runtime tests
with seventeen ignored, and all forty-five service-host tests. Exact rosters,
binary hashes and before/after source checks agree. Corrected strict Clippy,
formatting, runtime no-default, unsafe policy, parser fixtures and doctests
(27 KFD, 33 runtime and 22 service-host) pass. Complete GNU and musl rosters
match exactly. Initial native passes do not qualify this corrected source.

The first corrected native batch passed all four functional tests and removed
its owned scratch, but the cold I2 probe failed the accounting criterion: GPU 1
VRAM rose from 298,647,552 to 918,499,328 bytes at its postflight observation.
Other GPUs also changed, so the cause is unestablished. These `corrected-native-*`
records remain historical and do not qualify equal pre/post accounting. A
separately named `corrected-rerun-*` batch passes all four tests using the same
corrected binary and guard under fresh preflight checks. GPU 1 VRAM is exactly
298,647,552 bytes before and after every rerun, with matching UID/BDF. The exact
equality requirement is unchanged. These observations are not an exclusive
device reservation or a native fault campaign. Uploaded executable/guard hashes,
source endpoints and removal/absence of both owned scratch directories agree
with the recorded commands.
KFD qualification runs all 1,403 tests in disjoint partitions: 1,383 tests with
four test threads, and all twenty queue-Linux tests, including the self-spawning
cases, serially.
The audit requires the partition roster union to equal the complete executable
roster exactly. Runtime and service-host run complete serial harnesses.
Only the parallel partition permits exact libtest 60-second progress notices;
each must have a subsequent unique outcome. Fourteen negative parser fixtures
reject malformed, incomplete, incorrectly filtered or unmatched transcripts.

Use `audit-corrected.sh` for corrected qualification. It deliberately requires
fresh corrected commands, source/binary identities, complete rosters, gates and
native cleanup, with no fallback to initial successful records. Run it outside
`record.sh` so its closed-record requirement does not observe its own in-flight
receipt. The final successful audit is recorded in top-level
`qualification-audit.*`; `SHA256SUMS` seals the archive. Independent read-only
review also passes the corrected source, CPU/native receipts and cleanup.
The original `audit.sh` is retained as the initial cohort's unfulfilled
all-success audit, not a corrected qualification route.

New CPU tests cover initialized observation across generation/epoch/poison,
effect/kind/initialization and range/destination matrices; mixed RO/WO/RW full
roster copying; malformed final destinations before any write; exact pointer and
prefix/suffix preservation on injected copy errors/panics; and plan extents.
The copy failure fixtures exercise the shared production roster driver, not
injected failures through the full native backend/currentness path.

The new opt-in native cases exercise cold and SDMA-bootstrap primary/AUX/rebound
routes. They compare all four buffers byte-for-byte: two original RO inputs,
complete output plus a sixteen-byte initialized but unwritten tail, and eighty
bytes of unused RO storage. They check unchanged destination pointers/capacities,
pre-completion and foreign-source rejection, twelve DATA disposals and shutdown.
These probes bypass the generated carrier/Context registration and do not invoke
the protected Worker, host decoder, result gate or completion reply.

On the initial source, both new native cases and both prior I2 regression cases
pass on physical MI300X
GPU 1, UID `0xab83d2ffef0d3cdf`, BDF `0000:26:00.0`. All four invocations use the
same frozen musl executable, SHA-256
`b8ebfd9bd6c69622372c92a6c477b37d6141d4f01f0012da5496d754b42f3b3f`.
Each reports one pass and 912 filtered tests. Recorded device VRAM is
298,647,552 bytes before and after every invocation. These are debug-build
behavior checks, not timing comparisons or a native injected-failure matrix.

Only the owned binary and guard script were removed from
`/tmp/fe2o3-c4-harsh-20260917.myDEsE`; the empty directory was then removed and
its absence verified. No foreign workload or file was changed. The initial
inventory lost newline quoting across SSH and is preserved as imperfect history;
`remote-inventory-final` separately records the actual directory contents.

The result parser recognizes only source-checked parent tests that launch
aborting child harnesses. It requires their exact child-banner counts, all named
parent outcomes and the matching in-harness summary. Earlier parser run records
are retained; `parser-closed` checks the final parser against positive runtime
and KFD child fixtures, ten malformed transcripts and the real final GNU runtime
roster. Parser fixture checks do not substitute for unfinished KFD regressions.

## Remaining Join

Lend the original charged host destinations through a non-extracting view,
revalidate source/currentness, complete native/submission/shell/credit/hold
settlement, then consume the original R85 decoder and resolve the original
completion cell. No second decoder, result owner, readback reservation or reply
is needed. C5 public typed API, C6 graph/drain, production version journal/reuse,
native injected-failure qualification, aggregate memory, formal correspondence
and matched performance remain open.
