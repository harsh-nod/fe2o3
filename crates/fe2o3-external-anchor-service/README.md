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

Provisioning can atomically open existing state or initialize genesis while
holding the same exclusive directory lock. Genesis is created only for an
exactly absent state file; malformed, inaccessible, or key-substituted state
fails closed and is never reset.

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
content, execution-mode, and further-seal prevention. The shared protected
static-executable contract checks its complete loader-independent ELF form,
measurement, owner, seals, object identity, bytes, and static identity at
admission and revalidates the retained immutable object before key use.

This crate does not establish deployment authority by itself. The separate
measured provisioning helper now creates service-owned key custody, atomically
opens or initializes state, creates the unnamed socketpair after adopting the
dedicated identity, transfers one endpoint, and executes this daemon. The
root coordinator now prepares and launches that helper under the distinct locked
service account, retains pidfd and reaping custody, and authenticates the
completed exec and live endpoint. Its authoritative root-only qualification and
the concrete admitted endpoint/pidfd transfer into compiler-execution supervisor
construction remain open. Creating the socketpair after the credential
transition makes the supervisor's `SO_PEERCRED`, unnamed-address, and
distinct-UID checks simultaneously satisfiable.

`scripts/build-static-external-anchor-service.sh` builds the pinned musl release
through the shared syscall-only protected-service entrypoint and rejects any
dynamic loader, runtime dependency, undefined symbol, executable stack, wrong
entry address, production static-ELF parser failure, output on an empty contract,
or fail-open exit.
