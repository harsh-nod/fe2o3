# V6 Generated Writer Journal Integration

Development above `d91e470f15bc2c42078842d983a4bb2f36ed4a8c`, not A1/A2,
issue #182, native protected execution or HIP/HSA parity acceptance. Native R125
CPU/test, Admission R118B C1-C3 and Resources R116/V3 remain accepted.

Subsequent development adds [generated ReadOnly input leases](runtime-context-generated-read-leases-v1.md).
Those independent domain-bound readers settle before this writer boundary;
the original qualification archive below remains historical writer-only evidence.

## Begin And Observation

The opt-in Context journal now tracks protected generated mutation attempts.
Inside the existing opening source-currentness callback, ISSUE pairs every
original shell member with its corresponding authenticated roster ordinal.
WriteOnly and ReadWrite members enter one retained writer before backend
preparation; ReadOnly members do not. This includes writable buffers that have
no pointer fixup. No replacement allocation or submission identity is created.
All-read-only and journal-disabled attempts have no journal writer.

Preparation acquires storage before consuming the submission ID or performing
Begin. The generated attempt, optional submission record, independent writer root
and private completion receipt bind the same writer. Its domain additionally
binds stream, unpublished hold and shell key. Ordinary writers cannot settle a
generated root, and held-stream generic poll, wait, drain, cancel, release,
event creation and synchronization reject before backend entry. Owner-only
generic drain also checks this exclusion locally.

Generic KFD completion can establish physical quiescence without protected
readback. It must not publish Success or irreversibly change this writer to
Unknown before the protected completion path runs. Pure status queries and
completion callback registration remain available.

## Completion And Stop

Protected completion obtains the original readback, validates the result,
retires native DATA, releases the native submission and finishes closing source
currentness. Only the private exact settlement receipt then permits writer
Success. Successful model settlement precedes shell retirement and hold release;
the status callback is published afterward. Typed output still requires its
original decoder. NoEffect is not a generated settlement outcome.

Stop never reissues. Pending native work retains its entire custody. After the
existing lower-layer retirement premises hold, the Context tail marks the
generated writer Unknown and publishes QuiescentWithoutResult. Before the
single backend shell-disposal operation, it validates every original writable
member and independent read-only retirement member, exact pending markers,
Context/backend indexes and credits. After that complete batch succeeds, it
records every writable disposal receipt, performs whole-writer model disposal,
refunds exact credits and retires the read-only members. It never calls the
ordinary per-allocation backend release on generated virtual handles.

No storage is acquired after batch disposal. A post-model credit failure leaves
the finalizing writer counted, its original receipts retained and Context
terminal. Other errors and panics preserve the original diagnostic and remaining
custody. The public protected ISSUE failure wrapper remains conservative:
an inner pre-effect capacity rejection still terminalizes that outer path;
inner preflight tests do not establish public retryability.

For k writable members, Begin canonicalization is O(k log k), retained storage
is O(k), and exact generated validation is O(n + k log k) for n bounded shell
members. Disposal uses a fixed-capacity ticket and O(n + k) commit work.

## Evidence And Limits

See the [CPU qualification archive](evidence/dev-v6-generated-journal-2026-09-17/README.md).
Tests use genuine Context shell metadata and journal entries, but their native
completion/disposal premise is explicitly assumed. They are not native proof
receipts. Original source/roster bindings, writer markers, mixed/all-read-only/
all-writable membership, lineage before retirement, neighboring attempts,
generic observation rejection, terminal custody and post-model credit faults
are checked. Existing nonjournal generated and ordinary async tests remain.

No native GPU, solver or matched HIP/HSA benchmark is run for this packet.
Protected production verifier/machine refinement, initialized-input authority, aliases,
ordered overlapping writers, cross-run versions, content reuse, aggregate
residency, native high-depth/overlap/failure qualification and performance remain
open. This journal bookkeeping grants no new execution or content-reuse authority.
