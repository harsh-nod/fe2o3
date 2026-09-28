# Physical debugger startup refusal and bounded diagnostics — 2026-09-26

This advances #281's physical observation infrastructure, not physical capture
qualification or a new accepted milestone. Broad exits remain
**M1/V1/V2/U1/U2/U3 (6/18)**.

## Actual private attempt

After exact final-R10 MI2 startup, loaded-file review, source-bound controller
CPU qualification and benign owned-family controls, root made one bounded
private attempt. It failed during debugger setup with Changed, after spawning
the debugger child and before the controller sent any MI command. No physical
capture report was produced. The later empty-report framing error was secondary.

Independent source review narrows the failed operation to child/executable
identity, scope membership or exact argv. The old diagnostic does not retain
which of those checks failed. A startup race, GPU trap failure or particular
runtime-library error is not established.

The controller's own cleanup reported missing stream EOF and its separate
cleanup deadline expired. Source review explains the pre-reader limitation:
before readers start, no reader can supply EOF to the cleanup channel. The
diagnostic change below deliberately does not fabricate EOF or change cleanup.

Separate inner and outer receipts establish actual family cleanup: exact
request/generation/manager and terminal-ACK joins, pidfd exit, ECHILD, stream EOF,
empty scope and no child cgroups. Root independently reread the receipts,
rechecked absence of the three recorded process identities and exact scope,
and rehashed 90 fixed inputs plus 25 retained products. These checks establish
cleanup of this attempt, not physical success or host-global GPU inactivity.
Conservative unknown native-attempt and GPU-dispatch fields remain unknown.

| Retained failed-attempt record | Bytes | SHA-256 |
| --- | --- | --- |
| Failed root receipt | 95,650 | `43199b35fdddcdb93b439ef1396999a502eca6c2d1b9a84d1be1c99ae5fce447` |
| Same-generation wrapper audit | 17,052 | `9d840563a780e7a17620a3f0e28e37d6dd2d1701013e0dd270f5b6ddd5df555f` |
| Independent root cleanup audit | 1,275 | `13803ed2bc6e89b6d43ac28304bf1743e1aa628a4df9cb6778842a1781499306` |
| Independent diagnosis | 12,217 | `5fca86dcb086f0a69f6dd6bc8db072c5241e430b94365948c8c53e0792f6d19b` |

No failed generation, qualified controller/debugger or old deployment was
overwritten. There has been no retry with the diagnostic successor.

## Implemented diagnostic successor

The disabled physical-v2 controller now retains a fixed startup stage/substage
and facts already observed by the failing path: optional child PID and initial
stamp, the last successful wait observation, whether debugger custody completed,
actual reader-start bits and bounded cmdline length/equality. Missing facts stay
unobserved. No raw process strings, environment or paths are emitted.

The first failure is copied before teardown and appended only to stderr.
The original Refusal, successful protocol/publication, MI command roster,
identity checks, I/O observations and time/stream/record limits are unchanged.
Custody and executable checks share the diagnosed implementation with existing
callers, preserving observation order and short-circuit refusals. Bookkeeping
does not imply identical process timing.

The public PROFILE remains None, runtime bindings remain null, and the private
bound profile is not copied into the package. The source verifier checks 25
selected leaves / 21 Rust files / 225,296 bytes with its unchanged caps.
Historical evidence and V1 compatibility records remain historical, not
retroactively rewritten for this code.

## CPU qualification

The new public source was formatted, its exact manifest hashes regenerated,
then checked under the finite root runner on mi350:

- 25 Node package tests passed.
- 58 Rust protocol tests and 39 native-transport tests passed, including nine
  new pure diagnostic controls; these tests do not launch the debugger.
- Strict all-target Clippy, controller build and diff checks passed.
- No controller binary, debugger, target or GPU kernel was executed by this gate.

Completed receipt: 29,583 bytes /
`3d815f04985ef45214da0beb7026949dcb7c37e5cc7c0ac8cb8e45e231dc1d1a`.
Recorded HEAD: 97edb07a644dc0af11b9fd0824761bc603dbc9d6 plus source census
8,363 files / 119,566,651 bytes /
`60b0438f3341b3aea20adfa169787e498018bc086439a351af06d15f02ca91ae`.
This binds that source snapshot, not an arbitrary later checkout.

Synthetic controls preserve every original refusal through all 24 stages,
observation order, first-failure copying, unknown-versus-observed values and a
finite formatting envelope. They are not native identity qualifications.

A later private attempt needs a fresh bound controller build and acyclic
family/deployment/request pins plus applicable process qualification. The old
failure does not justify weakening checks, changing timeouts or retrying
automatically. Actual physical capture, same-stop register/memory bytes and
downstream hardware exits remain unqualified.
