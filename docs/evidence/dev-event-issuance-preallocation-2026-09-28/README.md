# Event Issuance and Pre-Native Output Reservation

Development qualification only. A1/A2 and issue #182 remain incomplete; accepted
lane checkpoints are unchanged. No GPU execution, HIP/HSA comparison or latency
measurement was performed. No remote machine was used.

## Qualified Source

- Initial implementation: `07a673ea31c132fb40af0f93eba294f971e1324d`.
- Diagnostic calibration: `a103ef2186e621e914a88f1836b1c8f8b02d98c4`.
- Final qualified source: `792c43ab48ce8389d223f2b9fbab4df5affbec3c`.

Batch issuance no longer allocates an occurrence scratch Vec. It validates all
retained rows and pin increments, reserves the ledger and returned event Vec,
then commits exact consecutive event IDs and selected-slot pins. Work remains
O(N). Pin checks now precede both retained reservations; the old scratch
reservation and its allocation-refusal branch no longer exist.

The source caller now reserves its lane-wrapped output before acceptance issuance
and recipe binding, after terminal, persistent-attachment and packet-count guards.
Reservation refusal is RejectedBeforeSideEffect, not Retryable: immutable recipe
binding has not yet been authenticated. It does not burn an identity, call native
submission or invoke cancellation. This deliberately changes precedence relative
to later acceptance/recipe errors. The later lane-event collect is replaced with
ordered pushes into that preallocated output.

## Proof and Trust

Production and Verus share the actual ledger constructor, logical-identity
validator, batch/single issuers, both batch forwarders, output reservation and
lane packing. The new root proves exact validation/error ordering, unchanged
logical state on refusal, exact ordered tokens and ledger/pin/counter updates on
success, and preservation of unrelated slots, readers and owner fields.

Issuance requires an explicit fresh event-ID frontier: next_event_id is nonzero
and all retained keys are smaller. Constructor and issuer preservation are proved.
A concrete constructor-to-batch-to-single witness uses synthetic Bound slots
63/64 and conditions success on actual reservation outcomes; it does not assume
allocation succeeds. This is not native queue-origin or full lifecycle provenance.
Arbitrary corrupted, colliding counters are not promised dynamic rejection.

The 12-file proof closure contains exactly two SHA-pinned, contents-only standard
library contracts: HashMap::try_reserve and Vec::try_reserve_exact. Their outcomes
remain unconstrained. Capacity, addresses, allocation counts, allocator termination
and panic behavior are not proved. This root does **not** use --no-cheating;
source guardrails are not a substitute for that flag. Standard Verus/vstd/Z3 and
compiler trust remains. Lane packing proves preservation of an arbitrary existing
output prefix and exact ordered lane/token association, not a capacity theorem.

No proof in this packet establishes complete reserve-through-native-publication
composition, physical signal/queue authority, native MMIO execution, outer
terminalization/unwind, destructors or end-to-end Context/Worker behavior.

## Qualification

All nine final campaigns and CPU/static checks bind the same signed source and
**6234 unchanged source hashes**. Each campaign verifies the pinned **190-file /
129019839-byte** Verus release closure and has original, exact relocated and
closing whole-crate positives.

| Campaign | Stages | Whole-crate obligations | Executable negatives | Trust profile |
| --- | ---: | ---: | ---: | --- |
| Batch issuance and forwarders | 40/40 | 46/0 errors | 32 | Two explicit issuance reservation contracts |
| Single issuance | 23/23 | 46/0 errors | 15 | Same two contracts |
| Output reservation and packing | 16/16 | 46/0 errors | 8 | Same two contracts |
| Event binding regression | 36/36 | 43/0 errors | 28 | --no-cheating |
| Source publication regression | 14/14 | 43/0 errors | 6 | --no-cheating |
| Bound cancellation regression | 31/31 | 26/0 errors | 23 | --no-cheating |
| Batch release regression | 32/32 | 29/0 errors | 24 | Two older hash-reservation contracts |
| Source rollback regression | 18/18 | 51/0 errors | 10 | Same older contracts |
| Rollback adapters regression | 11/11 | 51/0 errors | 3 | Same older contracts |

Total: **221 passing proof stages and 149 executable negatives**, including
**79 stages and 55 negatives** for the new root. Obligation counts overlap and
include helpers; they are not distinct runtime operations. Historical dispatch
epoch/native-source-failure mutation campaigns are not included in these totals.

The checker calibrates exact source closure, byte-preserving supplement hashes,
production wiring and reservation placement, mutation sites and strict diagnostics.
Bounds and arithmetic errors are accepted only for their named function selectors;
unknown, mixed resource/frontend, foreign and nested diagnostics are rejected.
Reservation-size/omission checks are source-wiring controls, not capacity proofs.

- KFD: **1431 passed, zero failures**, with **320 construction tests explicitly
  excluded**. This is not a full KFD-suite run.
- Runtime library: **1828 passed, zero failures, 30 hardware ignores**.
- **124 doctests passed**: 42 KFD, 53 runtime, 29 runtime-model.
- Strict all-feature/all-target Clippy, no-default-feature production checks,
  workspace/included-file formatting and diff checks pass.
- All **308 recorded process groups**, including stopped/superseded runs, were
  reaped and independently confirmed absent.

Seven new CPU groups cover one-shot thread-local allocator refusal, realloc
preservation and unwind reset; batch positional identities, aliases, neighbors,
readers, sparse high slots and ID exhaustion; both actual issuer reservation
failures; validation precedence; and primary/AUX source rejection and success.
Early output refusal leaves full owner snapshots, recipe state and process-gate
records unchanged, and can retry through ordinary cleanup. Guard tests cover
terminal, persistent attachment, zero count and acceptance failure precedence.

CPU tests observe **two allocations for fresh empty-ledger N=3 batch issuance**
and **zero allocations from the native-success callback's final action through
the actual source submission return** on both fixture lanes. These are scoped
CPU observations, not universal allocator guarantees or measured speedups. A
successful first reservation may grow HashMap capacity before a later refusal;
logical atomicity does not imply physical allocation identity. High-slot/capacity
fixtures deliberately manipulate CPU metadata, not native GPU authority.

The independent final audit hashes every relocated/mutated tree, checks exact
membership and isolated changed bodies, authenticates signed Git blob linkage,
and ties mutation selectors/root paths to recorded verifier commands. Separate
read-only agent reviews checked the three new campaigns and final CPU evidence.

## Retained Stops

The initial signed batch campaign rejected the wrong-commit-occurrence result
because its exact arithmetic-overflow diagnostic was not yet recognized. Verus
had correctly rejected the malformed body with status 1. The checker correction
is restricted to the two issuer functions and includes hostile diagnostics.

A subsequent 40-stage batch run passed, but its CPU driver stopped at Clippy's
question_mark suggestions. Two method-local allowances document why explicit
returns carry reservation observations in the shared proof. No executable body
or proof contract changed in these corrections. All nine final campaigns and
CPU/static checks were rerun at the final source; earlier runs remain retained.
All eleven proof-development stdout/stderr pairs are also included.

## Remaining Work

Next is transactional preparation and provenance of `bind_fixed_batch`: it still
marks slots Bound before building the dispatch-generation roster. Complete those
allocations/conversions before the state commit, then refine the actual slot,
dispatch and packet association without assuming a valid prepared object.

Native publication receipt authority and outer terminalization, Context/Worker
composition, target scheduling, physical overlap, high-depth reuse, multi-device
and distributed failures, device-language and machine-code atomic/collective
refinement, and matched HIP/HSA measurements remain open. No milestone exit gate
is promoted. Issue #182 was checked on September 28 Los Angeles time (September
29 UTC) and remained OPEN, last updated 2026-09-27T10:25:24Z.

## Archive

raw.tar.gz is **17441711 bytes**, containing **3698 manifested files plus the
inner SHA256SUMS manifest**. An independent restore passed every hash and a full
recursive byte comparison. The owned restore and sealing directories were
removed and absence checked. No remote machine or other users' files were touched.

The outer SHA256SUMS seals this report and raw.tar.gz. The archive retains signed
relocated/mutated inputs, development and stopped-run logs, final proof/CPU/static
process records, reproduction commands and independent audit results. See
COMMANDS.md, qualify.py, audit-final.py and final-audit.json inside it.
