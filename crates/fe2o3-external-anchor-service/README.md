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
daemon has a descriptor-only entrypoint with no arguments or environment. It
admits the sealed deployment and complete locked service profile before reading
the deployment-bound signing-key capability, opens only existing durable state,
retains only its private root and peer, and closes every other descriptor. The
deployment also pins the exact daemon SHA-256 measurement. `/proc/self/exe` must
be the same anonymous service-owned mode-`0555` executable under complete
content, execution-mode, and further-seal prevention; its bytes are hashed once
and the retained immutable object is revalidated before key use.

This crate does not establish deployment authority by itself. The remaining
root provisioner must create state, key, executable image, and the unnamed
socketpair under a distinct locked service account, return only the supervisor
endpoint and pidfd after authenticated child admission, and provision those into
the compiler execution supervisor. Creating the socketpair after adopting the
dedicated UID makes the supervisor's `SO_PEERCRED`, unnamed-address, and
distinct-UID checks simultaneously satisfiable.
