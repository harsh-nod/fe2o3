# Production Completion Settlement Control

The ordinary `RuntimeContextV1::settle_terminal_submission_v1` now shares its
four-call suffix with Verus through one private macro. The runtime still calls,
in order, input release, optional writer settlement, dependency release and
public status publication. Existing submission-domain, terminal-state,
producer-root, custody and directed-success checks are unchanged.

The first three calls use explicit `match`/early return, equivalent to the former
`?` operators. Publication remains the tail result. No scheduler, storage,
allocation, queue, public API or status/outcome mapping is added. In particular,
cancelled-before-publication work can still settle its writer as NoEffect.

## Proof Boundary

`context_completion_settlement_control_v1.rs` includes the actual production
macro. Its executable trace harness supplies opaque argument/result labels and
one scripted failure position. The proof establishes:

- The exact attempted-call and successful-call prefixes for success and each
  of the four error positions.
- No suffix call after a returned error, and exact error propagation.
- Unchanged submission, writer-outcome and requested-status forwarding.
- Publication is called once, only after three successful returns, and its
  returned value is propagated rather than replaced by the requested status.

These are control-flow properties. The harness methods are not refinements of
Context methods. A successful-call prefix is not a theorem about journal or map
effects, nor a rollback guarantee. Input release can release stable readers
before producer-reader release fails. Dependency counters, identity/custody maps,
callbacks, backend observations, panic/quarantine and retry remain outside this
proof. Existing journal proofs must be composed with this production boundary
before claiming completion-settlement refinement.

## Qualification

Development Verus passes eight obligations with zero errors at the pinned
default solver limits. The initial mutable-reference postcondition syntax
failure is retained and is not qualifying negative evidence. Two new CPU tests
cover 75 result/argument/failure combinations and four unwind positions with
the same macro and real runtime identity/status/outcome types. The whole GNU
runtime suite passes 1,406 tests, retaining 22 hardware-only ignores.

The source-bound campaign is pending. Its checker authenticates 23 inputs,
requires an exact Context delta against signed `decfecb13`, pins both the shared
body and proof root, and authenticates the inherited tool-closure script and
manifest before use. It requires whole-root positive brackets, 22 compile-clean
logical mutations, checker calibration and both 190-file tool-closure checks.
Each process has a fixed 120-second bound with inherited owned-group cleanup;
frontend failures, timeouts and resource exhaustion cannot qualify negatives.
Source staging preserves the real relative include and checks bytes around
every solver invocation. Four CPU-only checker groups pass.

This is not full Context reconciliation, global proof registration, protected
Worker execution, A1/A2 closure, native qualification or HIP/HSA parity.

## Next

Compose the journal release/settlement relations and exact Context custody with
the ordered calls, including partial failure prefixes. Then refine the retained
physical-success and producer-first reconciliation gate, without equating native
completion with a publishable logical result.
