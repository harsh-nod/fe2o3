# Independent SSA Storage Goldens

The frozen duplicate planner was removed after the parent central31f MIR
storage tests passed. These six tests retain the original case generator,
checks and public queries; their expected outputs come only from the old
planner retained in ART, never from the current implementation.

## Corpus

`goldens.bin` is 47,664 bytes, SHA256:
`d4c7849b716fa31f2184d1356aabc984bf9f4006247d93995093d40ca37e757d`.

Header: 16-byte magic `FE2O3SSAGOLD31V1`, then six little-endian u64 values:
case count, old Plan byte size, word size, Option<usize> byte size, Vec row
header size and format version. This is the pinned 64-bit-host profile,
including old Plan size 376, not a portable Rust layout promise.

There are 396 individually indexed, 120-byte records:
- 32-byte complete input fingerprint.
- 32-byte old canonical construction-plan identity (zero for errors).
- 32-byte public-getter or exact-error fingerprint.
- u64 old storage words, merge row item count and work units.

Cases 0..384 are the original PRNG sequence (384 mixed CFG cases);
384..390 are sparse/empty/maximum-local cases; 390..394 are the four
entry-definition/kill combinations; 394 is the diamond; 395 substitutes its
edge role. No case is dropped or selected by the current planner's result.
Repeated diamond assertions reuse its one independent expected record.

The 80-byte trailer contains exact old work-minus-one and replay-error
fingerprints, then old StorageWords required/limit at the reduced diamond
budget (9437/9297). Normal/unwind-edge separation, invalid block/event/edge
queries, row sharing, exact boundaries and same-input replay remain tested.

The small streaming fingerprint encoder freezes the complete fieldwise
Debug observations used by the original independent comparison. Every
field has a label; None/empty and all enum payloads remain distinct.
It never hashes private Plan Debug/layout or pointer addresses. Old
canonical plan identity is also compared independently as its actual bytes.
A formatting/schema change requires a deliberate fixture migration.

Storage expectations subtract only removed allocations from the OLD
record's storage and row count. They are not derived from the new report
or its current merge rows. Work and all other counters match the old
fingerprint exactly. SHA256 and profile validation fail closed; no runtime
golden regeneration or alternative compiler-profile skip is provided.

## Retained Evidence

ART directory:
`/home/harsh/work/fe2o3-47-endtoend-20260910/owned-ssa-storage-phase30/golden31`

It contains the frozen old ssa/planner/support, previous mounted tests,
old-only generator source/binary, per-case audit, original and independently
regenerated corpus, standalone harness and pre/post-mount test logs.
Baseline planner SHA256:
`f9c758052d6543b8a086082fa355be99370c81e3f9fcaab4ac80d67f09ba822f`.
Support SHA256:
`be83aa39f9e4c792b58cbba8a7b3fb4ede43fdf90d8a75dcbfd3e31a08ce5d2c`.

The generator links only the frozen old planner plus cached sha2 using
nightly-2026-04-03. The existing full differential evidence stays in ART.
These fixtures do not establish a kernel storage-frontier improvement.
