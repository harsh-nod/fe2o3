# Bounded Process Observations

This is infrastructure for the native compiler-execution launch path in #271
and #272. It does not establish a production session, protected proof, safe GPU
launch, or any of the 47 kernel qualifications. Native consuming process
creation, cleanup ownership, readiness and serving remain unfinished.

## One Set Of Predicates

`fe2o3-protected-service-profile::observations` holds the mechanical Linux
process-profile and namespace checks. The public V1 APIs are compatibility
adapters. `ProtectedServiceProcessProfileV2` and `ProtectedServiceNamespaceSetV2`
invoke the core directly; they do not convert admitted V1 owners or retry through
a legacy path. The supervisor's duplicated implementation has been removed.

Profile capture observes the kernel capability ceiling and checks the current
process. Revalidation requires exact real, effective, saved and filesystem
UID/GID; no supplementary groups or capabilities; locked securebits;
`no_new_privs`; nondumpability; zero core limits; umask `077`; no tracer; and an
unchanged capability ceiling. Namespace capture records ten device/inode pairs
and requires PID/time child namespaces to match the current namespaces.

The proc-visible check on a target PID is deliberately weaker than a complete
child admission. The child must independently check securebits, dumpability,
core limits and signal state. Namespace continuity is a separate observation.
The caller must pin PID identity/liveness and hold the child behind its gate.
Snapshots are not atomic confinement or process custody.

## Finite Observations

Status uses a fixed 64-KiB buffer plus an over-limit sentinel. Capability-ceiling
text uses 64 bytes plus a sentinel. A short positive read is progress, not EOF;
success requires an explicit zero-byte read. Each continuing read consumes at
least one buffer byte, so the maximum number of attempts is finite. Interrupted
or failed reads are refused without retry. A full sentinel buffer is rejected.

Paths, namespace identities, parser fields and error values use fixed storage.
Required, duplicate, malformed and overflowing security fields fail closed;
unknown status fields remain compatible with kernel additions. The parser
preserves field-validation order, including malformed extra UID/GID tokens
before cardinality errors. Errors contain static reasons and numeric errno,
not owned strings or boxed I/O payloads.

## Caller Ledger

Every native operation takes the existing caller's resource budget. Its entry
charge precedes input-owner floor validation. The remaining worst-case work and
scratch are prepaid before any observation; there is no fresh nested budget.
The scope restores entry storage on success, refusal and unwind, retaining
accepted work, peak storage and denial history.

Capture consumes no prepaid owner and returns an unreserved storage receipt.
Reserve that receipt before retaining the result. Revalidation requires the
owner's full retained charge. Drop or transfer the owner before retiring that
reservation; unrelated reservations must remain intact. Read-only getters
expose inert credentials and the observed capability ceiling, not inner owners
or descriptors.

Public per-operation `*_WORK` and `*_SCRATCH` constants describe conservative
logical envelopes. The core includes maximum buffer/parse work, bounded path
construction, syscall attempts and descriptor cleanup. These are not elapsed
time, syscall latency, allocator, generated-stack, RSS or kernel-memory limits.

## Shared Staging

The supervisor's private descriptor-only staging input borrows the launcher,
issuer, static manifest and twelve sources. Its fixed table installs the
manifest at 198, issuer at 199 and source descriptors at 200 through 211.
All eighteen duplicates, including the launcher and three handshake endpoints,
are CLOEXEC and at or above 216. Stdio retains source indices zero through two.
RAII closes every completed duplicate if any later stage fails. Staging performs
no admission and creates no child; authority checks belong to the caller.

## Validation

Unit tests cover parser compatibility/refusal order, bounded reads, fixed path
construction, namespace observations, allocation counts, budget limits, sticky
denials and retained-owner accounting. Staging tests cover target/source identity,
CLOEXEC, non-overlap and closure after each of eighteen duplication failures.

`tests/process_profile.rs` additionally provides an ignored, explicit-opt-in
isolated-container fixture. It locks a child to a dedicated non-root identity,
then exercises real native profile capture/revalidation and exact/short budgets.
The parent verifies its own credentials and capabilities remain unchanged and
reaps the child. Its dynamic test executable resets dumpability after startup;
it is not evidence for protected static startup or production issuer execution.

The next process boundary must prepay child and cleanup custody before `clone3`,
retain ownership on timeout/refusal, and use the existing gated direct-syscall
child machinery. A timeout alone is not a logical work bound, and an unbounded
background reaper is not prepaid caller cleanup.
