# fe2o3 artifact transaction

This crate coordinates local compiler artifact publication through bounded canonical records,
descriptor-relative filesystem operations, and one cooperative output-directory lock.

## Compiler provenance records

The compiler-module handoff protocol exposes V1 compatibility records,
closure-bound V2 records, and strict semantic V3 records. The Worker V2
publication-intent protocol exposes V1 and closure-bound V2 records. These
implementations use shared internal engines for slot ownership, bounded
filesystem operations, recovery, and fault boundaries; version-specific
schemas retain distinct names, domains, encodings, byte ceilings, and error
surfaces.

| Protocol | Version binding | Current production selection |
|---|---|---|
| Compiler module handoff V2 | The complete canonical `CompilerClosureV2`, attempt, producer, slot, and exact module bytes are committed by V2 publish/consume APIs. | Existing protected backend and wrapper/finalizer call sites have not been migrated by this crate-only change. |
| Compiler module handoff V3 | The native terminal identity and exact canonical bytes of `InertSemanticCompilerModuleHandoffV3` are bound to the attempt, producer, slot, and transaction identity in a separate V3 namespace. Additive currentness APIs retain pinned output/namespace/slot/record/payload descriptors in a move-only lease, mint a single-use token under the cooperative lock, and consume that token when committing the existing one-shot tombstone. | The transport and currentness custody are implemented and tested but production callers are not wired by this crate-only change. |
| Worker V2 publication intent | The complete closure is committed with the attempt, producer, durable plan, upstream evidence, output identity, length, and exact retained bytes; V2 persist/recover/clear APIs reject closure mismatch. | Protected publication and restart-marker paths still persist, recover, and clear V1 intents. |

V1 and V2 APIs, wire formats, and byte maxima remain unchanged and are not
silently upgraded. V3 uses the compiler-FFI V3 maximum only in its own schema
and never falls back to V1 or V2 decoding. None of these versions authenticates
compiler authorship or grants publication, linking, loading, launch, or
execution authority.

The V3 receipt and consumed value remain inert. A
`CompilerModuleHandoffCurrentnessLeaseV3` is private local custody rather than
serializable evidence: it binds one committed attempt generation, producer,
closed V3 slot, transaction identity, native outer identity, and pinned
directory/file metadata. `CompilerModuleHandoffConsumptionTokenV3` holds the
cooperative lock and is consumed by value, so a stale generation, replaced or
tampered path, replayed tombstone, or token from another lease fails closed.

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
