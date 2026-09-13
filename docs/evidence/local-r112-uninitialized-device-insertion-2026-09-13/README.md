# R112 Uninitialized Device Insertion Evidence

## Scope

Local source/test acceptance for N3-L3-D uninitialized device allocation and
live insertion. The sole existing direct-session API reuses shared settlement
and the DEVICE_LOCAL lower allocator. Actual None/Unmapped/Mapped custody stays
outside the model loan through retake and ledger commit. Failed prefixes retain
their original owners, charges and mapping progress without granting initialized
content, retry or disposal authority.

The [contract](../../runtime-uninitialized-device-insertion-custody-v1.md) defines
the boundary. Exactly thirteen non-documentation paths differ from accepted
R111 `29505205cc54bab1885a67845aaceda8b22b3a9c`, also the publication parent.
Issue #182 and A1/A2 remain open. No SSH, GPU or solver job was run. These results
do not establish native execution, authenticated formal correspondence,
aggregate-memory closure or HIP/HSA performance parity.

## Qualification Results

| Check | Result |
| --- | --- |
| Source campaign | 17/17 gates |
| Auxiliary campaign | 10/10 gates; exact R111 rosters retained |
| GNU tests | 2,683 passed, 5 ignored, 0 failed; 48 harnesses |
| Musl tests | 2,683 passed, 5 ignored, 0 failed; 48 harnesses |
| Frozen/restored device allocation insertion | 14/14 each |
| Frozen/restored device allocator | 12/12 each |
| Frozen/restored device initializer | 11/11 each |
| Frozen/restored uninitialized coherent insertion | 19/19 each |
| Frozen/restored initialized coherent insertion | 19/19 each |
| Frozen/restored initialized device insertion | 14/14 each |
| Frozen/restored coherent initializer | 10/10 each |
| Frozen/restored borrowed initialization | 8/8 each, KFD only |
| Frozen/restored model loan | 4/4 each |
| Compiled behavioral negatives | 16/16 fail exact planned assertions |
| Original evidence-clock contract | 16/16 tests; eight immutable helper pins |
| Continuation evidence-clock contract | 16/16 tests; five separate helper pins |
| Source restoration | All 5,679 non-doc identities match |
| Collector | Pass; closed transcript binds exact summary |
| Independent archive audit | Pass; all raw hashes, rosters, chains and source identities match |

GNU/musl passing-name multisets equal R111 plus twenty-six new functions, with
no removed or newly ignored tests. Eight doctest names relocate; each exact
compile-fail fence matches its parent bytes. Host/macros, strict Clippy, Python,
format/whitespace, dependency, CI, lockfile and production closure checks pass.
Proof-inventory checking is not solver execution.

The new roster contains eleven low-level allocation tests, one generalized
ownership-auditor regression, ten constructed insertion tests, three direct-API
tests and one source guard. Checks distinguish actual lease stage from native
progress, per-call admission from historical activity, and complete custody
from permission to extract output. Both terminal-root kinds block each other
without overwriting their preallocated slot. Constructed tests cover genuine
released holes, append, capacity, opening/native/currentness/commit failures,
and preservation of the original panic and retake error.

Constructed native leaves are scripted and composition uses configured accounts;
direct-root controls cover both accounting modes. Primary detached metadata is
fixture-local; later ordinals relocate the same actual auxiliary behind a
vacancy. Direct-API coverage exercises preflight/missing-engine failure, not
native success or new selected-lane facade methods. Dynamic commit-entry custody
and source-guarded full commit ordering are distinct evidence.

All sixteen negatives compile and run exactly one named test, failing at the
planned behavioral assertion rather than a compiler error or source guard.
Two exercise reused model-loan substrate. Every mutation is followed by exact
source restoration; all nine positive suites subsequently pass on that source.

## Preserved Attempts

All nineteen preliminary attempts and their 57 log/record/source artifacts are
preserved. The first integrated test build stopped before tests with seven
namespace/type errors. The first strict-Clippy attempt rejected a dead insertion
helper, non-Drop forget and the large terminal enum. Those failures are not
passing qualification. The helper was removed; intentional preallocated enum
storage has a narrow documented lint expectation rather than failure-time boxing.

The original restored allocator child passed twelve tests with exit zero, but
its evidence wrapper returned 125. Raw child-close UTC regressed by 1.175 seconds
while monotonic time advanced by 0.416783867 seconds. Its original JSON, log and
source map remain unchanged and explicitly excluded from positive acceptance.
The successful original restored insertion remains accepted. No original
mutation/restoration observation was replaced or rerun to hide the exclusion.

## Clock And Provenance

[`test-summary.json`](test-summary.json) indexes 263 exact raw artifacts. The
original manifest pins 137 historical artifacts and eight helpers and describes
42 planned causal events. Its sixteen-negative/restoration prefix and first
restored suite retain their original UTC contract. The original allocator was
rejected; its seven following suites and collector were never executed under
that original plan. The unfinished tail is not reported as completed.

The supplemental manifest pins all 218 original artifacts, five separate helpers
and three contract-test artifacts. Its nine commands are the fresh allocator,
seven remaining suites and collector. Exact closed artifacts and a fresh frozen
source check join the campaigns. Only the supplemental manifest and its runs are
boot-bound; original records have no boot ID and are not reclassified.

Supplemental ordering uses monotonic observations from manifest through start,
child close, source verification and the next run. Raw UTC observations remain
valid recorded values even when they rewind; no timestamp is clamped, synthesized
or used as a replacement child-close time. Predecessor bytes, helper/source
identities, boot/clock source and exact duration arithmetic are checked.

The closed collector transcript binds collector SHA-256
`5f26307a75408e084e8d528de8e38cee85ce30b45703943cf9946c6d1de606e6`.
Both raw UTC and monotonic durations are retained without treating them as equal
or inferring a clock cause or benchmark result. Source hashing verifies
snapshots, not continuous immutability, and excludes documentation.

The outer 7,200-second campaign and individual 1,800-second child deadlines,
four build jobs, four test threads and ambient stack settings are unchanged.
Raw bytes, including trailing blank lines, remain intact; only this raw subtree
is excluded from the edited-source/documentation whitespace check.

Native next takes N4 cleanup. Admission C1/C2 and isolated Resources V2 remain
separate implementation/qualification packets. Generated adoption/execution,
proofs, native reliability and matched performance retain separate exits on the
[current swarm map](../../runtime-swarm-next-packets.md).
