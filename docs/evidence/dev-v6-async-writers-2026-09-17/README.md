# V6 Ordinary Async Submission Writers

CPU/test-qualified development above canonical base
`d32d0f326e7b0af638c1e1f0fe9c18e3ca1f5809`. Accepted checkpoints remain Native
R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3. This does not complete
V5/V6, A1/A2, #182, protected native composition or HIP/HSA parity.

## Change

The opt-in Context journal now records ordinary async launch/copy destinations
under the original Context submission ID. Nonempty writable rosters are sorted,
deduplicated and revalidated before Begin. Three insertion indexes have headroom
before backend entry; read-only/default submissions need only the two facade
indexes. An independent writer root survives no-handle failures and release of
submission metadata. Installed records carry an exact expected writer reference,
so missing or substituted roots cannot silently publish completion or callbacks.

Conclusive Success settles before callback delivery. Only initial Rejected or
explicit unpublished cancellation permits NoEffect. Other failed/resultless
completion retains Unknown; Pending, rejected observations, flush and metadata
release do not establish settlement. Terminal failures and caught backend panics
quarantine all retained ordinary writer destinations, including failures of
neighboring Context operations. Original boxed diagnostics and unwind payloads
are preserved. Submission-Unknown physical disposal remains Unsupported.

KFD first progress can no longer return definite Rejected after installing
accepted compute custody. It retains the pending launch and all indexes/credits,
seals the backend, and promotes the stable failure kind to Terminal without
reformatting the owned detail. See the
[integration boundary](../../runtime-context-version-journal-async-v1.md).

## Qualification

The frozen source ran locally in
`/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917`. GNU and scoped musl each pass:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| fe2o3-runtime | 991 | 17 |
| fe2o3-host | 271 | 4 |
| fe2o3-runtime-model | 779 | 2 |

Each target has 2,041 passes and no failed, measured or filtered tests. All 21
added tests pass: fourteen Context journal tests, six boxed-diagnostic matrix
tests and one KFD accepted-custody classification regression. Matrices exercise
initial no-handle failures, observation/cancellation/flush outcomes, unrelated
facade calls, cleanup, allocation/host-write panics, exact accounting and
secondary-settlement diagnostic precedence. Callback, stale prepared-copy,
capacity, identity, canonical roster, event and retained-Unknown cases are covered.

Strict all-feature/all-target Clippy, scoped formatting, no-default checks,
86 doctests and unsafe-source policy (5 passed, 1 ignored) pass. Musl uses
`FE2O3_HIP_SYS_DISABLE=1`; unrestricted musl/HIP linkage is not qualified.
This is full runtime/host/model unit coverage, not a fresh full lower-KFD suite.

Fifteen closed raw records retain exact argv, UTC start/finish, output and exit
zero. The six executed unit binaries are hashed. `roster.awk` requires exactly
host/runtime/model once per target, exact executable target/package paths, unique
test rows and one matching summary per package. It includes split-line parent
results around expected-abort subprocesses and rejects incomplete outcomes.
Its policy test accepts both sealed host-write baseline logs and rejects ten
mutations. GNU/musl complete rosters match across all 2,064 named status entries,
with SHA-256 `769dfad3400c2e3cb1f943b7ba9376dc850c02fed3e1ffbc2f143c63c6a75d97`.

The manifest covers all 22 changed source files: 18 Rust and four docs, checked
before and after qualification. Its SHA-256 is
`a1a5a44cbd009ca9b7a6c131c34f04cf8affeb1dfd948445d4e00bd674e73737`.
The exact patch above the base has SHA-256
`e75495465d3e9ce9d69463a8c54da565d832f877375609e75dc10c882cb25293`.
Independent read-only reviews checked ownership, failure classification, test
scope, source coverage, evidence parsing and claims. They are not proofs.
`SHA256SUMS` seals this archive; the recorder refuses sealed or duplicate records.

## Limits

No native GPU, formal solver or matched HIP/HSA benchmark was run for this
packet. Scripted owners and mock memory are not GPU execution. The KFD missing
predecessor is a constructed invariant-fault fixture, not a healthy or native
workflow. Generated-path terminal wiring is covered by existing CPU regressions
and source review, not a new native mixed generated/ordinary workload. Hostile
boxed backend diagnostics are injected; secondary bookkeeping panics are not.

SPI classifications remain Contracted. The V4-J1 issuance proof is unchanged
and does not prove the production settlement or Rust/native correspondence.
Generated protected writers, backend aliases, input/read leases, ordered
overlapping writers, content recovery, cross-run versions, complete physical
disposal and aggregate residency remain open. No lineage or reuse authority is
exposed. Native depth/overlap/failure campaigns and matched performance remain
required by the full runtime parity/performance goal.
