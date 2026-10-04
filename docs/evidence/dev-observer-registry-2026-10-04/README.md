# Production Observer Registration and Launch Wiring

Date: 2026-10-04 UTC. Base: `4012c23798015b68d88b5508407a8118598329f7`.
This is a CPU/process integration checkpoint for the working multi-GPU critical
path, not an A3 completion, genuine compiler acquisition or GPU execution claim.

## Implementation

- Reuse the retained root coordinator for a private authenticated supervisor
  registration channel. Bind original compiler peer/client pidfd custody, the
  exact canonical launch and the actual child issuer's original pidfd.
- Keep supervisor bootstrap FD11 readiness/EOF-only. Registry/root pidfd use
  FD13/14. Issuer observer/root pidfd use FD12/13; private staging starts at 14.
  The fourteen-source static launcher checks exact cardinality and roles.
- Require production observer admission before issuer recovery/readiness; no
  optional local-observation fallback. Signing remains in an empty-capability
  service; only the root coordinator receives DAC/ptrace observation capability.
- Bound registration to sixteen sessions. Service each observer once per step,
  retain one backpressured control response, expire authority-free registrations
  and preserve active custody through exact issuer containment. Start the bind
  window only after successful delivery. Close-only transferred owners preserve
  inherited endpoint aliases.
- Bound registry mutex acquisition and issuer binding by the existing absolute
  launch deadline. Static continuity hashing stays at one-second cadence while
  observer traffic advances without waiting for that interval. Registration
  preparation has a separate bounded lock wait and RPC wait.
- Keep all observer owners through shutdown before releasing supervisor/anchor
  custody. Individual session failures do not prevent unrelated sessions from
  progressing; registry-control failure closes admission and contains issuers.

## Qualification

The archived `check.sh` runs locked/offline with `nightly-2026-04-03`, four build
jobs, optimized test bodies, debug assertions and overflow checks enabled.

| Check | Result |
| --- | --- |
| Broker library | 203 passed, 11 ignored helpers/root cases |
| Coordinator library | 31 passed |
| Issuer library | 3 passed |
| Supervisor library | 49 passed, 2 ignored |
| Deployment library | 78 passed |
| Four affected crates' doctests | 82 passed |
| Four affected crates' all-target Clippy | `-D warnings`, passed |
| Observer subset, four test threads | 20 passed, 7 ignored |
| Changed Rust files' formatting and diff whitespace | passed |

The observer subset repeats tests from the library suite, not additional unique
coverage. Registry subprocess cases cover successful Begin/Revalidate/Finish,
concurrent idle/active sessions, capacity, expired pending delivery, substituted
issuer pidfd, duplicate binding and registry loss while publication locks are held.
Existing replay, delayed-mutation and poisoned-commit cases remain passing.

Three explicitly selected ignored tests also pass in separate private real-root
namespaces: `cross_uid_owned_occurrence`,
`cross_uid_publication_lock_contention` and `cross_uid_registered_observer`.
The last uses a UID1000 waiting-rustc-shaped publication and UID61000 protected
supervisor/issuer with empty capabilities. Production registry, observer, service
admission and full process-profile checks are enabled. The existing test-key
durable-record helpers are still used; these are not the measured static issuer
entrypoint or a genuine selected-rustc receipt transaction.

The root harness has a read-only filesystem view and private PID, IPC, UTS,
network and temporary namespaces. Its UID-accessible executable copy is compared
byte-for-byte before execution, and the source binary is hashed before/after.
No MI300X resources, host systemd configuration or host ptrace policy were changed.

Repeated qualification found an existing fixture defect: chown after publication
changed payload ctime after the ready record was committed. The fixture now
transfers ownership before publication and publishes in an owned client-UID
subprocess with a private client-owned path-guard directory. No production
metadata check was weakened. The initial metadata-failure log is retained.

## Remaining Gates

The installed systemd 255 namespace filter rejects `clone3` with `ENOSYS` under
the unit's existing `RestrictNamespaces=yes`. These root tests do not apply the
systemd unit. Its launch compatibility must be fixed and qualified before the
measured static deployment campaign. Disabling namespace restrictions is not
an equivalent fix. The candidate next change is an ENOSYS-only legacy
`clone(CLONE_PIDFD)` path with full pre-clone signal masking, exact parent-mask
restoration and retained child containment; it is not implemented here.

Then acquire/verify a genuine selected-rustc receipt, consume the conditional
artifact into exact prepared invocation custody, and qualify admitted fill,
completed-value staging, settled H2D, PUBLIC XGMI and full guarded readback on
two selected free GPUs in both directions. This checkpoint proves none of those
remaining gates and introduces no new formal theorem or performance claim.

## Evidence

`evidence.tar.gz` contains the source patch, scripts, complete final logs,
executable hashes, base revision, failure log and inspected systemd library
identity/disassembly. Scripts retain the original local qualification paths;
adjust those paths when replaying elsewhere. The large test executable is not
included.

| Object | SHA-256 |
| --- | --- |
| Evidence archive | `6ff7c8de0fd8f5b853bde5e166f149a512f35ff227abafb1c302eb7736873f70` |
| Source patch | `2178088c1f7c86828e7a3fa7ed0d1c0f9b4dce45ad44ba46e4dc33fb850cbd11` |
| Root test executable, before and after | `becef50d27a2b3ddbb3e8c833546b22e0cc6de4b770726750ddfcd4fbcac6390` |
