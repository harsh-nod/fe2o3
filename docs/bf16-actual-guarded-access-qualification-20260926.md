# Actual retained-input guarded-access preparation

Status: a private compiler prerequisite, not complete nominal-kernel admission.
Accepted broad exits remain M1/V1/V2/U1/U2/U3 (6/18).

## What changed

The actual retained-input connector now continues through identity
`DisjointSliceGetMut` access preparation in the same pending assembly,
operation stream and value-ID namespace as the entry prefix and invocation
indices. The supported producer profile is unchanged. This does not splice
a separately prepared component into the real kernel.

Ordinary compilation and this bounded preparation share the mutable-access
normalization and append logic. Ordinary allocation, diagnostics, partial
emission and refusal ordering remain explicit legacy behavior. The paid path
retains incomplete operations, views, comparison lists, access indices and
predicates in the outer owner across callback/error/panic postflights.
Accepted storage is released only after its payload has been dropped.
Occupied owners, nonempty capacities, foreign ledgers and retries refuse.

The resulting borrowed view contains access data only. Its `semantic_site`
remains `None`: the accessor call is not the later dereference/store site.
An output extent is a source-identity proposal, not proof of whole-slice
equivalence. Reference origins, actual memory-use sites, guards/CFG/assertions,
complete effects and bounds/reference-write checks still need to form one
complete unverified root recipe before mandatory verification can admit it.

## Qualification

The regression passed 331 model tests and 2,654 backend tests (189 ignored),
plus the backend/extractor build. There are 18 new component tests and five
new genuine-observer tests. A first regression attempt failed to compile a
test-only `assert_eq!` on an intentionally opaque identity lacking `Debug`.
The corrected assertion preserves equality without adding `Debug`; the failed
receipt and exact one-line inverse remain retained.

Five fresh genuine Rust sessions cover Identity, Swap01, wrong launch,
callback error and callback panic. An independent oracle rescans actual
source calls and checks receiver/index identity, allocation provenance, width,
operations, cache rows, predicate rows and all nested access fields. It does
not call the shared normalizer, cache, width or predicate-emission helpers.
Summary counts do not supply source custody or replace payload checks.

Each prepared case reports one root, zero references/reserved reference
values, three operations, next value ID two, 31 locals, one index assignment,
zero processed index edges, one FIFO entry, one view, one guarded access and
one predicate. Wrong launch refuses before either actual preparation observer.

The nine guarded-access observation/control runs debit 2,026,119 logical work
units for Identity/error/panic and 2,026,443 for Swap01. Their accepted
assembly/access frames are 11,104 and 4,928. The existing prefix/index observer
now debits 1,921,275 or 1,921,599, with an accepted assembly frame of 10,944.
The handoff remains 15,040: consuming continuation 10,832 plus borrowed-input
view 4,208. These are resource-accounting observations, not GPU timings or
performance gains. They do not enforce native allocator, RSS or machine-stack
limits.

All 141 comparison controls passed. Separate lossless commands passed the
cumulative R16-to-R22 and direct R20-to-R22 checks, followed by an independent
R21-to-R22 check. They preserve exact keys, array order, types and numeric
lexemes, all full diagnostic sidecars and physical positions, the complete
8,465-file compiled-source census, all 389 genuine-session inputs and 42
explicit compiled roles. Cumulative checking rehashes three complete dependency
closures; direct checking rehashes two. Each closure contains 459 files and
352,670,356 bytes; dependency-byte deltas are zero.

Cumulative R16 and direct R20 each observe 232 changed fields; R21 observes
132, all within the unchanged 238-path policy. Numerical results, masks,
refusals and admission fields remain exact. From R21, all storage fields also
remain exact: only 96 work fields and 36 provenance fields change. Work
increases are 2,036,199 for Identity/error/panic and 2,036,523 for Swap01.
Cumulative increases are 63,874,963 and 63,875,982; direct R20 increases are
3,962,434 and 3,963,082.

Separate, source-bound calibration at the original canonical source path
measured all six accepted instances of three S3 frame classes. Assembly
changes from 9,296 to 10,944, complete graph from 1,472 to 1,488, and initial
graph from 8,912 to 8,928. Thus the S3 increase is exactly
six times (1,648 + 16 + 16), or 10,080. The R21 increase is this measured
change plus the nine S4 runs and zero handoff change. No report residual is
used as an inferred frame constant.

Both canonical calibration comparisons preserve all numerical, work, storage,
source-identity and admission fields. Each permits 41 actual provenance
differences within a separate 143-path policy. Its five additional metadata
digest fields are derived from complete metadata bytes after exactly two
fixed Cargo cache-path substitutions; no source-path or arbitrary digest
override is accepted. The original strict policy's cache-metadata refusal
remains retained. The comparator checks retained instrumentation inverses,
qualified source audits and the completed restoration of all three worktrees;
it does not claim to rehash both calibration snapshots or their full
dependency trees again.

The cumulative command reserved 1,243,303,089 content-plus-EOF bytes across
10,435 reads; direct R21 reserved 890,579,220 bytes across 9,963 reads.
Both fit their unchanged 1,280 MiB / 11,000-read / 180-second limits.
Synchronous filesystem interruption is not proved by those deadlines.

Earlier calibration attempts are retained as failures or nonqualifying
observations: a test-only module-alias compile error, a relocated-source run
with unexplained source/work drift, and a root-storage guard termination.
The accepted probes use the original source path and fresh output identities.
No failed attempt was relabeled or discarded.

Fresh ordinary ladders passed 36 composition and two direct-BF16 sessions.
All 38 observation bodies and 52 artifacts are byte-identical to S3. This
qualifies preservation of ordinary routes, not nominal-helper admission.

| Evidence | SHA-256 |
| --- | --- |
| Regression R1, test compilation failure | `639b314560dd61c3c04d3ed6d1602a429b20d5fc24c7cf49c9e07e0f6ba88b7a` |
| Regression R2, passed | `d99be4fcd2196cd114557c90f996c94c5b56053e62e624c799c2fd37a1a24f4e` |
| Genuine-source sessions, passed | `a112799b886b6d3364b9bff0b74e37ccd503e867e17c312dfce0658809b405c4` |
| Actual R22 observation | `a76bacff11f92de754cf463e5a614f72dc0db6513310f5c689878ef4a37acc34` |
| Ordinary ladders | `05ea729725cd2be5277d091bcc1a007abdd8b8816015221c4a4c706992ba18a5` |
| Normal output readback | `3d429dcb10cd4c8be0302e14c326aa70ae722bf5922679352e55f07823e30261` |
| 141 comparison controls | `71fe3ba0e2b8077e07fcc682539b5b75243825ca3bc183444c9c96b6442dc19d` |
| Canonical S3 probe | `8827333bb8d963c104cc60667b1f409e570f7f7f2feae8f60ada46337927fdb9` |
| Canonical S4 probe | `e07d01a9c82ff386ff188e984f52679c5410b217a0cf99165988edb04256dbb2` |
| Full source restoration | `513e6fb5273b76963ea5b4f5c34724b6a40a29d5c5040a8dfed1100a75bba540` |
| Cumulative R16 / direct R20 comparison | `dc519fc3a8ed4cb95b8ba5738ccbb8fbd50066eb8c9b546a435962ff9b35b418` |
| Direct R21 comparison | `6ffc8bee869451f22124add1b172cd24133da580fa0471ebecd7f0c902b01d28` |

## Boundaries

Component controls cover every short work/storage boundary and retained
partial state, invalid writable-access metadata, cache conflicts, late
destination refusal, direct-write ordering and occupied-owner variants.
These are logical-denial tests, not injected allocator/OOM failures.
Genuine error/panic controls run after preparation, not at every allocation.
Nonempty reference bindings still refuse before actual retained preparation;
their inert component fixtures do not establish actual-source admission.

The connector remains private genuine-test routing. This checkpoint adds no
public ready token, detached/edited-input promotion, ordinary nominal-helper
admission, nominal LLVM continuation, GPU launch or debugger capture.
