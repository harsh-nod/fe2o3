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

All nine campaign phases pass from signed source
`53aba0d658f0d874f3c4f7ef18ff35f971504375`:

| Check | Result |
| --- | --- |
| Runner calibration | Six tests pass |
| Source-body extraction | Rustfmt-parsed bodies match exactly |
| Rust compiler identity | Retained |
| GNU runtime, all features | 1,416 pass, 22 hardware-only ignores |
| musl runtime, all features | 1,416 pass, 22 hardware-only ignores |
| Runtime doctests | 46 pass |
| Default-feature check | Pass |
| All-feature/all-target strict Clippy | Pass |
| Workspace formatting | Pass |

The opening and closing source brackets match all 5,955 inputs. The baseline
`b8804de3ec7513d7bb41be52bbaa7d25570ba9c3` and final source signatures were
verified. Preliminary focused tests, checker calibration and Clippy logs are
also retained; final full suites cover the final source, including the counted
helper's nested function.

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

## Retention And Cleanup

[retention.json](retention.json) inventories all 52 scratch artifacts copied
byte-for-byte into `retained/`, including phase commands, outputs, results,
source brackets, signature receipts and both formatted function bodies.
[cleanup-before.json](cleanup-before.json) records exact owned path identities
and all nine terminal process groups. [cleanup-after.json](cleanup-after.json)
records successful exact-owned removal, independent lstat and parent-listing
absence, continued process-group absence and matching retained hashes.
The two removed paths accounted for 1,940,402,176 allocated bytes. No remote
resources were created. `SHA256SUMS` seals the packet files except itself.

## Reproduction

From a committed tree, with the baseline Git object, pinned toolchains and
dependencies available, and an existing task-owned Cargo target:

```sh
python3 -I -B docs/evidence/dev-completion-reconciliation-body-2026-09-25/run.py --output OUTPUT --target CARGO_TARGET
```

The runner uses the authenticated inherited owned-process controller, brackets
source inputs, retains each command/result/group observation, and allows reuse
of the owned build cache. This is not a clean-room tool/dependency closure audit
or relocated evidence replay. Hardware-only ignores are not execution passes.
A1/A2 and accepted runtime lane checkpoints are unchanged.
