# Production Unknown Transition

Development qualification of the shared Unknown mutation on actual journal
declarations. This advances gate 1, not registered proof-runner acceptance,
full-wrapper refinement, native admission or HIP/HSA parity/performance acceptance.

## Scope

`context_journal_unknown_execution_v1.rs` includes the unchanged production
declarations, lossless content views and actual-typed retained admission helpers.
The new executable function expands the same `retained_unknown_body!` as ordinary
Rust, with its original calls and arities. Production runtime behavior is unchanged.

The unconditional contract gives both the exact result and the complete sequence
post-state. A successfully validated Pending header becomes Unknown with the same
key, head and count. Every scalar, every non-writer sequence, writer length and
every other writer entry are unchanged. Rejection preserves all content. An
already-Unknown header also preserves all content, but still revalidates its
chain and returns that validation result. No constructor validity, custody,
canonical global state or historical allocated-Vec existence premise is added.

The historical bridge proves a precise equivalence: the old Unknown relation is
the new sequence relation plus the old identity requirement on rejection and
already-Unknown branches. That identity condition is explicit because equal
sequence views do not establish equality of opaque historical `Vec` values. The
forward historical-to-view implication is unconditional. The reverse implication
does not silently discard identity or claim that arbitrary ghost sequences have
an allocated historical realization. Result/error embedding is proved injective.

## Qualification

- Two default-limit whole-crate runs: **472 verified, 0 errors** each.
- Nine scoped controls: **0 verified, 1 error** each in the named production
  function. Seven change the executable macro: header error propagation, mutation
  on rejected chains, wrong phase, discarded head/count, skipped Unknown-chain
  revalidation and an unrelated scalar write. Two alter the historical identity
  specification, independently omitting rejection or repetition identity.
- Four controls fail at the exact supporting writer-sequence assertion. Five fail
  at the exact postcondition. These are not whole-crate negatives; unselected
  helper contracts are assumed at calls.
- **859 unit tests**, two ignored, and **27 doctests** passed. Formatting and
  Clippy with all targets and warnings denied passed. Four added tests cover
  malformed unrelated writers, matching zero/MAX raw identities and max epochs,
  malformed repeated-Unknown cardinality, and shared-source wiring.

Counts overlap earlier packets and are not coverage ratios. The new root reuses
the historical settlement root and the actual declaration/view/admission files;
it does not nest the previous private production module.

The checker pins new inputs and the inherited staging policy, brackets source and
tool identities, authenticates the 190-file Verus closure and records owned-process
completion. Exact typed assertion/postcondition and exit spans are required,
including macro expansion trees where applicable. Compiler failures, partial
positives, timeouts and unrelated diagnostics do not qualify.

`SOURCE` binds the packet to a signed source commit. The offline auditor rebuilds
staged cases from Git objects, checks exact rosters/manifests and validates solver
and CPU receipts. Selftests exercise all three diagnostic locations: a shared
macro exit, a supporting assertion and local executable/ghost exits. They reject
changed source inputs, altered diagnostics and rehashed corrupt packets after
checking the pristine packet. Runs began in a development worktree; later source
binding is not a claim of execution from a clean signed checkout, and hashes alone
do not prove execution. Development probes are excluded.

## Reproduce

```sh
python3 -I -B docs/evidence/dev-journal-unknown-execution-2026-09-21/check.py \
  --repo "$PWD" --verus /path/to/pinned/verus --output /new/owned/proof
python3 -I -B docs/evidence/dev-journal-unknown-execution-2026-09-21/selftest.py \
  --repo "$PWD" --proof docs/evidence/dev-journal-unknown-execution-2026-09-21/proof \
  --packet docs/evidence/dev-journal-unknown-execution-2026-09-21
python3 -I -B docs/evidence/dev-journal-unknown-execution-2026-09-21/audit.py --repo "$PWD"
```

Offline auditing needs Git objects and this packet, not the original solver,
temporary paths, build artifacts or GPUs. New campaigns have their own path
identity and cannot replace retained receipts.

## Boundaries

Actual-typed settlement mutation, constructor/enrollment, Begin, reader
admission/release and unread guards remain open. Vector capacity, pointers,
allocator behavior, panic/unwind, test instrumentation and derived trait semantics
are outside this content contract. CPU snapshots check pointer/capacity stability
for the exercised cases; they are not universal physical-storage proofs.

Context event/member binding, independent producer-result custody, success-gated
backend support, bounded producer-first progress and native pending XGMI testing
remain open. Pending native consumers remain fail-closed. No GPU or performance
test ran for this packet. See the
[producer-read roadmap](../../runtime-producer-read-reservations-v1.md).
