# Owner-Local Operations V1

R75 implements GEN-2B-1: transportable operation factories, owner-local drivers
and custody retained through owned Context shutdown. Ordinary launch, tracked
launch, copy, peer-copy and frozen-request entry points use this factory path.
There is no new public generated-launch API or native publication mechanism.
GEN-2B-2 through -6 remain in the [dispatch plan](runtime-a1-a2-next-wave.md#gen-2b-breakdown).

## Admission And Progress

The private [factory](../crates/fe2o3-runtime/src/async_engine/operation/factory.rs)
is `Send`; the private driver trait no longer requires `Send`. Materialization
has no Context parameter and must not issue native work or acquire native
custody. Context-bound preparation belongs in the driver's first advance, after
the engine has installed it in the retained registry. An outer panic catcher
cannot preserve objects already unwound inside a factory or callback.

1. Reserve the existing reply cell and bounded command-channel position.
   Command or reply exhaustion returns before owner materialization.
2. The owner checks progress availability, its shutdown capability and the
   existing operation-capacity predicate before materializing a driver.
3. Local factories require owned shutdown by default. Only the existing
   handle-only adapter opts out: its native resources live entirely in Context.
   A Context-returning engine rejects local factories before materialization;
   it does not attempt to transfer non-Send custody through its thread join.
4. Cache the inert stream identity and install the driver before its first
   advance. Advance borrows the installed front entry instead of removing it.
5. A true advance result permits driver disposal, not merely reply completion.
   Even that result cannot retire the entry while Context is terminal. Pending
   entries rotate one position; panicked entries stay installed without retry.

The Context type gains no local-driver field or changed Send requirement.
Command transport remains Send even for the existing non-Send backend profile.
No unsafe Send/Sync implementation or checked-device lifetime widening is added.

## Reply And Payload Lifetime

The ordinary factory transfers its unique `Option<Reply>` and operation control
to the driver once. Success or rejection takes, completes and drops that producer;
quarantining the remaining driver does not itself retain a completed reply cell.
The future still retains its cell and result until actual disposal. Existing
pre-submission cancellation precedence is preserved.

On engine stop or panic, the ordinary handle-only driver also disposes its
never-called `Submit` callback after reply detachment. That field owns unissued
host payload, not possibly live native custody. Issued submissions have already
consumed it. Capacity rejection does not refund a payload still retained by its
factory/driver; actual disposal remains the refund boundary.

Private factory/driver rejection must detach and resolve its reply infallibly
before any fallible handling. Post-detachment panic containment preserves custody
and continues stopping other replies. It cannot recover a producer hidden by an
implementation that violates this contract before detachment. Existing concrete
rejection uses the contained reply/waker protocol, not a caller-supplied callback.
The tests do not establish progress for arbitrary broken private drivers.

## Shutdown And Bounds

The owned engine preallocates its bounded operation roster before native Context
construction, then rebinds it after Context for destruction ordering. The roster
lives outside the owner-loop/cleanup unwind boundary. Stop resolves observations
without removing unresolved drivers. Context cleanup and native backend shutdown
both run while that custody remains alive.

For unresolved drivers, only complete cleanup plus successful native shutdown permits disposal:
drivers are dropped before Context/backend. Incomplete cleanup, native failure
or outer unwind retains both owners until process exit. Existing Context-owned
graph machinery is unchanged; this packet does not admit generated graph custody.

One registry accounts for installed, advancing and stopped-retained entries under
the existing `waiter_capacity`. Retained entries cannot create free operation
capacity or a quiescent drain observation. There is no additional growing
quarantine list. Per-tick advance/flush limits and cyclic stream selection remain
unchanged; single-entry rotation is constant work, stream-count updates use the
existing ordered map, and each stop-observation pass scans the bounded roster once.
These are algorithmic bounds, not a measured performance gain.

Record bounds do not bound arbitrary arguments, factories, drivers, wakers or
result bytes. R73 charged generated storage and R74 production authority have
not yet been placed in these drivers. MEM-5 and exact generated completion remain
separate integration requirements.

## Validation Boundary

Sixteen new CPU tests use private inert factories, Rc-holding drivers and the
existing thread-bound mock backend. They cover owner-thread construction/use/drop,
successful cleanup order, all four cleanup/finalizer failure modes, advance and
outer-loop unwind, inert factory panic, terminal retirement, observer drop,
mixed local/ordinary capacity, capacity reclamation and retired-stream flushing,
reply/queue/closed admission, waker replacement and post-detachment rejection
panic. Existing cancellation races now move factories to the owner before
materialization; queued-drop tests still dispose factories without materializing.

The focused async suite passes 186 tests. The [local evidence](evidence/local-r75-owner-local-operations-2026-09-10/README.md)
records broader current-source gates and failed intermediate regressions.
No model/proof source or Verus theorem changes in this packet. Existing pure
capacity and shutdown predicates do not prove the new Rust factory, registry,
destructor or reply composition. No positive generated execution, native readback,
GPU performance, full HIP/HSA parity or whole-executor proof is established here.
