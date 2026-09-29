# Root Control Foundations Checkpoint

## Scope

Code checkpoint: `dece10f8fd5819b43bfbc87ef93d080c88df271b`, following
`3b9d968c6`. This checkpoint adds:

- Bounded, single-attempt packet send and authenticated receive, reusing the
  existing readiness syscall and ancillary ownership implementation.
- A metered, opaque original-trace identity, retained by root observations and
  checked before broker revalidation. It grants no liveness or compiler authority.
- A private completed-publication gate used by both native issuer families before
  local occurrence retirement, including replay without a local occurrence.

The gate requires Ready plus the exact durable Worker/Published-anchor/advanced
issuer join and matching publication/ACK. A previous completed receipt remains
queryable during the next transaction but cannot retire that pending transaction.

The [root-control contract](../compiler-execution-root-control.md) records the
remaining integration. There is **no root RPC session or FD12 ABI migration in
this checkpoint**. No kernel changes production classification, and no protected
proof, GPU result, or 47-kernel completion is claimed.

## Validation

Final runs used pinned nightly `2026-04-03`, locked offline Cargo, one build job,
serial tests, a 1,200-second timeout, a 12-GiB address-space cap, and disabled GPU
visibility. Source and tool snapshots stayed unchanged throughout each run.
Final tested snapshot: 9,296 git-visible files, before this documentation-only
evidence update:
`e27c42533ef6bf6259c1d9b4657f64b801b60bbe8ace0043e712d9db2cd86838`.

| Run | Result |
| --- | --- |
| `root-control-integration-final-ra` | Broker, issuer, coordinator, and spawn all-target checks passed |
| `root-control-docs-final-ra` | 77 broker and 56 spawn doctests passed |
| `root-retirement-broker-ra` | 256 passed, 97 failed, 6 ignored |
| Native issuer subset | All 96 passed, including all 10 new retirement tests |
| `root-control-spawn-final-ra` | 214 passed, 27 failed, 7 ignored |
| New packet subset | Eight passed; seven blocked by socket prerequisites |
| Original-trace identity lifecycle | Blocked at the initial ptrace operation |
| Hygiene | Eight checker self-tests, source delta policy, scoped formatting, and whitespace checks passed |

The retirement fixtures exercise actual signed journal recovery at all five
durable commit positions, exact replay after a lost completion response, stale
carriages during subsequent Prepared/Issued states, substitution, durable
tampering, and exact/insufficient resource bounds. They construct neither
Admission nor NativeOccurrence and do not establish actual lock release.

A new 64-case matrix executes real packet receive and descriptor cleanup without
socket-option changes. Missing credentials always refuse. It does not substitute
for successful authenticated-socket tests. The original-trace quota test and
inactive subprocess entrypoint pass; neither proves the blocked lifecycle matrix.

## Failures and Limits

The broker's 97 failing test names exactly match the preceding checkpoint. That
checkpoint separately reproduced all 97 on its prior executable. See the
[previous evidence](native-session-custody-20260929.md); the full package is not green.

All 19 pre-existing spawn failures reproduce on the preserved prior executable,
SHA256 `61dc33e9d7522b05777e503910a0ab20764ad09388fa4253951e234ebe523243`.
Its current run reports 204 passed, 19 failed, 7 ignored. The eight additional
failures are new tests denied at `SO_PASSCRED`, send/buffer operations, or ptrace.
No check was weakened and no test was skipped to obtain a passing result.

Live broker coverage still needs same/different-trace and Root/Service
substitution checks using real custody, plus lease/token retention on retirement
failure and release before ACK exposure. The subprocess identity matrix also
needs an environment allowing its actual trace operations.

All three SSH aliases (`mi350`, `mi350-2`, `mi300x`) failed DNS resolution. Git
access to GitHub also failed DNS resolution. No remote job or remote scratch
directory was created during this checkpoint. Browser access to the public issue
is separate from Git/SSH connectivity and does not establish publication.

## Next Integration

Keep the original trace and root occurrence in a per-attempt owner, separate
from the replaceable issuer connection. Authenticate every RPC against the
actual issuer and launch generation. Bind retirement to the private durable gate,
retain an exact tombstone across failed replies, and gate all receipt-exposing
operations and subsequent Prepare. Coordinate both V3 launch paths before
requiring FD12; preserve V2 and refuse unsupported routes explicitly.

Then validate the owning production attempt, runtime/descendant custody,
protected proof execution, safe host activation, and target-matched positive and
negative qualification for all 47 kernels. These requirements remain incomplete.
