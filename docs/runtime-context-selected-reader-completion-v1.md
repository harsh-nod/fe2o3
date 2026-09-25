# Selected Context Reader Completion

## Production Change

Joint ordinary completion validates the entire stable-reader roster and then the
entire producer-reader roster before either journal effect. Previously, the
stable adapter repeated its full validation after that preflight, with no
intervening mutation or callback. A private prevalidated adapter now removes that
redundant scan. Generated-domain release still validates its complete roster and
domain before invoking the same raw effect adapter.

Both reader classes use `completion_selected_reader_release_body!` to:

1. Select the retained root by submission identity.
2. Derive the consumer from its marker and pass its exact reference slice to the journal.
3. Return immediately on journal error, without removing the root or clearing its marker.
4. Run the existing test-only post-effect fault hook.
5. Remove that root and clear only its corresponding marker in an existing submission record.

There is no new persistent token, allocation, roster copy, fallback, or runtime
map. Record lookup remains `get_mut`, including initial rejection before record
publication. Panic catching, payload retention, quarantine, stable-before-producer
ordering, and late writer validation remain in their existing ownership layers.

The optimization removes one linear stable-roster validation, not the journal's
own checks. Overall complexity remains linear in the input roster size. No
latency, throughput, HIP/HSA ratio, or hardware-overlap improvement is inferred.

## Proof Boundary

The successor proof includes the same field-level release body and input-order
body as production. Executable selected-entry projections contain actual reader
references, optional record markers, and the actual/logical journal pair.
Logical reference vectors are generated from the selected actual roster inside
verified executable functions; callers do not supply a matching second roster.

The release contracts establish exact journal correspondence, root removal and
marker clearing on success, root/marker preservation on journal error, unchanged
opposite-reader ownership, and writer-marker preservation. Composition retains
the stable-success prefix when producer release fails. Missing roots and missing
submission records remain separate cases. Constructor-origin witnesses exercise
all optional-root/record combinations and corrupt stable/producer references;
the corrupt-reference cases test the adapter contract, not reachability through
the complete Context prevalidator.

This is **not full Context executable refinement**. The remaining boundaries are:

- `std::HashMap` selection/removal and framing of other map entries.
- The full Context prevalidator, generation/domain/source-allocation binding,
  and the uninterrupted handoff into the private prevalidated adapter.
- Actual allocator capacity. The proof supplies the journal arena length as a
  storage bound; production observes `Vec::capacity`. Constructor preallocation
  correspondence and malformed-roster capacity/error precedence remain outside
  this result.
- Post-effect panic handling, quarantine, dependency retirement, callback/status
  publication, and late writer settlement. CPU tests exercise these boundaries;
  they do not convert them into theorems.

No new `assume`, `admit`, or `external_body` is introduced. The historical
completion-effect checker and its accepted source pins remain unchanged; this
successor has separate development evidence and negative controls.

## Validation

See the [development packet](evidence/dev-selected-reader-completion-2026-09-25/README.md)
for commands, results, rejected attempts, and cleanup. A1/A2, #182, and the broader
Native R125, Admission R118B C1/C2/C3, and Resources R116/V3 acceptance checkpoints
are not advanced merely by this extraction.
