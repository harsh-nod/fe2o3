# Shared Settlement Commit

Development qualification of the production Rust/Verus shared settlement commit.
This is not registered proof-runner acceptance, whole-wrapper refinement, native
admission, HIP/HSA parity or performance acceptance.

## Scope

`settlement_commit_body.rs` is included unchanged by the private ordinary Rust
adapter and the Verus root. It retains the production `scratch[index].take().expect`,
allocation `as_mut().expect`, in-place conditional lineage update, backlink clear,
member clear, member-free append, writer clear and writer-free append. The existing
writer store/return helper operations are inlined with their counters. A lexical
block ends the allocation borrow before subsequent journal accesses. No copied or
reconstructed allocation entry replaces the actual mutation.

The syntax adapter receives empty annotations in Rust and only the pinned
invariant/decreases list in Verus. All executable loop conditions, indexed access,
Option operations, field mutations, appends and counter calls are shared. Counter
instrumentation is empty in non-test production builds and in the proof stub.
The production wrapper retains preflight, then staging, then commit under the same
exclusive borrow. Commit work is `4 * count + 2`; complete settlement remains
`9 * count + 3` counted accesses.

The raw proof retains the existing logical commit precondition: storage readiness
in the original prestate, an in-bounds writer slot, the stage frame and exact staged
scratch plans. It does not add global validity, issued provenance, unique member or
allocation slots, canonical keys, terminal links, writer identity or physical spare
capacity. Repeated member slots and aliased allocation destinations retain their
sequential behavior. Success is last-write-wins in plan order; NoEffect preserves
the allocation's current lineage, not a plan's prior lineage. Each member occurrence
is appended, and dirty scratch tails and unrelated malformed state are framed.

The composed logical wrapper uses the shared retained admission, scalar storage
guard, scratch scan/staging and final commit. Its issued wrapper preserves reader
custody and history under the existing issued invariant. Constructor/enroll/register/
Begin witnesses cover empty/two-member writers, both outcomes and rejected storage
followed by accepted settlement. Their reader arenas are empty; this is not a new
mixed-reader construction proof.

## Retained Checks

- Two whole-crate positives: **398 verified, 0 errors** each.
- Ten executable negative controls: **397 verified, 1 error** each. Seven fail
  exact loop invariants: wrong success lineage, an unconditional success update,
  omitted backlink clear, corrupted allocation metadata, omitted member clear,
  omitted member return and omitted scratch clear. Three fail the exact raw commit
  postcondition: omitted writer clear, wrong writer return and unrelated scalar
  mutation. Contracts and annotations are unchanged.
- **847 unit tests passed**, two ignored, and **27 doctests passed**. Formatting
  and Clippy (`--all-targets -- -D warnings`) passed.
- Six direct behavioral tests cover normal counts, aliased allocations with distinct
  epochs/current/prior lineages, repeated members and nonterminal cycles, dirty tails
  and malformed metadata, absent/mismatching writer contents and zero-count identity
  outside writer return. One structural test binds actual operation ordering and the
  empty Rust annotations. Existing rejection, mixed-reader and complexity tests run
  unchanged except for wrapper delegation assertions.

The 392 inherited obligations overlap earlier packets; do not add archive counts or
interpret them as coverage ratios. Full CPU snapshots include all seven vector
pointer/capacity pairs. Direct commit fixtures reserve return capacity before their
snapshot, rather than strengthening the weak logical theorem with a physical premise.

## Evidence Custody

The checker pins the complete new root/body/Rust adapter, inherited source/checker
chain and Verus release closure. It audits fixed macro envelopes and proof-only
annotations, authenticates inherited adapters and applies the pinned proof-source
policy. Mutation controls edit exactly one specified executable fragment; the
auditor reconstructs those candidates from Git, not from archived diagnostic text.

Invariant failures must identify the exact intended clause within the intended
macro invocation. Postcondition failures require exactly one typed primary for the
complete raw relation and one typed secondary for the complete wrapper block exit.
Source path, byte/line/column offsets, excerpts, highlights, labels and null expansion
are matched exactly. Span order is not semantic. Wrong genuine invariants and
source-consistent non-exit spans are rejected, as are partial/compiler/timeout results.

The archive retains before/after source/tool brackets, all frozen completed cases,
owned-process completion and before/after measurements of the pinned 190-file Verus
release closure. Solver runs use default limits, four threads, no-cheating,
whole-crate verification and a 180-second outer timeout. `SOURCE` binds the packet
to a signed source commit. Runs began in a development worktree; later Git binding
is not a claim of execution from a clean signed checkout. Hashes alone do not prove
execution. Initial probes and the incomplete checker-development campaign are not
accepted evidence and are excluded.

The offline auditor reconstructs all source and checker inputs from Git objects,
checks exact receipts/diagnostics/manifests and binds CPU input hashes to `SOURCE`.
The selftest rechecks all known negatives, rejects altered solver/source inputs and
tests rehashed corrupt packets only after the pristine archive passes. Corrupt packets
must fail for their intended reason, not merely an unrelated error.

## Reproduce

```sh
python3 -I -B docs/evidence/dev-shared-settlement-commit-2026-09-21/check.py \
  --repo "$PWD" --verus /path/to/pinned/verus --output /new/owned/proof
python3 -I -B docs/evidence/dev-shared-settlement-commit-2026-09-21/selftest.py \
  --repo "$PWD" --proof docs/evidence/dev-shared-settlement-commit-2026-09-21/proof \
  --packet docs/evidence/dev-shared-settlement-commit-2026-09-21
python3 -I -B docs/evidence/dev-shared-settlement-commit-2026-09-21/audit.py --repo "$PWD"
```

Live checking retains development-layout pins. Offline auditing needs Git objects
and the packet, not the original solver, temporary case paths, build output or GPU.
A new campaign needs its own path identity; it cannot replace retained receipts.

## Remaining Gates

This advances gate 1 of the [producer-read roadmap](../../runtime-producer-read-reservations-v1.md).
Construction/enrollment and sorting/search, Begin, reader admission/release and unread
guards still need complete production correspondence. Sharing executable commit tokens
does not compile the entire ordinary Rust wrapper/types into Verus. Type/view mapping,
the vstd specifications of indexing/Option/Vec operations, compiler and machine
refinement remain explicit boundaries. Sequence append/equality does not prove
physical capacity, pointer identity, allocation-free execution or unwind behavior.

Context event/member binding, independent producer-result custody, default-false
success-gated backend support, bounded producer-first progress, native pending XGMI
dataflow and matched HIP/HSA comparisons remain open. Pending native consumers stay
fail-closed. No GPU or performance test ran for this packet.
