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
