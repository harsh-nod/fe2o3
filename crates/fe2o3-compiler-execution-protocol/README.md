# fe2o3 compiler execution protocol

This crate owns the canonical, inert compiler-execution issuer policy, public
client profile, expected-client launch manifest, attestation, receipt-carriage,
current-record verification, and bounded service packet records. The 352-byte
current-record verification binds one exact carriage to the policy, subject,
issuer journal, Worker record, sequence, both rollback anchors, and the
protected policy and Worker-ledger verification identities. Decoding that
record proves canonical structure only. It grants no authority unless a caller
received it over an authenticated protected-service session and independently
matches every field to the request. The 240-byte client profile binds the exact
dedicated supervisor UID/GID to one complete caller-pinned issuer policy; it
contains no endpoint, path, secret, timeout, or authority. The launch manifest
binds an exact PID/UID/GID tuple to an exact policy identity. A canonical
supervisor-handoff record additionally
binds the direct Cargo parent PID/UID/GID to that complete manifest; parent and
rustc must be distinct processes with equal credentials. A separate readiness
record binds the admitted issuer PID to the exact manifest and policy after
durable recovery. None of these records grants process or signing authority.
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
