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
bounded checked-u32 prefix. They do not cover a complete kernel.

The remaining path is actual ABI/statement normalization, semantic-to-machine
entry/continuation/memory/completion evidence, protected invocation custody and
the concrete deployment-approved Worker V3 verifier/refinement providers. Do not
replace these with qualification metadata, caller digests or an always-allow
backend. Qualify one complete bounded kernel profile before widening the language.

## Deferred Work

Do not place eight-device coverage, two-host distribution, broader collectives,
same-process device reopen, more benchmark wrappers or performance tuning ahead
of these functional paths. Post-arm hardware fault injection requires isolation;
retain CPU rejection/retention tests in the native packet now. Physical overlap,
matched HIP/HSA speedups and A3/A7 exits remain separate acceptance gates.
