# fe2o3 compiler-execution client

This crate owns the bounded Linux `SOCK_SEQPACKET` client state machine for the
protected compiler-execution service. One acquisition first requests exact
subject recovery and, only after a canonical `ReceiptAbsent` response, resumes
the issuer journal from `Ready`, `Prepared`, or `Issued`. It then publishes the
exact signed receipt and returns the complete inert receipt carriage.

The client uses one absolute monotonic deadline, fixed stack packet storage,
strict request/response identity correlation, pinned-policy validation, and no
ancillary descriptors. It grants no compiler, artifact, load, or launch
authority.

The crate also owns the authority-free child-channel handoff used by the direct
rustc parent. Its post-fork callback creates the unnamed `SOCK_SEQPACKET` pair
inside the rustc child, installs only the client endpoint at FD 195, and
transfers only the service endpoint to the parent. Parent admission binds the
transfer to the exact child PID, child-reported direct-parent PID,
`SO_PEERCRED`, and a live pidfd under one absolute deadline. The resulting
move-only value can cross exactly one authenticated Unix `SOCK_SEQPACKET`
control connection to a dedicated supervisor. The production operation creates
that endpoint itself with exact close-on-exec and nonblocking custody, connects
only to `/run/fe2o3/compiler-execution-supervisor.sock`, requires an unnamed
local address and that exact remote address, and authenticates the configured
non-root supervisor UID/GID and positive PID with `SO_PEERCRED`. Callers cannot
inject another pathname or descriptor. One monotonic deadline of at most two
minutes covers connection and the canonical transfer.

The transfer sends one canonical direct-parent/launch-manifest record and
exactly two ordered `SCM_RIGHTS` descriptors, then retains the same control
connection for pending readiness. This avoids attributing an
outer-Cargo-created service socket to rustc or accepting a same-user relay as
the direct parent. After the supervisor admits issuer readiness, it sends that
same canonical record over the control connection and closes its endpoint. The
pending client accepts exactly one descriptor-free packet followed by EOF,
rechecks its launch manifest and pinned policy, and rejects truncation,
extension, substitution, trailing data, or timeout. Binding-wrapper service
acquisition, the deployed distinct-UID entrypoint, HSACO publication, and
runtime admission remain outside this checkpoint.
