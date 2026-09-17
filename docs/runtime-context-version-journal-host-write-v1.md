# Context Journal Synchronous Host Writes

This V6 development increment extends the opt-in V5 allocation journal. It does
not close V5/V6, A1/A2, #182, protected native composition or HIP/HSA parity.
Accepted checkpoints remain Native R125 CPU/test, Admission R118B C1-C3 and
Resources R116/V3. No new solver proof, GPU qualification or performance claim
is supplied by this increment.
CPU qualification is recorded in the
[development receipt](evidence/dev-v6-host-writes-2026-09-17/README.md).
The later [async writer extension](runtime-context-version-journal-async-v1.md)
adds ordinary submission settlement; the historical receipt's scope is unchanged.

## Writer Boundary

Only `RuntimeContextV1::write_allocation` joins this projection. After the usual
Context, allocation and byte-range checks, it checks exact journal membership,
writer capacity and epoch headroom before consuming the genuine Context ID.
The private move-only ticket binds that ID, Synchronous kind, full allocation
reference, device and whole extent. Begin precedes backend entry; dropping the
ticket does not release membership. Writes to another allocation may proceed
while capacity remains; writes to a Pending or Unknown allocation reject.

The exact synchronous SPI result supplies the settlement premise:

- `Ok` advances this projection's content lineage and returns writer capacity.
- `Rejected` burns the attempt epoch but preserves prior lineage and returns
  writer capacity. Its contract requires no device-visible mutation and no
  changed custody from this call.
- `Quiescent` retains Unknown membership and the original allocation owner.
- `Terminal` or panic retains Unknown, seals Context and quarantines that
  allocation's request credits. Secondary bookkeeping failure cannot replace
  the original backend diagnostic or unwind payload.

These are **Contracted SPI premises**, not authenticated machine-refinement
evidence. The safe public backend/codec traits can be implemented incorrectly;
classification alone is not transferable verifier, lease or reuse authority.
Default `open` does not consume new writer IDs or change legacy panic behavior.
The writer-count accessor and cleanup count report metadata only, not completion.

## Disposal Is Not Settlement

An exact one-member Synchronous Unknown can be disposed through explicit
release or cleanup. A private preflight binds the original allocation ID,
backend handle, full record, member and writer. Only backend disposal `Ok`
permits credit refund and joint removal of the allocation/member/writer.
Rejected or Quiescent disposal retains all of them for retry; Terminal or panic
retains them and seals Context. A post-disposal bookkeeping failure seals Context
and never retries the consumed native owner.

Pending disposal rejects before effects. Multi-member or non-Synchronous Unknown
disposal is Unsupported at this production boundary. Cleanup skips such retained
allocations while continuing unrelated ones, and its writer count prevents a
false complete report, including an empty retained writer.

The model's `dispose_unknown` accepts an inert exact full-roster premise. It
validates every reference/device/extent, chain, return-stack capacity and scratch
entry before mutation; commit removes all allocations and members and returns
the writer last. It never makes an allocation available or asserts successful
content. Work is O(k), with no allocation or callback, assuming a valid global
prestate. Writer issuance history stays burned; the model does not tombstone
disposed allocation keys. Context supplies fresh nonwrapping allocation IDs.
Model multi-member support does not authorize production partial disposal:
that needs retained successful-prefix receipts and retry progress.

## KFD Classification

The ordinary KFD host-write tail no longer returns Rejected for an impossible
range mismatch after upload. The range is checked before effects; violation
afterward is an invariant panic, handled as Unknown/terminal by the journal path.
Missing XGMI authority after successful unmap now seals that backend and returns
Terminal, for both read and write. Genuine pre-effect unmap rejection is unchanged.
Scripted copy tests and source guards are not native XGMI qualification.

## Remaining Integration

Launches, copies, generated DATA adoption, failed disposals that mutate bytes,
completion/cancellation and backend aliases still need complete writer hooks.
Ordered writers, input leases, aggregate residency, content recovery and native
qualification remain open. No public lineage query or cross-run reuse API is
exposed. The existing V4-J1 issuance proof is unchanged and proves neither the
new disposal transition nor this Rust adapter's correspondence.
