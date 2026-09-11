# Primary Queue Construction Custody V1

R92 implements the NATIVE-2A.2 outer primary-constructor owner above signed
R91 `a04060137e8df20f434682f7ee4dd6f06fb10174`. It composes the existing
[lower handoffs](runtime-queue-construction-handoffs-v1.md) and
[R89 preparation](runtime-fixed-dispatch-preparation-custody-v1.md).
The full per-stage integrated failure campaign remains NATIVE-2A.3, and Linux
qualification and executable refinement are separate requirements.

## Retained Owner

`queue_live/construction_primary.rs` owns one preallocated private `Box` from
the returned admitted memory session to the completed queue. Its memory session,
foundation, engine initializer, engine and completed-session slots transfer
ownership in that order, without creating replacement session/account anchors.
Typed ring, control, completion, EOP and context-save prefixes, preparation,
runtime/control descriptor, event, shadows, CREATE outputs and submission/dependency
owners remain siblings until final assembly.

Borrowed token preflight precedes every consuming seal/map/role handoff. Actual
returned tokens are installed before another fallible operation. On admitted
failure, the exact identity marker points into R88 custody in the same retained
session; a marker is neither a reconstructed usable token nor disposal authority.
R88 can retain a CPU precursor even after a native mapping succeeded: typestate
and native progress are checked independently.

The ordinary fixed-dispatch entry roots its existing session, packets, programs
and complete data roster before fixed-batch validation or geometry planning.
It invokes the existing ordinary/new-generation preparation sequencer in place.
Default, budgeted and debug constructors root a successfully returned generic
preparation value in an initially empty slot before ring allocation. No `Send`,
`Sync` or `'static` bound is added to that value.

All fallible assembly values, including the dependency owner and shadow-page
count, are computed before session fields move. The completed session is rooted
before pre-doorbell currentness, the doorbell map and post-doorbell currentness.
Returned doorbell ownership is installed before reading its observation fields.
The primary constructor still takes its model foundation once; it does not
perform a closing live-model retake.

## Failure Settlement

Errors and panics retain the original root allocation for process lifetime.
Native authority is not inserted into public error types, and no implicit
unmap, free, runtime disable or budget refund is introduced. Pre-control errors
keep their existing classification; they are not necessarily pre-effect errors.

A separate local terminal guard is armed immediately before the first USERPTR
control attempt. The diagnostic USERPTR-ring path now arms before its own ring
allocation, closing its earlier panic-before-control poisoning gap. Ordinary
AQL and executable-probe ring failures remain before the control threshold.

On terminal failure, settlement explicitly poisons the existing global gate
before unpublished cleanup and retention. Cleanup panic is caught separately;
the root is retained before unwinding. If work and cleanup both panic, the
original work payload wins and the secondary payload is retained without running
its destructor. Concrete poison/retention callbacks are nonpanicking; a process
abort, allocator abort or arbitrary replacement callback is not unwind recovery.

## Runtime And Shadows

Owned runtime admission, or the existing external-runtime handoff, precedes
global creation-arm acquisition. Arming earlier would reject the constructor's
own runtime admission. External runtime references are not retained: the exact
runtime and control descriptor leave their caller slots at the existing handoff.

After final currentness, mutex-linearized checked finalization requires the
active creation arm, no poison/teardown arm and an enabled nonzero runtime lease
for the same opener. Ordinary `disarm()` is not a health certificate and is not
used to promote this constructor. Existing SDMA disarm behavior is unchanged.

Overlapping constructor attempts remain unsupported. Refcounted runtime leases
permit overlapping queue lifetimes, not concurrent bootstrap: two admissions
can precede the first creation arm, and the losing constructor has already
entered USERPTR effects. Its failure is terminal, not retryable `Busy`. Existing
runtime primary/SDMA and multi-device child construction is sequential. Supporting
concurrent bootstrap needs a separate pre-effect reservation protocol; checked
finalization alone does not provide it.

Before the native CREATE boundary, terminal cleanup releases only the unpublished
CWSR payload through the existing zero/protect/unmap primitive. Cleanup failure
still aborts. The wrapper retains exact shadow descriptors, binding and extent
in a disposed state: later access/publication rejects, and Drop cannot release
the payload again. Publication remains inside `create_at_native_boundary`.
Published shadows receive no implicit payload cleanup on constructor failure.

## Acceptance Scope

Focused tests compose the production settlement, returned-value capture and token
handoff helpers with real R88/R89 fake-native owners and configured accounts.
They check exact allocation/token/descriptor identity, pre-control classification,
poison/cleanup/retention order, first-panic preservation and one success transfer.
Local mapped-shadow and mutex fixtures check disposed metadata, abort-on-cleanup-
failure and final gate admission. Existing R89 matrices now call the same ordinary
forwarding helper used by the primary constructor.

Source guards check actual integration placement and native ordering. These are
not a complete Linux constructor simulation: the single same-session per-stage
allocation/initialization/CREATE/doorbell error-and-panic matrix is NATIVE-2A.3.
The primary owner implementation does not by itself complete that acceptance.

## Remaining Boundaries

VM/session acquisition before a memory session is returned remains outside this
owner. The generic callback and private source-complete builder may consume
native values internally and fail before returning; rooting their successful
output cannot recover such unreturned prefixes. Recycled replacement still needs
NATIVE-2C's earlier predecessor/input preparation root. Auxiliary construction
remains NATIVE-2B and does not use this primary envelope.

No new ring/control/EOP/context-save backing charges, aggregate budget bounds,
generated adoption hooks or public runtime APIs are added. Terminal process-lifetime
retention is not bounded resource closure. Native generated issue/completion,
formal adapter refinement, shared-machine qualification, matched HIP/HSA
performance and full A1/A2/parity remain open.
