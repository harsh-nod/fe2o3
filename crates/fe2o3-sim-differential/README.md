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

## Production Bundle V5 conformance

`production_semantic_capabilities_v3` and
`run_production_semantic_conformance_v3` add a production-source conformance
boundary without changing either earlier differential wire. The runner accepts
only an `AdmittedSimulationBundleInputV5`, revalidates its complete bundle,
checks the retained Bundle V5 content/subject, source-lineage receipts, ABI, and
canonical KIR V10 identities, then executes the already admitted request with
its admitted target and limits. Expectations name unique physical KIR argument
ordinals and compare exact bytes and initialization state. Case IDs, output
count, and total expected bytes are hard bounded.

The compiler integration suite exports ordinary attributed Rust and checks
deterministically generated `i8/i16/i32/i64` and `u8/u16/u32/u64` scalar
comparison/bitwise cases, exact f32/f64 IEEE corner tables, scalar/buffer
layout, and checked `DisjointSlice` output bounds. This is narrower than the
manual KIR V7 V2 corpus. In particular, the following ordinary producer paths
remain explicitly unavailable even though lower-level KIR may model some of
them:

| Family | V3 production disposition |
| --- | --- |
| `i128`/`u128` source arguments | type not retained by the current typed frontend ABI |
| f16/bf16 source arguments | type not retained by the current typed frontend ABI |
| integer switch | emitted fallback trap remains an unsupported external call for simulator preflight |
| core atomic RMW | ordinary semantic-to-ranked projection remains incomplete |
| pointer distance | `MemoryOffsetFrom` is rejected by the ordinary semantic importer |
| volatile load/store | ordinary intrinsic expansion is rejected |
| copy-nonoverlap | ordinary intrinsic expansion is rejected |
| recursive aggregate Bundle V5 input | outside this scalar conformance contract |

These dispositions do not infer hardware behavior or performance. The suite
does not compile on the CPU, predict GPU performance, use HIP/HSA, or grant
artifact/load/launch authority.
