# fe2o3 compiler execution protocol

Conditional SubjectV3 has nominal policy, profile, request, receipt, carriage,
external-anchor transaction and service-packet components. Typed V3 launch,
readiness, handoff and currentness joins reuse their existing identity-only
wires. The closure-capability crate provides sealed V3 policy/key, profile and
launch custody; the client crate adds terminal V3 exchanges on the original
budget. These components do not activate a protected V3 service or the production
conditional publisher. See the [service/profile checkpoint](../../docs/evidence/conditional-service-profile-20260926.md)
for verified scope, unavailable socket validation, and remaining compiler,
issuer, Worker and safe-launch integration.

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

`CompilerExecutionSupervisorDeploymentV2` and `V3` provide distinct 184-byte
native deployment records. Construction, decoding and matching are metered on the
original ledger; decoding requires the complete supplied native policy identity.
They bind dedicated supervisor/anchor credentials and exact supervisor/launcher
measurements without converting V1 owners. They remain inert configuration:
sealed transport, independent trusted-parent pinning and native production
deployment are separate obligations. The existing V1 format is unchanged.
Native `CompilerExecutionSupervisorReadyV2` and `V3` add distinct 88-byte
bootstrap records for those deployments. Their metered decoder requires the
expected child PID and actual same-family deployment together. A rehashed foreign
deployment identity or changed PID still fails that contextual check. Neither
construction nor decoding establishes private-channel provenance, actual service
admission or pidfd liveness; the deployed root coordinator still uses V1.

`CompilerExecutionExternalAnchorDeploymentV2` and `V3` add move-only native
external-anchor configuration with Copy nominal identities. Their distinct
168-byte records use magic `F2O3CEA2`/`F2O3CEA3`, version 2/3, and SHA-256 domains
`FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V2\0` and `.../V3\0`.
The shared private codec uses this little-endian transcript:

| Bytes | Content |
| --- | --- |
| 0..8 | Family magic |
| 8..10 | Family version |
| 10..12, 16..24 | Reserved zeroes |
| 12..16 | Total length, 168 |
| 24..28, 28..32 | Exact anchor UID and GID |
| 32..64 | Exact policy external-anchor Ed25519 key |
| 64..96 | Complete same-family supervisor deployment identity |
| 96..128, 128..136 | Executable SHA-256 and nonzero byte length, at most 128 MiB |
| 136..168 | SHA-256(domain, LE64(136), bytes[0..136]) |

Both `new(supervisor, policy, executable, budget)` and
`decode(bytes, supervisor, policy, budget)` require the actual same-family
supervisor and policy owners. Every working operation calls the supervisor's
metered `matches_policy` on the original ledger before processing the anchor
record. Decoding also requires exact supervisor identity, anchor credentials
and policy anchor key equality. A resealed foreign identity, mismatched policy,
substituted key or substituted credential cannot satisfy that context. Equality
to the already validated native policy key avoids repeated curve validation;
hostile keys cannot produce a public owner. The V1 implementation is unchanged.

`matches_supervisor_and_policy` checks that complete context;
`matches_supervisor_policy_and_executable` additionally compares both executable
digest and length. Identity `matches_canonical_bytes` likewise requires both
actual owners and returns false for malformed bytes or mismatched context.
These are configuration checks. Neither public construction nor canonical
decoding establishes trusted provisioning origin, process custody, live service
admission or signing/launch authority. A caller must independently pin and measure
the intended executable; changing and resealing a valid executable measurement
describes different inert configuration. Sealed capability transport and
production coordinator integration are separate work.

Each working operation consumes the exported fixed `..._WORK_V2/V3` total of
11,280 logical units: 5,384 outer units plus the supervisor comparison's 5,896.
The exported `..._STORAGE_V2/V3` is the total additional logical peak, including
both nested frames. The outer scope charges 8 entry units, checks the combined
borrowed-owner floor, prepays its remaining work and scratch, then enters the
supervisor check on the same Budget. Thus a nested refusal preserves accepted
outer work and peak. Keep the full supervisor, policy and any borrowed wire owner
prepaid; owner matching additionally requires the full anchor owner. Fixed-size
wire decoding requires at least 168 input bytes, while wrong lengths require no
wire floor and are not scanned. Checked arithmetic and scoped cleanup preserve
entry storage, cumulative work, peak and first-denial history on every outcome.
Successful construction/decoding return the full unreserved
`size_of::<(Deployment, Storage)>()` charge: reserve it before retaining the
result and release `retained_storage()` only after dropping that owner. Accessors
are unmetered stored-value reads. These quotas do not bound allocator behavior,
generated stack size or RSS. Shared integration cases cover both families and
compile-fail API examples cover nominal isolation and missing context/budget.

`CompilerExecutionExternalAnchorProvisioningV2` and `V3` add move-only native
provisioning configuration and Copy nominal identities. The V1 layout remains
128 bytes: header 0..24, full deployment identity 24..56, helper SHA-256 56..88,
nonzero little-endian helper length 88..96, and terminal identity 96..128.
Native magic is `F2O3CEP2`/`F2O3CEP3`, with version 2/3 and SHA-256 domains
`FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-PROVISIONING/V2\0` and `.../V3\0`.
The terminal hash covers the domain, little-endian u64 preimage length (96),
then those 96 preimage bytes. Reserved header bytes remain zero.

Both `new` and `decode` require the actual same-family anchor deployment;
identity `matches_canonical_bytes` also requires that complete owner. The actual
deployment already binds its supervisor, policy, credentials, key and executable,
so provisioning does not reconstruct or revalidate those inputs. The helper
measurement must have a nonzero digest and length no greater than 128 MiB.
`matches_deployment` checks the complete deployment identity, while
`matches_deployment_and_helper` additionally compares exact helper digest and
length. Validly resealing a helper change describes different inert configuration,
not trusted provisioning origin or process/signing/launch authority. The caller
must independently pin and measure the helper. There is no context-free decode,
identity-only construction, V1 upgrade or foreign-family fallback; V1 is unchanged.

Every working operation consumes the fixed `..._PROVISIONING_WORK_V2/V3` quota
of 4,104 logical units on the original Budget. It charges 8 entry units, checks
the full borrowed deployment plus wire/owner floor, then prepays remaining work
and exported `..._PROVISIONING_STORAGE_V2/V3` scratch before inspection. Exact
wire lengths require 128 input bytes; wrong-sized slices are not scanned and
need no wire floor. Keep complete borrowed owners prepaid. Checked scopes restore
entry storage while retaining accepted work, peak and first-denial history.
Successful construction/decoding return the full unreserved
`size_of::<(Provisioning, Storage)>()` charge, to reserve before retention and
release after owner Drop. These are logical quotas, not stack or RSS limits.

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
