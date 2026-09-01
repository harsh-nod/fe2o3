# fe2o3 Worker V3 verification protocol

This crate defines bounded canonical request and response frames for moving one exact Worker V3
roster-verification job across a process boundary. The request binds caller-owned challenge,
roster, policy, and expected verifier-measurement identities; an ordered roster; and exactly two
file-descriptor payload positions for the complete load envelope and finalized HSACO.

The frames are deliberately authority-free. A frame identity establishes byte equality only. It
does not authenticate a peer, prove challenge freshness, validate file-descriptor custody, perform
compiler or machine verification, or construct protected roster evidence. The fresh protocol
challenge is caller-generated entropy with replay exclusion; it is distinct from the host's
deterministic lineage-and-roster challenge. The response vocabulary can acknowledge framing or
reject a request; it cannot encode theorem success.

Logical and export names are carried losslessly with the same bounded canonical validation used by
the kernel descriptor. They remain untrusted duplicate coordinates: the protected service must
cross-check them against its pinned roster policy and the exact descriptor table carried by the
envelope, then recompute the roster identity over those names, marker bindings, and generated host
contracts. Generated host-contract identities may repeat because multiple distinct marker bindings
can intentionally share one ABI and effect contract.

The Unix transport owner must receive the frame and exactly two `SCM_RIGHTS` descriptors as one
authenticated exchange, reject missing or extra descriptors, acquire independent descriptor
custody, enforce the required file type and immutability policy, read exactly the declared length,
reject trailing payload bytes, and compare the SHA-256 digest. The caller owns cryptographically
fresh challenge generation and replay exclusion. A later theorem-record contract must separately
define authenticated verifier output and the only reviewed promotion into host authority.
