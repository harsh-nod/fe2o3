# fe2o3 compiler execution protocol

Native `CompilerExecutionIssuerPolicyV2` and `CompilerExecutionClientProfileV2`
provide move-only, budgeted SubjectV2 trust inputs using shared private codecs.
They do not activate a V2 deployment or supply protected execution evidence.
The attestation/service/durable production path described below still uses V1
trust inputs. See [the V2 contract](../../docs/compiler-execution-trust-inputs-v2.md)
for wire, resource, ownership and remaining integration requirements.
Native SubjectV2 bindings, challenges and requests now share V1 framing mechanics
while retaining nominal owners and same-ledger nested subject decoding. They are
inert content records, not signed execution evidence. See the
[request contract](../../docs/compiler-execution-request-v2.md); production
issuance, durable state and consumers remain on the previous family.
Native move-only signed receipts and pinned-key verification are also available
with explicit work/storage admission. Signature validity does not establish a
protected compiler occurrence or advance a durable ledger. See the
[receipt contract](../../docs/compiler-execution-receipt-v2.md).
Native publication, ACK and complete carriage now validate nested owners and
their exact relationships on the same resource ledger, without proving durable
publication. See the [publication contract](../../docs/compiler-execution-publication-v2.md).
Versioned durable state, service transport and production consumer integration
remain pending.

`CompilerExecutionServiceReadyV2` provides move-only, metered readiness framing
and exact native PID/manifest/policy matching. It shares the unchanged 120-byte
V1 wire codec, not admitted V1 owners. A decoded frame is inert: private-channel
provenance, live process custody and production issuer integration remain separate
obligations. See the [native lifecycle](../../docs/compiler-execution-consuming-launch-v2.md).

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
The deployment constants also pin the sole runtime socket and mode-`0700`
service-owned durable-root path plus a distinct root-only lifecycle-lock file.
The root coordinator and provisioner use the dedicated file as their
shared/exclusive lock domain, leaving the issuer's state-root singleton lock
independent. Neither pathname grants authority.
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
part of the production protocol. Its runtime directory is root-owned mode
`0755`; the socket pathname is root-owned, owned by the deployment supervisor
GID, and exactly mode `0660`. This keeps replacement authority out of the
unprivileged supervisor while allowing explicitly enrolled group members to
connect.
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
