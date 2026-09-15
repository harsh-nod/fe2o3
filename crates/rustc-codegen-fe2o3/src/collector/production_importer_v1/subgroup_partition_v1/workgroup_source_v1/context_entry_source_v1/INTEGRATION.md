# Exact Context Entry Transport

`ContextEntrySourceV1::check` authenticates no nominal identity by itself. The
production caller supplies the registered root, collector-authenticated logical
helper identity, trusted issuer and concrete Context type. The child checks the
actual HIR initializer binding and its single helper argument use, resolved MIR
call instances, exact erased constant, and must-live issuer dataflow. Replay
checks the same source nodes, boundary-borrow roster and MIR occurrence
coordinates. The only additional uses admitted are direct temporary shared
borrows to the four trusted Global boundary binders (read-only, disjoint-write,
exclusive-read-write and atomic), completed before the unique helper transfer.
The binder receiver type must exactly match the borrowed owner. The only
nonempty receiver adjustment chain admitted is built-in Deref to that exact
owner followed by shared Ref(Not) to the original reference type, comparing
all intermediate/final types after region erasure. The owner itself must have
no adjustments. Names alone, other coercions, aliases, mutable/raw borrows,
custom calls, closures, reassignment,
ambiguous calls and exhausted work fail closed. These reviewed binder calls do
not store the Context reference. Original MIR must-live checking remains active.

`context_entry_transport_v1::attach_context_entry_transfers_v1` maps those rustc
coordinates through the preflight producer tables to canonical coordinates.
Its source-edge commitment includes the exact admitted semantic MIR digest.
Private `AuthenticatedProductionKernelContextsV1` custody retains the optional
record and includes every field in its digest. No semantic body is rewritten.

`ProductionKernelContextEntryTransferV1` and
`ProductionKernelContextLoweringInputV1::with_entry_transfer` are inert APIs.
The lowerer's `KernelContextEntryPlanV1` checks the unchanged owner, exact source
issuer/helper calls and types, replayed ParameterTransfer origin and SSA
definitions, and issuer lifetime. At the one checked transfer assignment it
reuses the original issuer's lowered KernelContext binding. It does not issue
another token or accept arbitrary same-typed ZST constants. Existing Context
borrows, Workgroup/subgroup loans, epoch and occurrence checks remain active.

Pauli's mounted `checked_ssa_relation` API exposes this same checked relation to
the policy consumer; it is not a second independently reconstructed origin.
The parent's earlier pre-admission move-restoration hook has been removed.
No terminal tag, MIR/KIR schema revision, machine authority or launch evidence
is introduced by Context entry transport.

## Verification Boundary

Checkpoint28e: the unchanged real WG callback's 28d diagnostic confirmed the
two-step identity reborrow above; callee identity and source ordering already
matched. The narrow matcher now admits it without accepting other adjustment
kinds. Two new tests in `tests/boundary_uses/adjustments.rs` use observed host
HIR adjustments and reject kind, order, intermediate/final type, overloaded
deref, pin, mutable/two-phase/raw borrow, and pointer-coercion substitutions.
They do not authenticate the local test provider. Production rustc API
typechecking passes standalone; central tests and real AMD reruns are pending.
Verbose temporary HIR diagnostics were removed; rejected use counts remain.

Checkpoint28c narrow correction is mounted in `context_entry_source_v1.rs`,
`tests.rs`, and `tests/boundary_uses.rs`. Four new tests cover the closed binder
set, exact-use accounting, and real host-HIR negatives for custom/lookalike
borrows, aliases, mutable/raw borrowing, reassignment, moved aliases and capture.
The HIR fixture explicitly grants no provider or entry authority. Parent28c
reports these four tests passed, but its unchanged real AMD typed Global and
Workgroup positives still rejected because of the identity reborrow. No Cargo was run by
the worker, and no lowerer, schema or ranked hooks changed in this correction.

The pinned standalone rustc driver checked the unchanged registered AMD source
against exact cached core/device metadata, with production macro/session binding.
Source replay and wrong-helper/type/argument/work mutations passed. The actual
full-import callback now mounts the same source checks plus changed issuer-local
and source-binding replay negatives. Lowerer integration tests under
`kernel_context_entry_tests` cover inert owner/coordinate mutations and retained
single issuance; they are not frontend or machine authentication fixtures.

Parent AMD27f reports the real callback clears Context argument transport and KIR
lowering. Its next failure was AMD target closure treating logical partition16
as hardware wave16. The separately mounted target dependency fix retains the
logical participant count and explicit physical Wave64 requirement. Full AMD
adapter/simulator callback completion still needs central verification.
