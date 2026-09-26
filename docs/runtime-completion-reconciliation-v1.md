# Shared Completion Reconciliation Development

The complete bounded planner in `completion_reconciliation_body.rs` is shared
by production Rust and the standalone `context_completion_reconciliation_v1.rs`
Verus root. This is an executable finite-graph safety projection, not complete
Context or journal refinement. It does not close A1/A2 or establish native or
performance parity.

## Executable Boundary

The only global graph premise is a finite table of at most 1,048,576 distinct
full submission IDs. Graph validity is not assumed: executable visited-node
validation derives profile, dependency identity, backend, context generation,
cursor-prefix and strict depth/local-order facts. The table is a linear Vec
projection, not the production HashMap or a proposed production replacement.
The per-node dependency roster and traversal stack remain bounded by 256,
independently of the total graph size.

One production body implements record lookup, validation caching, terminal
pop, selected backend observation, physical-success gating, cursor advancement,
descent, quiescence, input classification, fallible settlement and bounded yield.
Verus frontend compatibility requires explicit while-loop fuel, immediate
reference dereference, hygienic local names and Result bindings. Empty production
hooks add no executable work. The source checker permits only those reviewed
syntactic changes relative to signed `53aba0d65`; Rustfmt-parsed normalized
bodies must match. This is syntactic correspondence, not a semantic-equivalence
theorem for the transformations.

## Current Safety Statements

The standalone development root verifies 53 obligations at the pinned default
solver limits. Its planner has no graph-validity or settlement-readiness
precondition. It establishes:

- Every successful observation is a validated Pending operation with its exact
  backend identity. It is reachable from the requested operation through current
  selected cursor edges, not merely arbitrary dependency-roster membership.
- Stack ancestry preserves full IDs, generation, backend and profile identity,
  strictly decreasing local IDs and depth, and the 256-entry depth/stack bound.
- Cursor advances cross only logically successful dependencies. Existing terminal
  status/state is preserved; new Success requires physical Success, completed
  dependencies and successful/absent input. Quiescence has its own dependency or
  Unknown-input gate and cannot be promoted to Success.
- Settlement is attempted only after current validation and the corresponding
  readiness gate. Adapter errors may preserve committed effects; they do not
  imply rollback. Progress and quarantine monotonicity hold on returned errors.
- Returned Local status is the requested operation's current status. Local
  Pending is returned only after all 513 iterations. Each invocation uses one
  through 513 iterations, independently of total graph size.
- Returned errors equal an adapter-recorded rejection, except an initially
  missing requested ID. This is error-value provenance, not a proof identifying
  the rejecting call site.
- A valid requested directed leaf with physical Success, no dependencies and
  successful root validation returns exact Success for absent/Success input or
  exact Quiescence for Unknown input in two iterations. An injected settlement
  error returns its exact code in one iteration, retaining Pending custody and
  setting quarantine. The theorem also frames every unrelated node, directed
  state and the synthetic settlement marker. It is conditional on leaf shape,
  without strengthening the general planner's graph precondition.

Opaque predicates, isolated proof queries and explicit quantifier triggers
control solver expansion. They do not change assertions or increase limits.
No new assume, admit or external-body shortcut is used.

## Remaining Correspondence

Executable singleton constructors witness the leaf outcomes for both directed
profiles, all three permitted input classifications and every u8 settlement
failure code. Custody and root validators have exact result contracts; settlement
has exact custody/quiescence/quarantine frames. These are finite-projection
constructor witnesses, not constructor-origin real-Context theorems. In
particular, real empty-dependency leaf input queries yield None; this model's
Some(Success)/Some(Unknown) leaves are not established real constructor cases.

The successor development runner checks signed-source positives, four logical
mutants and an exact relocated source replay. These checks do not substitute for
the broader authenticated mutation/admission campaign or production refinement.

The adapters deliberately omit actual HashMap allocation, full event/device/
allocation custody metadata, journal ownership, callbacks, dependency-release
effects and quarantine implementation. Root/input observations are fixed bounded
inputs. Those boundaries must be composed with their separately owned production
proofs; validation of this projection cannot supply that composition.

The projection's `settlement_prefix` is an opaque synthetic marker, not a count
of production journal stages. Its updates are not generally monotonic and its
failure branch preserves projected dependency custody, whereas real errors may
occur after dependency release. No concrete journal-prefix or failure-cleanup
claim follows from it. Exact real-Context failure-prefix coverage remains in the
separate CPU packet; it is not promoted to a formal adapter theorem here.

The existing real-Context regressions exercise exact 513/513/219-step resumptions
on a 256-node chain, retained successful cursor prefixes before later failure,
and a 303-operation graph. Hardware ignores and all native/generated/resource
and matched-performance gates remain open.
