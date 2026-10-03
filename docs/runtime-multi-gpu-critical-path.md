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

The checked-u32 helper lane still needs ABI discovery and terminal source
validation. Complete application admission needs semantic-to-machine entry,
continuation, memory and completion evidence, protected invocation custody and
the concrete deployment-approved Worker V3 verifier/refinement providers. Do not
replace these with qualification metadata, caller digests or an always-allow
backend. Affirmative Worker provider implementations remain test-only. Bound the
first complete profile to the actual `fill_write_only` kernel described below,
not a separate checked-add prefix. Its conditional compiler proof now survives
the singleton handoff and import, but actual packed launch discharge, physical
entry, output stores, termination and exact source-to-machine correspondence
still precede application admission.

The [conditional transport checkpoint](evidence/dev-conditional-fill-transport-2026-10-03/README.md)
retains the signed coverage condition through genuine Rust extraction, V9
handoff, conditional import and target replay. The old unconditional importer
and protected Worker constructor remain unchanged. Next bind actual packed
length and output backing to the selected descriptor and AQL grid, then complete
machine refinement and the protected provider. Only then qualify admitted fill,
tracked upload, native XGMI and readback on two GPUs in both directions.

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

### Completed: Host Preparation Entry Points

The normal authenticated helpers in `fe2o3-host/generated_runtime_invocation.rs`
now include `prepare_generated_multi_context_invocation`, its async counterpart,
and `GeneratedWorkerV3ContextInvocationV1::validate_multi_context`. Existing
single-device signatures remain unchanged. Both variants reuse the original
`require_runtime_evidence`, `prepare_context_payload` and `project_persistent`
bodies, result account and decoder. No new backend abstraction or authority is
introduced. The [host checkpoint](evidence/dev-host-multi-2026-10-03/README.md)
qualifies the public APIs with genuine generated argument types and ownership,
wrong-kernel and privacy compile controls.

Missing protected evidence still rejects before Context access, argument
callbacks, budget cloning or queue admission. The source-wiring checks isolate
each wrapper independently; they are not concrete-KFD execution or a new adapter
proof. Sync preparation remains inert. The async example compiles the existing
reservation/adoption/issue/completion lifecycle while retaining failure tickets.
No new native run or production positive verifier/refinement provider is claimed.

For owner-driven staging, authenticate/encode on the caller and enqueue only
scratch plus ordinary staging handles, retaining the original output if enqueue
rejects. Require successful H2D completion before ordinary peer admission; pending
H2D writers are not peer producers. A dropped observer or timeout is not physical
quiescence. Scratch is caller-owned outside the result-credit budget.

Staging creates a new ordinary host-write version, not the original generated
allocation or pending generated producer event. Generated DATA is coherent
HostVisible storage with no native SDMA-promotion bridge. Zero-copy ownership
transfer needs separate typed transition, accounting and failure evidence.

### Next: One Real Output Kernel

The next acceptance target is one genuinely extracted, lowered and compiled Rust
device-entry kernel, then admitted compute -> native peer -> compute/readback.
Do not add more preparation wrappers or substitute a qualification-only kernel.

The [entry-layout checkpoint](evidence/dev-entry-layout-2026-10-03/README.md)
captures genuine Rust `fill_write_only` through normal KIR V9/LLVM lowering and
ROCm machine-code emission. Its exact function is 68 bytes and 14 instructions;
trailing NOPs are outside the function symbol. The new HSACO query derives the
scratch-free gfx942 input register locations from the selected inspected
descriptor. This is CPU-qualified descriptive layout, not a formal ABI proof,
machine-value relation or execution authority.

1. Reuse the existing same-owner ABI/descriptor transition. The normal target
   stage already retains semantic/KIR/formal ownership and typed roots; inert
   worker-handoff construction validates the descriptor and embeds it into LLVM.
   The plain LLVM capture stops before that construction. Do not copy those
   fields into another owner or treat standalone diagnostic LLVM as a protected
   handoff. Compose the existing canonical descriptor with physical inspection
   and generated packing for the exact selected artifact.
2. Check the complete real `fill_write_only` source/KIR chain: Index1d -> get ->
   u32 truncation -> guarded write through the original output binding. The
   retained ranked projection now records `ValueAccess`, and exact global-X KIR
   value normalization preserves the u64-to-u32 cast. The generic validator still
   excludes complete indexed-address/operational equivalence.
   Bind actual call spans, predicate `index < len`, output address and stored value,
   and reject all unsupported reachable effects. Use the distinct conditional
   coverage boundary and discharge `N <= G` from actual packed output length and
   AQL geometry; do not promote it into unconditional TotalView coverage. Bind
   the source-output ordinal through descriptor `SliceLengthU64`, never through
   the ranked extent ordinal. A bounded formal-memory witness alone does not
   establish full result initialization. Do not prioritize isolated helper proofs.
3. Prove the captured entry-to-exit machine relation. Inputs are kernarg `s[0:1]`,
   workgroup X `s2`, workitem X `v0`; the new layout query derives, not assumes,
   these locations. Cover the exact kernarg load, shift/OR index construction,
   wait, unsigned comparison/EXEC mask, scaled address, four-byte store and both
   paths to `S_ENDPGM`. In particular, the load captures its base before that pair
   is overwritten, OR equals addition only for local X below 64, and Y/Z geometry
   must not duplicate writes. Pointer validity, checked extent/address arithmetic,
   active lanes and memory visibility/completion remain separate obligations.
   The existing shared per-wave execution proof still accepts projected lane IDs
   and EXEC. Derive them from the inspected descriptor and validated dispatch;
   prove dispatch-wide unique coverage parametrically, without a grid-sized
   simulation or caller-supplied activity mask.
4. Qualify altered parameters/components, offsets/address spaces, owner/artifact
   substitution, changed result/literal, wrong store address/value, missing
   termination and extra effects. Only a complete proved profile and authenticated
   producer can satisfy the existing semantic-machine provider contract. Keep
   partial relations non-authoritative and the production admission gate closed.

Deployment-approved measurements, protected ledger access and rollback policy
remain separate deployment inputs. A complete semantic-machine verifier is
missing implementation, not a configuration toggle. Existing native peer,
adoption and issue witnesses do not substitute for it.

## Deferred Work

Do not place eight-device coverage, two-host distribution, broader collectives,
same-process device reopen, more benchmark wrappers or performance tuning ahead
of these functional paths. Post-arm hardware fault injection requires isolation;
retain CPU rejection/retention tests in the native packet now. Physical overlap,
matched HIP/HSA speedups and A3/A7 exits remain separate acceptance gates.
