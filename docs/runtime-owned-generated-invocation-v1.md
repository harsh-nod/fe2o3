# Owned Generated Invocation V1

GEN-2A adds `GeneratedWorkerV3RuntimeInvocationV1<K>` and the consuming
`AuthenticatedWorkerV3ExecutableV1::prepare_generated_runtime_invocation` API.
This is nonexecuting preparation, not Context admission or a completed launch.
GEN-2B still owns publication, native retirement, charged decoding and Future
wakeup integration.

## Ownership And Admission

The preparation method consumes an authenticated executable, raw generated
owned arguments and an actual checked gfx942 device. It also takes geometry,
dynamic group bytes, timeout, per-invocation limits and an R73 result budget.
Detached packed arguments and caller-created authorizers are not accepted.

1. Reject unavailable protected execution evidence before argument callbacks
   or encoding. This rejection-only preflight does not refresh publication or
   grant execution authority.
2. Check current publication, observable device currentness and the canonical
   gfx942 target.
3. Use the existing authenticated charged-argument preflight and packing route.
   Move its kernel identity, packing observation and logical footprint; do not
   clone input buffers or packing metadata in this adapter.
4. Prepare the request from the retained token's exact artifact bytes and
   generated kernel name. Validate its exact artifact digest, length and entry.
5. Recheck device and retained publication currentness, then consume the unique
   semantic-to-machine receipt through existing application admission.
6. Retain the same private production authority used by the borrowed direct-KFD
   path. Missing admission always rejects, including with verifier test-support
   enabled. The borrowed path's separate qualification behavior is unchanged.

Device observations may perform syscalls. This preparation creates no native
VM, allocation, queue or dispatch. Public getters expose descriptive kernel,
dispatch, device, footprint and application-binding observations only.

## Storage Lifetime

Private `GeneratedRuntimeStorageV1<P>` declares its complete payload before its
decoder. Inputs move only through the closed runtime-preparation function; its
error cannot retain input storage. On preparation failure or unwind, consumed
inputs are disposed before the retained decoder. On success, the complete
prepared request, including read-only initialization policies, remains guarded
through subsequent identity/currentness checks and receipt admission.

The final invocation declares storage before production authority and device.
It has no `Clone`, unchecked constructor, extraction or execution method. An
explicit `PhantomData<Rc<()>>` enforces `!Send` and `!Sync`; this does not rely on
the checked-device rustdoc or alter the existing device's auto traits.

R73 charges typed-result peak storage and returned read-only bytes. It does not
account for prepared executable images, full/hidden kernargs, retained read-only
initialization copies or all metadata. GEN-2A does not close those resource gaps.

## Verification Boundary

CPU tests exercise actual charged packing and actual runtime preparation with a
structurally valid, deliberately nonexecutable COV6 fixture. They cover loader
rejection, ordinary disposal, unwind, a payload-disposal witness, read-only
policy retention and preservation of the existing scratch rejection. The
original runtime fixture is unchanged; a separately named preparation variant
sets metadata/descriptor private size and descriptor scratch enablement to zero.

Downstream fixtures type-check the consuming API and reject cloning, thread
transfer/sharing, storage/decoder extraction, reuse of the consumed executable
and execution. Existing tests cover bound-coordinate mutations, exact runtime
identity and refinement-receipt admission; the protected-provenance test covers
every currentness/receipt combination for protected and synthetic provenance.

These are adapter/type-check results, not a positive production constructor
or GPU execution result. The repository still lacks the concrete production
protected-verifier/refinement backend and exact artifact handoff. Test evidence
cannot replace that dependency. No new Verus theorem or whole-executor
refinement is claimed for Rust ownership, allocation, decoder or device adapters.

## Next Transition

R74 owns a standalone checked device. Persistent Context integration cannot
construct one such owner per launch: the device model rejects a second live
admission of the same physical GPU. GEN-2B must instead prepare against the
backend's retained device, binding private Context/backend/device generations.
The same owner may move into its lazy VM/queue; actual native incarnation checks
belong at adoption/publication. No fresh-device or second-queue workaround is
part of this contract.

GEN-2B needs a runtime-defined admitted interface because host already depends
on runtime. It must bind Context/allocation generations, revalidate at actual
publication, consume authority once, retain resources through exact retirement
and authorize decoding only for that completed invocation. Blocking execution
must join this same path. Timeout, observer drop or failed observation cannot
grant replay or early-release permission. Contention retry/wakeup and lost-wakeup
tests are required; R73's `try_take` does not provide Future progress by itself.

The [six-packet dispatch](runtime-a1-a2-next-wave.md#gen-2b-breakdown) separates
owner-local drivers and cleanup retention, reusable device preparation, checked
persistent projection/publication, exact readback/results, public API convergence
and generated graph/drain qualification. Current operation drivers require
`Send` and their registry drops before owned Context cleanup; neither boundary
can hold the proposed owner-local issued authority unchanged. Native completion
status alone also supplies no decoded host output. These remain unimplemented
integration work, not acceptance implied by GEN-2A's storage tests.
