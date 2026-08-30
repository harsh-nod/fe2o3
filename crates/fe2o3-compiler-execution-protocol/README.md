# fe2o3 compiler execution protocol

This crate owns the canonical, inert compiler-execution issuer policy, public
client profile, expected-client launch manifest, attestation, receipt-carriage,
current-record verification, and bounded service packet records. The sole
1,440-byte V3 current-record verification binds one exact carriage to the
policy, subject, issuer journal, Worker record, sequence, both internal rollback
anchors, policy-pinned external-anchor key, complete 528-byte signed commit
receipt, complete 528-byte fresh currentness receipt, and protected policy and
Worker-ledger verification identities. Decoding proves canonical structure and
re-verifies both receipts under the embedded anchor key. A separate 1,624-byte
V3 attestation binds that complete record to a nonzero caller challenge and an
issuer Ed25519 signature. Issuance and verification additionally require both
keys to equal the caller's policy, every record coordinate to equal the original
expected carriage, the retained receipt to be a proposed-position advance for
the exact reconstructed compiler transaction, and the currentness receipt to be
a proposed-position recovery observation of that same transition. Its recovery
nonce is derived from the caller's fresh challenge, exact carriage identity, and
retained commit-receipt identity, so a stale response or cross-record receipt
cannot be substituted. The result authenticates the signed external commit and
fresh signed current-head observation, but grants no authority and does not by
itself prove protected key custody or that the anchor service is independently
administered, monotonic, and crash durable. The
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
The 184-byte supervisor deployment manifest pins the exact dedicated
supervisor UID/GID, distinct external-anchor service UID/GID, protected-supervisor
executable measurement, static pre-exec launcher measurement, and issuer-policy
identity supplied by trusted service provisioning. The two executable roles must
have distinct measurements. The manifest carries no path, descriptor, secret,
timeout, or authority.
The private root bootstrap carries one fixed 88-byte supervisor-readiness record.
It binds the exact deployed child PID and supervisor-deployment identity under a
domain-separated terminal identity. The record is authority-free: the root
coordinator must independently establish private-channel provenance and exact
pidfd liveness.
The 168-byte external-anchor deployment manifest derives the anchor verification
key from that exact issuer policy and binds the dedicated anchor UID/GID, key,
exact supervisor deployment identity, and bounded SHA-256 executable
measurement. Trusted provisioning must retain both sealed manifests and the
policy and recheck their complete relationship; the anchor manifest contains no
secret key, endpoint, state, path, descriptor, or execution authority.
The separate 128-byte external-anchor provisioning manifest binds that complete
deployment identity to one bounded exact provisioning-helper executable
measurement. It is inert configuration transported at helper FD 223; it carries
no seed, state, endpoint, process, or launch authority and is never inherited by
the final anchor daemon.
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
anchor key. Receipt publication invokes that transport before committing the
Worker record or ACK. No descriptor is serialized in these records, and none
of them grants process or signing authority.
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
