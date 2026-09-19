# Native XGMI Retirement V1

Development implementation of exact retained custody for directional XGMI queue
teardown. This extends the earlier creation-root work and addresses the separate
retirement gap identified in the host-attribution design. It is not a new runtime
milestone, Rust/native refinement proof, driver fault-injection result, or
HIP/HSA parity or performance claim.

## Ownership

Each `Gfx942NativeXgmiSdmaQueueV1` has a private, inline retirement root. The
existing public `destroy_and_release(&mut self, source, destination)` signature
is unchanged. Live authority moves into that root before the opening full
two-session route validation. One borrowed owner-release implementation is shared
with ordinary SDMA retirement; XGMI retains its exact directional route and both
sessions rather than adopting the ordinary generic-engine profile.

The rooted suffix is opening route validation, source currentness, borrowed
`DESTROY_QUEUE`, retained doorbell unmap, source currentness, all three resource
unmaps, all three resource disposals, and closing full route validation. Native
destroy arguments and result, doorbell progress, and mapped/unmapped/disposed
resource receipts remain rooted through the closing check. Only complete success
clears the root. No cross-call topology cache or validation elision is added.

Errors and unwinds retain that custody and poison the owner. Source quarantine,
destination quarantine, and process-gate poisoning are independently attempted.
The original panic takes precedence over terminalizer panics; otherwise the first
terminalizer panic is resumed. Secondary panic payloads are not dropped because
their destructors can execute arbitrary code. Occupied-root Drop aborts, including
during unwind. Terminal roots offer observation only, never retry or cleanup
authority. Callers must keep both sessions and the queue until process teardown.

## Preflight And Reentry

Pure preflight checks liveness, the route's exact engine, all four roster lengths,
the three resources, and the doorbell. Only a structurally valid live owner can
reject pending ordinary, XGMI, persistent-slot, persistent-record, or uncertain
work without effects. This rejection precedes route/session currentness checks.
Doorbell-error storage preparation is also before the rooted suffix and leaves
the valid owner reusable on failure. Malformed or poisoned owners instead enter
terminal custody without attempting native cleanup.

Queue keys and native IDs are preserved, not independently reauthenticated
against a second identity snapshot. Their private fields cannot be mutated by
safe callers. This preflight is not an arbitrary-memory-corruption detector.

Retirement reentry is inert. Other effectful queue API families check pure queue
state before session/native entry and preserve their existing input-mapping,
request-vector, or ticket failure authority. Batch finish still performs its
required closing validation. The public terminal-retirement query grants no
resource authority.

## Runtime Integration

Explicit shutdown borrows each directional queue in its runtime slot and removes
only a completely retired shell. It preserves reverse-direction order, latches
terminal state before error formatting, and resumes the original panic with the
failed slot and sibling still owned. Runtime liveness and diagnostic extraction
also reject occupied retirement roots.

Runtime Drop additionally catches retirement panics and aborts before any backend
field destruction. This matters because memory sessions precede queue fields in
declaration order. Failure must not reach automatic field drop glue.

## Validation Boundary

CPU fixtures execute the production retirement driver and shared borrowed
release mechanics with real typed fixture resources and anonymous test
doorbells. They cover both admitted route directions, all effectful callback
errors and panics, all eleven native memory-cleanup failure prefixes, malformed
owners, five pending classes, mutated destroy arguments, incomplete cleanup,
closing failure after complete disposal, hostile terminalizers, original panic
identity, inert reentry, and subprocess Drop aborts. Runtime helper tests cover
slot identity, sibling preservation, reverse prefixes, and latching before
hostile error formatting. Source-wiring tests guard public entrypoints and the
runtime Drop envelope; these are not live-session execution tests.

Live driver failure injection, machine-code/native refinement, stress recovery,
matched HIP/HSA benchmarks, and performance acceptance remain separate work.
