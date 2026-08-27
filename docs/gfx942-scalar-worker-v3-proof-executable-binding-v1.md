# gfx942 Scalar Worker V3 Proof/Executable Binding V1

## Status

The exact scalar-GEMM Worker V3 audit now has one proof-to-executable path. It
does not create a second compiler or verifier pipeline and it does not grant
publication, load, launch, or GPU authority.

The joined audit closes only `ProofExecutableBinding`. Six production authority
obligations remain open:

1. authenticated compiler execution provenance;
2. Rust/semantic-MIR-to-Kernel-IR refinement;
3. Rust and Kernel IR IEEE-754 binary32 agreement;
4. Kernel-IR-to-final-gfx942-machine refinement;
5. Rust type/layout-to-kernarg ABI agreement; and
6. one Rust-to-KIR-to-machine memory-effect contract.

The authoritative ordered sets are
`PRODUCTION_SCALAR_GEMM_WORKER_V3_CLOSED_OBLIGATIONS_V1` and
`PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1`. The Worker V3 authority
gate remains unreachable while any open obligation exists.

## Joined flow

`ProductionScalarGemmWorkerV3VerifierV1` owns both protected executors: the
retained Verus runtime closure and the authenticated upstream-LLVM machine
worker. One audit performs these transitions in order:

1. Revalidate the borrowed Worker V3 request, current publication, semantic
   capsule, compiler receipts, target, COV6 descriptor, and exact final HSACO.
2. Analyze those in-memory HSACO bytes with a caller-pinned worker policy. The
   policy fixes the worker executable, runtime closure, analyzer identity, and
   upstream-LLVM toolchain identity. Candidate inspection is deployment tooling,
   not production trust on first use.
3. Validate the reviewed scalar profile: finalized artifact SHA-256 and length,
   logical and raw descriptors, singleton entry range, and all 19 physical
   memory-effect sites.
4. Generate one Verus source containing the Worker V3 challenge, lineage,
   generated host contract, compiler/KIR identities, exact executable profile,
   machine execution challenge, worker/runtime identities, and lossless
   canonical machine request, evidence, and receipt bytes.
5. Execute that source through the retained Verus runtime and require the exact
   pinned result. The generated theorem fixes the final artifact, descriptors,
   entry range, and effect offsets, kinds, and widths.
6. Rejoin every retained identity and canonical byte sequence. The resulting
   audit owns both the authenticated machine execution and retained Verus proof;
   callers cannot split them into independent authority inputs.

Semantic preparation is deliberately non-executable. The only production proof
builder consumes both `PreparedScalarGemmWorkerV3ProofV3` and
`ScalarGemmWorkerV3ExecutableBindingV1`. There is no semantic-only executable
builder and no V4 verifier route.

This is identity and reviewed-profile binding, not a proof that Kernel IR
semantically refines to machine code. Static physical effect sites are not by
themselves memory-safety, race-freedom, dynamic execution-count, or compiler
correctness evidence.

## Fixed profile

The bounded profile admits only:

- target `gfx942:xnack-` and AMDHSA code-object version 6;
- entry `scalar_gemm_v1`;
- finalized HSACO SHA-256
  `f415c040606b56cdbc1467ab34b7d2da7d99b57b9997fef9e4200ac03b365a75`;
- finalized length `10008` bytes;
- code range `0x1b00..0x25b0`; and
- the exact 19-site machine-effect profile encoded by
  `SCALAR_GEMM_WORKER_V3_MACHINE_EFFECTS_V1`.

No COMGR API or shell GPU linker participates. Machine analysis and code-object
production use the pinned upstream LLVM implementation.

## Qualification

CPU-only and direct pinned-Verus checks:

```bash
cargo test -p fe2o3-verifier --lib --locked scalar_gemm_worker_v3

FE2O3_TEST_VERUS=/path/to/pinned/verus \
  cargo test -p fe2o3-verifier --lib --locked \
  generated_request_bound_source_verifies_with_pinned_verus \
  -- --ignored --nocapture

FE2O3_TEST_VERUS=/path/to/pinned/verus \
  cargo test -p fe2o3-verifier --lib --locked \
  executable_profile_substitutions_fail_verus \
  -- --ignored --nocapture
```

The positive proof must report exactly `94 verified, 0 errors`. The negative
test independently substitutes the final HSACO, raw descriptor, effect offset,
effect kind, and effect width and requires every source to fail verification.

The complete retained-runtime audit additionally requires the reviewed,
root-owned runtime closure and native machine worker:

```bash
FE2O3_GFX942_SCALAR_GEMM_V3_RAW_HSACO=/path/to/raw-gfx942.hsaco \
FE2O3_GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT=/opt/fe2o3/verus-runtime-v2/VERSION \
FE2O3_MACHINE_EFFECT_NATIVE_WORKER=/path/to/fe2o3-llvm-link-worker \
  cargo test -p cargo-fe2o3 --locked \
  --features worker-v3-envelope-integration-test-only \
  --test worker_v3_load_envelope_vertical \
  production_verifier_audits_exact_proof_and_preserves_admission_custody \
  -- --ignored --exact --nocapture --test-threads=1
```

The runtime root must be provisioned and audited with
`scripts/general-gemm-verus-runtime-v2.sh`; a user-owned lookalike is rejected.
