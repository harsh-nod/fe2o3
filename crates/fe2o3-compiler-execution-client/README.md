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
extension, substitution, trailing data, or timeout.

The crate also provides a separate authority-free return channel at fixed child
FD 196. The rustc child creates this `SOCK_SEQPACKET` endpoint after fork, and
the parent admits it against the spawned PID, child-reported parent PID,
`SO_PEERCRED`, and a live pidfd. The child can send exactly one canonical inert
receipt carriage followed by clean EOF; the receiver rejects short, extended,
truncated, ancillary, trailing, wrong-policy, and wrong-subject packets. The
compiler-service client can consume fixed child FD 195 into private
close-on-exec custody before performing the bounded acquisition.

The protected `rustc-codegen-fe2o3` `DeviceTransaction` path now invokes the
composed child round trip only after strict V3 publication derives the exact
`InertCompilerExecutionSubjectV1`. That one-use operation consumes and
revalidates sealed policy FD 202, consumes and admits service FD 195, acquires
the exact receipt carriage under one absolute deadline, and consumes FD 196 to
return that same policy/subject/carriage join. Every present canonical slot is
closed on admission failure, and any custody, deadline, acquisition, or return
failure terminates the protected compilation without fallback.

`PendingCompilerExecutionChildSessionV1` is the move-only join over those two
channels and an explicitly supplied sealed policy capability. It preflights FD
195, FD 196, and policy FD 202, installs all three on one unspawned rustc
command, and admits both parent endpoints against one exact PID under one
absolute deadline. Its next transition transfers only the service endpoint to
the authenticated distinct-UID supervisor while retaining the policy and
receipt-return endpoint. Exact supervisor readiness must then match that
retained policy before receipt reception becomes available. Completion exists
only after FD 196 returns one real canonical carriage matching both the retained
policy and the caller's exact compiler subject; no transition creates or
synthesizes a carriage. On the other endpoint,
`ProtectedIssuerServiceV1::serve_one` composes fixed-listener acceptance,
authenticated handoff, static launch, readiness publication, natural service
exit, and exact reaping.

Production still needs Cargo-side provisioning of the complete FD 195/196/202
session, the corresponding parent receipt join, externally provisioned custody
for the admitted static launcher and issuer images, a distinct supervisor
UID/GID, a service-owned root, a sealed signing key, and a deployed service
process receiving the fixed listener descriptor. Those authorities cannot be
derived from the production build config or environment. Until that complete
parent path is joined to the active binding wrapper, protected device
compilation reaches the strict post-publication child call and fails closed on
missing descriptors. HSACO publication and runtime admission remain outside
this crate.
