# Producer Input Fold Refinement

Status: unsigned development refinement. Six focused CPU groups passed, and a
full unfiltered `--no-cheating` proof discovery measured 13 verified obligations
with zero errors. The logical mutation campaign has not run. This is not signed
source, native hardware, full-runtime, milestone, or performance acceptance.

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

## Mutation Preparation

`check-producer-input-fold.py` pins the accepted proof bytes and all 317 runtime
Rust files, including the actual adapter and unchanged native validation helper.
The executable proof closure is only the proof plus shared macro file. That file
also contains the native per-input macro, which the proof does not expand.

The 22 proposed mutations change only the production fold macro: substituting or
swallowing its first error; stopping at `Unknown`; forgetting prior aggregate
statuses or weakening precedence; incorrect initial aggregate/input/family
cursors; swapped family arguments; missing, cross-wired or weakened final count
checks; promoted final-count failure; duplicate validation; and stopping after
the first success. Every candidate selects only `Observations::reconcile`.

The calibration constructs these exact candidates and checks source guards,
unchanged native-helper bytes, controller flags and synthetic classifier refusal.
It does not establish that any candidate fails logically. Rust/VIR errors,
warnings, timeouts and unexpected selection diagnostics must not be counted as
logical negatives. The opaque owner is not directly mutable by the controller;
no disconnected owner mutation or native accounting authority is invented.

The inherited campaign controller retains its original `--no-cheating`, bounded
timeouts, signer/source binding, full positives before/relocated/after and strict
logical-negative classifier. Its normal entrypoint requires clean signed source;
this unsigned worktree is preparation only until a separately reviewed execution
path or signed candidate is available.

Remaining gates include the measured logical mutation campaign, broader existing
producer-input preflight/producer-launch regressions, scoped static checks and
source-bound integration. The full native-helper refinement remains separate.
