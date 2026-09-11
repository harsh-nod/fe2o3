# Persistent Bind Settlement V1

R90 implements CONTROL-3A/B/C/D above the
[R89 preparation owner](runtime-fixed-dispatch-preparation-custody-v1.md).
It covers initial single/three-binding persistent construction and single-binding
retained-control replay. The
[local evidence](evidence/local-r90-persistent-bind-2026-09-11/README.md)
separates CPU/source acceptance from Linux execution and formal refinement.

## Loan Settlement

The production-used `execute_live_model_custody_v1` catches opening, operation
and closing retake independently. An opening error runs neither operation nor
retake. An opening panic poisons before resuming. Successful opening is followed
by exactly one retake, including when the operation panics. Operation panic or
retake error/panic poisons before return or unwind. If both panic, the original
operation panic wins. Secondary error/panic payloads are deliberately forgotten
on that terminal path so their destructors cannot replace the original panic.
This is retention, not cleanup or recovery authority.

Ordinary result/error precedence is unchanged. The helper does not protect an
arbitrary owning return value from a later retake panic: these bind callers keep
all native owners outside the closure and return only status. Generic constructor,
SDMA and consuming release outputs are not qualified by this helper.

## Initial Bind

Both cardinalities borrow original allocations during early mapped-facts checks
inside a common catch boundary. Prepared-use authority remains externally rooted.
Error/panic settles exact entries without granting publication. These immutable
record checks are not native-currentness syscalls.

Preparation and later dispatch-memory validation share an outer catch boundary.
Validation borrows `preparation.completed()` after a successful model round trip;
it does not extract the completed dispatch. Failure or panic retains the entire
preparation, prepared entries and generation in terminal queue custody. Only
successful validation permits extraction and installation.

Installation is a nonfallible commit under exclusive queue borrowing. Public
ingress rejects an occupied attachment; three-binding ingress also rejects an
existing dispatch. Single-binding ingress takes an existing dispatch directly
into replay and returns, leaving the initial-construction path empty. Intervening
callbacks receive memory, not queue slots. Final commit moves owners, clears
non-owning bookkeeping and builds a bounded inline attachment; no native call,
allocation or fallible validation follows the move. There is no claimed recovery
from arbitrary injected slot mutation or process-aborting allocation failure.
Tests preserve the exact existing roster and incoming allocation on occupied
ingress rejection; source guards check the final commit shape.

For admitted self-owned inputs, cancellation allows retry only while the queue
remains healthy. A closing-retake failure followed by successful local cancellation
still returns opaque process-teardown custody. Compatibility is preserved at
foreign ingress: the existing single-binding path returns the foreign input
before receiver terminal handling; the existing three-binding terminal-first
policy is unchanged.

## Retained-Control Replay

The existing Request/Storage/Data/Attached payloads now occupy one external
in-place phase owner. Pipeline callbacks borrow it and return only status.
Each fallible check happens before moving the relevant owner; a successful move
immediately installs the next phase without callbacks or allocation. The same
phase owner survives closing retake. Panic settlement retains the original
request or exact detached/attached native partition and poisons before resuming.

Data retention validates replay state, predecessor/control identity, layout,
initialization and native mapped record while borrowing the original data slot.
The existing dispatch's retained vector capacity is checked before taking data.
The final move preserves the original initialization descriptor and exact native
authority, without reconstruction or a new allocation. Rejected retention leaves
the original data in its slot. No second allocator, planner or authority is added.

Replay still uses one model loan and the existing operational currentness audit.
It does not rebuild code/kernarg control or add lifecycle-currentness checks or
the stricter R86 whole-roster pool-admission predicates to this existing route.

## Evidence Boundary

CPU matrices exercise production preparation and retention helpers with actual
token owners, fake-native records, N1/N2 charges, descriptors and control/generation
identities. Retake and final-validation outcomes are scripted adapter-boundary
callbacks, not Linux foundation round trips. Replay completion is model-only;
unchanged fixture bytes do not establish a GPU result. Separate early-check and
cancellation tests use the real persistent-use ledger with synthetic native
leases. Phase-sequencer tests and source guards are supplemental, not a second
native implementation or a substitute for hardware qualification.

NATIVE-2 primary/auxiliary/replacement constructor custody is next. Complete
native adoption, issue, runtime completion, generated API/graph/drain, budgets,
unreturned backend mappings and teardown/unmap/release custody remain open.
No new Verus theorem, executable-refinement proof, Linux execution, performance
gain, A1/A2 completion or HIP/HSA parity is claimed.
