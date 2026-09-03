# fe2o3 simulator differential harness

This integration crate translates bounded cases from independent CPU evaluators
into verified canonical KIR V7, executes that KIR through `fe2o3-kir-sim`, and
compares every lane.

```text
fe2o3-sim-differential --seed-start 0 --cases 256
```

The V2 semantic family corpus is a separate, fixed-size contract:

```text
fe2o3-sim-differential semantic-capabilities-v2
fe2o3-sim-differential semantic-run-v2 --seed 0
fe2o3-sim-differential semantic-replay-v2 --seed 0 --case CASE --kir-sha256 SHA256
```

It covers `i8/i16/i32/i64/i128`, `u8/u16/u32/u64/u128`, 32-bit and 64-bit
target `index`, and `bool`; exact finite additions for `f16`, `bf16`, `f32`,
and `f64`; global pointer load/GEP/store; conditional branches, block
arguments, typed integer switch, and an internal call; overlapping views of
one shared backing; and typed bounds, initialization, and division-by-zero
failures. Integer expectations use a standalone bit-vector evaluator. Float
expectations are an independently enumerated table of exact IEEE result bits,
not host floating-point evaluation.

Success evidence binds the capability/exclusion contract, every case ID,
canonical KIR identity, expected bytes, observed bytes, and rejection disposition. Replay requires the exact seed,
case ID, and lowercase KIR SHA-256. A mismatch retains the canonical KIR and a
bounded first-mismatching-scalar reduction. The capability query lists the
intentional exclusions, including nonfinite/rounding-edge floats,
transcendentals, concurrency families, and physical-GPU parity. Unsupported
semantics are not approximated.

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
