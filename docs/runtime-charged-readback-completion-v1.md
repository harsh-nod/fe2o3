# Charged Readback Completion

R85 implements the private COMPLETE-HOST-SUBSTRATE after
[R84 generated shells](runtime-generated-allocation-shells-v1.md). It composes
the original R73 charged decoder with the actual R80 staged readback owner.
It does not admit native completion, publish work, add a public async API or
establish that any kernel produced the decoded bytes.

## Closed Ownership

The concrete generated-storage owner has a private consuming
`decode_reserved_readback` transition. A staged readback now retains the exact
result-gate Arc that reserved its overlap credit. Sharing one budget or matching
buffer lengths is insufficient to substitute another invocation's readback.
No extra readback or completion-reply reservation occurs during decoding.

Before consuming original storage, the transition validates the exact gate and
complete source/readback cardinality, access, lengths and destination capacities.
Every read-only byte, including unused buffers, must match the still-live original
source. Physical ordinals include only nonzero buffer expectations. Empty typed
outputs retain their result members without introducing phantom native buffers.
The fixed projection still rejects all-empty native rosters.

The existing charged decode algorithm now uses one private transaction with
either detached data or the complete reserved readback owner. Both variants
provide borrowed byte views; the reserved variant never separates its vector
from its overlap debit. Existing detached charged decoding and legacy decoding
remain separate from native execution authority.

## Disposal And Readiness

1. Validate the complete original/readback join while payload, readback and
   decoder remain in their declaration-order owner.
2. Root the readback owner and decoder in the decode transaction, then destroy
   the original generated payload before decoding can expose a result.
3. Reuse the existing complete-roster shape/custody preflight and decode into
   the original charged typed seeds. A shared gate keeps partial results hidden.
4. Destroy returned buffers, explicitly settle their overlap credit, then settle
   original read-only credits. Any settlement error prevents readiness.
5. Commit the existing result gate once. Writable encoded-plus-typed debits stay
   with the returned typed results until those results are disposed.

The transaction's field order destroys returned storage before decoder custody
on rejection or panic. Failed or abandoned transitions use best-effort Drop
cleanup; that cleanup is not sufficient to report success. Failed core refunds
retain/quarantine their debit under the existing resource-accounting contract.
Timeout, observer drop and returned bytes do not establish native quiescence.

Completion uses no new encoded allocation or typed allocation. Its work is
linear in the bounded expectation roster, read-only bytes checked and writable
bytes decoded, with constant additional ownership metadata. These are algorithmic
properties, not measured latency or bandwidth claims. Account arenas, Arc/Vec
metadata, allocator overhead, native backing and aggregate quarantine remain
outside this host byte-accounting packet.

## Evidence Boundary

Ten new host tests exercise actual R73 storage and R80 readback ownership,
including valid completion, foreign gates on shared/separate budgets, late
read-only corruption, malformed shapes, missing ownership, decoder panic,
dropped observers, read-only-only completion and zero-length output indexing.
The settlement-failure test deliberately quarantines the real overlap credit
before removing its private token. It verifies failure after decoding without
ready outputs, leaving only the deliberate quarantine; it does not induce or
claim core mutex poisoning.

One temporary gate-check mutation makes the otherwise valid foreign-owner test
fail. The original source is restored byte-for-byte before acceptance. This
mutation check and the CPU tests are not Verus verification. The unchanged R73
shape predicate remains used by production decoding; no new theorem or whole
adapter refinement is claimed. Exact results are in the
[local evidence](evidence/local-r85-charged-readback-2026-09-11/README.md).

## Native Handoff

This helper remains disconnected from generated native preparation/completion.
A future caller must first validate the exact invocation, submission, device,
queue and allocation generations, complete readback and closing currentness,
and establish the required conclusive native ownership disposition. Unknown
construction/publication/retake/retirement outcomes retain the carrier, native
owner and charges; they must not invoke host decoding. NATIVE-1/2 construction
custody, DATA-ADOPT, ISSUE and runtime COMPLETE integration remain open.

The host result gate is not the engine's reserved completion reply. The runtime
must still compose both with one exact completed operation. R85 does not close
A1/A2, #182, HIP/HSA parity, hardware qualification or performance acceptance.
