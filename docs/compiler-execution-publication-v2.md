# Native Receipt Publication V2

## Status And Authority

Move-only `CompilerExecutionReceiptPublicationV2`,
`CompilerExecutionReceiptPublicationAckV2`, and
`CompilerExecutionReceiptCarriageV2` carry complete native
[signed receipts](compiler-execution-receipt-v2.md). Private codecs share V1/V2
framing, hashing and comparison mechanics; each public adapter selects one
nominal family. V1 wire, getters, cloning and diagnostic order remain unchanged.

A publication binds a signed receipt to an issuer-journal identity and compiler
occurrence. An ACK additionally names a Worker-ledger record. These are inert
claims: constructing, decoding or comparing them does not establish durable
storage, protected compiler execution, key custody or rollback currentness.

Carriage verifies internal consistency under its carried policy, not a policy
independently pinned by a protected consumer. It grants no compiler, load or
launch authority. No service or producer is activated by these APIs. No protected
proof, simulator or GPU qualification is credited; M0-M7 and 47/47 remain open.

## Canonical Wire

All integers are little endian. Every header is 24 bytes: eight-byte magic,
version 2, zero flags, exact total length, zero reserved. All identities hash
`domain || u64_le(prefix_length) || prefix`, excluding the final 32-byte identity.

Publication is 584 bytes, magic `F2O3CES2`:

| Offset | Bytes | Field |
|---:|---:|---|
| 24 | 32 | PolicyV2 identity |
| 56 | 32 | Issuer-journal identity |
| 88 | 32 | Compiler-occurrence identity |
| 120 | 32 | ReceiptV2 identity |
| 152 | 400 | Complete signed ReceiptV2 |
| 552 | 32 | Publication identity |

ACK is 288 bytes, magic `F2O3CEA2`. Offsets 24 through 151 repeat the four
publication bindings above. Offset 152 holds its publication identity, 184 the
Worker-ledger record identity, 216 the eight-byte sequence, 224 the 32-byte next
rollback anchor, and 256 the ACK identity. No signature or durable I/O is added.

Carriage is 2090 bytes, magic `F2O3CRG2`:

| Offset | Bytes | Complete Native Record |
|---:|---:|---|
| 24 | 216 | PolicyV2 |
| 240 | 946 | RequestV2, including ChallengeV2 and SubjectV2 |
| 1186 | 584 | PublicationV2, including ReceiptV2 |
| 1770 | 288 | AckV2 |
| 2058 | 32 | Carriage identity |

Identity domains are `FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION/V2\0`,
`FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK/V2\0`, and
`FE2O3/COMPILER-EXECUTION-RECEIPT-CARRIAGE/V2\0`. Resealing an outer record cannot
admit a nested V1 record. There is no legacy fallback, owner cloning or projection.

## Validation Order

Publication decoding verifies the nested receipt before journal/occurrence
nonzero checks, duplicated policy/receipt bindings, and terminal identity.
ACK decoding checks its bindings, Worker identity, sequence and anchor before
its terminal identity. V1's deliberate constructor/decode asymmetry is retained:
a correctly signed zero-policy receipt can form a publication and ACK, while
ACK decoding rejects its zero policy. Neither constructor establishes trust.

Carriage decodes policy, request, publication and ACK before contextual checks.
It then verifies the receipt signature and policy/request relationship, checks
the ACK/publication relationship, and finally checks its own identity. The
receipt check uses the request's **prior** anchor; the ACK carries the receipt's
**next** anchor. A crate-private borrowed check shares consuming receipt
verification's signature-first logic, charges the same quota, and creates no
verified owner or authority.

ACK/publication matching intentionally does not authenticate Worker-ledger
identity. `matches_worker_ledger_record` is a separate comparison against the
caller's independently reacquired durable record. Even successful comparison
cannot itself establish where those expected bytes came from.

## Resources And Ownership

Every public constructor, decoder, identity comparison and relationship check
uses the caller's ledger. Logical work totals are:

| Operation | Work |
|---|---:|
| Publication construction, identity, issued-record comparison | 18696 |
| Publication decode | 170768 |
| ACK construction, decode, identity, relationship comparisons | 9224 |
| Carriage identity | 66888 |
| Carriage construction | 224088 |
| Carriage decode | 475656 |

Only outer work is prepaid by a nested scope; real child decoders/verifiers
charge the rest on that same ledger. Carriage decode performs five public-key
validations and two strict signature verifications, with no signing. Request
decode still invokes SubjectV2's real decoder and preserves its 256 MiB ledger
storage-limit ceiling. No fresh or unlimited child budget is substituted.

Keep complete input owners prepaid. Publication construction transfers ReceiptV2
retention and returns only growth. ACK construction borrows publication and
returns full ACK retention. Carriage construction transfers all four child
reservations and returns only growth. Refusal drops consumed owners but leaves
their reservations for caller retirement. All wire decoders return full output
retention, not growth above internally decoded children. Reserve returned
`additional_storage()` before retaining the output.

Each child decoder's full returned charge is reserved immediately and kept until
outer cleanup. Publication already includes receipt retention; request already
includes challenge and subject retention. Do not add those again. Exact wire
minimum floors are 584, 288 and 2090 bytes; backing-allocation metadata remains
the caller's responsibility. Wrong lengths reject without traversal.

Let `Dx` denote retained child storage and `Sx` its complete decoder peak,
with P/Q/U/A denoting policy/request/publication/ACK and R receipt verification.
Let `I = Dp + Dq + Du + Da` and `Fc` be the fixed carriage frame. Additional
carriage decode peak is:

```text
Fc + max(Sp, Dp + Sq, Dp + Dq + Su, Dp + Dq + Du + Sa, I + Sr, I + Sa)
```

Construction adds `Fc + max(Sr, Sa)` above its prepaid inputs. Publication decode
adds its fixed frame plus receipt-decoder scratch. Compile-time guards ensure
child scratch covers returned retention and result/control layouts fit the
logical schedule. Exported `*_STORAGE_V2` constants identify fixed frames;
`*_DECODE_STORAGE_V2` and `*_CONSTRUCT_STORAGE_V2` identify composite peaks.
Scope cleanup restores the entire entry reservation without refunding accepted
work or clearing peaks/first denials. These quotas are not machine-instruction,
latency, generated-stack, allocator or RSS bounds.

## Validation And Next Steps

Tests freeze independent V1/V2 identities and full-wire hashes, mutate every
byte, reseal mixed families, assert paired-error precedence and ACK asymmetry,
exercise nonzero prior anchors, and check full/delta retention, exact/one-short
work/storage/input floors, nested ceiling propagation and cumulative denial
history. Compile-fail contracts reject cloned or mixed-family native owners.

The production path still requires versioned durable/service records, broker
reconstruction of the actual V4 transaction, Worker replay, runtime/host joins,
both Cargo restart paths and coherent provisioning before producer activation.
Preserve external-anchor commit, durable Worker publication/reacquisition and
issuer ACK ordering; old ledger presence must never be mistaken for empty state.
