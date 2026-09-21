# Shared Settlement Scalar Storage Admission

Development qualification for a production Rust/Verus shared executable body.
This is not the registered shared proof runner, full Rust refinement, native
admission, HIP/HSA parity or performance acceptance.

## Scope

`settlement_return_body.rs` is included unchanged by ordinary Rust's private
`SettlementReturnStorageV1::check` and the Verus scalar executor. It contains
the same two checked additions and five ordered bounds as the previous inline
settlement preflight. The live Rust callsite captures the two free-list lengths,
their actual `Vec::capacity()` values, the configured logical bounds and scratch
length. Header, evidence and retained-chain validation still precede the guard;
the indexed scratch-prefix scan follows it. The same exclusive borrow protects
preflight and subsequent staging/commit. No new public authority, allocator call,
mutable helper, admission fallback or constructor restriction is introduced.

The guard's failure order is unchanged. Constructing the private scalar struct
eagerly evaluates seven infallible observations; this is not a claim of identical
machine instruction/evaluation order. Unknown disposal and reader release are
not changed because they have different admission paths.

The new Verus root proves the shared scalar result equals an integer-sum decision
including both `usize` overflow boundaries. Logical wrappers compose it with the
existing scratch scan, exact raw preflight/staging/commit, and invariant-conditional
issued producer/read custody. Its constructor/enroll/register/Begin trace executes
empty and two-member settlement for both Success and NoEffect, including a
storage-bound rejection; reader arenas in this witness are empty.

## Retained Checks

The accepted packet contains:

- Two whole-crate Verus positives: **367 verified, 0 errors** each.
- Ten whole-crate executable negatives: **366 verified, 1 error** each, rejected
  at the intended postcondition. Eight mutate the actual included scalar body:
  writer/member checked-add inputs, five independent bounds, and success result.
  Two insert an early rejecting check using the wrong modeled physical-capacity
  observation; the unchanged normal path retains its scratch-loop invariant.
  These two controls isolate spurious rejection, not every possible mapping bug.
- **826 unit tests passed**, two ignored, and **27 doctests passed**; formatting
  and Clippy (`--all-targets -- -D warnings`) passed.
- An independent widened-`u128` scalar boundary oracle: **6,996 cases**, comprising
  819 accepted and 6,177 rejected. Each logical/physical bound and scratch length
  varies independently, including checked-add overflow and exact boundaries.
  Extreme scalar values do not imply those sizes can be allocated as arenas.
- Existing full-journal corrupt-state, storage-identity and indexed-access tests
  continue to pass. A source-shape regression checks the production observations,
  guard ordering, single callsite, unchanged scratch scan and allocation-free body.

The 360 inherited obligations overlap the preceding settlement execution packet.
Do not sum inherited counts across packets or interpret counts as coverage ratios.

## Evidence Custody

The solver compiles the real included macro, not an expanded replacement algorithm.
Each case preserves the nested `verus/../src/context_version_journal/` layout.
The checker pins the prior roots/checkers, shared root and body, and authenticates
the exact macro envelope before scanning its interior with the pinned proof
policy. The ordinary Rust adapter, callsite and source-shape tests are captured
in the proof input bracket and the full CPU source bracket.

Macro negative diagnostics must identify the exact wrapper postcondition,
macro body exit, invocation, macro name and definition site. Byte ranges, line and
column coordinates, excerpts, highlights, labels, expansion nesting and JSON types
are checked. The ordinary non-macro classifier remains unchanged. Partial runs,
compiler failures, timeouts, extra errors and malformed reports are not accepted.
Source/hash brackets, frozen completed-case receipts and before/after pinned
190-file Verus release-closure measurements accompany the campaign. Default
solver limits, four threads and the 180-second outer timeout are retained.

`SOURCE` names the signed source commit. The portable offline audit reconstructs
all generated proof inputs and mutations from Git objects, rechecks diagnostic
classification and exact command/process receipts, checks the complete packet
manifest and ties CPU source hashes to that commit. Hashes alone are not evidence
of execution. The runs began in a development worktree; source identity is checked
afterward, not represented as a clean signed-checkout run.

`selftest.py` rejects changed result summaries, macro expansions and nested spans,
source injections and rehashed corrupted packets. Interrupted development attempts
are excluded from this packet. Only complete terminal receipts are accepted.

## Reproduce

From the repository root, use the pinned Verus release:

```sh
python3 -I -B docs/evidence/dev-shared-settlement-storage-2026-09-21/check.py \
  --repo "$PWD" --verus /path/to/pinned/verus --output /new/owned/proof
python3 -I -B docs/evidence/dev-shared-settlement-storage-2026-09-21/selftest.py \
  --repo "$PWD" --proof docs/evidence/dev-shared-settlement-storage-2026-09-21/proof \
  --packet docs/evidence/dev-shared-settlement-storage-2026-09-21
python3 -I -B docs/evidence/dev-shared-settlement-storage-2026-09-21/audit.py --repo "$PWD"
```

The live checker currently pins the existing development tool/source layout.
The retained-packet auditor uses Git objects and does not require the original
solver installation, generated case paths, local build output or GPU hardware.
Reproducing at a new path creates a new campaign whose paths need its own audit
identity; it does not retroactively replace the retained receipts.

## Remaining Gates

The production callsite binds observations in source, but the Verus physical
capacity parameters are still modeled observations: no theorem connects the
standard-library `Vec::capacity()` implementation to them. This packet does not
prove `Vec::push`, physical capacity/pointer stability, fallible allocation,
OOM/unwind behavior, compiler/machine refinement or full production settlement.
Header/chain validation, indexed scratch scanning and staging/commit remain
separate modeled/production bodies. Mixed-reader constructor traces, other
journal operations and their full Rust correspondence remain separate work.

The [producer-read roadmap](../../runtime-producer-read-reservations-v1.md) retains
Context event-to-producer/member binding, independent producer-result custody,
default-false success-gated backend capability, bounded producer-first progress,
native journal-enabled XGMI qualification and matched HIP/HSA comparisons.
Native pending consumers remain fail-closed. No GPU was used for this packet and
no performance result or full HIP/HSA parity claim follows from it.
