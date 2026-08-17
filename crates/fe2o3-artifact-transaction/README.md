# fe2o3 artifact transaction

This crate coordinates local compiler artifact publication through bounded canonical records,
descriptor-relative filesystem operations, and one cooperative output-directory lock.

## Retained service directory

`RetainedDurableDirectoryV1` is the lower-level descriptor-only mechanism used by the W1 durable
broker-session journal. It admits an already-open, `FD_CLOEXEC`, service-owned `0700` directory
and never accepts a path. Synced temporary files, durable redo promotion, exact-mode artifact
staging, `RENAME_NOREPLACE`, and directory syncs are exposed through bounded fault boundaries.

This mechanism is `AUTHORITY=none`. It validates file type, owner, mode, link count, retained
directory identity, and operation ordering, but it does not interpret journal records, exclude
multiple writers, authenticate checksums, or prevent same-host rollback.

## Durable published claims

`AttemptScopedHsacoPublicationResultV1::published_claim` returns a
`DurablePublishedHsacoClaimV1`. The claim is inert, cloneable data intended for persistence in a
later cross-process bundle. Its canonical encoding binds:

- the complete `DurableLinkPublicationPlanV1`;
- the exact backend provenance receipt and caller-supplied upstream evidence identity;
- the publication-time output-directory, canonical-record, and artifact file identities; and
- the exact artifact length, finalized digest, generation, scope, and publication identities.

`reacquire_current_hsaco_publication_lease_v1` opens the configured output directory, takes its
exclusive lock without blocking, and checks the decoded claim, durable attempt receipt and owner,
complete plan, current scope generation, directory identity, canonical record, and
content-addressed artifact. Success returns a fresh non-`Clone`
`DurableCurrentLinkPublicationLeaseV1` that owns read-only descriptors for those exact files.

The claim and lease grant no load or launch authority. They do not authenticate the compiler,
upstream evidence, artifact semantics, ABI, memory safety, or race freedom. The protocol assumes a
cooperating local filesystem with the durability and atomic-rename behavior described by
`publish_durable_link_v1`; its checksum is not a keyed authenticator against a same-user attacker.
It also does not provide rollback resistance: restoring the claim, attempt registry, publication
record, and filesystem identities together can restore an older locally consistent state.
