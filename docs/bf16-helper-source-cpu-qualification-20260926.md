# Genuine Rust helper emission and CPU observation — 2026-09-26

This checkpoint connects authenticated Rust BF16 helpers to canonical
Function/Call/Return emission and the two-frame CPU observer. It is a closed
diagnostic qualification, not normal LLVM or GPU qualification. Broad accepted
exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Implemented contract

The typed nominal materializer consumes actual retained semantic SSA and its
launch roster. The helper has twelve logical scalar inputs (four A, four B,
four accumulator values) and four results, with checked Identity or Swap01
return order. The zero-runtime-component context is not a claim that the
original physical Rust FnABI is erased or interchangeable.

Borrowed emission views join the same source owner, exact operation spans,
helper/caller identities, logical SSA components, and return permutation.
Equivalence replay reconstructs from retained source and compares canonical
bytes, correspondence and source coverage on the original resource ledger.
The supported graph is acyclic and has one full-wave helper call. Unsupported
assertions, shapes, launches, and normal consumers refuse explicitly.

The optional relation is boxed: ordinary source owners retain only its pointer
header. Its heap payload remains prepaid by the retained emission envelope;
replay pays for both live owners. This avoids the stack overflow observed with
the earlier inline 312-byte relation, without increasing the resource caps.

## Executed evidence on mi350

The emitter regression passed 4,575 test executions across 36 groups, zero
failures and 270 ignored cases. This includes focused emission tests, lowerer
and backend all-target suites, and lowerer documentation tests. Counts include
overlapping configurations, not unique tests.

The separate genuine-source parent passed five fresh rustc sessions:

- Identity and Swap01 each execute 18 positives: six independent exact-integer
  matrix patterns and three output lengths. Each checks all 256 helper result
  words and all 256 caller result words, the actual two-frame stack, allocation
  identities, initializedness, committed stores, untouched tails and canaries.
- Each also executes 16 invalid-request cases: uninitialized inputs, unsupported
  BF16 domains, step/record limits, sink stop/failure, wrong grids and wave size.
- Wrong launch refuses before the numerical callback.
- Error and panic callbacks each complete one numerical observation, then test
  owner-drop-before-refund while retaining caller-owned charges and consumed work.

Both positive layouts measured a logical replay/phase peak of 1,632,943,151
bytes, within the unchanged 2,147,483,648-byte limit. The retained nominal
receipt is 816,287,332 bytes; the separate occurrence receipt is 11,424 bytes.
These are conservative logical reservations, not allocator/RSS measurements.
The existing cumulative work limit remains 2^54. The report's final_storage
is measured before normal-consumer handoff, not after normal teardown.

The same owner reaches the explicit normal source-ranked refusal:
"BF16 nominal source-ranked projection does not yet support checked
source-local helpers." This expected boundary is not normal compilation success.

The completed compatibility gate additionally passed 4,569 test executions
across 35 groups, zero failures and 273 ignored cases. It rebuilt the extractor,
ran the historical helper-transport and root-only core/normal/CPU ladders in
fresh sessions, checked the unsafe-source inventory, and passed diff checks.
These counts overlap the earlier emitter regression; they are not unique tests.

### Retained records

Paths are relative to the task root on mi350:
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr`.

| Record | Bytes | SHA-256 |
| --- | ---: | --- |
| logs/phase28-resume-r19-compiler-bf16-nominal-helper-emission-r6/receipt.json | 27523 | de31973d10cd4b2979ed17825560e7023caf52f61284f415656f08669fe8d980 |
| logs/phase28-resume-r19-compiler-bf16-helper-source-cpu-r1/receipt.json | 28865 | da556f800684969893fc669ba24bcf9481ea3ebe22ed10e36cf4198167104da8 |
| logs/phase28-resume-r19-compiler-bf16-helper-source-compat-r1/receipt.json | 39611 | 2c3e2fc73b278e94d9c8c5971fcc2e7c86851c977f54d677073720a6aad9f78b |
| phase28-bf16-call-source-cpu-actual-r1/observation.json | 281023 | b353f15ce346e3c8f8e2d0ec2f8d64f3ca4108925c05dd0ff20b3cff32a06a8b |
| phase28-drafts/bf16-call-source-cpu-root-audit-r1/audit.json | 10581 | 30991107b1898a4d0d8dac580a4e9d0f1ede42984eb067ed9378b304e662ff6e |
| phase28-drafts/bf16-call-source-cpu-ladder-independent-review-r1/REVIEW.md | 3305 | 7d3719cf6b54b56632c2c8372831ab3a6e2348dcca93095451312b4ca982ec2f |

Earlier failed emitter attempts remain retained: compile errors, missing
exhaustive consumer refusals, exact legacy header-accounting mismatches, and
the inline-relation stack overflow. The corrected run did not enlarge resource
caps or suppress tests. Historical debugger/UI observations grant no authority.

## Reproduction and remaining work

Use pinned nightly-2026-04-03 and the repository's offline/locked Cargo setup.
Set FE2O3_TEST_BF16_CALL_CPU_OUTPUT_V1 to a fresh absolute path outside the
repository, then run:

```sh
cargo test --offline --locked -j2 -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::gfx942_bf16_call_source_cpu_qualification_v1_tests::actual_bf16_call_source_cpu_ladder \
  -- --exact --ignored --nocapture
```

This builds actual dependencies and runs isolated rustc sessions; it is not a
public compile/launch CLI. Inspect successful parent and raw/accepted child
records together. Supervision covers direct children/process groups, not
arbitrary whole process families.

Normal ranked/formal/LLVM continuation, edited-tile source promotion, broader
helper signatures and floating-point semantics, physical-register control,
final-code and hardware qualification remain separate work. No snapshot or
copied diagnostic row can replace source custody or mint artifact, proof,
debug-stop, or launch authority.
