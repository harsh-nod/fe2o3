# Generated Typed Completion

Development C5 above the CPU-qualified [private C4 path](runtime-generated-completion-v1.md).
R125 Native CPU/test, R118B C1/C2/C3 and R116/V3 remain accepted. This is not
A1/A2, #182, protected native execution, formal correspondence or HIP/HSA parity
acceptance.

## Original Ownership

Reservation discovers the exact original host result-gate identity before any
readback reservation effects. Ordinary domain rejection returns the unchanged
prepared ticket; panic seals the Context and retains the original driver.
The identity travels linearly with the reserved ticket. It retains gate/account
metadata, not charged output storage or native custody, and allocates no domain
object. It is neither a compiler proof nor execution authority.

`try_activate_generated_v1` admits the exact reserved KFD invocation. Its finite
activation acknowledgement transfers the original completion consumer while the
original driver, producer and unpublished hold remain on the runtime owner.
Reentrant and foreign-generation checks precede domain and reply-capacity checks.
Ordinary live preflight rejection and discarding a queued activation before
execution return the exact ticket in an inner activation failure. Terminal
execution and panic can instead retain custody for shutdown and return an outer
engine error. Activation does not create another completion cell.

`RuntimeAsyncGeneratedCompletionV1` observes the original reply. Only successful
C4 settlement and the original decoder/gate commit produce its move-only
`RuntimeGeneratedCompletionReceiptV1`. Engine and readback errors remain distinct;
no receipt is fabricated for failure. The receipt keeps only metadata alive.

## Typed API

`GeneratedRuntimeChargedResultV1::bind_completion_v1` consumes one move-only
output observer and its completion observer. A short `try_lock` checks that the
output belongs to that exact invocation before waiting. Contention, mismatched
identity, unavailable output and poisoned custody return both unchanged owners.

The resulting executor-neutral `GeneratedRuntimeTypedCompletionV1<T>` has one
source of `Pending`: the original completion future. It adds no waiter, reply,
decoder, allocation or polling loop. After successful completion it moves the
original charged typed allocation under the slot lock. Exact prebinding excludes
foreign in-flight decoders; the original decoder and its custody destructors have
returned and released all slot locks before the reply is published. The consumed
public output observer cannot race another public extractor.

The result retains its full result-peak credit until storage disposal. Successful
typed completion also returns the same receipt, so other heterogeneous outputs
can use `take_completed_v1`. Extraction failure preserves both the original
output observer and the successful receipt; engine/readback failure preserves the
observer without manufacturing a receipt. The tuple adapter below adds
all-or-nothing collection of multiple selected outputs without changing this
one-output adapter's public behavior.

`try_join` waits on the original completion future and uses exactly the same
typed extraction as async polling. Runtime's park/unpark loop handles wake-before-
park and spurious wakeups without polling the GPU or issuing a Context command.
Owner-thread blocking is rejected with the unchanged observer. There is no
deadline: uncertain process-retained custody can remain pending indefinitely.

## Heterogeneous Bundles

`GeneratedRuntimeTypedOutputBundleV1` is sealed and implemented for tuples of
2 through 64 `GeneratedRuntimeChargedResultV1<T>` observers, covering the current
`MAX_ABI_FIELDS` bound. Each element may have a different supported scalar type.
The existing scalar API remains available for one output. A compile-time check
forces tuple coverage to be revisited if the ABI bound changes.

`(words, halves).bind_completion_bundle_v1(completion)` consumes the provided
tuple and the same original completion observer. It rejects duplicate slots
before locking, then checks each slot nonblockingly against that completion.
Already-ready outputs may bind, as with the scalar API. Rejection returns the
entire unchanged tuple and completion. Binding does not assert that the tuple
contains every output of the invocation; omitted outputs remain independently
owned and can use the returned original receipt.

After the original completion supplies its receipt, collection acquires every
slot mutex and retains every guard. Duplicate checking and lock acquisition
precede left-to-right state validation. A poisoned later lock or invalid later
state leaves every earlier output untouched. Only after every member is Prepared,
ready, and matched to the same receipt does an infallible move phase replace all
states with Taken and return the corresponding tuple of original
`ChargedTypedResultV1<T>` owners. No typed allocation is copied or re-created.
Each result independently retains its exact debit until storage disposal.

Slot lock contention after a valid prebinding is excluded by the existing C4
completion ordering and unique public observers: decoding and producer cleanup
release their slot locks before publishing the reply. The adapter uses the same
blocking post-receipt lock policy as scalar completion, not another source of
Pending or a wake loop. It does not add a producer, decoder, gate, reply,
Context operation or cancellation protocol. Duplicate detection uses bounded
stack storage and at most 2,016 pointer comparisons for 64 members; the guarded
validation and move phases are linear in bundle size.

`GeneratedRuntimeCompletedBundleV1` carries the result tuple and the one original
receipt. Post-receipt failure returns all observers plus that receipt;
engine/readback failure returns all observers without a receipt. Await and
`try_join` use the existing shared poll/join drivers. Owner-thread join rejection
preserves the complete bundle future. Drop and Stop/drain semantics remain those
of the original completion, not a new bundle-specific execution mechanism.

The [bundle development archive](evidence/dev-c5-typed-bundle-2026-09-18/README.md)
records its separate CPU qualification. The original C5 archive below predates
this adapter. Host tests use the real charged storage and transaction code with
inert domain metadata; they do not mint protected completion receipts. Genuine
public protected/native bundle completion, formal Rust/native refinement and
performance remain separate requirements.

## Canonical Application Integration

Source review at `ecad7245f` identifies two independent prerequisites for a real
public protected bundle execution. The repository ships no production
`WorkerV3ProtectedVerifierBackendV1` plus
`WorkerV3SemanticMachineRefinementBackendV1` implementation or corresponding
semantic-to-machine proof artifact. Test-only verifier implementations and
lower native vecadd fixtures do not satisfy that authority boundary.

Separately, the canonical `cargo fe2o3 run` application sandbox prohibits thread
creation, while every public runtime async engine starts an owner thread. Even
with a genuine deployment verifier, those APIs cannot currently compose in that
single-threaded process. The synchronous inherited application helper creates
no original async completion receipt and cannot stand in for this path.

The runtime-side next step is caller-driven owned progress on the current
thread, sharing the existing scheduler, operation registry, original reply
cells, Stop/drain and cleanup machinery. Nonblocking enqueue between ticks must
remain distinct from callback reentrancy and blocking self-waits. This is an
unimplemented integration requirement, not permission to relax seccomp, expose
receipt constructors, or promote inert CPU metadata to execution authority.

## Cancellation Boundary

Host-only prepared/reserved custody can be explicitly discarded. After accepted
activation, dropping either acknowledgement or completion loses observation but
does not cancel execution or refund retained producer/native resources. The old
preparation control does not cancel an activated invocation. Stop suppresses
still-pending typed delivery and uses the existing disposal path; already-published
completion results remain observable. Ordinary drain preserves
delivery. Subsequent [C6 development](runtime-generated-graph-v1.md) adds graph
dependencies and corrects direct typed drain before ISSUE; the frozen C5 evidence
below predates that correction. Native graph qualification remains open.

## Qualification Scope

CPU tests separately exercise the original runtime reply/receipt lifecycle,
domain errors and panics, exact ticket/owner retention, waker replacement,
blocking join, Stop/drain, dropped observers, host identity/credit extraction and
the shared typed poll/join drivers. Host driver fixtures return inert metadata,
not forged completion receipts. Public compile-only examples connect real types
through activation, await, blocking join and charged output. Neither those
examples nor the CPU fixtures execute a GPU.

The [development evidence](evidence/dev-c5-typed-completion-2026-09-17/README.md)
records frozen-source CPU qualification: GNU and scoped musl each pass 915 runtime
and 271 host tests, with seventeen and four opt-in ignores respectively. Strict
Clippy, formatting, both crates' no-default-features checks, unsafe-source policy
and all 58 doctests pass. The musl build disables optional legacy HIP linkage,
not direct KFD. Canonical integration applies the frozen patch above `8fa6485ec`
and matches all eighteen source identities and local candidate `9309b1a642`.
The sealed archive retains its original qualification paths, not implied fresh
executions in the integration worktree. Actual protected Worker/carrier/native typed
success and native injected-failure coverage remain open. The concrete Worker V3
protected verifier and semantic-to-machine refinement backend are still required;
lower native readback probes do not supply that authority. Production journals,
cross-run reuse, aggregate residency and matched HIP/HSA performance remain
separate obligations.
