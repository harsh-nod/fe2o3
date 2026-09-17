# V5 Allocation Journal Foundation

CPU/model-qualified development above canonical base
`12b4558aa7f0748821d97c5ff69266c1881b3890`. Accepted checkpoints remain Native
R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3. This is not completion
of V5/V6, A1/A2, #182, protected native composition or HIP/HSA parity.

## Change

The production Context now has a construction-only, explicitly bounded allocation
journal profile. Ordinary allocations and complete generated shell rosters use
the original Context identities. Provisional registration precedes backend entry;
ambiguous no-handle attempts remain counted through cleanup. Exact disposal is
required before retirement in explicit release, cleanup and generated retirement.
Post-return handles are rooted before credit/journal commits that may unwind.
The constructor returns the owning backend on returned errors, and invalid
capacity bounds reject before enumeration. Default `open` remains unconfigured.

The model adds atomic canonical batch enrollment and exact batch retirement,
without rejecting unused lower keys in later batches or allocating during those
operations. Production guarantees fresh nonwrapping identities after retirement.
Metadata observations grant no data lineage, initializedness or reuse authority.
See the [integration boundary](../../runtime-context-version-journal-allocation-v1.md).

## Qualification

The final frozen source ran locally in
`/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917`. GNU and scoped musl each pass:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| fe2o3-runtime | 955 | 17 |
| fe2o3-host | 271 | 4 |
| fe2o3-runtime-model | 773 | 2 |

The complete named test/status rosters match across targets. Musl sets
`FE2O3_HIP_SYS_DISABLE=1`; this does not qualify unrestricted musl/HIP linkage.
Twenty new tests cover allocation batches, stale/foreign references, exact
retirement, constructor ownership, capacity-before-ID/backend entry, ambiguous
allocation retention, confirmed no-owner refunds, diagnostic/panic identity,
credit coexistence, mixed live/provisional cleanup, and generated whole rosters.

Strict all-feature/all-target Clippy, scoped formatting, no-default-feature
checks, 86 doctests and unsafe-source policy (5 passed, 1 ignored) pass. Fourteen
closed raw command records have exit zero. Logs retain exact commands, UTC start
and finish, toolchain identity, and hashes of the six executed unit-test binaries.
Independent read-only reviews checked production ownership, model transitions,
source coverage and the limited claims. They are not mechanical proofs.

The 13-entry source manifest covers all 11 changed/new Rust files and both source
docs. Its SHA-256 is
`eda356d1cae90bb0d310625a753fb713f1f18e0b96b1572937c38156d8ca0252`.
The complete source patch above the base has SHA-256
`0f550a9a810c8a9e46a41bd1954dc6a21d9c2b596400e7ac51d0ebba013f2e91`.
Source checks bracket the qualification commands. `SHA256SUMS` seals this archive;
the recorder refuses to modify a sealed archive or overwrite an existing record.

## Limits

No native GPU, formal solver, matched HIP/HSA performance or storage-allocation
fault-injection run was performed. No MI300X files or processes were created.
The existing V4-J1 proof is unchanged and does not prove these allocation
transitions or their production correspondence. Backend-enumeration unwinding
is not fault-injected. Model partition preservation assumes valid prestates;
local guards do not comprehensively detect unrelated corruption.

Writer issuance/settlement, all-mutation coverage, ordered writers, Unknown
disposal/recovery, input leases, aggregate residency and native qualification
remain open. No journal lineage query or reuse API is exposed while these joins
are absent. The broader runtime parity/performance objective remains active.
