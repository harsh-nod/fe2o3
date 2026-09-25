# Shared Completion Reconciliation Body

The production planner and a counted test entry use one complete executable
body, including record selection, validation caching, cursor updates, stack
traversal, settlement calls, rejection and the fuel-exhaustion result. The
production step hook is empty. No scheduling policy, public API, allocation or
backend call is added.

## Scope

This is extraction and CPU regression development, not a new Verus theorem,
Context refinement, native qualification or performance result. The complete
finite-graph proof remains open. In particular, real map/custody correspondence,
journal-derived input status, settlement failure prefixes, quarantine and
callback effects cannot be replaced by an assumed success gate.

The graph may contain up to the Context submission bound, not just 256 nodes.
The 256 limit bounds each dependency roster and the traversal stack. A single
planner pass performs at most 513 local steps; it may yield Pending after all
physical success facts are retained. A public observation can invoke the planner
more than once, and adapter validation costs are separate from the step count.

## Checks

Validation is in progress. No campaign success is claimed until final results,
source continuity and cleanup records are retained.

The new real-Context tests cover:

- A 256-node chain, with and without the version journal, physically completed
  by the mock backend and retained through the normal observation method. Passes
  take exactly 513, 513 and 219 steps; committed prefixes contain 86, 201 and 256
  successes. Cursors, original terminal facts, dependency custody/counts, journal
  readers, callback order, full destination bytes and no observation calls are
  checked at the relevant boundaries.
- A successful cursor advance followed by a contradictory retained Failed child.
  The original success fact, cursor prefix, readers, dependency retains and
  pending callbacks survive terminal quarantine without another backend call.
- A 303-operation, three-level graph with a 256-dependency node. It resumes over
  bounded passes without backend observations, settles all retained operations,
  releases readers/dependencies and validates the complete destination buffer.

The source comparison parses and formats the old function and the extracted
macro body using Rustfmt, after expanding the fixed Context/identity parameters
and empty production hook. Exact formatted equality is an extraction check,
not a Rust semantic theorem. Its negative controls preserve valid Rust syntax.

## Reproduction

From a committed tree and an existing task-owned Cargo target:

```sh
python3 -I -B docs/evidence/dev-completion-reconciliation-body-2026-09-25/run.py --output OUTPUT --target CARGO_TARGET
```

The runner uses the authenticated inherited owned-process controller, brackets
source inputs, retains each command/result/group observation, and allows reuse
of the owned build cache. This is not a clean-room tool/dependency closure audit
or relocated evidence replay. Hardware-only ignores are not execution passes.
A1/A2 and accepted runtime lane checkpoints are unchanged.
