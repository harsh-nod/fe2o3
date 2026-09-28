# Original-meter Option source preparation qualification — 2026-09-26

This checkpoint extends the published postflight/helper preparation at
`84e5026188e8a41ba5118334dc75bb2e8618613d`. It is CPU/model/proxy evidence,
not complete-root projection or ordinary compilation. Broad accepted exits
remain **M1/V1/V2/U1/U2/U3 (6/18)**.

## Implementation

The MIR model exposes `semantic_option_producers_with_meter_v1` and
`SemanticOptionDominanceV1::analyze_with_meter_v1`. They share the existing
collector and dominance algorithm. Original semantic refusal order, local
work-unit counter and its independent limit remain; added traversal,
initialization, copying and generic error/result-frame costs are charged to
the caller's original meter. Every allocation is admitted before use.
Meter denials remain terminal and retain accepted reservations until the
caller drops all partial/results; no replacement budget, refund or empty-fact
fallback is introduced. The existing enum specialization retains its exact
generic-frame calculation.

The rich source-preparation callback now owns actual Option producers and
dominance alongside its existing enum/scalar/allocation/provenance facts.
Collection and analysis use the same ModelMeter/PreparationResources and
original source function. Immutable accessors are borrowed within that scope;
partially produced facts and reports drop before owned-only refunds, including
error and unwind. Existing four-input C2 preparation remains unchanged.

The genuine source observer requires a nonempty actual DisjointSliceGetMutCall
producer with exact local/continuation membership and present Some availability.
The source scan is prepaid. Synthetic empty tables cannot satisfy this witness.
This establishes source-derived Option availability, not correspondence for a
subsequent output store or a complete ranked root.

Ten new model control groups cover nonempty Some/None facts, exact/one-short
work and storage, every meter-denial position, sticky/unwind behavior, malformed
and duplicate producers, the independent local cap, and large generic error
frames. Existing rich callback resource tests now include the actual Option
tables.

## Qualification

Root ran the unchanged offline, locked toolchain on mi350:

- MIR model: **318 passed**.
- Backend library: **2,446 passed, 189 ignored**.
- Backend/extractor build passed.
- Explicit genuine-source parent: **passed**, starting five fresh Rust sessions:
  Identity, Swap01, wrong source launch, callback error and callback panic.
- Diff checks and selected source/input/tool pre/post checks passed.

The fresh R11 parent retains 36 numerical positives, 32 request refusals,
two normal-route refusals, one wrong-source refusal and two callback-failure
controls. A lossless JSON comparison against R10 checks keys, value types and
original numeric tokens. Numerical results, masks, refusals, storage and the
1,632,943,151-byte logical peak are unchanged. Only generation paths,
dependency/artifact digests and uniform cumulative work increments differ:
**1,726** for Identity/error/panic, **1,745** for Swap01. These are logical
analysis costs, not kernel timing or a performance claim. All 459 dependency
files were independently rehashed in both generations; each closure totals
352,670,356 bytes.

Retained evidence:

- Gate receipt: 119,638 bytes,
  `6e50514e325a5ec6623b8316aa09cd9bcae135e2e01e8421529591eca907640e`.
- R11 observation: 281,064 bytes,
  `0018a451600c1bb169c4f5463a28fa2e316bfc56c3ad36a7ad83bfebdf1708d3`.
- Root lossless audit: 9,635 bytes,
  `dedbcc1eb0ed44f140589788e7500392e5f429d74dd8638eaeb6cba8cc5d4de6`.
- Qualified source: 8,374 files, 119,703,306 bytes,
  `1eb0f172f99180d84ab5c5ce15b5fdeb9c88c4f2472e843aabb54babce92f6c5`.

## Remaining boundary

Strict complete-CFG induction/report retention, assertion/CFG facts, generic
root recipe storage and strings, actual input reads/output-store correspondence,
final ranked coordinates, full source/ranked attachment and formal/target/LLVM
continuation remain separate work. Normal helper compilation still refuses;
nominal-pending, RawEmpty and attachment guards are unchanged. This checkpoint
authorizes no artifact, native helper execution, GPU dispatch, physical capture,
public runtime activation or milestone completion.
