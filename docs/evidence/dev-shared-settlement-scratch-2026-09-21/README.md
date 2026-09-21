# Shared Settlement Scratch Scan and Staging

Development qualification of two allocation-free production Rust/Verus shared
executable loops. This is not registered proof-runner acceptance, whole-wrapper
refinement, native admission, HIP/HSA parity or performance acceptance.

## Scope

`settlement_scratch_bodies.rs` is included unchanged by the private ordinary Rust
adapter and the Verus root. The scan examines only the admitted scratch prefix and
returns the original error at the first occupied cell. Staging follows the linked
member prefix and writes all four fields of each `BeginMemberPlanV1`. It retains
the production `expect` calls, member-read/store ordering and two counted accesses
per member. The runtime callsite keeps scalar storage admission before scanning,
and complete preflight before staging, under the same exclusive settlement borrow.
The final allocation/member/free-list commit is unchanged.

Both loops take a syntax adapter and annotations. Rust's pinned identity adapter
receives empty annotation lists. The pinned Verus adapter receives only fixed
invariant/decreases clauses. The actual loop conditions, reads, checks, plan
construction, indexed stores, cursor advancement and counters are shared. Test
instrumentation calls the existing indexed-access counter; production non-test
instrumentation and the proof stub are empty and cannot decide admission.

The standalone scan requires `count <= scratch.len()`. The existing scalar guard
establishes that bound before the production call; the logical composition proves
the same rejection precedence. The standalone stage retains the original
`settlement_storage_ready_v1` precondition, without adding global validity, issued
provenance, canonical or unique member/allocation keys, a terminal `None`, matching
writer identity, globally empty scratch or physical spare capacity. It permits
bounded repeated/cyclic prefixes and an arbitrary unused head when count is zero.

Staging proves the exact prestate-derived scratch prefix and frames all non-scratch
modeled contents, including unrelated malformed state. Untouched scratch tails are
preserved. The proof uses fixed loop invariants, including extensional scratch
equality; no executable proof callback or stronger admission restriction is added.
The complete raw logical settlement wrapper composes the shared retained checks,
scalar return guard, scratch scan and staging with the existing modeled commit.
Issued wrappers preserve history and reader custody under their existing invariant
premise. Constructor/enroll/register/Begin witnesses cover empty/two-member writers,
both settlement outcomes and a storage rejection. Their reader arenas are empty.

## Retained Checks

- Two whole-crate positives: **392 verified, 0 errors** each.
- Ten executable negative controls: **391 verified, 1 error** each. Five fail
  exact loop invariants: scan bypass and incorrect staged prior lineage, attempt
  epoch, member slot or allocation key. Five fail exact postconditions: wrong scan
  error/success results, clearing a completed plan or untouched tail cell, and
  mutating a non-scratch journal field. Contracts and annotations are unchanged.
- **840 unit tests passed**, two ignored, and **27 doctests passed**. Formatting
  and Clippy (`--all-targets -- -D warnings`) passed.
- Seven new tests include five direct staging tests before commit clears the plans.
  These cover nonzero lineage distinct from the attempt epoch, noncontiguous linked
  slots, every copied field, malformed untouched contents, dirty tails, zero-count
  identity and bounded repeated/cyclic prefixes. One scan test covers each first dirty
  position, multiple dirty cells, ignored tails and short storage rejected by the
  preceding guard; one structural test binds the shared loops and adapters.
  Full snapshots include all seven vector pointers/capacities;
  counted work is `2 * count` for staging and the examined prefix for scanning.
  Existing ranked-fault, mixed-reader and full settlement suites remain included.

The 382 inherited obligations overlap prior packets; do not sum archive counts or
treat them as coverage ratios. Physical pointer/capacity stability is CPU evidence,
not a theorem from modeled sequence-content equality.

## Evidence Custody

The checker compiles actual shared macros in their nested include layout. It pins
the new root/body/complete Rust adapter, prior source/checker chain and Verus release
closure. Live and offline staging authenticate both the new and inherited identity
adapters. Exact macro envelopes and fixed proof-only annotation invocations are
checked before the pinned proof-source policy scan. Full CPU input brackets capture
the production delegates, tests and all runtime-model source files.

Invariant controls must identify the exact intended invariant within its macro
invocation, with a single primary span and no substituted expansion. Postcondition
controls must identify the complete intended clause and a complete return expression
or exact shared/wrapper block exit. The checker authenticates JSON types, byte/line/
column coordinates, excerpts, highlights, labels, invocation and definition sites,
and the complete expansion tree where present. Partial runs, compiler errors,
timeouts, extra diagnostics and source-consistent non-exit spans are not accepted.

Before/after input hashes, frozen cases, exact owned-process completion and
before/after 190-file Verus release-closure measurements accompany the campaign.
Default solver limits, four threads and a 180-second outer timeout are retained.
`SOURCE` names the signed source commit. The portable offline auditor reconstructs
all nested inputs/mutations from Git objects, rechecks solver/process receipts,
verifies the complete manifest and binds CPU source hashes to that commit. Runs
started in a development worktree; later source binding is not a claim of execution
from a clean signed checkout. Hashes alone do not prove execution. Diagnostic-only
partial probes and other development attempts are excluded from this packet.

`selftest.py` rechecks all ten known negatives, then rejects altered result summaries,
macro diagnostics, genuine-but-wrong invariant spans, source-consistent non-exit
relocation, source injections and changed identity adapters. Its rehashed-packet
controls first require the pristine packet to pass and then require the intended
failure, not merely an unrelated error.

## Reproduce

From the repository root, using the pinned Verus release:

```sh
python3 -I -B docs/evidence/dev-shared-settlement-scratch-2026-09-21/check.py \
  --repo "$PWD" --verus /path/to/pinned/verus --output /new/owned/proof
python3 -I -B docs/evidence/dev-shared-settlement-scratch-2026-09-21/selftest.py \
  --repo "$PWD" --proof docs/evidence/dev-shared-settlement-scratch-2026-09-21/proof \
  --packet docs/evidence/dev-shared-settlement-scratch-2026-09-21
python3 -I -B docs/evidence/dev-shared-settlement-scratch-2026-09-21/audit.py --repo "$PWD"
```

The live checker retains development-layout pins. The retained-packet auditor uses
Git objects and does not require the original solver, case paths, build output or
GPU. A new campaign needs its own path identity; reruns do not replace retained
receipts retroactively.

## Remaining Gates

This advances gate 1 of the [producer-read roadmap](../../runtime-producer-read-reservations-v1.md).
Final settlement commit, construction/enrollment and sorting/search, Begin, reader
admission/release and unread guards still need complete production correspondence.
Type/view mapping and standard-library indexing/Option behavior are reviewed/model
assumptions, not complete Rust/compiler/machine refinement. Physical Vec storage,
fallible allocation and normal/unwind semantics remain separate contracts.

Context event/member binding, independent producer-result custody, default-false
success-gated backend support, bounded producer-first progress, native pending XGMI
dataflow and matched HIP/HSA comparisons remain open. Pending native consumers stay
fail-closed. No GPU or performance test ran for this packet.
