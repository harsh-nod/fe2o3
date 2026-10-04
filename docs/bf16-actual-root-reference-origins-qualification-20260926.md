# Actual source-call reference origins

Status: a private compiler prerequisite, not complete nominal-kernel admission.
Accepted broad exits remain M1/V1/V2/U1/U2/U3 (6/18).

## What changed

The same actual retained-input assembly now joins each identity
DisjointSliceGetMut call to its prepared guard and propagates reference origins
on the original work/storage ledger. A retained association identifies the
source-call ordinal, source block, callable, destination local and exact
guarded-access index. The ordinal counts every Call terminator, not just accessors. Nonaccessor
calls advance the source ordinal without advancing the guard index.

The constructor borrows the real source, complete-for-profile graph, input
loan, guard vector and origin storage from one pending owner. Call counts,
equal copied payloads and replacement ledgers cannot establish that join.
Ordinary normalization, reference-origin seed order, FIFO propagation,
definition/refusal behavior and legacy allocation semantics are preserved.
The supported producer whitelist is unchanged.

Partial associations, nested origin tables and consumed FIFO entries stay
outer-owned through source/facts/context postflights. Accepted storage is
released only after those payloads are dropped. Occupied length or capacity,
foreign-ledger reuse, source-identity mismatches, missing availability and
ambiguous seed definitions refuse without exposing a completed view.

This is source-associated reference-origin data. It does not assign a later
memory-use site: semantic_site remains None. The accessor's source block and
ordinal are not the statement/terminator location of a later dereference or
store, nor an operation insertion cursor. Checked use-site projection,
bounds/effects, complete guards/CFG/assertions and a full unverified root recipe
still have to reach mandatory verification before normal nominal admission.

## Qualification

The scoped regression passed 331 model tests and 2,683 backend tests (189
ignored), plus the backend/extractor build. The initial S5A regression retained
a test-only E0597 borrow-lifetime failure. An explicit borrowed-argument type
annotation corrected that oracle-test closure without changing production
behavior; the failed receipt is retained.

Five fresh actual Rust sessions cover Identity, Swap01, wrong launch, callback
error and callback panic. An independent oracle rescans real source calls and
all five association fields, reconstructs source definitions and seed ordering,
and checks every propagated origin and the exact FIFO cursor/order. It does
not call the production association validator, origin preparation helper or
definition visitor. The preexisting S3 and S4 payload oracles remain active.

Each prepared case has one source/guard association, two origins and two FIFO
entries from one seed. The underlying access state still has three operations,
next value ID two, one view, one access and one predicate. Wrong launch emits
no preparation or accepted-frame rows.

Test-only fixed thread-local telemetry records credits only after both work
and storage admission. It adds no runtime fields, layout changes or generic
closure captures. Each positive session contains 77 closed versioned rows:
28 for S3, 28 for S4 and 21 for S5A. S3 and S4 each have six admitted
preparations among nine observation/control runs; S5A has five admitted runs.
Those counts are source-derived per observer, not assumed to be interchangeable.

| Current observer | Assembly | Complete graph | Initial graph |
| --- | ---: | ---: | ---: |
| S3 prefix/index | 11,432 | 1,504 | 8,944 |
| S4 guarded access | 11,592 | 1,584 | 9,024 |
| S5A reference origins | 11,688 | 1,632 | 9,072 |

Current S3 work is 1,924,395 for Identity/error/panic and 1,924,719 for Swap01.
S4 work is 2,030,727 or 2,031,051. The new five-run origin observation debits
1,550,028 or 1,550,208. The accepted access frame is 4,952; origin_frame=4,280
reports only the new join frame. The inherited FIFO frame is separately charged
and already included in measured work. These are logical accounting
measurements, not native memory/stack limits, GPU timings or a performance gain.

A separately qualified historical S4 probe uses the original canonical source
path and a fresh Cargo cache. It preserves all original S4 work observations.
Its own S3 frames are 10,944/1,488/8,928; its own S4 frames are
11,104/1,568/9,008, with access frame 4,928. Old S4 generic frames are measured
in that S4 scope, not borrowed from old S3 or inferred from a report residual.
The historical probe emits 56 rows per positive case, none for wrong launch.
Qualified restoration rehashes both complete source trees, exact Git status,
original directory identities, backlinks and the historical source inverse.

All 181 comparison controls passed. The completed direct R22-to-R23 comparison
reports 132 changed fields within the strict 138-path
policy. Its separate historical zero-work calibration reports
47 changes within the fixed-cache 143-path policy.
Direct changes are 96 work fields and 36 provenance fields; every old storage/peak, kernel numerical result, mask, refusal and admission field remains exact. Work increases by 1,557,756 for Identity/error/panic and 1,557,936 for Swap01.

Each old/current S3 or S4 same-scope frame difference contributes six times (488 + 16 + 16), or 3,120. The additional S4 work is six times the accepted access-frame difference of 24 plus source-association preparation over all 12 Calls and one accessor: six times (16*12 + 32), or 1,344. Thus S4 increases by 4,608; S3 increases by 3,120. Adding the separately measured five-run origin work exactly explains the direct increase, with no handoff change.

Historical calibration has no work or storage drift. Its 47 provenance changes include six dependency-byte fields for a measured 1,116-byte increase and five metadata digests derived from exactly two fixed Cargo cache fields. Original/current/historical dependency trees contain 352,670,356 /352,670,356 /352,671,472 bytes respectively; all three have 459 files.

The full comparison rehashes all 455 current selected inputs, the complete
8,468-file current source roster, and three complete 459-file dependency trees.
It joins all 440 historical scoped request/receipt inputs and exact source
inverses through qualified restoration; it does not rehash the historical
source tree again under today's path. The actual comparison consumed
10,484 reads and 1,237,630,855
content-plus-EOF bytes under unchanged 11,000-read /1,280 MiB /180-second
limits. Synchronous filesystem interruption is not proved.

The work reconciliation separates same-scope frame differences from actual
source-association work and the new five-run origin work. Source association
cost uses the complete real Call roster, including nonaccessor calls. No
unexplained work residual, absolute storage exception, arbitrary metadata
digest or source-root relocation is accepted. Original-ledger and independently
prepaid fresh-probe peaks keep their distinct scopes.

Fresh ordinary ladders passed 36 composition and two direct-BF16 sessions.
All 38 observation bodies and 52 artifacts are byte-identical to S4.
This preserves ordinary routes; it does not admit the nominal helper.

| Evidence | SHA-256 |
| --- | --- |
| Initial S5A regression, retained test-only failure | b169eac8be6666fa3a3f8d71e2e33313a075ee7ddbb98a39e12f6ba0648d099b |
| Scoped regression | 85fd182a02183842960fa0e31a5e745c9b07574dba225d957cd03f00370c149c |
| Current genuine sessions | d8ebc77107c608c49e6d6be40f415ffd5819868586339c6eab6eb955902a0c0c |
| Historical scoped sessions | 5f7f6b467b0194e08e3049d205d26615f66fccf9588f889105132ac1adc76aef |
| Complete source restoration | 80eb53e87b6f9acae152a2ef49b0540b97ce777b4b3a33f2e163be299f3fb83f |
| 181 comparison controls | 498cd797b2ffff7fc79390136b6032bb710c926c024e794668c3ffb18669f52a |
| Ordinary ladders | 25deb1df8462f16d5984a035bd0b9c2daaba1970cf3036aab2919d3b28d809d7 |
| Ordinary output readback | e76919d91647b98a49d0bc9f12ca4b192c901ad6eced57fff379f61d8a5fd6f0 |
| Direct and historical lossless comparison | 60a71385c4f75a9ce6718b7016f501694e8c8741fda2e7500a00d9ed03a92da0 |

## Boundaries

Component controls include wrong/missing/duplicate/reordered associations,
equal-count source substitutions, all excluded accessor families, intervening
nonaccessor calls, missing Some availability, multiple seed definitions,
foreign ledgers, occupied owners and short resource budgets. Raw inert controls
do not create nominal source custody. They do not exhaust every new byte
boundary or inject allocator/OOM failures. Genuine callback error/panic cases
occur after preparation, not at every allocation point.

The private lexical view is not a durable ready token or a public source API.
Nonempty retained reference bindings and broader producer families remain
refused. No later memory-use-site binding, CheckedReferences construction,
complete root recipe, ordinary nominal-helper routing, edited-input promotion,
nominal LLVM continuation, GPU execution or debugger capture is claimed.
