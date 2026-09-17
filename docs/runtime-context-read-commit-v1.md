# Context Reader Commit Contents V1

V4-J3 is a conditional contents-correctness development packet. It composes the
unchanged V4-J2 ordered preflight with executable acquisition and release loops.
It does not advance the accepted native runtime milestone or close A1/A2 or #182.

## Contract

The proof includes the exact pinned `context_read_preflight_v1.rs`, which imports
the exact pinned V4-J1 journal definitions. There is one nominal instance of each
type; the qualified preflight source, checker and contracts remain unchanged.

Acquisition requires reader/allocation extent agreement and a request length
representable as `u64`. Only **if preflight succeeds**, the selected reverse free
suffix must contain distinct slots. This extra premise is necessary: preflight
checks selected-slot vacancy, but deliberately does not detect duplicate free
indices in an unreachable, corrupted arena. Rejections do not need uniqueness.

On success, the proof establishes:

- The complete base journal is unchanged.
- Free slots are popped from the original tail in request order.
- Each selected lease contains exactly its request, consumer, physical slot and
  `before.next_incarnation + request_index`.
- Every reader count increases by that allocation slot's exact multiplicity.
- Every output entry contains the corresponding installed reference.
- The next incarnation advances by exactly the batch length, without overflow.

The installed-output theorem proves that returned references name their exact
new leases and belong to the nonzero newly minted interval. This does not by
itself establish historical freshness against an arbitrary malformed prestate.

Release needs only reader/allocation extent agreement in the composed execution
contract. Successful exact lookup and canonical ordering imply distinct released
lease slots; uniqueness is proved, not assumed. Each selected lease is cleared,
its allocation's reader count is decremented exactly once, and its slot is
appended in caller order. Unselected lease contents are retained. The complete
base journal and next incarnation remain unchanged.

Both composed operations return the exact ordered preflight result. On every
rejection, all modeled contents are unchanged; acquisition also preserves the
entire caller output. Neither commit introduces a new recoverable error after
successful preflight.

## Proof Structure

Recursive sequence multiplicities provide exact prefix count equations for the
loops. A canonical-order induction relates each scan's same-local group to its
prefix. Exact allocation lookup proves that equal physical allocation slots
have equal local IDs. Per-slot multiplicity is therefore bounded by the group
count even if a malformed base journal aliases one local ID to different slots.
The bridge does not assume global allocation-ID uniqueness.

The executable loops follow production `context_read_leases.rs`: acquisition
pops, installs, increments and writes outputs before advancing the incarnation;
release reads/takes a lease, clears it, decrements and pushes. Proof-side vector
stores model contents, not allocation failure, physical addresses or panic
behavior. The extra length observations only expose finite Rust index bounds.

The current whole-crate count is 127 obligations: 103 inherited and 24 new.
`check-read-commit.py` admits only the exact initial pinned include, audits the
remaining commit body with the unchanged general source policy, and recursively
authenticates/audits preflight and J1. No global include exception is introduced.

The campaign requires two exact 127/0 positives and 15 reversible executable
body mutations rejected with exactly 126 verified obligations and one intended
postcondition error. Mutations skip commits or corrupt journal, incarnation,
free-list, reader-count, lease, output or rejection-frame contents. Compiler,
VIR, timeout, unrelated, extra or mislocated diagnostics are not accepted.

Two added Rust tests compare exact batched contents, partial-release order,
LIFO reuse and storage identity, and exercise the last admissible incarnation
batch followed by irreversible exhaustion. These are production witnesses, not
a mechanical refinement theorem.

## Remaining Obligations

- Reachable free/occupied partition and reader-count invariant preservation.
- Constructor-to-commit composition and interaction with enrollment, writer
  membership, settlement, retirement and Unknown disposal.
- Historical incarnation freshness, rather than only the current mint interval.
- Production Rust refinement, actual vector capacity, nonallocation and unwind.
- Authentication of quiescence evidence and observed free capacity. The latter
  remains an external preflight observation, not a proved physical reservation.
- Ordinary/generated Context composition, initialized inputs, native custody
  and machine-code execution.

No new GPU benchmark or HIP/HSA parity claim follows from this proof packet.
