# Auxiliary Queue Construction Custody V1

R96 locally accepts the production-used preparation and CREATE/install phase
composition (NATIVE-2B.5A), above signed planning checkpoint
`10902ca32a853448f79b59cfcf22072e3cdd9325` and signed R95
`da90a0038c6ec4c697faf0fbe93d597e9fa36e1a`. The
[R96 evidence](evidence/local-r96-auxiliary-shared-engine-2026-09-11/README.md)
covers same-engine CPU fixtures with scripted platform leaves, not the complete
production outer settlement or .5B platform matrix. The public Linux session
and existing ownership/capability boundaries remain unchanged.

R95 implements the auxiliary construction owner and checked lane handoff above
signed planning checkpoint `cb29dc6216cdf57c76d1f952264ceed9e257c40a`.
Implementation began above `d464dc442c903e6915fe6ed11dcdfff8fee5db94`;
the accepted preceding runtime source is R94 `363ce6b79938def9365016c34f020a00493c1f87`.
The [local evidence](evidence/local-r95-auxiliary-custody-2026-09-11/README.md)
distinguishes composed CPU acceptance from the still-open NATIVE-2B.5 integrated
platform matrix. This packet does not close NATIVE-2B, A1/A2 or issue #182.

## Ownership

`queue_live/construction_auxiliary.rs` owns one preallocated boxed scope containing
a borrowed parent and an auxiliary construction root. It retains returned data,
R89 preparation, completed dispatch, typed ring/control/completion/EOP/context
prefixes, runtime/event/shadows, resource authority, submission/completion owners,
CREATE key/outputs, the completed lane and the creation arm.

The borrowed parent continues to own the original engine, VM, memory session,
native records, pending R87/R88 custody, accounts and session-wide dependency
owner. Auxiliary construction creates none of those again. The shared primary
prefix helpers preserve borrowed token preflight, the exact in-session marker,
consuming transition and returned-output retention. A marker is not a substitute
for its original session's ownership.

Program envelopes remain borrowed by the preparation call. R89 retains owned
program/code identities; the original envelope vector is not terminal native
custody. No new `Send`, `Sync` or `'static` requirement is imposed on it.

## Sequence

1. Keep terminal/profile/attachment, ring and lane-capacity preflight before
   preparation. Reserve an append slot before native work. Preserve the pure
   journal-capacity rejection through the engine's shared borrowed preflight.
2. Place the scope outside the unwind and model-loan envelopes. Run actual
   opening currentness inside rooted settlement before any loan or preparation.
   Capture returned callback data immediately and use in-place R89 preparation.
3. Return only `Result<(), Error>` from the model-loan callback. Preparation and
   controls stay in the outer root across closing retake error or panic.
4. Enter the existing USERPTR terminal boundary before control allocation. Root
   each returned control/platform owner before the next fallible operation.
5. Admit and validate runtime ownership, check currentness, then acquire the
   process creation arm before event creation. Retain it across retake and CREATE.
6. Admit on the original engine. Publish CWSR shadows only inside its native
   CREATE boundary. Retain CREATE outputs before recovering the queue ID and
   reject collisions with any session-owned compute or SDMA queue.
7. Root the completed lane before doorbell currentness checks, and root the
   returned doorbell before reading its observations.
8. Revalidate the exact destination, finish the creation gate while holding that
   exclusive vacancy, then install through nonallocating, nonfallible moves.

The auxiliary model-admission `AuthorityPoisoned` diagnostic retains its specific
terminal stage. Shared Linux leaves additionally preserve their existing
quarantine behavior; this is not a new fallback execution path.

## Terminal Settlement

An opening error/panic retains the parent without entering the loan or changing
the pre-USERPTR error classification. When entered, the original model-loan
envelope settles before parent evacuation. Its first
operation panic wins over a closing failure or panic. The shared construction
settlement also preserves that panic over a cleanup panic, including a secondary
panic payload whose destructor would panic.

After poisoning, a nonallocating `mem::replace` moves the entire original parent
into the outer scope's `terminal_parent` slot, beside the auxiliary construction
root. It includes its separate runtime-control descriptor,
both ledgers, dispatch/submission/completion ownership, existing auxiliary lanes,
SDMA sets/caches, memory/account anchor and metadata. Private Option-backed
completion/dependency slots allow a terminal empty shell without constructing
replacement authorities. Repeated poisoning tolerates those empty slots.

The shell preserves queue identity/observation and inert pool configuration.
It rejects owner-using operations before dereferencing empty slots. Its Drop has
no engine to restore and performs no native work. A zero shell lane count is not
evidence that retained queues were destroyed; unavailable usage is not a refund.

Only unpublished CWSR payload receives the existing terminal cleanup, retaining
disposed metadata and its abort-on-cleanup-failure policy. Published shadows and
possibly live resources receive no implicit cleanup. The original boxed scope
is retained before returning an error or resuming panic; its leaked borrowed
parent reference is never subsequently accessed. Errors carry no native recovery
authority. Process-gate escalation preserves the USERPTR and model-uncertainty
thresholds; scope retention is not an aggregate memory-budget proof.

## Checked Installation

Preflight rejects oversized or zero-generation rosters, generation exhaustion,
stale plans, occupied/replayed destinations and unreserved append capacity.
It borrows the exact Vec or vacant slot, so another check or callback cannot
change the destination before installation. The completed owner remains outside
the fallible preflight. The existing two-compute-lane profile is unchanged.

## R95 Acceptance

Seven new auxiliary CPU functions cover eleven rejected vacancy plans, three successful
append/reuse transfers, exact parent/auxiliary completion storage and slot
records, dependency epochs, terminal-shell calls and Drop, completed-lane
retention, a nine-cell operation/retake matrix, three opening outcomes and
successful installation. One additional queue test and the extended existing
capacity regression check the exact 253/254/255 history boundary, poison
precedence and observation-free preflight. Opening coverage uses a real
missing-engine rejection and injected panic, not Linux currentness.
The matrix retains an actual charged completion token from the R88 fixture,
but uses a scripted loan and a synthetic parent with no native engine. It is
not evidence of complete auxiliary foundation, runtime, CREATE or doorbell
execution. Existing primary integration tests are not auxiliary acceptance.

## R96 Acceptance And Remaining Matrix

Private `AuxiliaryConstructionV1` phases and `AuxiliaryQueueTargetV1` make the
production preparation/CREATE/install sequence usable with the existing primary
fixture environment. The Linux wrapper calls those phases inside the original
loan/retake envelope and forwards its actual engine, observation, slots and both
SDMA rosters. It does not introduce another constructor implementation.

Two new integration functions cover success and eight late failures after a
completed primary constructor using the same foundation, engine, accounts and
original platform owners. Exact combined native-record/owner partitions,
original data/preparation snapshots, primary ledgers and per-record charges
are checked. A third new function guards the production wrapper's bindings and
ordering. Four compiled mutations and all final source/auxiliary gates pass.
The duplicate-primary-ID cell is lower CREATE `Ambiguous` rejection with no
committed ID or outputs, not execution of the later retained-roster check.

NATIVE-2B.5B still needs original-parent production outer settlement through
opening/preparation/control-prefix and real loan/reclaim failures; retained
primary runtime-lease/local Linux-helper composition; full CREATE
uncertainty/malformed/panic; both failing late currentness checks; occupied/reused
slots and retained auxiliary/SDMA rosters; cleanup failure and exact first-panic
transport. .5A uses a fixture outer scope and scripted platform leaves. Its
acceptance does not inherit the existing primary local-Linux-helper coverage.
Live KFD and new adapter-refinement proofs remain separate.

Callback/installer-internal owners that fail before returning remain outside
the returned-prefix guarantee. Concurrent bootstrap, replacement/insertion,
native generated adoption, aggregate budgets, protected compiler integration and
matched HIP/HSA performance remain open. No performance improvement or new
formal theorem follows from this ownership change.
