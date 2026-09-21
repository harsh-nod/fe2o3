# Production Retained Admission

Development qualification of allocation lookup, writer-header lookup, retained
member validation and bounded chain traversal on the actual journal declarations.
This advances gate 1, not registered proof-runner acceptance, full-wrapper
refinement, native admission or HIP/HSA parity/performance acceptance.

## Scope

`context_journal_retained_execution_v1.rs` includes the unchanged production
declarations and value/content/error views in its own production namespace. Its
four executable admission helpers expand the same `retained_bodies.rs` macros as
ordinary Rust, using the actual private production entry types. Runtime helpers
keep their original arities; no model-only ghost parameter is added to a shared
call. The explicit key comparison bodies are also verified on actual types.

Each helper unconditionally returns the exact embedding of a decision over the
production journal's ordered sequence view. There is no valid-state or historical
`Vec` existence premise. Separate bridge lemmas prove equality with the existing
historical decisions for every historical journal; the recursive scan bridge is
proved by induction on its remaining count. A conditional chain wrapper composes
those bridges with the established `represents` relation. Exact successful entries
and errors are preserved, not just success/failure classifications.

The contracts preserve intentionally weak raw behavior: an empty chain accepts
any writer reference; a nonempty chain does not independently authenticate a
writer header; header lookup returns malformed head/count payloads; allocation
lookup does not validate device, extent or pending backlinks; member lookup leaves
the next link for subsequent traversal. These are internal helpers, not public
authorization guarantees. No production runtime behavior was changed.

## Qualification

- Two default-limit whole-crate runs: **465 verified, 0 errors** each.
- Ten scoped mutation runs in the directly affected production helper: eight
  **0 verified, 1 error** and two chain controls **1 verified, 1 error** (the loop
  obligation passes). Controls alter allocation context/error, header context and
  Unknown admission, member writer/backlink/epoch/lineage, chain rejection error
  and terminal-tail admission. These are not whole-crate negatives. The unchanged
  helper contracts are assumed at calls, so each mutation targets its own body.
- **855 unit tests**, two ignored, and **27 doctests** passed. Formatting and Clippy
  with all targets and warnings denied passed. Five added CPU tests cover raw
  acceptance boundaries, rejection without mutation and shared-source wiring.

Counts overlap earlier packets and are not coverage ratios. The new root reuses
the 398-obligation historical settlement root and the declaration/view proof
files; it does not include the earlier private production module itself.

The checker authenticates new inputs plus the complete inherited staging policy,
uses the pinned 190-file Verus closure, brackets all source/tool identities and
records owned-process completion. Negative qualification requires exact typed
postcondition and executable-exit spans, including the complete macro expansion.
Compiler errors, partial positives and unrelated failures do not qualify.

`SOURCE` binds the packet to a signed source commit. The offline auditor rebuilds
each staged candidate from Git objects and checks exact rosters, manifests,
commands, diagnostics and CPU receipts. Selftests reject changed source inputs,
altered diagnostics and rehashed corrupt packets after checking the pristine
packet. Runs began in a development worktree; subsequent source binding is not a
claim that they ran from a clean signed checkout. Hashes alone do not prove execution.

## Reproduce

```sh
python3 -I -B docs/evidence/dev-journal-retained-execution-2026-09-21/check.py \
  --repo "$PWD" --verus /path/to/pinned/verus --output /new/owned/proof
python3 -I -B docs/evidence/dev-journal-retained-execution-2026-09-21/selftest.py \
  --repo "$PWD" --proof docs/evidence/dev-journal-retained-execution-2026-09-21/proof \
  --packet docs/evidence/dev-journal-retained-execution-2026-09-21
python3 -I -B docs/evidence/dev-journal-retained-execution-2026-09-21/audit.py --repo "$PWD"
```

Offline auditing needs Git objects and this packet, not the original solver,
temporary paths, build artifacts or GPUs. New runs have their own path identity
and cannot replace retained receipts.

## Boundaries

This slice is read-only admission. Actual-typed Unknown and settlement mutation,
constructor/enrollment, Begin, reader admission/release and unread guards remain
open. Physical vector capacity, pointer identity, allocation, unwind behavior,
test-only instrumentation and derived trait semantics remain outside the content
contract. Public status/evidence wrappers are not newly mapped here.

Context event/member binding, independent producer-result custody, success-gated
backend support, bounded producer-first progress and native pending XGMI testing
remain open. Pending native consumers remain fail-closed. No GPU or performance
test ran for this packet. See the
[producer-read roadmap](../../runtime-producer-read-reservations-v1.md).
