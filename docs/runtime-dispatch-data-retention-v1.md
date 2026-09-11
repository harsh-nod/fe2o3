# Whole-Roster Dispatch Data Retention V1

R86 implements the data-conversion portion of NATIVE-1 on signed R85
`9f8779faab169271d716f4a241f849fef0af3567`. It does not complete NATIVE-1's
code/kernarg or outer construction ownership. The
[swarm dispatch](runtime-a1-a2-swarm-dispatch-r83.md) retains those dependencies;
the [local evidence](evidence/local-r86-dispatch-retention-2026-09-11/README.md)
separates CPU acceptance from proof and hardware qualification.

## Problem And Boundary

The old fixed-dispatch preparation loop consumed inputs one at a time. A
Device retention rejection could return only the failing input, losing the
typed converted prefix and unvisited suffix. Host retention rejection returned
no data. Native session records and charges still retained backing: the defect
was lost typed custody, not observed premature native freeing.

The production preparation path now calls one private whole-roster helper in
`crates/fe2o3-kfd/src/shared_memory/dispatch_retention.rs`. It borrows the original
`Vec<Gfx942FixedDispatchDataV1>` exclusively, validates all entries before any
move, and returns concrete retained authorities with the original initialization
metadata. Persistent preflight rejection returns the entire original vector.
Ordinary error-only wrappers retain their existing externally visible behavior.

## Validation And Commit

The helper requires an active engine and at most sixteen data allocations.
Before consuming any entry it checks:

- Exact original storage identity and no duplicate native storage, regardless
  of initialized/uninitialized wrapper labels.
- Device record identity, generation, device/VM, layout and mapped phase;
  native handle/reservation and exact optional N2 backing charge.
- Host session identity, record generation, canonical coherent layout, mapped
  phase, native handle/reservation/mapping and exact optional N1 charge/domain.
- No attempted free and no malformed initialized-content extent.

It reuses the existing host/device pool record validators. Facts are derived
locally while the engine remains immutably borrowed and paired only with the
same exclusively borrowed original roster. There is no externally reusable
facts-plus-arbitrary-token constructor. Unconfigured accounts preserve the
existing None/None behavior; there is no new budget reservation or refund.

Facts and retained outputs use fixed-capacity `ArrayVec` storage. Commit moves
the exact tokens and descriptors in ordinal order, with no allocation, callback,
native call, currentness sampling or fallible validation. The original vector's
backing remains reusable. The enclosing preparation function reserves its full
authority/premise vectors before calling this conversion helper.

All failures and injected preflight panics leave the borrowed original owner
unchanged, including pointer, capacity, order, token identity, initialization
state and content descriptor. This is a pre-effect transition, so retrying this
isolated conversion after a pure preflight panic is permitted. It is not retry
authority after native construction or publication uncertainty.

## Cost And Tests

The helper performs bounded indexed record lookups, with the existing bounded
linear fallback on an index miss. Duplicate comparison is O(n squared), at most
120 comparisons for n <= 16. Conversion is O(n), uses bounded inline scratch and
does not copy buffer bytes or allocate another heap-backed roster. This is an
algorithmic accounting statement, not a measured latency or throughput gain.

Thirteen new CPU tests invoke the production conversion helper over actual
fake-native engine records. They cover configured/unconfigured accounts,
exact projected facts, complete mixed rosters, rejection at every ordinal,
backing/charge loss, wrong phases/generations/domains, foreign storage,
cross-label duplicates, capacity-before-member-validation, panic custody, malformed
content and refunds only after actual fake-backend disposal.

Four storage variants use the real fake-backend initialization paths. The
separate five-variant test consumes the existing pristine-abort fixture,
including initialized-after-dispatch storage; that fixture's injected content
premise is not Linux initialization evidence. Deliberate early-move and missing
duplicate-check mutations are tested separately from restored positive source.
The fixtures execute the common conversion helper, not the live-session
wrapper or full fixed-dispatch constructor. Those integration gates remain open.

## Remaining Construction Work

This packet does not yet root the complete preparation plan, generation,
packet descriptions, code prefixes, current code typestate, kernarg typestate
or successful dispatch output outside the fallible constructor. Existing later
code/kernarg failure recovery still uses post-dispatch reconstruction and can
erase an unpublished authenticated content descriptor. Outer panic and closing
retake handling still need a non-discardable preparation owner.

NATIVE-1 must integrate those owners with the existing persistent terminal
custody before NATIVE-2 covers initial/auxiliary constructors and closing retake.
Unknown native transition results must retain actual session-record custody,
not reconstruct a usable mapped token or pristine retry continuation. Native
generated adoption remains disabled. No new Verus theorem, executable adapter
refinement, Linux completion, protected compiler authority or matched HIP/HSA
performance result follows from these tests.
