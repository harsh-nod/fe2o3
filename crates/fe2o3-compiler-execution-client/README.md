# fe2o3 compiler-execution client

## Diagnostic Native Client

`CompilerExecutionClientV2` and `CompilerExecutionClientV3` consume a connected unnamed `SOCK_SEQPACKET` peer
and exclusively borrow the original canonical resource budget. They support
native receipt acquisition, journal-stage recovery, and fresh-challenge
currentness authentication using actual owners from their own family. Neither
retries V1 or the other native family. The V3 client joins conditional SubjectV3
records and uses `verify_native_v3` for currentness; the identity-only
current-record V3 wire is unchanged. Both share one private exchange body.
Transport, randomness attempts, protocol work and retained storage use that
same ledger. Inputs must be prepaid; successful admission transfers the peer's
charge, which is released when the terminal session closes. On admission
failure the closed peer's original reservation remains caller-owned. Returned
values include their full, unreserved logical output charge.

Both native families can instead consume the fixed inherited child slot with
`admit_inherited_child`. The original account prepays the peer, descriptor
inspection and private duplication; resource denial still closes the input.
Successful admission retains one private CLOEXEC duplicate and consumes FD 195.
No policy-family discovery or decoder fallback occurs at this transport step.

This API is diagnostic: it does **not** activate a protected native issuer.
Signed fixture transcripts do not prove protected signing-key custody, live
compiler observation, durable commit-before-publication, independently
administered anchors, or safe GPU launch. The shipping production route remains
V1 until those native service integrations are validated.

The consuming `prepare` step lends that same account to compiler preparation.
It returns the client and prepared value only after checking account identity
and the inherited storage floor. Errors or unwind close the peer, retain inner
charges, and never repair a replaced account. Preparation does not extend the
session deadline. This permits preparation before the terminal exchange without
a fresh admission budget; it does not select a protected V3 launch policy or
activate the V3 path in the compiler.

`prepare_and_acquire` connects preparation, subject publication, one receipt
acquisition and transport completion on that same account. Publication cannot
run until preparation's postchecks pass and the unchanged deadline is live.
The publication callback prepays its subject and carries required prepared
owners forward; the completion callback receives a fully reserved carriage
after the peer closes. Callbacks cannot refund inherited floors. Inner failure
or unwind stays charged and never triggers a retry. The caller still supplies
the real compiler ownership and independently pinned policy; this API does not
turn callback results or inert records into compiler authority.

The following type example relays an already-published, prepaid inert subject;
it does not construct compiler preparation or grant publication authority:

```rust
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3 as Subject;
use fe2o3_compiler_execution_client::{CompilerExecutionClientV3 as Client,
    CompilerExecutionClientErrorV3 as Error};
use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionReceiptCarriageV3 as Carriage};
fn relay(client: Client<'_, '_>, policy: &Policy, subject: Subject) -> Result<Carriage, Error> {
    client.prepare_and_acquire(policy, |_| Ok(subject),
        |subject, _| Ok((subject, ())), |carriage, (), _| Ok(carriage))
}
```

An original budget borrow cannot escape through the completion result:

```compile_fail
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3 as Subject;
use fe2o3_compiler_execution_client::{CompilerExecutionClientV3 as Client,
    CompilerExecutionClientErrorV3 as Error};
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as Policy;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
fn escape<'b, 'w>(client: Client<'b, 'w>, policy: &Policy, subject: Subject)
    -> &'b mut Budget<'w> {
    client.prepare_and_acquire::<_, _, _, Error>(policy, |_| Ok(subject),
        |subject, _| Ok((subject, ())), |_, (), budget| Ok(budget)).unwrap()
}
```

## Native V3 Supervisor Handoff

`CompilerExecutionServiceLaunchV1::handoff_to_supervisor_v3_until` consumes the
existing child-created descriptor pair, sends a native V3 handoff to the fixed
supervisor endpoint, and receives one credential-bound V3 readiness packet plus
EOF. The child-channel V1 suffix versions descriptor construction; this method
does not admit V1 policy, manifest or readiness records.

Transfer and readiness share one continuously borrowed resource account and one
absolute deadline. There is no public pending continuation that could outlive
the account's borrow. Every transport step makes one attempt; EINTR, AGAIN,
short packets, extra descriptors and trailing packets refuse. Per-message
credentials distinguish an empty packet from EOF. The returned move-only
`CompilerExecutionSupervisorReadinessV3` exposes only the manifest and readiness
records, not descriptors or compiler authority.

Prepay the full profile and `CHILD_LAUNCH_STORAGE`; retain that input reservation
and add the returned storage growth after success. Both successful and failed
calls restore entry storage without refunding work or denial history. Child
preparation/spawning and profile authentication remain caller obligations.
`HANDOFF_WORK` and `HANDOFF_SCRATCH` bound logical work/storage, not kernel memory,
OS latency or process RSS. This API is not yet selected by Cargo, client-check or
the installed deployment, and local fixtures do not establish protected boot.

## Production V1 Client

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
