# Protected Launcher Namespace-Filter Compatibility

Date: 2026-10-04 UTC. Base: `48e1ff24600e5c6301720542f1aac2d08b514c9f`.
This removes a concrete launcher compatibility blocker on the working multi-GPU
critical path. It is not full systemd deployment startup, genuine compiler receipt
acquisition, a new formal proof, GPU qualification or HIP/HSA parity.

## Implementation

Both the root protected-service spawn and supervisor issuer spawn retain their
existing `clone3(CLONE_PIDFD | CLONE_CLEAR_SIGHAND)` first attempt. Only `ENOSYS`
permits a legacy x86-64 `clone(CLONE_PIDFD | SIGCHLD)` retry. The legacy syscall
writes the original CLOEXEC pidfd through `parent_tid`; there is no numeric-PID
reopen, shared VM, shared handlers, shared file table or namespace creation.
The unit's `RestrictNamespaces=yes` remains unchanged.

The calling thread blocks the complete eight-byte kernel signal set before
either attempt, including libc-reserved signals. The child retains that mask
until every catchable disposition is reset, then clears it through the existing
normalization path. The parent first owns the pidfd, restores its exact previous
mask and only then proceeds to validation/readiness waits. Its private mask owner
cannot move to another thread. Failed clone errno is captured before restoration.

Restoration failure cannot return successful launch custody. With a created
child, cleanup retains the owner and artifact spawn reservation until exact
containment/reaping. A final restoration failure then aborts; a successful retry
returns the original error. Supervisor unexpected wait errors retain pidfd and
reaper reservation. Root Drop retries synchronous containment instead of ignoring
failed cleanup. Persistent unknown effects can therefore retain custody without
a finite timeout; they are not treated as confirmed exit.

## Qualification

`final-check.sh` runs locked/offline on `nightly-2026-04-03`, with four build jobs,
optimized test bodies, debug assertions and overflow checks enabled.

| Check | Result |
| --- | --- |
| Coordinator library | 31 passed |
| Supervisor library | 53 passed, 3 ignored helpers/explicit cases |
| External-anchor coordinator library | 3 passed |
| Protected-service spawn library | 5 passed, 2 explicit root cases ignored |
| Four crates' doctests | 34 passed |
| Four crates' all-target Clippy | `-D warnings`, passed |
| Freestanding static-launcher CTest | 15 passed |
| Complete supervisor suite under installed systemd filter | 53 passed, 3 ignored |
| Real static launcher across both exec boundaries under that filter | passed |
| Isolated root cases, with and without that filter | 20 passed |
| Changed Rust formatting and diff whitespace | passed |

Filtered reruns repeat library tests; they are not additional unique unit tests.
The syscall matrix forces ENOSYS, EPERM, EINVAL, EAGAIN and fallback-clone failure.
After fixture setup, `pidfd_open` is denied even for successful launches, while
the original pidfd supports liveness, signaling and reaping. Nonempty masks,
including reserved signal 32, are restored after successful and failed launch;
sibling masks remain unchanged and unwind restoration is checked.

Test-only syscall probes observe the actual legacy child's fully blocked mask,
inherited caught and ignored dispositions, and queued SIGWINCH before the real
normalizer runs. They check the canonical dispositions/mask afterward; executing
the inherited handler fails the child. Restoration and transient-wait faults are
thread-local `cfg(test)` injections, not production environment switches.
Two injected wait errors with a real pidfd are followed by successful containment;
root Drop has a separate retry case. Persistent restoration failure is observed
by an isolated subreaper, which confirms no abandoned issuer remains after
fail-stop. The supervisor controller separately retires its known compiler fixture.

An initial root filtered fail-stop repeat timed out after containment. Diagnostic
readback showed empty child lists, `CoreDumping: 1` and the aborting thread blocked
in `pipe_write`: WSL's piped crash collector was delaying intentional SIGABRT.
`RLIMIT_CORE=0` does not disable that collector. The persistent-failure fixtures
now set `PR_SET_DUMPABLE=0` after exec and before fault injection. This changes
only the disposable test processes; the cleanup loop and production profile are
unchanged. The initial timeout log is retained alongside the final repeat.

The namespace wrapper loads the installed
`libsystemd-shared-255.so::seccomp_restrict_namespaces(0)` after setting
`no_new_privs`, then execs the selected test. It observes `clone3 -> ENOSYS` and
`setns`, user/mount `unshare`, and user-namespace `clone -> EPERM`. This uses the
installed implementation, not a hand-written equivalent of its namespace policy.
The separate error-matrix BPF filters are test fixtures for syscall failures.

Root cases run in private PID, IPC, UTS, network and temporary namespaces with a
read-only host filesystem view. They create a real UID61000 protected child,
validate the full process profile while gated, then execute a minimal static
exit image. Source test binaries are hashed before/after; the root-accessible
copy is compared byte-for-byte. No MI300X resources, host ptrace setting or host
systemd unit were changed. Owned temporary builds and helper artifacts are removed
after evidence packaging.

## Limits and Next Gate

The supervisor fixtures use test credentials, test-key policy and the explicit
nonproduction twelve-source path. The real static-launcher witness crosses both
exec boundaries but does not exercise production observer registration or genuine
compiler publication. The root profile fixture is not a measured service daemon.
Applying the actual namespace filter does not exercise every systemd unit setting,
private mount arrangement, activation descriptor or deployment measurement.

Rebuild and qualify the complete measured static deployment next, then acquire
and verify a genuine selected-rustc receipt, consume conditional artifact evidence
into exact prepared invocation custody, and run admitted fill, settled staging,
PUBLIC XGMI and full guarded readback across two selected free GPUs.

## Evidence

`evidence.tar.gz` contains the complete source patch, scripts, final logs, source
and executable hashes, installed systemd identity/disassembly, and the small
freestanding launcher. Test executables and intermediate build trees are omitted.
Scripts retain the original local paths; adjust those when replaying elsewhere.

| Artifact | SHA-256 |
| --- | --- |
| Evidence archive | `b90c59cf76b1976eda4cbac34cc0f1d58797b1234e71bfca6e5c933f6fa3ae46` |
| Affected-crate source patch | `e4aa6ab1a93597e06d19790bbb318d7cb89515ac15259614b8793043e8dd5171` |
| Namespace-filter wrapper | `0095de334aa84750dbeabdd1ea0ec74e5284be99ce0b45786af99b8f92588a9a` |
| Static launcher | `aebb28754e5eeabb24c0aa2e727125241bda4de8afae2ef4cd55b4927865eced` |
| Root spawn test executable | `d268577cfd3cfdd85d51c6e6d079543431e2322026f39b3c06c7fea4498a0c65` |
| Supervisor test executable | `d82b152151bba9e2344ae99ff2cbd5efc679efab8c76333231310a32eb85bf39` |
