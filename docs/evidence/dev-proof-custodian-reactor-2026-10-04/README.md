# Proof Custodian Reactor Qualification

Date: 2026-10-04. Source commit:
`5c724a9732a6cc21999df2165eccb7861225318f`.
Parent: `427864f63a9b7e7ccf177d71e5b2670a9fb371e8`.
Source patch SHA-256:
`ee44bcd18334d363724b32333f871763c2f22d0e61dcc703df1bc6fd327b64df`.

This qualifies the [pollable root lifecycle](../../runtime-proof-custodian-polling-v1.md),
not a deployed manager, application proof handoff or multi-GPU execution.
Primary integrated the code; three native read-only agents reviewed lifecycle,
qualification and the remaining ordinary two-GPU critical path.

## Results

- 22 unit tests passed across proof-custodian, protected-service-spawn and
  protected-service-profile. Sixteen compile-fail doctests passed.
- Strict all-targets Clippy passed for proof-custodian and protected-service-spawn;
  formatting and diff checks passed.
- The rebuilt musl controller has the secure entry point, no interpreter,
  dynamic segment, NEEDED entries or unresolved symbols.
- Independent non-root measurement produced a fresh fixed deployment record.
- All 18 private real-root cases passed; every outer cgroup was removed and every
  private PID namespace was independently observed empty after its wrapper exited.

The seven new polling cases cover:

| Case | Observed result |
| --- | --- |
| `poll-pair` | Two controllers started and genuinely proved on one root thread; each retained its own proof subject. Releasing one left the other probeable. |
| `poll-cancel-proof` | Cancellation during a live Verus `--no-cheating` process completed while the other controller progressed to a usable retained proof. |
| `poll-cancel-gated` | Cancellation before profile/exec polling reaped the original child and removed its still-unattached scope. |
| `poll-drop-gated` | Dropping a gated pending owner contained and reaped its child and removed the scope. |
| `poll-deadline` | Expired startup poisoned the retained pending owner; polling cancellation completed cleanup. |
| `poll-proof-deadline` | Expired proof operation retained custody and was cancelled while the other genuine proof completed. |
| `poll-probe-timeout` | Timeout after a genuine Probe permanently poisoned the channel; later probing rejected, cancellation completed and the other proof remained usable. |

The existing eleven cases were rerun against the same rebuilt controller: good
proof, empty-scope Drop, ready cancellation, live-Verus cancellation, wrong analyzer
measurement, wrong runtime identity, altered payload, wrong controller measurement,
same-byte config replacement, FIFO config and illegal post-proof control transition.
Controller cases check original-child reap, owned-scope removal and survival of an
unrelated sibling. Pending shells also reject cancellation after moving their still-
live controller/proof owner out, rather than falsely reporting containment.

## Evidence And Limits

`evidence.tar.gz` contains the canonical deployment, source patch and hashes, binary
hashes, build/qualification/cleanup logs, original inputs, retained subjects,
review notes and exact qualification scripts. `MANIFEST.sha256` covers its members.
Archive SHA-256:
`fde4977b5e4531cd8d6aecedc9ae61f18580f1689ed414b3e0e4f08c510d13d3`.

Inputs came from the preceding [fixed-launch campaign](../dev-proof-custodian-launch-2026-10-04/README.md).
The compiler was not rerun. Authenticated analysis and protected Verus execution
were fresh; independent executions need not have equal proof-subject bytes.
The first pair run incorrectly asserted cross-run equality after both proofs
completed. That fixture assertion was corrected; its failure/cleanup logs and an
initial Clippy failure remain in `diagnostics/`.

Source review and execution tests do not formally prove every Rust/syscall
lifecycle transition. The cancellation-latch unit constructs a failed state;
the reaping-loss test induces actual `ECHILD`. Neither fabricates GPU settlement.
Aggregate-empty timeout starts only after direct-child reap; a stuck child can
remain pending, and Drop is blocking. Retained root Probe/Release remain blocking.
No MI300X, hardware benchmark, production manager deployment or application lease
was used. Host installation paths were untouched: all fixed-path installation
and runtime provisioning occurred inside disposable namespaces. Only owned scratch
and fresh qualification cgroups were used.
