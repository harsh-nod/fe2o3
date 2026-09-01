# fe2o3 Worker V3 verification service boundary

This crate owns one bounded Linux Unix `SOCK_SEQPACKET` exchange for the authority-free Worker V3
verification protocol. It receives one canonical request with exactly two ordered `SCM_RIGHTS`
descriptors, resolves the caller's policy and expected verifier measurement through injected
fail-closed interfaces, consumes the fresh challenge through an injected atomic replay guard, and
captures each exact immutable payload into a receiver-owned sealed memfd.

The receiver enables Linux `SO_PASSCRED` before receiving. The sole request packet must carry
exactly one kernel-stamped `SCM_CREDENTIALS` record matching the connection's `SO_PEERCRED`, plus
exactly one `SCM_RIGHTS` record containing the two ordered descriptors.

`prepare_worker_v3_verification_receiver_v1` must run before the peer can queue a packet: on a
listener before `listen`/`accept`, or on a private socket-pair receiver before exposing the sender.
The service rejects an endpoint without `SO_PASSCRED`, and it rejects any already-queued packet
that lacks the required kernel-stamped credentials.

The caller must shut down the connected socket's write half immediately after the sole packet. The
service requires exact EOF before it resolves policy or consumes replay state; a second packet or
ancillary object terminates the session without a response.

The boundary emits only `RequestFramed` or `RequestRejected`. Framing, descriptor custody, byte
identity, policy selection, measurement selection, and replay exclusion are not verification
theorems. This crate cannot construct protected roster evidence and grants no load or launch
authority. A later protected verifier must independently establish every compiler, executable,
layout, effect, and universal-safety property before a reviewed host promotion can grant authority.

Production resolver implementations must authenticate their policy and measurement stores. A
production replay guard must atomically persist used challenges across every service instance and
restart covered by the policy. The crate deliberately provides no permissive default implementation
for any of these injected decisions.
