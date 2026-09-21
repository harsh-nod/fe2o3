# Private runtime-origin retention qualification

This test-only slice retains the simulator's opt-in operation-origin context next
to the actual legacy debugger transcript. It is not a public debugger API,
serialized trace format, browser capture format or completion of #281 V2.

## Ownership and bounds

An origin-aware collector wraps the existing transcript collector and forwards
terminal callbacks. Context is recorded only for records the wrapped collector
actually accepts: Stop retains the accepted record; Drop does not. A mismatch in
ordinal, invocation or runtime operation site invalidates the join.

The final wrapper move-owns both transcript and origin rows. There is no Clone,
Deserialize, public attach or replacement constructor. Lookup borrows the pair
from that sealed owner. The compact sidecar stores activation and dynamic-attempt
identity; it borrows invocation/site from the matching retained record.

Each row is at most 32 bytes. Construction charges actual reserved capacity plus
fixed retention metadata, with caps of 1,000,000 rows and 32 MiB metadata; no later
growth is allowed. Capacity, allocation and byte failures preserve an explicitly
unavailable suffix rather than guessing identities. Coverage distinguishes
disabled, complete, prefix-truncated and invalid-join states. Simulator-origin
unavailability remains separate.

The private pairing scan matches exact full invocation, activation, attempt and
static runtime site. It charges cumulative scan work before examination and
returns explicit missing, malformed, exhausted or unmatched outcomes. It does
not infer caller frames, lifetime identities, resource reuse or GPU register
values.

## Qualification

On the primary checkout at HEAD b8f7abf8c661fec5b7f16049777393defac0ba8f
plus the recorded worktree, logs/phase11-debug-retention-r4.json passed all
15 selected tests. Receipt SHA-256:
da3304dd7115d1e669b8e3d1674b21f3b36e697863739b819f66569c65e03c39.

The unchanged before/after Rust/Cargo/crate-README census was
8788c22879b07b23e434aa1f97f6dea6429466020082514177825f430331e724
(3,766 files, 73,619,030 bytes). HEAD alone did not yet contain the worktree additions.

Tests cover actual collector acceptance, prefix cutoffs, bad joins, malformed
reservation, borrowed lookup, exact pairing, full-invocation separation and
runtime success/error/stop behavior. Runtime comparisons preserve complete
successful execution equality or the exact structured execution error. The lookup
comparison additionally requires successful nonempty captures, avoiding vacuous
equality between two failed executions.

Earlier failures are retained:

- r1 did not compile because the top-level simulation error type is not PartialEq.
  The corrected helper compares the actual supported success and execution-error
  structures; it does not stringify errors or accept arbitrary failures.
- r2 exposed fixture limits whose default implied resident bound exceeded the
  fixture's 32 MiB cap. Fixtures now declare small explicit operation, invocation,
  allocation, step, event and memory-record limits.
- r3 exposed schedule decision limits above the simulation step limit. Decisions
  now use the smaller explicit bound, including tiny-step fault controls.

The r4 pass applies only after those corrections. The unchanged simulator's
normal callbacks and terminal errors remain authoritative; sidecar failure cannot
turn a failing kernel into success.

## Remaining work

Origin-aware cursor/session navigation, reverse break/watch replay, actual
loop/helper source acceptance for that navigation, public API and protocol
contracts, and resource lifetime visualization are separate work. The foundational
runtime-origin source observation is documented in
[runtime-operation-origin-qualification-v1.md](runtime-operation-origin-qualification-v1.md).

This module remains under cfg(test). No existing resource budget, production
policy, wire schema or CLI command was changed.
