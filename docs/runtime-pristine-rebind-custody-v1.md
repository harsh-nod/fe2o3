# Pristine Rebind Custody V1

R105 extends R104's [shared rebind settlement](runtime-ordinary-rebind-custody-v1.md)
to the continuation returned by [pristine abort](runtime-pristine-dispatch-abort-v1.md).
It removes the separate consuming pristine rebind/finish orchestration, without
changing abort/control disposal, public signatures, the queue engine or memory
model. The [local acceptance record](evidence/local-r105-pristine-rebind-custody-2026-09-12/README.md)
establishes the named CPU/shared-sequence matrix, not native execution or new
authenticated formal refinement.

## Ownership And Provenance

Common preflight retains its previous order and error classification. Original
programs, packets and data already occupy the R104 input root. Only after
successful pristine preflight does the root acquire the session continuation
and enter the pristine failure policy. Preparation is rooted before opening
the existing memory-model loan.

Opening failure retains the unconsumed continuation in that root, not in the
usable session. Preparation entry consumes it once. The in-place forwarder
passes `continuation.resume()` directly into preparation, so even rejected
generation creation records failed-Generation custody. No early `?` discards
that state, and no failure manufactures a retry continuation.

Resume preserves the exact next generation and creates a fresh recipe
occurrence. It neither increments as ordinary detached/recycled preparation
does nor grants a recycled receipt. Legal continuation producers admit next
generations 1 through `u64::MAX - 1`; synthetic corruption tests do not expand
that public provenance contract.

## Settlement

Both routes now use one model-loan/retake sequence. Rejected opening runs no
preparation or retake; an opened loan retakes once. Operation panic takes
precedence over closing failure. Otherwise closing panic/error precedes the
operation error. Borrowed live-memory validation runs only after successful
operation and closing settlement.

Partial and completed preparation remain rooted through closing and validation.
Only successful checked extraction installs the dispatch and clears detached
metadata in the existing nonallocating, callback-free commit. Failed Complete
cannot be extracted even if an injected caller suppresses its error and skips
validation.

Admitted pristine rebind errors remain session- and process-terminal, including
opening errors. Ordinary returned errors keep their existing local policy;
pristine preflight does not newly acquire the entered-pristine process policy.
Existing terminal poisoning clears any session continuation. The R104 facade
still records transport monotonically, restores the selected lane first, and
retains the complete parent after callback-panic poisoning.

## Acceptance Boundary

The CPU fixture prepares, aborts and rebinds in the same original memory,
accounts and model foundation. Its control-disposal roster records exact
identities only after successful release and verifies their fully released
tombstones. Rebind ownership checks exclude only those verified identities and
require no further cleanup relative to the post-abort baseline.

The separate facade fixture constructs queue/completion owners in the same
model VM but has `engine=None`. Its injected preparation and transport request
exercise primary and the first two auxiliary vector slots; its actual public
bind is exercised after terminalization, not as a successful native route.
Production routing guards are textual checks, not executable correspondence.

Caller module bytes remain borrowed. Allocator abort and panic-abort are outside
unwind recovery. Original Linux-engine composition, real native abort/rebind,
new lower native fault campaigns, authenticated proofs and matched HIP/HSA
performance remain separate qualification. Insertion and cleanup custody,
generated adoption/publication/completion, versions and aggregate memory closure
remain open roadmap work.
