# R47 gfx942 striped-SDMA tail wait

R47 changes only the blocking
`ComputeAqlQueueSessionV1::wait_gfx942_striped_sdma_copy_batch_for_v1`
implementation. The nonblocking whole-submission poll and its full observer are
unchanged.

## Native contract

Every admitted ordinary gfx942 SDMA copy submission contains one linear-copy
packet followed by the existing `mtype=3`, `sys=1`, `snoop=1` completion fence.
R47 contracts that a gfx942 SDMA queue executes packet occurrences in submission
order and that observing the exact tail ticket's queue occurrence, slot, and
generation value means every preceding copy and fence on that shard is complete
and system-visible. The runtime checks the packet encoding and exact retained
ticket/record binding. It also requires each bound owner's engine index to equal
`queue_ordinal % 2`, matching the admitted alternating-engine striped topology.
Firmware ordering and CPU/GPU coherence remain external native premises.

## Wait epoch

Binding validates the sealed whole-submission roster and stores one tail ticket
per active shard in fixed bounded storage. No heap allocation occurs after
binding. Each pre-deadline round observes exactly the active tail roster. It does
not scan all request completions.

All tails ready, or expiration of the shared monotonic deadline, triggers one
full ordered request-roster pass. That pass combines exact ticket/status
observation with retirement preflight. A ready tail with a pending prefix is a
terminal contract violation. A pending tail and a pending final audit is a
timeout. The closing operational-currentness check occurs after this pass.

The sole sealed submission owner remains in the public wait frame, outside the
live-memory unwind-catching envelope, throughout every fallible wait, audit, and
currentness step. An all-ready audit produces a lifetime-bound private witness
borrowing that exact submission. The behavior-owned module consumes the witness
and immediately moves the sole owner into an abort-on-unwind retirement suffix.
That suffix moves prevalidated records into the already-reserved output in
original request order and performs no completion observation or
record-validation scan. No prefix is retired on error or timeout.

A panic before that suffix is translated at the consuming public boundary into
terminal process-teardown custody retaining the exact plan, shards, tickets, and
immutable ordered completion roster. Both the queue-local state and the
process-global KFD admission gate are poisoned. Drop is not used to restore this
custody.

The public timeout retains the unchanged whole submission. Calling the public
wait again starts a new native wait epoch and therefore permits one new final
audit. This behavior is deliberately not presented as a refinement of the R46
model's terminal timeout receipt.

## Evidence and exclusions

Host fault-injection tests cover multi-round tail work, ready and timeout
classification, tail errors, pending prefixes under ready tails, identity and
engine substitution, closing-currentness loss, panic custody, local and
subprocess-global poison, and the single full-audit source shape.
The existing exact gfx942 aggregate benchmark invokes this public wait path and
fully validates copied bytes, but R47 does not run it. There is no Rust-to-R46
refinement, hardware validation, or measured performance claim in this tranche.
