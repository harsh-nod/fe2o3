# fe2o3 simulator differential harness

This integration crate translates bounded cases from the independent
`fe2o3-differential` wrapping-`i32` evaluator into verified canonical KIR V7,
executes that KIR through `fe2o3-kir-sim`, and compares every lane.

```text
fe2o3-sim-differential --seed-start 0 --cases 256
```

The stable JSON result separates its evidence origin from an explicit
`authority: none` and binds the generator configuration and a digest over the
exact case/KIR/output sequence. A mismatch or simulator failure is reduced with
the existing deterministic case reducer while preserving its failure class;
the failure JSON retains canonical source-case bytes and reduced-case bytes,
plus reduced KIR bytes whenever translation succeeds, as a bounded reproducer.
No encoded response may exceed the compiled 1 MiB limit.

Agreement is a differential model observation only. It grants no compiler,
artifact, load, launch, KFD, hardware, performance, parity, proof, or universal
correctness authority.
