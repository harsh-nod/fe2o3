# Working Multi-GPU Critical Path

Priority refresh: 2026-10-03. This is an implementation work order, not an A3
completion claim. Native agents reviewed Context admission, backend custody and
existing MI300X evidence independently. Primary owns edits, integration and tests.

## Reuse What Works

The selected-pair copy controller already qualifies production deny-all kernel
constructors with native and staged copies in both directions. Finite qualified
compute/peer/readback pipelines have run on 2/3/5/7 GPUs. These paths do not need
another compiler proof to run. They also do not authorize arbitrary kernels.
See [the scoped hardware evidence](runtime-multi-device-qualification-v1.md).

Do not use the generic hardware-smoke lane on the shared MI300X host: its KFD
examples select all devices. Select explicit freshly observed free GPU identities,
bound each owned process, preserve outputs, and clean only owned resources.
No resets, disruptive fault injection or exclusive-performance claims.

## Completed Native Capability

The existing public API now supports this pipeline:

```text
A --ordered segment lists--> B --ordered segment list--> C --whole-frame D2H
```

The B-to-C list reads B's exact latest retained destination frame without a host
join, including initialized gaps outside the earlier list envelopes. The
[eight-case native checkpoint](evidence/dev-frame-segments-forward-2026-10-03/README.md)
passes prequeued and late admission, overlap and packet-tail shapes, and reversed
routes on MI300X GPUs 1/6/7. The complete runtime suite passes 2352 tests with
32 hardware ignores. This closes this functional work item, not A3.

| Lane | Scope | Acceptance |
| --- | --- | --- |
| Context and journal | Separate default-false capability; distinct frame-source origin; existing immutable frame receipt; generic source-dependency accessor without changing compute-producer meaning | Exact latest event, owner, allocation, envelope and dependency rank; source lease covers the bounding envelope; reject missing/stale events and pending destination writers |
| Native backend | Generalize existing frame-source transfer identity from scalar window to window or immutable segment plan; reuse existing list queue, packet plan and progress | Authenticate exact plan and paired ancestry; wait for successful parent completion and both original owner restorations before successor publication; preserve successor destination-frame receipt |
| Qualification and review | Extend existing Context/backend forwarding fixtures and three-GPU destination-segments witness | Final-readback-only progress; full output and guard bytes; exact callbacks/native counts; released event custody, cancellation/failure and explicit cleanup |

The first profile deliberately excludes a pending destination writer and pending
compute consumption of the new frame-derived list. Its Frame source origin does
not inherit compute-origin ordering permission. It retains whole original owners,
not per-descriptor owners. Parent quiescence or failure is not successful input
readiness. Unknown effects remain retained and cannot authorize retry.

Fifteen new CPU tests cover immutable descriptors, source-envelope gaps,
plan/owner/rank drift, released events, bounded linear ancestry validation,
failed/cancelled/Unknown parents and queued-reader rollback. The full suite also
retains unwind quarantine and independent-pair controls. Native cases use two
changed-content rounds and whole-C readback. Existing descriptor arithmetic proof
bodies are unchanged; they do not prove the new Context/backend adapter.

Relevant implementation boundaries are `context/peer_segments/custody.rs`,
`context/versions/producer_readers.rs`, `context/versions/submissions.rs`,
`kfd_backend/peer_frame.rs`, `kfd_backend.rs` and
`kfd_backend/compute_xgmi/segments.rs`. Existing fixtures are
`context/tests/compute_peer_tests/segments/forward_segments.rs` and
`kfd_backend/compute_peer/frame_segment_tests.rs`.

## Next: Application Admission

Advance the first ordinary application kernel through the existing production
constructor, then qualify compute -> native peer -> compute/readback across
selected GPUs. Genuine Rust extraction,
owner-bound captured KIR and shared argument-basis/fold proofs now exist for a
bounded checked-u32 prefix. The actual borrowed source-statement normalizer now
also has a shared-executable exact acceptance and denotation proof, with checked
schema/getter correspondence and explicit irrelevant-payload erasure. See the
[normalization checkpoint](evidence/dev-source-normalization-2026-10-03/README.md).
These proofs do not cover a complete kernel.

The [source-assembly checkpoint](evidence/dev-source-assembly-2026-10-03/README.md)
now composes the actual retained span scan/walk with source normalization and
pre-add AST/fold denotation. Its bounded private ordinal vector replaces the
source tree map, making the scan/walk linear. Terminal AST evaluation is separate.

The [KIR assembly checkpoint](evidence/dev-kir-assembly-2026-10-03/README.md)
now proves actual borrowed Constant/CheckedAdd assembly and direct evaluation.
Sparse V8 IDs map directly to origins, removing redundant dense constant scratch
and folding. Exact acceptance preserves original rows, typed outputs and the
immediately preceding literal identity. Terminal source AST validation and ABI
discovery remain open.

The remaining semantic path includes ABI discovery, terminal source validation and
semantic-to-machine entry/continuation/memory/completion evidence, protected invocation custody and
the concrete deployment-approved Worker V3 verifier/refinement providers. Do not
replace these with qualification metadata, caller digests or an always-allow
backend. Affirmative Worker provider implementations remain test-only. Bound the
first complete profile to a single-workitem u32 transform storing its value and
overflow; it still needs physical entry, output-store, termination and exact
LLVM/HSACO correspondence before an application launch can be admitted.

## Required Multi-Device Bridge

Preparation, DATA adoption, generated issue and completion now share their
Context bodies across single-device and multi-device backends. Production
application admission and an authenticated output-to-peer composition still
need end-to-end qualification.

1. Completed: route nonexecuting preparation through the exact retained
   multi-backend child, preserving Context generation, logical device, backend
   UID, native admission, selected-child exclusion and pre/post callback
   currentness. Generic async preparation tickets and cleanup are reused.
   The [two-GPU preparation checkpoint](evidence/dev-multi-preparation-2026-10-03/README.md)
   passes both device orders on MI300X GPUs 6/7, direct reverse-order validation,
   callback errors, plain-ticket reservation rejection and owner-thread disposal.
   It creates no VM, queue, allocation or execution authority. CPU acceptance is
   2359 runtime tests (32 hardware ignores), 170 example tests and 71 doctests.
2. Nonpublishing reservation -> shell registration -> DATA adoption -> retirement
   is implemented with paired immutable global/child-local plans, private
   generated allocation routes and the existing move-only commit owner.
   Readiness and quarantine capture the exact child; routing capacity is reserved
   before transfer. Pristine adopted DATA blocks peer owner extraction and
   coherent capture even before submission indexes exist. The
   [two-GPU DATA checkpoint](evidence/dev-multi-adoption-2026-10-03/README.md)
   passes native adoption, independent retirement and primary-lane rebound in
   both device orders. This is finite native mechanics, not protected Worker
   application admission.
3. Generated issue/completion routing and owner-thread activation are implemented.
   A private global submission map retains the original child, shell and local
   receipt, including malformed returned identities and unwind. Generic
   poll/wait/drain cannot publish Ready work or report physical completion as
   delivered output. Retired DATA can release its exact receipt before shell
   disposal. The [two-GPU issue checkpoint](evidence/dev-multi-issue-2026-10-03/README.md)
   passes three native dispatches, full four-buffer readback, independent
   retirement and primary-lane rebound per device order on MI300X GPUs 6/7.
   Context/async qualification remains CPU bookkeeping and rejection evidence,
   not a protected Worker positive-path launch or a new adapter proof.
4. Completed-value staging now preserves the original charged result and uses a
   complete ordinary HostVisible write, settled upload and PUBLIC peer. The
   [staging checkpoint](evidence/dev-result-staging-2026-10-03/README.md) passes
   synthetic charged host data through real uploads and native XGMI in both
   directions on GPUs 6/7. CPU tests cover the shared receipt-matching body,
   encoding and failure custody; this is not a protected Worker positive launch
   or authenticated native completion-to-peer witness. Generated allocation slots
   remain distinct from PUBLIC allocations.
5. Qualify the complete admitted compute -> native peer -> compute/readback path
   on freshly observed free devices. Existing finite native witnesses do not
   substitute for this application-admission gate.

### Next Integration Packet: Host Front Door

The normal authenticated helpers in `fe2o3-host/generated_runtime_invocation.rs`
still accept only the single-device backend. Add multi-context counterparts for
`prepare_generated_context_invocation`, its async variant, and the returned
invocation's `validate_context`. Preserve existing signatures and reuse the same
`require_runtime_evidence`, `prepare_context_payload` and `project_persistent`
bodies. The retained invocation and preparation future are already backend-neutral.
No public backend trait or private-storage extraction is needed.

Missing protected evidence must still reject before Context access, argument
callbacks, result-budget work or queue admission. Keep sync preparation inert;
the multi async path can reuse the existing reservation/adoption/issue/completion
hooks and original decoder. Compile both public entry paths and retain rejection,
ownership/privacy, exact-device and currentness tests. This removes an API
restriction for genuine protected authority; it does not supply the still-missing
production positive verifier/refinement providers.

For owner-driven staging, authenticate/encode on the caller and enqueue only
scratch plus ordinary staging handles, retaining the original output if enqueue
rejects. Require successful H2D completion before ordinary peer admission; pending
H2D writers are not peer producers. A dropped observer or timeout is not physical
quiescence. Scratch is caller-owned outside the result-credit budget.

Staging creates a new ordinary host-write version, not the original generated
allocation or pending generated producer event. Generated DATA is coherent
HostVisible storage with no native SDMA-promotion bridge. Zero-copy ownership
transfer needs separate typed transition, accounting and failure evidence.

## Deferred Work

Do not place eight-device coverage, two-host distribution, broader collectives,
same-process device reopen, more benchmark wrappers or performance tuning ahead
of these functional paths. Post-arm hardware fault injection requires isolation;
retain CPU rejection/retention tests in the native packet now. Physical overlap,
matched HIP/HSA speedups and A3/A7 exits remain separate acceptance gates.
