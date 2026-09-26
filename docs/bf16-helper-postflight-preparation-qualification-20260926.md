# Postflight helper candidate and retained source preparation

This is a CPU/source qualification checkpoint for #280 and #282. It does not
enable ordinary nominal-helper compilation, formal verification, target/LLVM
lowering, artifact publication or GPU launch. Broad accepted exits remain
**M1/V1/V2/U1/U2/U3 (6/18)**.

## Implemented boundaries

- A strict entry-resource scope captures the real incoming storage before
  reserving generic callback/result frames. It checks the exact retained
  owner/capture/inventory requirement against that original storage, so a large
  callback cannot hide an incoming floor deficit.
- A source-owned final candidate is exposed only after preparation, canonical
  facts and all three dense passes return through their postflight checks.
  A private actual-Final payload is rejoined to the same immutable owner,
  inventory, source Call, canonical rows and function-qualified Return mappings.
  A fresh checked source query also completes before the external observer.
- A separate rich preparation view retains the actual scalar definition tables
  and argument/allocation provenance alongside enum-payload dominance,
  allocations and constants. Tables remain borrowed within the callback.
  The old four-input preparation API keeps its algorithm charge sequence and
  original drop boundary.
- Callbacks remain higher-ranked and return only Copy + 'static values.
  Original ledger identity, protected storage, monotone work/peak, sticky
  denials, callback surplus and owned-only refunds are checked. This is logical
  resource accounting, not native allocator/RSS enforcement.

The final candidate is not an owned complete-root recipe or an admission token.
The rich view still lacks the separately metered Option and induction
preparation needed for complete root projection.

## Qualification on mi350

The strict entry change passed all **1,861 lowerer library tests**. The combined
candidate/rich source passed **2,446 backend tests** (189 ignored by the default
suite), backend/extractor build, and the explicitly selected genuine parent
test containing **five fresh Rust source sessions**.

The fresh R10 report retains 36 positive numerical runs, 32 request refusals,
two unchanged normal-route refusals, one source refusal, and callback error/panic
controls. Exact work/storage/floor boundary probes remain part of the genuine
route. Identity and Swap01 source cases still use actual compiler-produced
owners; synthetic fixtures are not substituted for source qualification.

A lossless comparison against R8 preserves JSON key/type distinctions and number
tokens. Only generation paths, dependency digests/independently rehashed byte
totals and uniform per-observation cumulative-work increments changed:
50,688,702 for Identity/error/panic and 50,688,837 for Swap01.
Numerics, masks, refusals, storage and peak storage are unchanged. The added
work includes explicitly prepaid negative-control ledgers, not a kernel runtime
performance measurement. The dependency closure still contains 459 files;
its fresh total is 352,670,356 bytes, 182 bytes larger than R8.

| Evidence | SHA-256 |
| --- | --- |
| Entry lowerer gate receipt | `e3bdd2053ffa0edcadd4cbca9ed230451393e5a7d2887393c89cb06a3da9eca1` |
| Combined backend/build/genuine receipt | `45d7b9a1545a133196d9052eef819ec8e1e3eb4f6b34b75025a944b123525189` |
| Fresh R10 observation, 281,064 bytes | `e6a0a0813150c834b67c0553b77b5c0baeca7a4bda5503acbe559858d5b3102d` |
| Independent root comparison, 9,913 bytes | `5156aae6d0dd5430844af997dc510eb25eb464c09b073bee6269f88280d5c5a5` |

Gate inputs, tool/source snapshots and output stream pins were independently
rechecked. Local paths are retained in the qualification records, not portable
execution authority.

## Preserved failed attempt and corrected test

The first combined gate passed its 2,446 backend tests but its genuine R9 run
failed a stale nested-refund expectation. A damaged inner candidate reservation
must not be refunded; the intact outer entry scope may still refund its own
reservation. The earlier test incorrectly expected both reservations to remain.

The correction changes only the genuine test. Direct and wrapped initial-source
refusals measure the actual entry frame using the same callback type, avoiding
a guessed ABI byte count. The erosion case now checks the exact measured
subtraction, unchanged ledger/work/peak/denials and preserved damaged inner
reservation. Two additional bounded probes are prepaid on the original account;
no phase or per-probe limit was raised. R9 and its failed receipt remain retained.

## Remaining integration

Complete root preparation must retain metered Option/induction and assertion/CFG
facts, consume actual final effects without rerunning an unmetered legacy path,
place the proxy at the correct generated ranked coordinate, and preserve real
input reads, output stores, bounds and control flow. Full source-to-ranked
correspondence/attachment must then pass before formal/target/LLVM continuation.
Existing nominal-pending, RawEmpty and attachment guards remain unchanged.

See the [preceding dense/proxy checkpoint](bf16-helper-dense-proxy-qualification-20260926.md)
and [implementation status](assembly-authoring-implementation-status.md).
