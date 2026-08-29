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

This increment does not establish deployment authority. A production deployment
must still provide a distinct locked service account, an independently managed
signing key, a descriptor-only `SOCK_SEQPACKET` entrypoint, authenticated peer
admission, and root-owned provisioning into the compiler execution supervisor.
