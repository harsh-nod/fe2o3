# Authenticated Compiler Observer Channel

Date: 2026-10-04 UTC. Base: `873fb3e3d940985b5d267e3320a898af776daeb7`.
This is a broker integration checkpoint on the ordinary multi-GPU application
critical path, not production deployment or A3 completion.

## Implemented

- Move-only prepared root session, bound root observer and protected issuer channel.
  Root creation checks exact root credentials and the original compiler client;
  binding checks retained supervisor/issuer pidfds, parentage, namespaces and
  service profiles. Measured executable provenance remains the deployer's duty.
- Exact kernel `SCM_CREDENTIALS` on every packet, original pidfd continuity,
  canonical bounded packet/descriptor rosters, immutable launch/session binding,
  monotonic request/operation identities and fresh nonces. Socketpair endpoints
  are eagerly kernel-autobound before exposure, retaining both exact abstract
  addresses. Ordinary compiler-peer admission still requires unnamed addresses.
- One nonblocking root send or receive per step, retained backpressured replies,
  full occurrence revalidation before delayed replies, and bounded operation/session
  deadlines. Synchronous filesystem observation has input bounds, not an interruptible
  wall-clock bound.
- Begin transfers both original publication lock descriptions with the independently
  reconstructed subject and occurrence identity. The private issuer guard retains
  them through record signing, durable commit and the post-commit continuity check.
  Explicit successful Finish retires the operation; failure/abandonment poisons the
  retained observer across all issuer admission paths, with no local fallback.
- Root retains active custody until its exact issuer pidfd confirms exit on failure.
  Idle EOF instead allows bounded clean exit without admitting further requests.
  Drop may wait indefinitely for containment; abrupt root death relies on the
  already qualified transferred-lock retention, not on a destructor running.

## Qualification

| Campaign | Result |
| --- | --- |
| Artifact transaction unit suite | 218 passed, 1 ignored |
| Broker unit suite | 198 passed, 8 ignored |
| Coordinator unit suite | 31 passed |
| Supervisor unit suite | 47 passed, 2 ignored |
| Issuer unit suite | 3 passed |
| Broker and artifact doctests | 52 + 13 passed |
| Observer subset, four test threads | 15 passed, 4 ignored |
| Broker all-target Clippy, `-D warnings` | Passed |
| New-module rustfmt / diff whitespace checks | Passed |

The main campaigns total **497 unit tests and 65 doctests**. Nested subprocess
helpers and the concurrent rerun are not added again.

New coverage includes two fresh observations across actual durable Prepare/Issue
commits and record recovery, injected journal failure with terminal guard poison,
same-UID wrong-sender rejection, stale root precedence over queued data, replay
containment, exact/missing/excess descriptors, truncation, backpressure, overlapping
operations, live-root session closure, and EOF versus zero-length datagrams.
A backend mutation between Revalidate handling and response transmission rejects
before reply. Independent lease probes remain Busy during active operations and
reacquire after exit. Native reviewers found the autobind/EOF, clean-exit and delayed
revalidation cases; all were fixed and directly tested.

The subprocess driver uses the existing waiting-rustc-shaped executable and
synthetic semantic handoff, with real process/publication observation and the
production journal bodies under test keys. It deliberately uses same-UID,
test-only profile admission. It does **not** exercise full protected issuer
admission, genuine Rust extraction/receipt acquisition, a production coordinator
registration table, or a new formal proof. Existing root/UID1000 cases were not
rerun in this campaign. No MI300X state was touched and no GPU/performance claim
is made.

## Next Integration

Add authenticated root/supervisor registration using only the accepted handoff's
original compiler peer/pidfd. Do not retain Cargo's readiness/control endpoint.
Bind the actual measured launched issuer before waiting for its readiness; maintain
a bounded root session table and reject live issuer identity reuse. Contain all
bound issuers before releasing their root sessions during deployment shutdown.

Use supervisor FD13/FD14 for registration endpoint/original root pidfd. Transfer
each observer endpoint/root pidfd to issuer FD12/FD13, raise the issuer private
descriptor floor to 14, and require observer attachment before recovery/readiness.
The deployed entrypoint is not wired by this checkpoint. Then qualify genuine
compiler acquisition, conditional application admission and selected-pair GPU
fill/copy/readback in both directions.

## Evidence

`evidence.tar.gz` contains final campaign logs, exact commands/limits and the code
patch. Code patch SHA-256:
`0ed358cb59b51cc650bd1d0adb3ab0cc9a1fa4eae70df79b3ed93ee99fe30763`.
The tested broker executable SHA-256 is
`715256607cf4a02121cbfbb67e72e0475dae2b3ee00c077f3c764117db0d2418`.
Archive SHA-256:
`f7fb4c6c8ab3153d40d47850077347381f9b4abc4d93300f4a43677e27df6903`.

Owned scratch was removed after archiving. Existing unrelated work and shared-host
resources were left untouched.
