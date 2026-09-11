# Unpublished Operation Lifecycle V1

R83 implements the private engine substrate of ADOPT-LIFE after R82. It does
not implement native DATA adoption, issue or completion. The public generated
preparation route still installs no native adoption hooks and activation of
those preparations rejects `Unsupported` before installing a hold. No public
adoption/submission API or execution authorizer is added.

## Ownership And Admission

The [driver](../crates/fe2o3-runtime/src/async_engine/generated_operation/adoption.rs)
retains its complete original payload, R80 roster and completion producer. Exact
activation consumes the same reserved ticket and completion consumer into that
driver. Only a finite activation acknowledgement allocates another reply cell;
no new readback, result account or operation slot is reserved.

Preflight validates the exact ticket and calls a private monomorphized adapter.
This hook may reserve metadata and check currentness but must acquire no native
custody, including pool checkout or detached authority. Missing hooks, ordinary
preflight rejection and unqueued/queued Stop preserve the original ticket.
Panic or terminal currentness never returns a retryable ticket.

The [registry](../crates/fe2o3-runtime/src/async_engine/operation.rs) moves the
same preallocated entry from parked to active before changing its phase. Both
discard keys disappear. The preparation control remains finished; it is not
reset to describe adoption. The activation acknowledgement is not native readiness,
submission or completion. Its observer may disappear without disposing custody.

## Stream Hold

A private move-only [hold](../crates/fe2o3-runtime/src/context/unpublished.rs)
uses an existing stream cell and the Context's checked identity allocator. It
has no destructor that releases ownership and is not a native lane lease.
Foreign/unknown streams, another hold, an active graph or any exact-stream
nonquiescent submission reject before installation. The last condition matters
because polling an older deferred submission can itself publish work.

The hold blocks ordinary launch, copy, peer copy, flush and destruction on that
stream. Prepared launch/copy submissions recheck it. Other streams are not
globally blocked. Graph admission rejects while any hold exists. Direct Context
cleanup returns an incomplete retained-resource report without calling native
cleanup. Ordinary stream lookup paths reuse their lookup for the hold check.

## Progress And Retirement

Active adoption retains a `None` flush identity, so the pending-publication
roster cannot discover it. Before its first adoption callback the driver enters
the conservative quarantined phase; only callback success with a live Context
advances to adopted. Error or panic seals Context, keeps the owner and hold,
and cannot re-enter the callback.

After the accepted command queue is exhausted, cooperative drain calls bounded
unpublished retirement before its quiescence decision. Owned shutdown uses the
same retirement helper before Context cleanup. Retirement does not rotate the
progress queue: a second equal-budget rotation can starve progress and retirement.

The retirement adapter receives only Context and the exact hold, not the host
payload. It must handle an empty prefix when Stop precedes first advancement.
Success means conclusive native disposal including closing currentness; only
then is the hold removed and driver disposal permitted. Failure or panic keeps
the driver/hold, seals Context and prevents retirement retry. Unknown outcomes
do not become a successful drain or native shutdown.

## Acceptance Boundary

CPU tests exercise the real private registry, command, Context and owned-engine
helpers with scripted adoption/retirement callbacks and owner-local payloads.
They cover capacity/reply and pointer preservation, exact/foreign/replayed
tickets, preflight/error/panic, held-stream ingress, prior pending work,
observer loss, Stop before first advancement, both-order unit-budget fairness,
owned drain and shutdown/quarantine. They do not instantiate actual R73 charged
storage through protected construction, native prefixes or Linux adapters.

R83 adds no authenticated Verus theorem or executable adapter refinement.
DATA still needs complete native registration without ordinary Arc-backed
shadows, a closed consuming packet-transfer API, native lane exclusion and
prefix retention/disposal. ISSUE must retain publication-time authority;
COMPLETE must validate the full roster and destroy original encoded storage
before typed result readiness. Linux qualification and matched HIP/HSA
performance remain separate open gates.
