# Profiler Regression Explanation V1

`ProfilerRegressionExplanationV1` is the bounded explanation layer over the
existing exact Profiler Variant V1, V2, V3, and complete structural-catalog
comparisons. It is kernel-neutral: inputs are identities, admitted evidence,
resource kinds, observations, and structural coordinates. Kernel names and
algorithm families do not participate in admission or ranking.

## Input and replay

`explain_profiler_regression_v1` accepts the same request and treatment owners
as `compare_profiler_complete_structural_v1`. It recomputes that complete
comparison instead of trusting a serialized result. When a treatment supplies
an admitted `ProductionProfilerKirArchiveV1`, archive admission also decodes
the existing production `amdgpu_lowering` receipt, validates it against the
exact neutral KIR, and retains its optional optimizer V4 audit.

The archive wire remains V1 and unchanged. Current receipts expose the exact
optimizer policy, pre- and post-optimization target KIR identities, bridge and
correspondence identities, resource limits, epochs, graph work, work units,
and every ordered pass decision. Legacy receipts remain valid and report
`legacy_replay_has_no_optimizer_audit`; no decision is inferred.

## Explanation rules

The result retains the entire complete comparison, both optimizer statuses,
ranked hypotheses, and ordered next measurements. A hypothesis is emitted only
for an exact comparable treatment pair with at least one longer captured
dispatch duration and one of these independently represented changes:

1. replayed optimizer or exact structural mapping;
2. final HSACO static resources;
3. exactly bound counter values;
4. positively observed source/MIR/KIR/LLVM/ISA occurrences; or
5. complete same-domain structural multiplicity.

Rules have stable IDs and fixed order. Each result carries supporting evidence
IDs, evidence from non-longer dispatches as contradictions, typed missing facts,
and low or moderate confidence. Confidence is a disclosure of evidence shape,
not a probability. The origin is always `inferred`, and causal attribution is
always `unavailable`.

The planner asks for only missing discriminators in deterministic order:
exact comparable recapture, counters, PC sampling, decoded ATT, authenticated
schedule execution, and bounded controlled repetitions. Every action is
descriptive and requires separate collection authorization. PC absence remains
sampled absence; ATT remains target-scoped and loss-aware; repetitions do not
become deterministic GPU replay.

## Agent operation

The restartable service adds `explain_regression` to:

```text
fe2o3-profiler-service variant-v3-jsonl
```

The request embeds the unchanged Variant V2 treatment wire and optionally
cites the same two previously opened archive identities as V3 comparison. The
response schema is `fe2o3-profiler-regression-explanation-v1`. Requests retain
the existing unique request ID, exact expected revision, archive-owner, input,
and response-identity checks. The complete response is capped at 20 MiB; the
service envelope adds only its fixed bounded overhead.

The operation is read-only. It grants no execution, attach, scheduling,
collection, decoder, compiler, proof, publication, load, launch, dispatch, or
runtime authority. It does not authenticate external profiler provenance or
claim that a compiler pass, resource delta, counter, or changed occurrence
caused a duration delta.
