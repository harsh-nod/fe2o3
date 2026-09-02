# fe2o3 compiler-execution client

This crate owns the bounded Linux `SOCK_SEQPACKET` client state machine for the
protected compiler-execution service. One acquisition first requests exact
subject recovery and, only after a canonical `ReceiptAbsent` response, resumes
the issuer journal from `Ready`, `Prepared`, or `Issued`. It then publishes the
exact signed receipt and returns the complete inert receipt carriage.

The terminal `verify_current_only` operation sends one complete expected
carriage and a fresh internally generated challenge to the protected service.
`verify_current_only_with_challenge` instead consumes a typed nonzero challenge
owned by its caller so a downstream protected verifier can generate the
expected bytes and enforce replay exclusion itself. The typed challenge is
move-only but cannot prove that its source was fresh; caller-supplied freshness
and uniqueness remain explicit unsafe deployment obligations.
It accepts only a canonical issuer-signed `VerifiedCurrent` response under the
caller-pinned issuer key, bound to that challenge and the exact request, policy,
subject, carriage, issuer journal, Worker record, sequence, and rollback
anchors. The same policy pins a distinct external-anchor key. The sole V3
response carries both the retained signed transition receipt and a fresh signed
recovery receipt. The client re-verifies both under the pinned anchor key,
requires a proposed-position advance for the exact compiler transaction
reconstructed from its original carriage, and requires a proposed-position
recovery observation for the same sequence and heads. The recovery nonce is
derived from the client's challenge, carriage identity, and retained receipt
identity. The returned move-only evidence therefore authenticates the issuer
response, external transition commit, and fresh signed current-head observation.
It remains non-authoritative until protected key custody, independently
administered monotonic-anchor deployment, and compiler-refinement evidence are
joined.

The client uses one absolute monotonic deadline, fixed stack packet storage,
strict request/response identity correlation, pinned-policy validation, and no
ancillary descriptors. It grants no compiler, artifact, load, or launch
authority.

The crate also owns the authority-free child-channel handoff used by a selected
compiler or application parent. Its post-fork callback creates the unnamed
`SOCK_SEQPACKET` pair inside the selected child. Preparation first reserves FD
195 with an exact close-on-exec duplicate of the private control endpoint so a
concurrent descriptor allocation cannot occupy the fixed target between
preparation and `fork`. The child requires that exact reservation before it
atomically installs only the client endpoint at FD 195, and
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

The transfer derives the launch manifest from the client-profile-pinned
external-anchor service UID/GID and policy. It sends one canonical
direct-parent/launch-manifest record and
exactly two ordered `SCM_RIGHTS` descriptors, then retains the same control
connection for pending readiness. This avoids attributing a parent-created
service socket to the selected child or accepting a same-user relay as
the direct parent. After the supervisor admits issuer readiness, it sends that
same canonical record over the control connection and closes its endpoint. The
pending client accepts exactly one descriptor-free packet followed by EOF,
rechecks its launch manifest, pinned anchor-service identity, and pinned policy,
and rejects truncation, extension, substitution, trailing data, or timeout.
Compiler and application parents both use this channel in the production Cargo
path. Binding-wrapper service acquisition, the deployed distinct-UID entrypoint,
external monotonic rollback, final verifier authority, HSACO publication, and
runtime admission remain outside this checkpoint.

Cargo supplies one absolute monotonic deadline across child admission,
supervisor connection and transfer, and readiness. Individual duration-based
operations remain available, but production does not reset the timeout between
those transitions.

The `fe2o3-compiler-execution-client-check` executable exercises that exact
production path as a non-root member of the profile's supervisor group. Its
parent admits the root-owned production client profile, inherits the sealed
profile policy into the child at FD 202, creates the child's service endpoint
at FD 195 through `PendingCompilerExecutionChildChannelV1`, transfers the live
child to the fixed supervisor, and admits exact readiness under one deadline.
The child admits both inherited descriptors and performs `recover_only` for one
fixed domain-derived qualification subject. Success requires canonical absence
and the terminal cancel response, so a prior record for the same subject makes
later qualification fail instead of being bypassed by a fresh subject.

The child writes a private bounded report to a captured pipe. Only after the
parent strictly decodes that report, matches its policy to the retained profile,
revalidates both capabilities, and reaps the child does it emit the ordered
`fe2o3-compiler-execution-client-check-report-v1` report. Every identity and
rollback anchor is lowercase 64-hex, every integer is strict decimal, and the
terminal line is exactly `complete=true`.
