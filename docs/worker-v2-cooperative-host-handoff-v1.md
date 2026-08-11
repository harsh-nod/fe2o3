# Worker V2 Cooperative Host Handoff V1

Status: cooperative-process inert foundation. This is not production authority.

## Threat Model

The inherited consumer must run during single-threaded application startup,
before installing signal handlers that can touch environment or file-descriptor
state, before spawning descendants, and before unrelated code can close,
duplicate, change flags on, or mutate a handoff descriptor. Cargo and the
application are assumed to cooperate with this protocol.

The consumer removes all five handoff environment values before parsing. It
takes ownership of the inherited envelope, artifact-directory, and ACK
descriptors, snapshots all three descriptor identities, and sets `FD_CLOEXEC`
on every matching occurrence visible in `/proc/self/fd`. It repeats the seal
after recovery because durable lease acquisition duplicates the pinned
directory descriptor. Exact envelope bytes, the pinned directory, and the
reacquired lease remain retained through unload.

The ACK is canonical liveness and possession data only. The child receives all
inputs needed to reproduce it. ACK bytes establish no recovery provenance and
grant no host, publication, load, or launch authority.

The bounded artifact profile is exactly `gfx942` with XNACK explicitly
disabled. SRAM ECC is intentionally unconstrained: omitted, enabled, and
disabled artifact target states are accepted, while the runtime observation
must remain compatible with the artifact state.

## Currentness And Revocation

Recovered HSA handoff acquires one non-clone durable currentness token after
prerequisite checking and before HSA environment authorization and loading.
The token owns the cooperative publication lock. Its retained record and
artifact identities are revalidated immediately before load, each prepare,
each dispatch, and unload. The token remains alive until executable unload has
completed.

A cooperative generation-N+1 publisher blocks on that lock while generation N
is loaded. It can complete turnover only after generation N unload returns and
the token is dropped. This is prevention under a cooperative lock, not
asynchronous revocation of an already running kernel and not protection from a
same-process attacker that bypasses the lock or mutates descriptors.

## Trust Boundary

Safe Rust visibility and non-`Clone` types prevent accidental authority
duplication; they are not a security boundary against malicious code sharing
the process. An untrusted same-process application must not receive
descriptors, lease tokens, authenticators, or HSA authority. That deployment
requires a separate broker which retains those resources and exposes a bounded
request protocol to the application.

Cargo's application launcher separately pins and validates the exact initial
static image and applies a no-fork/no-re-exec seccomp profile. It does not
constrain arbitrary same-process behavior: permitted `openat`, `mmap`,
`mprotect`, and `pwrite64` operations do not prevent in-process loading or
self-modification. Dynamic HIP runtime closure remains outside the boundary.

This handoff does not carry authority from genuine compiler, Verus,
proof-to-executable, effect, and prerequisite issuers across the remaining
unsafe caller-supplied authenticator boundary. ACK acceptance does not change
that status.
