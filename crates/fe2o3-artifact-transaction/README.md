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
| Compiler module handoff V3 | The native terminal identity and exact canonical bytes of `InertSemanticCompilerModuleHandoffV3` are bound to the attempt, producer, slot, and transaction identity in a separate V3 namespace. Cross-process receipt recovery validates the exact durable ready record and payload under the cooperative lock; additive currentness APIs retain pinned output/namespace/slot/record/payload descriptors in a move-only lease, mint a single-use token under the lock, and consume that token when committing the existing one-shot tombstone. | The transport, inert receipt recovery, and currentness custody are implemented and tested but production callers are not wired by this crate-only change. |
| Worker V2 publication intent | The complete closure is committed with the attempt, producer, durable plan, upstream evidence, output identity, length, and exact retained bytes; V2 persist/recover/clear APIs reject closure mismatch. | Protected publication and restart-marker paths still persist, recover, and clear V1 intents. |
| Worker V3 publication intent V1 | A side-by-side namespace commits one outer handoff entry, an ordered provider archive with one entry per supplied external payload, compact opaque replay metadata, one finalized output entry, the complete durable plan, and the producer occurrence. The record is committed last. Current-generation inputs remain protected pending load-envelope readiness; successor-authorized scavenge uses a restartable retirement marker. | Exact restart recovery, strict V3 pending/final receipts, durable claims, currentness reacquisition, completed-state reconstruction, and successor retirement are implemented. Safe publication requires a move-only verified-finalizer authority supplied by `fe2o3-hsaco-finalize`; V1/V2 wires are unchanged. |

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

The rustc child and Cargo parent do not share process-local leases. The parent
recovers only an inert `CompilerModuleHandoffReceiptV3` from the exact default
or named V3 slot, then independently asks for a parent-local currentness lease.
Recovery requires the requested attempt to remain current and claimable,
strictly checks the record, native handoff identity, transaction identity, file
metadata, and complete canonical payload, and never consumes or consults V1 or
V2 state.

`publish_compiler_module_handoff_with_currentness_v3` deliberately preserves a
lease-minting failure even if its first phase already committed the publication.
Callers must not interpret that error as proof that no durable record exists.
After transient failure, a cooperating process can recover the exact inert
receipt and retry parent-local lease acquisition. A stale generation, consumed
record, changed producer or slot, or any record/payload mismatch remains an
error; recovery never converts those states into success.

## Worker V3 restart topology

Worker V3 publication intent V1 stores four attachments beside its fixed record:

- `.handoff` contains the exact outer semantic V3 handoff once;
- `.providers` is a versioned, checksummed archive with each ordered external-provider payload once;
- `.transcript` contains only compact worker, option, plan, diagnostics, provider-evidence, and
  reconstruction metadata; and
- `.output` contains the exact canonical finalized HSACO once.

"Once" describes only this occurrence namespace's physical storage layout. This crate does not
infer that caller-supplied provider payloads are semantically equal or distinct, deduplicate across
occurrences, assign provider identities, or claim higher-level request/response equivalence.

The protocol does not persist bootstrap or replay request/response aggregates. It also does not
persist a second raw HSACO: production integration must use the independently verified
`derive_unfinalized_hsaco_from_finalized_v1` operation introduced on public main by `8385253bf4`.
The finalizer must then deterministically reconstruct or stream-hash both canonical worker
exchanges from the handoff, providers, derived raw HSACO, finalized HSACO, and compact metadata.

Each attachment is written to a private `0600` temporary file, synced, renamed without replacement,
and followed by a directory sync. The checksummed record is written only after every attachment is
durable, promoted through `.record.redo`, and renamed to `.record` last. Recovery uses
descriptor-relative `O_NOFOLLOW` opens, checks file type/mode/link count/length and descriptor
identity, validates every attachment hash, strictly decodes the provider archive and record, and
rechecks the requested producer occurrence against the independent durable attempt registry.
Byte-identical retry attachments are revalidated and both their file and directory name are
resynchronized before the record commit. If any record-absent attachment differs, the complete
private attachment set is removed before the retry. Record-absent recovery removes validated
uncommitted files and returns `NotFound`.

Backend publication alone cannot authorize current-generation cleanup: replay preimages must
remain available until a later protocol durably records that the V3 load envelope is ready.
`clear_worker_v3_publication_intent_v1` therefore returns `ReceiptNotDurable` for the current
occurrence even after an exact V1, protected V2, or strict V3 receipt. A strictly newer same-source,
same-crate generation is independent revocation authority for its predecessor.

Retirement validates every record and attachment before atomically renaming `.record` or
`.record.redo` to `.record.retiring`. That marker name is synchronized before any attachment is
removed. Attachment removals are synchronized before the marker is removed and synchronized last.
The decoded and authorized record descriptor remains pinned from its canonical, redo, or retiring
name through the final marker deletion. After an interruption, normal recovery returns
`RetirementInProgress`; clear or successor scavenge resumes from the marker, tolerates only
attachments already removed by that state machine, and never makes the restart inputs recoverable
again. `resume_worker_v3_publication_intent_retirement_v1` reconstructs successor authorization
without requiring an identity retained by the process that started cleanup, but it refuses to
start retirement from a canonical or redo record.

Each destructive removal first moves its pinned source to a unique quarantine temp. A restart
removes an attachment quarantine and continues from the still-pinned `.record.retiring` marker. If
the retiring record itself reached quarantine before process loss, marker-only resume decodes and
successor-authorizes that exact pinned temp, requires every attachment and ordinary record name to
be absent, removes it, synchronizes the directory, and returns success. The terminal quarantine is
therefore durable completion evidence rather than an ordinary stale temp. The marker replaces the
normal record name, so steady-state and retirement both fit the existing five-final-entry budget.

The exported stale-occurrence scavenge operation accepts only the exact current producer occurrence
or an occurrence superseded by a strictly newer generation for the same stable source and exact
crate name. Current committed records remain protected. A superseded `.record`, `.record.redo`, or
`.record.retiring` is retired through the same ordered protocol; uncommitted exact and temporary
entries use bounded cleanup. Missing registry evidence is not cleanup authority.

Destructive cleanup opens each candidate with `O_NOFOLLOW`, binds its descriptor to a private
single-link `0600` inode, rechecks the name immediately before moving it to a unique
descriptor-relative quarantine name, rechecks the moved name against the open descriptor, and
rechecks both name and descriptor immediately before unlink, then verifies that unlink reduced that
inode's link count to zero. This materially narrows same-UID name substitution windows while
retaining the pinned-directory and cooperative-lock model. Linux does not provide an atomic
operation that unlinks a pathname only if it still names an already-open descriptor, so a
noncooperating same-UID process racing the final check and unlink remains outside the guarantee.

The compact transcript ceiling is a checked formula, not a request/response-sized allowance. It is
the sum of two independent strict-V3 response metadata shells, each with its own maximum diagnostic
and provider-evidence body, plus shared worker/target/option reconstruction metadata and audited
fixed framing/identity bytes. Bootstrap and replay metadata are not assumed equal. On this schema
the exact transcript ceiling is 2,195,505 bytes. The complete logical recovery ceiling is:

```text
MAX_COMPILER_MODULE_HANDOFF_BYTES_V3
+ 64 MiB aggregate external-provider payloads
+ 64 MiB finalized output
+ record + compact transcript + provider framing
+ parsed provider length/hash table + provider/top-level Vec owners
= 388,610,319 bytes (about 370.6 MiB on 64-bit targets)
```

This logical formula is distinct from the hard caller-owner capacity ceiling. Persisted inputs and
recovered results add the actual capacities of the outer, transcript, output, provider-list, and
every provider-payload `Vec` with checked arithmetic. On 64-bit targets the maximum is:

```text
MAX_COMPILER_MODULE_HANDOFF_BYTES_V3
+ 2,195,505 transcript bytes
+ 64 MiB finalized-output capacity
+ 64 MiB aggregate provider-payload capacity
+ 127 * size_of::<Vec<u8>>() provider-list capacity
= 388,599,264 bytes
```

This owner budget is enforced before persistence and again before recovered owners are returned.
The logical recovery budget is validated from the record before allocating any variable-size
attachment. A fresh intent also reserves five missing final directory entries before creating any
of them; the exact limit-minus-five boundary is accepted and one entry beyond it is rejected.
Provider payload order and boundaries are bound by per-entry hashes, a domain-separated archive
checksum, and the record's archive hash. The returned record and bytes remain inert: this crate does
not authenticate transcript origin, assign semantic content identities, or grant publication,
loading, or launch authority.

The remaining integration blocker is in the higher-level finalizer API. On the assigned
`9ca3226635` base, `InertProtectedFirstBuildWorkerV3EvidenceV1` owns complete bootstrap/replay
request byte vectors and canonical response owners; it does not expose a public compact transcript
codec that can reconstruct or stream-hash those canonical wires from the stored components.
`516a101b8e` factors private replay-validation and identity seams, `8385253bf4` supplies raw-HSACO
derivation, and newer public main supplies zero-copy outer-wire ownership extraction. This branch is
intentionally not rebased onto those higher-level changes. A public bounded transcript
constructor/decoder/replayer is still required before this storage foundation can be wired into
production without recreating complete canonical request/response aggregates.

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
