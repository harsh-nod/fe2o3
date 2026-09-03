# Profiler Variant V2

Status: additive production evidence contract for GitHub issue #215 T3.

Profiler Variant V2 composes two exact Variant V1 treatments with optional
per-treatment PC-sample/Source-ISA and decoded-ATT/Source-ISA evidence. It does
not change the Variant V1 library API, wire schema, or `variant-jsonl` route.
The separate agent route is:

```text
fe2o3-profiler-service variant-v2-jsonl
```

## Exact inputs

Each treatment retains the exact Variant V1 manifest, semantic workload,
rocprof source, Bundle V4, schedule bytes, HSACO, and optional ISA, Counter V2,
and PC Capture V3 bytes. V2 additionally accepts:

- PC: the exact rocprof PC source, exact PC code-object relation, exact Characteristic archive, and a
  strictly ordered bounded set of capture-local sample identities; and
- decoded ATT: the exact decoded interchange, exact Characteristic archive,
  exact content-bound code-object identity, and a strictly ordered bounded set
  of decoded-record identities.

The V2 request binds both treatment manifests, both exact schedule byte
sequences, every optional evidence byte sequence, every selector, and the ATT
code-object selector. Replacing any of those inputs after request construction
fails admission. Existing PC and decoded-ATT sessions revalidate the exact
HSACO digest and length, target, code object or relation, kernel-symbol domain,
Characteristic identity, cursor, and selected local record. Unknown or foreign
selectors, target/artifact/Characteristic substitutions, ambiguous overlapping
correlations, duplicate selectors, unknown fields, non-lowercase hex, and
bounded-resource violations fail closed.

## Result semantics

Every positively observed occurrence cites the session binding, selector,
query, Characteristic occurrence, item, and Characteristic identities. It
retains semantic-operation, neutral-KIR, target-KIR, LLVM, and symbol-relative
ISA coordinates. PC sampling coverage and ATT decoder completeness/loss are
copied from the admitted inputs without caller-supplied upgrade fields.

Cross-treatment pairing requires exactly one positive occurrence on each side
with the same observation kind and exact source-node, source-file/span,
MIR-node, and MIR-coordinate identity. A paired result reports changed
semantic-operation, neutral-KIR, target-KIR, LLVM, ISA-interval,
transformation, and classification axes in canonical order. Multiple
occurrences at the same stable key are not paired arbitrarily.

An observation present on only one side is not called added or removed. PC
sampling is stochastic, decoded ATT has no positive complete-execution signal,
and Characteristic intervals may be sparse. The result therefore returns a
typed `unmatched_observation_cannot_establish_addition_or_removal` state.

The schedule has a separate domain-separated identity over the exact supplied
bytes and remains caller-declared content, not observed or producer-
authenticated semantics. Variant V1 profiler KIR claims are not substituted
for Characteristic KIR identities: until an admitted production structural
bridge connects those axes, V2 reports
`profiler_kir_to_characteristic_kir_bridge_unavailable`.

Variant V1 ranked schedule/resource evidence remains a ranked co-observation.
V2 always reports `positive_co_observations_do_not_prove_causation`; it does
not claim causal attribution, superiority, complete workload coverage, live
capture, decoder custody, collection, scheduling, attach, or execution
authority.

## Bounds and compatibility

- at most 64 selectors per evidence kind and treatment;
- at most 512 projected occurrences per treatment;
- at most 4 MiB per comparison result;
- at most 64 JSONL requests per service session;
- canonical lowercase hex and strict unknown-field rejection; and
- deterministic response identities over the complete response preimage.

The implementation is generic over admitted kernel artifacts and
Characteristic records. It contains no kernel-name, workload-shape, HIP/HSA,
or simulation-bundle dependency. Direct-KFD runtime evidence remains a
separate query surface; runtime/copy/dependency causality and distributed
overlap remain dependent on their own admitted identities and #182 producers.
