# Production Journal Representation

Development qualification of shared journal declarations and their lossless
logical content view. This is not registered proof-runner acceptance, full Rust
operation refinement, native admission, HIP/HSA parity or performance acceptance.

## Scope

Ordinary Rust and Verus now include the same `declarations.rs` tokens. The original
declaration block is unchanged inside the new macro envelope: public/private
visibility, docs, derives, field and variant order, lifetimes, `must_use` and the
test-only access counter are preserved. Rust uses an identity declaration macro;
Verus uses its ordinary declaration parser, not replacement field definitions.

Ghost views map complete writer/allocation/device keys and references, all writer
entry variants, allocation/member/plan fields, and whole-allocation write values.
The journal view preserves all five scalars and seven ordered vector contents.
Inverse laws and independent semantic field/discriminant anchors establish
losslessness without requiring valid IDs, canonical order, equal arena lengths,
unique members, terminal links or clean scratch tails. The relation preserves
duplicates and trailing vacant slots. No ghost sequence is assumed to correspond
to an allocated historical `Vec`; recovery constructs only ghost sequences.

The three historical error domains embed into the actual production error enum
with explicit partial reverse projections. Their union covers 22 of its 23
variants. `StorageAllocationFailed` has no historical counterpart and is never
coerced to another error or declared unreachable. Independent variant anchors
rule out coordinated permutations that would satisfy inverse laws alone.

The existing shared retained writer-key equality and allocation-key ordering
bodies are also verified against the actual production value declarations and
their logical views. This does not prove the compiler-generated `Eq`/`Ord` trait
implementations. Boundary CPU tests exercise those derives separately.

## Qualification

- Two whole-crate positives: **452 verified, 0 errors** each, including 398
  inherited obligations. Counts overlap earlier packets and are not coverage ratios.
- Twelve targeted mutation controls, each **0 verified, 1 error** in its one named
  production-module function. These are deliberately scoped negatives, not failed
  whole-crate runs. Ten alter view definitions; two alter shared executable key
  comparisons. Targets cover writer kind/reference/phase, extent, paired lineage
  swaps, capacity, reversed free-list order, discarded scratch tails, allocation
  failure, paired error permutations and both explicit comparisons.
- **850 unit tests**, two ignored, and **27 doctests** passed. Formatting and
  Clippy with all targets and warnings denied passed.

The checker pins complete new sources and inherited proof adapters, brackets
source/tool identities, verifies the 190-file Verus release closure before/after,
and records owned-process completion. Solvers use default limits, four threads,
no-cheating and a 180-second outer timeout. Exact typed postcondition and function
exit spans are required; partial positives, compiler failures and unrelated
diagnostics cannot qualify. Paired-swap controls specifically target independent
semantic anchors, not merely inverse laws.

`SOURCE` binds the packet to a signed source commit. The offline auditor rebuilds
staged candidates from Git objects, checks exact case/file rosters and manifests,
and authenticates CPU commands and source brackets. The selftest rejects altered
diagnostics, changed inputs and rehashed corrupt packets after first checking the
pristine packet. Development probes and incomplete checker campaigns are excluded.
Runs began in a development worktree; later Git binding is not a claim of execution
from a clean signed checkout, and hashes alone do not prove execution.

## Reproduce

```sh
python3 -I -B docs/evidence/dev-journal-representation-2026-09-21/check.py \
  --repo "$PWD" --verus /path/to/pinned/verus --output /new/owned/proof
python3 -I -B docs/evidence/dev-journal-representation-2026-09-21/selftest.py \
  --repo "$PWD" --proof docs/evidence/dev-journal-representation-2026-09-21/proof \
  --packet docs/evidence/dev-journal-representation-2026-09-21
python3 -I -B docs/evidence/dev-journal-representation-2026-09-21/audit.py --repo "$PWD"
```

Offline auditing needs Git objects and this packet, not the original solver,
temporary paths, build artifacts or GPUs. A new live campaign needs its own path
identity and cannot replace retained receipts.

## Boundaries

The content relation excludes vector capacity, pointer identity, allocator and
unwind semantics, and the test-only access counter. It does not recover ghost
registration history or storage labels. Public status/evidence wrapper semantics
and the separate enrollment declaration are not mapped in this slice.

Production operation preservation of the relation remains open beyond the two
explicit value comparisons. Constructor/enrollment, Begin, whole settlement,
reader admission/release and unread guards still need complete correspondence.
Context event/member binding, independent producer-result custody, success-gated
backend support, bounded producer-first progress and native pending XGMI testing
remain open. Pending native consumers remain fail-closed. No GPU or performance
test ran for this packet; it advances gate 1 of the
[producer-read roadmap](../../runtime-producer-read-reservations-v1.md).
