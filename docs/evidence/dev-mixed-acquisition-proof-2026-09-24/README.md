# Independent Mixed-Input Acquisition Correspondence

Status: the source-bound developer proof campaign and collection pass. This is
a bounded journal correspondence result, not global proof-inventory acceptance,
a native result, or A1/A2 closure.

## Results

- All fifteen stages pass from signed source `2d56e6564`, with 459 unchanged
  source/control inputs and the pinned Verus `0.2026.08.09.92f466f` closure.
- Full-root positives before and after the mutations each verify 738 obligations
  with zero errors. The unchanged stable-reader root verifies 720 with zero
  errors. Whole roots use four threads and a 1,200-second wall bound; scoped
  negative controls use 600 seconds. Default SMT limits are unchanged.
- All nine mutations fail through normal logical verification errors: early
  stable commit, missing combined budget, omitted pending preflight, omitted
  stable or producer commits, early error-output mutation, missing pending
  headroom, and removal of either the actual or logical executor call.
- Eleven checker-calibration groups, six final packet-calibration groups
  (including relocated read-only replay and cleanup-record corruptions), eight
  finalizer groups and five inherited cleanup groups pass. The first
  `post-collection-tests` run is rejected as described below; the corrected
  six-group run and unchanged control bracket are in `post-collection-tests2`.
  Six additional publication-receipt corruptions are rejected.
- All 78 scratch files, including development failures and controller logs,
  are retained byte-exactly. Exact-owned removal accounts for 3,293,184 allocated
  bytes; independent path absence passes. The final same-UID scan inspected 117
  processes and reported 45 inaccessible fields, rather than asserting global
  absence. No remote build, GPU test or performance measurement was run.

Audit the signed packet from this checkout with:

```sh
python3 -I -B docs/evidence/dev-mixed-acquisition-proof-2026-09-24/release.py
```

The auditor supports `--packet` for a relocated packet and `--repo` for the
matching source checkout. It replays retained records without invoking Verus.
`release.py` authenticates the corrected inner auditor and checks both
post-collection attempts, exact commands, terminal statuses, diagnostics,
control brackets and the measured README before invoking the signed packet
audit. The failed first attempt is never promoted to acceptance.

## Scope

The new logical model independently composes the existing stable-reader and
producer-reader transitions. It preserves consumer and output-shape error
precedence, checked combined capacity, empty-side identity, and stable-before-
pending validation. The pending decision frame requires sufficient capacity both
before and after stable acquisition; unchanged producer storage alone is not
enough.

The paired theorem derives exact result agreement, final represented ownership,
both output rosters and the final logical producer invariant. Its executable
wrapper calls the shared production method and the independent logical model
separately. Neither final representation nor output agreement is a premise.

Synthetic represented-storage witnesses exercise a late pending-input failure,
a successful one-stable/two-pending batch with exact references and counters,
and subsequent combined-capacity rejection. These are not production
constructor-origin traces. Initial represented producer invariants and bounded
inputs remain explicit premises.

## Qualification Boundary

The extension authenticates and scans the old 449-input lifecycle closure in an
isolated historical tree, without changing any inherited pins. The current
baseline is the signed, CPU-qualified `c21027839` mixed-input implementation.
Four model paths changed between the historical and current baseline, and four
mixed-acquisition paths were added. The historical closure also captured
`crates/fe2o3-runtime/src/context.rs`, which advanced through separately qualified
runtime work. It is bound to the exact current baseline bytes, but is not
compiled into or proved by this bounded root.

Only four new formal Rust files, one checker and its calibration tests are added
by source candidate `2d56e65642cde5752fb40396b47ea8605bc2f9e6`. Production runtime
sources are unchanged. Existing CPU results are historical evidence bound to
their original source; no new CPU-runtime, native or performance result is
claimed here.

The final campaign requires pinned Verus closure checks, eleven checker
calibration groups, full-root positives before and after nine logical negative
controls, and the unchanged stable-reader regression. A logical negative must
exit normally with a verification failure. Syntax/frontend failures, arithmetic
failures, timeouts, signals and solver exhaustion do not qualify. Staged-source
continuity and owned process-group absence are checked before controller
acceptance. Failed stages remain retained.

`check-mixed-acquire.py --verify` performs relocatable semantic replay and
reconstructs the mutations without invoking Verus. It uses separate temporary
scratch, not the packet directory. `audit.py` additionally authenticates the
checker before import, closes the raw manifest and packet seal, and by default
checks the signed evidence commit. Its explicit `--unpublished` mode makes no
evidence-commit authenticity claim.

## Retained Development Diagnostics

The `dev1` through `dev9` log pairs are exploratory measurements, not accepted
source-bound campaign receipts. `dev1` used an unsupported thread option and
`dev2` omitted the library crate type. `dev4` exposed a missing explicit stable
decision correspondence call. `dev6` exhausted the default solver limit in a
single long witness; splitting the witness removed that exhaustion without
increasing the solver budget. None of those failures is an accepted logical
negative. The scoped successes and the earlier 738/720 whole-root measurements
do not replace the final signed-source campaign.

The first two checker-test attempts also remain retained: one rejected an
unapproved logical-model macro, and the other exposed the independently changed
Context input in the historical closure. The subsequent tests pass after an
explicit enum match and separate exact-baseline binding of that nonproof input.

`finalize.py` runs semantic replay, packet calibration, finalizer refusal tests
and the existing cleanup calibration before collecting every scratch file. It
requires the exact scratch roster, same-owner ordinary entries, no hard-linked
files, an available controller lock and absent recorded command groups. A
byte-exact retained copy and an unchanged full tree precede removal. Scoped
same-UID process-reference scans preserve inaccessible fields and exclude the
collector's own read-only lock. They refuse observed references, but do not
establish global process absence. Cleanup claims only this exact owned path and
the recorded command groups. It does not touch the separate owner-inspection
packet, Cargo caches, the Verus installation or any remote host.

The auditor checks finalization commands, ordered terminal receipts, captured
control brackets, exact retained inventory and cleanup/absence records. The
captured in-progress README and collection programs are kept separately. Eight
collector calibration groups include copy-before-delete, lock contention,
source/control drift and interrupted or ineffective deletion. An exploratory
test edit initially left three assertions in the wrong method (`NameError`);
that test-layout error was corrected before the recorded qualification.

The first post-collection run passes the original five packet groups but rejects
the new cleanup-evidence group: the auditor mistakenly expected dot-style
unittest output, while the inherited five-test suite emits its exact named tests
with `verbosity=2`. The corrected auditor requires those five names in order and
a closed successful summary. Original receipts and all collection controls are
retained unchanged and independently pinned; the collector now imports its
captured original audit helper. This is a post-collection audit correction, not
a rerun or replacement of the completed cleanup or source-bound proof campaign.

## Remaining Work

Global proof-inventory registration, constructor-origin mixed traces, Context
maps and binding coverage, writer Begin plus readers as one transaction,
panic/unwind, asynchronous carriage and producer-first completion reconciliation
remain open. This packet cannot establish native/machine-code refinement,
physical overlap, aggregate residency, HIP/HSA parity or any speedup. The broader
Native R125, Admission R118B C1/C2/C3, Resources R116/V3 and A1/A2 checkpoints are
not advanced by this bounded qualification alone.
