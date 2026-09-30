# Producer Input Fold Refinement

Status: integrated conditional component refinement at `14e4253eb`, from signed
candidate `205a6a03e`. The candidate's signed campaign passes three full
`--no-cheating` runs of 13 obligations and all 22 logical negatives. Its broader
checks pass 136 selected CPU tests and 53 doctests. This is not native hardware,
full-runtime, milestone, or performance acceptance; the merged-source guard
calibration is not a merged-source solver rerun.

## Shared Production Body

`producer_input_fold_body.rs` is included by `producer_readers.rs` and the
conditional Verus root `context_producer_input_fold_v1.rs`. The runtime still
performs the existing retained-root preflight first. Its private observation
adapter then runs the extracted original per-input validation body.

The shared controller visits inputs in order, retains separate active and queued
family cursors, returns the first error, and reduces successful statuses with
`Unknown > NoEffect > Pending > Success`. `Unknown` does not stop later input
validation. After all inputs, both family cursor counts must match the retained
reference arrays; active-count failure short-circuits the queued-count check.

The native per-input body preserves this order:

1. Select the next reference from the input's family.
2. Validate request equality, consumer and checked consecutive incarnation.
3. Perform journal lookup, then advance that family cursor, then query status.
4. Check retained launch or directed-peer dependency/source binding.
5. Check producer identity, submission ordering, allocation ordering, retained
   allocation identity and backend allocation membership.
6. Observe expected credit once, only if all preceding checks were reached and
   passed. A false observation returns `InvalidReference`.
7. Validate the live allocation and exact device, extent and byte region.

The credit adapter calls the existing `has_expected_credit` directly. Its
General/Composed dispatch and any account lock acquisition remain unchanged.
Each call is its own observation; no combined ledger snapshot is introduced.

## Conditional Proof Scope

The conditional theorem concerns the actual shared fold controller against ordered
per-input observation receipts. A receipt records its family, whether that
family cursor advanced, its exact result, and an individual credit call's
arguments/return value or `NotReached`. The replay consumes receipts in order;
errors and local ownership payloads need not implement `Copy` or `Clone`.

This is not yet a proof that native helper executions produce those receipts.
Correspondence with the reached journal queries, short-circuit checks and credit
calls is an explicit adapter boundary. In particular, the credit metadata does
not prove ledger validity, native freshness, lock atomicity or poison recovery.
The local owner frame does not extend into shared `Arc` interiors. Physical
allocation, unwinding, compiler lowering and ISA correctness are outside scope.
The full native fold theorem remains open until those helper boundaries are
refined and a source-bound qualification campaign succeeds.

## Development Evidence

The six passing focused CPU groups instantiate both shared production macros with real
retained/request types and scripted query observations. They cover all status
precedence pairs, mixed-family cursor order, `Unknown` followed by a malformed
later input, first-error precedence, missing references, `usize::MAX` family
cursors, checked incarnation overflow, excess final references, lazy credit/live
checks, and both launch and directed-peer bindings. They are not native account
or journal qualification. The focused run reported six passed and 1,886 filtered,
with no ignored cases or compiler warnings. Its source was formatted with the
pinned formatter; later edits were confined to the proof and this checker/docs
preparation, not the production body or CPU tests.

Proof discovery V5 passed all six recorder gates, including the 190-file verifier
release closure before and after the full proof, with unchanged source/tool/raw
identities and six fresh child groups closed in the same live recorder namespace.
The positive stderr contained only 13 permitted `--multiple-errors=0` informational
notes. Earlier rejected frontend and SMT attempts remain retained; they are not
accepted positives. This is not historical-group or host-wide absence evidence.

## Signed Campaign

`check-producer-input-fold.py` pins the accepted proof bytes and the complete
runtime Rust roster, including the actual adapter and native validation helper.
The signed candidate had 317 runtime Rust files. Integration refreshes only the
roster count/hash to 319 for the separately reviewed 13-path XGMI custody change;
the proof, shared macros, adapter and focused tests are unchanged.
The executable proof closure is only the proof plus shared macro file. That file
also contains the native per-input macro, which the proof does not expand.

The 22 executed mutations change only the production fold macro: substituting or
swallowing its first error; stopping at `Unknown`; forgetting prior aggregate
statuses or weakening precedence; incorrect initial aggregate/input/family
cursors; swapped family arguments; missing, cross-wired or weakened final count
checks; promoted final-count failure; duplicate validation; and stopping after
the first success. Every candidate selects only `Observations::reconcile`.

All 30 signed campaign phases pass: the three complete positive runs, all 22
strict logical negatives and their source/tool/release checks. Each selected
negative reports one verified obligation and one logical error, not a frontend
rejection. All 6291 selected signed inputs remain unchanged. The calibration
alone only constructs candidates and checks guards/classifiers; it does not
establish logical rejection. The opaque owner is not directly mutable by the
controller; no disconnected owner mutation or accounting authority is invented.

The inherited campaign controller retains its original `--no-cheating`, bounded
timeouts, signer/source binding, full positives before/relocated/after and strict
logical-negative classifier. Its normal entrypoint requires clean signed source.
The signed component campaign used that entrypoint. Its managed child-group
closure receipts are not an independent historical PID census or host-wide
absence claim. The broader CPU recorder separately closed its own 17 fresh
groups in its unchanged live namespace.

Broader candidate checks cover seven disjoint CPU groups: fold, preflight,
producer launch, directed peer, peer custody, allocation admission and version
journal (136 total). All 53 doctests, strict Clippy, formatting and whitespace
checks pass. The candidate's no-default compilation retains exactly two known
baseline warnings; it is not warning-free. The original six focused cases are
included in the 136 and are not additive.

The full native-helper and live-ledger refinements remain open. See the
[current milestone snapshot](runtime-a1-a2-swarm-current.md) for integration and
broader qualification limits. Raw campaign evidence remains local and has not
yet been published as a release packet.
