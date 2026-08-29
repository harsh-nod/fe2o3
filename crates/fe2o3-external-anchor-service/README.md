# fe2o3 external anchor service

`fe2o3-external-anchor-service` owns the durable single-writer state behind the
external anti-rollback protocol. It retains an exclusive lock on a private
descriptor-pinned directory and stores one canonical sequence/head/key-identity
record with a domain-separated checksum.

For an `Advance` challenge naming the current prior state, it writes and syncs a
private next-state file, atomically renames it over the current state, syncs the
directory, and only then signs a `Proposed` observation. Exact retries are
idempotent. A `Recover` challenge only observes the exact prior or proposed
position. Stale, future, malformed, and key-substituted challenges fail closed.
Any uncertain persistence failure poisons the in-memory service so it cannot
issue another response before restart and state revalidation.

The connected-peer transport accepts only an unnamed, nonblocking Unix
`SOCK_SEQPACKET` with close-on-exec custody. It receives one exact 184-byte
challenge per packet, rejects truncation and all ancillary data, applies the
durable transition, and sends one exact 288-byte signed observation. The anchor
child is expected to create this unnamed socketpair after adopting its dedicated
UID, return the supervisor endpoint to the root provisioner, and retain its own
endpoint across `exec`; this is what makes the supervisor's `SO_PEERCRED`,
unnamed-address, and distinct-UID checks simultaneously satisfiable.

This increment does not establish deployment authority. A production deployment
must still provide a distinct locked service account, an independently managed
signing key, a descriptor-only `SOCK_SEQPACKET` entrypoint, authenticated peer
admission, and root-owned provisioning into the compiler execution supervisor.
