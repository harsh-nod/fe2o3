# fe2o3 compiler execution protocol

This crate owns the canonical, inert compiler-execution issuer policy, public
client profile, expected-client launch manifest, attestation, receipt-carriage,
current-record verification, and bounded service packet records. The 352-byte
current-record verification binds one exact carriage to the policy, subject,
issuer journal, Worker record, sequence, both rollback anchors, and the
protected policy and Worker-ledger verification identities. Decoding that
record proves canonical structure only. A separate 536-byte attestation binds
that complete record to a nonzero caller challenge and an Ed25519 signature.
Verification requires the embedded key to equal the caller-pinned policy key,
the challenge to equal the caller's fresh challenge, and the complete nested
record to equal the expected record. The result grants no authority and does
not by itself prove protected key custody or external anti-rollback. The
1,874-byte external-anchor transaction binds the complete issuer policy,
attestation request, signed receipt publication, sequence, and prior/current
internal rollback anchors without including a path, descriptor, or final
acknowledgment. Its frozen identity derives the transaction digest consumed by
`fe2o3-external-anchor-protocol`. The transaction is inert: it does not contact,
advance, or authenticate an external monotonic service. The policy itself pins
distinct issuer-signing and external-anchor Ed25519 keys; equal or weak keys
fail closed. A fixed 2,682-byte Worker anchor-journal record preserves that
complete transaction and one exact advance challenge across
`PreparedAnchor`, `AnchorCommitted`, `Published`, and `Aborted`. Committed and
terminal records verify the complete signed external receipt under the
policy-pinned anchor key; only `Published` binds a nonzero final Worker-record
identity. The codec enforces legal same-transaction and next-transaction
transitions but does not persist them, contact the anchor, or grant authority.
The 280-byte client profile binds the exact dedicated supervisor UID/GID and
external-anchor service UID/GID to one complete caller-pinned issuer policy; it
contains no endpoint, path, descriptor, secret, timeout, or authority. The
112-byte launch manifest binds an exact client PID/UID/GID tuple and that exact
external-anchor service identity to an exact policy identity. A canonical
184-byte supervisor-handoff record additionally
binds the direct Cargo parent PID/UID/GID to that complete manifest; parent and
rustc must be distinct processes with equal credentials. A separate readiness
record binds the admitted issuer PID to the exact manifest and policy after
durable recovery. The supervisor separately admits the manifest-named anchor
service endpoint and pidfd and transfers them at issuer FDs 10 and 11; the
issuer revalidates their continuity and binds the transport to the policy-pinned
anchor key. No descriptor is serialized in these records, and receipt
publication does not yet invoke the transferred transport. None of these
records grants process or signing authority.
The sole production supervisor endpoint is the named Unix `SOCK_SEQPACKET`
socket `/run/fe2o3/compiler-execution-supervisor.sock`; alternate paths are not
part of the production protocol.
The sole production profile source is
`/etc/fe2o3/compiler-execution/client-profile-v1`. Admission walks that fixed
tree without following symlinks, requires root-owned non-writable directories,
and reads one root-owned single-link mode-0444 canonical record before sealing
it for authenticated process transfer.
The crate contains no process launcher, signer, durable ledger, compiler,
artifact publisher, loader, or GPU execution authority.

`fe2o3-runtime-protocol` re-exports these records for compatibility with its
existing load-envelope API. New compiler-execution components should depend on
this crate directly.
