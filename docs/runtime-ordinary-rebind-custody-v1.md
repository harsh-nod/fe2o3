# Ordinary Live Rebind Custody V1

R104 extends the existing preparation and loan settlement to ordinary
`ComputeAqlQueueSessionV1::bind_fixed_dispatch` and its borrowed compute-lane
facade. It does not replace the queue engine, memory model or public API.
The [local evidence](evidence/local-r104-ordinary-rebind-custody-2026-09-12/README.md)
separates CPU/shared-sequence acceptance from native execution and formal proof.

## Input And Preparation Ownership

The public entry allocates a private root containing the original program
vector, packet array, data vector and detached predecessor before inspecting
them. Common preflight borrows that root. Healthy rejection retains consumed
inputs without newly poisoning or moving the parent. Already-terminal or
already-attached rejection preserves the previous classification; invalid
detached provenance or identity metadata retains the existing terminal policy.

The ordinary route places `FixedDispatchPreparationCustodyV1` in the root before
the model loan. Its generation-aware in-place helper borrows the original
programs and preparation. The callback returns only status, so an operation or
closing-retake failure cannot discard a returned native owner.

The unchanged [loan settlement](runtime-persistent-bind-settlement-v1.md#loan-settlement)
runs no operation/retake after rejected opening, exactly one retake after
successful opening, and preserves operation-panic precedence over closing
failure. Completed preparation remains rooted through retake and borrowed
full live-dispatch-memory validation. Checked extraction rejects incomplete or
failed preparation. Only then does a callback-free, nonallocating commit
install the dispatch and clear detached identity bookkeeping.

Ordinary `after_detached` generation handling remains distinct from R103's
recycled replacement and from pristine continuation authority. Helper-level
zero compatibility is not evidence that native recycled detach produces zero.
R105's [pristine continuation custody](runtime-pristine-rebind-custody-v1.md)
now uses this same preparation/settlement root. Its exact-generation resume and
entered-pristine process-terminal policy remain distinct from ordinary rebind.

## Deferred Parent Retention

An internal settlement result carries only the returned error/panic and a
terminal-transport request. Failed input/preparation custody is retained before
this result leaves settlement. Returned errors retain their prior local poison
classification; existing loan boundaries own any additional process poisoning.
The shared root also preserves entered-pristine process poisoning. Ordinary
returned errors are not newly process-poisoned by that separate policy.

Direct session bind can retain the complete original parent immediately. The
borrowed lane facade instead accumulates the request monotonically before it
returns the error or resumes the panic. Catching the panic, swallowing the error
or making a later already-terminal call cannot erase the request.

`with_compute_lane_v1` catches the callback and restores the original primary
and selected auxiliary slot first. It then applies the existing callback-panic
poisoning policy and honors pending parent retention. This order preserves
the selected lane, sibling lanes, account owners and metadata in one retained
parent. The caller receives an inert terminal shell. Retention grants no
release, retry, cancellation, recovery or native disposal authority.

## R104 Verification Boundary

Nine dynamic tests cover 112 scenarios; a tenth test checks source routing.
Preparation tests use original fixture memory, accounts and certified
foundation with actual model-loan/reclaim and memory-validation helpers. The
facade fixture is separately constructed with `engine=None`. These tests do
not establish original queue-engine/account/platform composition or live KFD
execution. Auxiliary restoration covers the first auxiliary vector slot;
later vector slots remain unqualified.

The packet preserves envelope descriptors that borrow caller-owned module
bytes; it does not acquire those bytes. Allocator abort and panic-abort are
outside unwind recovery. R105 separately covers pristine prefixes at its named
CPU boundary. Insertion, release/teardown, DATA-ADOPT, ISSUE and generated
completion remain open work. Authenticated
adapter correspondence, native incarnation succession and matched HIP/HSA
performance are separate acceptance requirements, not consequences of the
CPU matrix or compiled behavioral mutations.
