# fe2o3 Worker V3 verification client

This crate provides the authority-free Unix `SOCK_SEQPACKET` client transport for one Worker V3
verification framing exchange. It admits exactly two caller-owned, immutable memfd snapshots in
protocol order, binds their exact lengths and SHA-256 digests to one canonical request, reopens and
retains only read-only close-on-exec handles, sends the request and both descriptors in one
`SCM_RIGHTS` message, and accepts one exact framing-only response.

The client authenticates neither the service peer nor a verification theorem. A framing receipt
does not grant compiler, executable, load, launch, or safety authority. Protected peer identity,
fresh-challenge replay exclusion, theorem-record authentication, and host promotion remain owned by
later reviewed boundaries.

`WorkerV3VerificationClientV2` adds a strict multi-phase typestate on one connection and one
absolute deadline. `begin` sends the canonical V1 request and its two snapshots without closing the
write half. A reserved response yields two separate move-only values: a service-issued current-record
challenge and a pending client session. The challenge can be consumed directly into
`CompilerExecutionCurrentRecordChallengeV1`; the pending session privately retains the same bytes
and opaque reservation identity only for later correlation.

The pending session accepts only the exact fixed-size current-record verification and attestation
arrays, sends their canonical V2 frame, closes the write half, and accepts one bounded terminal
packet followed by exact peer EOF. Phase packets cannot carry descriptors or other ancillary data.
The terminal response is opaque and authority-free even when the remote application selected the
application-response disposition.
