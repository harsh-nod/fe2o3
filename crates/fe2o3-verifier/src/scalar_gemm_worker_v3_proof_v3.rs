//! Request-bound generated Verus input for the strict scalar GEMM Worker V3 path.
//!
//! The generated source embeds the exact Worker V3 challenge and compiler/KIR identities, then
//! includes the reviewed scalar source model, an exact decoded-KIR projection, and exact-profile
//! KIR integer equations. Its retained execution closes request-to-transcript and projection
//! identity binding, but decoded-KIR operational refinement, Rust/MIR-to-model, IEEE `f32`, and
//! emitted-machine refinement remain explicit obligations.

use std::fmt::Write as _;
use std::time::{Duration, Instant};
use std::{error::Error, fmt};

use fe2o3_compiler_lineage::InertLineageContentIdentityV3;
use fe2o3_kernel_ir::ScalarGemmSemanticProjectionErrorV1;
use sha2::{Digest as _, Sha256};

use crate::general_gemm_runtime_closure_v2::{
    GeneralGemmRuntimeClosureErrorKindV2, GeneralGemmRuntimeClosureErrorV2,
    GeneralGemmRuntimeProcessOutputV2, GeneralGemmVerusRuntimeClosureLeaseV2,
};
use crate::{
    CanonicalGeneratedVerusProofInputV3, Digest, GeneratedVerusProofInputErrorV3,
    GeneratedVerusProofInputIdentityV3, MAX_SCALAR_GEMM_VERUS_OUTPUT_BYTES_V2,
    MAX_SCALAR_GEMM_VERUS_TIMEOUT_SECONDS_V2, ValidatedCompilerProofBindingAssociationV3,
    ValidatedScalarGemmCompilerKirV3,
};

const SOURCE_MODEL: &[u8] = include_bytes!("../verus/scalar_gemm_v1.rs");
const KIR_INTEGER_REFINEMENT: &[u8] =
    include_bytes!("../verus/scalar_gemm_kir_integer_refinement_v1.rs");
const KIR_PROJECTION_REVIEW: &[u8] =
    include_bytes!("../verus/scalar_gemm_kir_projection_review_v1.rs");
const EXPECTED_STDOUT: &[u8] = b"verification results:: 26 verified, 0 errors\n";
const INPUT_BINDING_DOMAIN_V3: &[u8] = b"fe2o3-scalar-gemm-worker-v3-proof-input-binding-v3\0";
const OUTPUT_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3-scalar-gemm-worker-v3-proof-output-v3\0";
const EXECUTION_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3-scalar-gemm-worker-v3-proof-execution-v3\0";

#[derive(Clone, Copy, Debug)]
struct ScalarGemmKirProofBindingV3 {
    canonical_kir_identity: [u8; 32],
    semantic_projection_identity: [u8; 32],
    semantic_projection_byte_len: u64,
}

/// Exact generated source and identity bindings for one scalar Worker V3 request.
#[derive(Debug)]
#[must_use = "generated Worker V3 proof input must be executed by the retained verifier"]
pub struct ScalarGemmWorkerV3ProofInputV3 {
    challenge: [u8; 32],
    compiler_inputs: [InertLineageContentIdentityV3; 5],
    proof_binding_sha256: [u8; 32],
    proof_binding_byte_len: u64,
    canonical_kir_identity: [u8; 32],
    semantic_projection_identity: [u8; 32],
    semantic_projection_byte_len: u64,
    binding_identity: Digest,
    source: CanonicalGeneratedVerusProofInputV3,
}

impl ScalarGemmWorkerV3ProofInputV3 {
    pub const fn challenge(&self) -> [u8; 32] {
        self.challenge
    }

    pub const fn compiler_inputs(&self) -> &[InertLineageContentIdentityV3; 5] {
        &self.compiler_inputs
    }

    pub const fn proof_binding_sha256(&self) -> [u8; 32] {
        self.proof_binding_sha256
    }

    pub const fn proof_binding_byte_len(&self) -> u64 {
        self.proof_binding_byte_len
    }

    pub const fn canonical_kir_identity(&self) -> [u8; 32] {
        self.canonical_kir_identity
    }

    pub const fn semantic_projection_identity(&self) -> [u8; 32] {
        self.semantic_projection_identity
    }

    pub const fn semantic_projection_byte_len(&self) -> u64 {
        self.semantic_projection_byte_len
    }

    pub const fn binding_identity(&self) -> Digest {
        self.binding_identity
    }

    pub const fn source_identity(&self) -> GeneratedVerusProofInputIdentityV3 {
        self.source.identity()
    }

    pub fn canonical_source(&self) -> &[u8] {
        self.source.source()
    }

    /// The generated source commits to the exact request challenge.
    pub const fn binds_worker_v3_challenge(&self) -> bool {
        true
    }

    /// Input generation does not authenticate Verus execution.
    pub const fn authenticates_verus_execution(&self) -> bool {
        false
    }

    /// Identity embedding alone is not source-model-to-KIR refinement.
    pub const fn establishes_source_to_kir_refinement(&self) -> bool {
        false
    }

    /// The generated source includes reviewed profile equations for the KIR integer model.
    pub const fn includes_reviewed_kir_integer_profile_equations(&self) -> bool {
        true
    }

    /// The generated source carries every byte of the checked decoded-KIR projection.
    pub const fn binds_exhaustive_decoded_kir_projection(&self) -> bool {
        true
    }

    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Builds one canonical request-bound scalar proof harness.
pub fn build_scalar_gemm_worker_v3_proof_input_v3(
    challenge: [u8; 32],
    association: &ValidatedCompilerProofBindingAssociationV3,
    scalar_kir: &ValidatedScalarGemmCompilerKirV3,
) -> Result<ScalarGemmWorkerV3ProofInputV3, ScalarGemmWorkerV3ProofInputErrorV3> {
    if challenge == [0; 32] {
        return Err(ScalarGemmWorkerV3ProofInputErrorV3::ZeroChallenge);
    }
    if scalar_kir.proof_binding_receipt_identity() != association.receipt_identity() {
        return Err(ScalarGemmWorkerV3ProofInputErrorV3::AssociationSubstitution);
    }
    let inputs = association.association().inputs();
    let compiler_inputs = [
        inputs.semantic_mir(),
        inputs.middle_end(),
        inputs.kernel_ir(),
        inputs.mir_to_kir_correspondence(),
        inputs.formal_memory(),
    ];
    let scalar_kir_receipt = scalar_kir.kernel_ir_receipt_identity();
    if compiler_inputs[2].sha256() != *scalar_kir_receipt.sha256()
        || compiler_inputs[2].byte_len() != scalar_kir_receipt.byte_len()
    {
        return Err(ScalarGemmWorkerV3ProofInputErrorV3::AssociationSubstitution);
    }
    let proof_binding = association.receipt_identity();
    let proof_binding_sha256 = *proof_binding.sha256();
    let proof_binding_byte_len = proof_binding.byte_len();
    let canonical_kir_identity = scalar_kir.canonical_kir_identity();
    let semantic_projection = scalar_kir.semantic_projection();
    semantic_projection
        .revalidate()
        .map_err(ScalarGemmWorkerV3ProofInputErrorV3::SemanticProjection)?;
    if semantic_projection.source_kir_identity().digest() != &canonical_kir_identity {
        return Err(ScalarGemmWorkerV3ProofInputErrorV3::AssociationSubstitution);
    }
    let semantic_projection_identity = *semantic_projection.identity().digest();
    let semantic_projection_byte_len =
        u64::try_from(semantic_projection.canonical_token_preimage().len())
            .map_err(|_| ScalarGemmWorkerV3ProofInputErrorV3::Formatting)?;
    let kir_binding = ScalarGemmKirProofBindingV3 {
        canonical_kir_identity,
        semantic_projection_identity,
        semantic_projection_byte_len,
    };
    let generated = generate_source(
        challenge,
        compiler_inputs,
        proof_binding_sha256,
        proof_binding_byte_len,
        canonical_kir_identity,
        semantic_projection_identity,
        semantic_projection.canonical_token_preimage(),
    )?;
    let source = CanonicalGeneratedVerusProofInputV3::new(generated)
        .map_err(ScalarGemmWorkerV3ProofInputErrorV3::GeneratedSource)?;
    let binding_identity = input_binding_identity(
        challenge,
        &compiler_inputs,
        proof_binding_sha256,
        proof_binding_byte_len,
        kir_binding,
        source.identity(),
    );
    Ok(ScalarGemmWorkerV3ProofInputV3 {
        challenge,
        compiler_inputs,
        proof_binding_sha256,
        proof_binding_byte_len,
        canonical_kir_identity,
        semantic_projection_identity,
        semantic_projection_byte_len,
        binding_identity,
        source,
    })
}

fn generate_source(
    challenge: [u8; 32],
    compiler_inputs: [InertLineageContentIdentityV3; 5],
    proof_binding_sha256: [u8; 32],
    proof_binding_byte_len: u64,
    canonical_kir_identity: [u8; 32],
    semantic_projection_identity: [u8; 32],
    semantic_projection: &[u8],
) -> Result<Vec<u8>, ScalarGemmWorkerV3ProofInputErrorV3> {
    let mut generated = String::with_capacity(
        SOURCE_MODEL.len()
            + KIR_PROJECTION_REVIEW.len()
            + KIR_INTEGER_REFINEMENT.len()
            + semantic_projection.len() * 6
            + 4096,
    );
    generated.push_str("// @generated by fe2o3 scalar Worker V3 proof input V3\n");
    generated.push_str(
        "pub const FE2O3_PROOF_INPUT_SCHEMA_V3: &str = \"fe2o3.scalar-gemm.worker-v3-proof-input.v3\";\n",
    );
    push_digest(&mut generated, "FE2O3_WORKER_V3_CHALLENGE_V3", challenge)?;
    for (name, identity) in [
        ("SEMANTIC_MIR", compiler_inputs[0]),
        ("MIDDLE_END", compiler_inputs[1]),
        ("KERNEL_IR", compiler_inputs[2]),
        ("MIR_TO_KIR_CORRESPONDENCE", compiler_inputs[3]),
        ("FORMAL_MEMORY", compiler_inputs[4]),
    ] {
        push_digest(
            &mut generated,
            &format!("FE2O3_{name}_SHA256_V3"),
            identity.sha256(),
        )?;
        writeln!(
            generated,
            "pub const FE2O3_{name}_BYTE_LEN_V3: u64 = {};",
            identity.byte_len()
        )
        .map_err(|_| ScalarGemmWorkerV3ProofInputErrorV3::Formatting)?;
    }
    push_digest(
        &mut generated,
        "FE2O3_PROOF_BINDING_SHA256_V3",
        proof_binding_sha256,
    )?;
    writeln!(
        generated,
        "pub const FE2O3_PROOF_BINDING_BYTE_LEN_V3: u64 = {proof_binding_byte_len};"
    )
    .map_err(|_| ScalarGemmWorkerV3ProofInputErrorV3::Formatting)?;
    push_digest(
        &mut generated,
        "FE2O3_CANONICAL_KIR_IDENTITY_V5",
        canonical_kir_identity,
    )?;
    push_digest(
        &mut generated,
        "FE2O3_SCALAR_KIR_SEMANTIC_PROJECTION_IDENTITY_V1",
        semantic_projection_identity,
    )?;
    writeln!(
        generated,
        "pub const FE2O3_SCALAR_KIR_SEMANTIC_PROJECTION_BYTE_LEN_V1: u64 = {};",
        semantic_projection.len()
    )
    .map_err(|_| ScalarGemmWorkerV3ProofInputErrorV3::Formatting)?;
    generated.push('\n');
    let mut generated = generated.into_bytes();
    generated.extend_from_slice(SOURCE_MODEL);
    append_semantic_projection(&mut generated, semantic_projection)?;
    generated.extend_from_slice(KIR_PROJECTION_REVIEW);
    generated.extend_from_slice(KIR_INTEGER_REFINEMENT);
    Ok(generated)
}

fn append_semantic_projection(
    output: &mut Vec<u8>,
    projection: &[u8],
) -> Result<(), ScalarGemmWorkerV3ProofInputErrorV3> {
    let mut source = String::with_capacity(projection.len() * 6 + 256);
    source.push_str("\npub mod scalar_gemm_kir_projection_generated_v1 {\n\n");
    source.push_str("use vstd::prelude::*;\n\n");
    writeln!(source, "verus! {{\n").map_err(|_| ScalarGemmWorkerV3ProofInputErrorV3::Formatting)?;
    writeln!(
        source,
        "pub const FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1: [u8; {}] = [",
        projection.len()
    )
    .map_err(|_| ScalarGemmWorkerV3ProofInputErrorV3::Formatting)?;
    for chunk in projection.chunks(24) {
        source.push_str("    ");
        for (index, byte) in chunk.iter().enumerate() {
            if index != 0 {
                source.push_str(", ");
            }
            write!(source, "{byte}")
                .map_err(|_| ScalarGemmWorkerV3ProofInputErrorV3::Formatting)?;
        }
        source.push_str(",\n");
    }
    source.push_str("];\n\n} // verus!\n\n");
    source.push_str("} // mod scalar_gemm_kir_projection_generated_v1\n");
    output.extend_from_slice(source.as_bytes());
    Ok(())
}

fn push_digest(
    output: &mut String,
    name: &str,
    digest: [u8; 32],
) -> Result<(), ScalarGemmWorkerV3ProofInputErrorV3> {
    write!(output, "pub const {name}: [u8; 32] = [")
        .map_err(|_| ScalarGemmWorkerV3ProofInputErrorV3::Formatting)?;
    for (index, byte) in digest.into_iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        write!(output, "0x{byte:02x}")
            .map_err(|_| ScalarGemmWorkerV3ProofInputErrorV3::Formatting)?;
    }
    output.push_str("];\n");
    Ok(())
}

fn input_binding_identity(
    challenge: [u8; 32],
    compiler_inputs: &[InertLineageContentIdentityV3; 5],
    proof_binding_sha256: [u8; 32],
    proof_binding_byte_len: u64,
    kir_binding: ScalarGemmKirProofBindingV3,
    source_identity: GeneratedVerusProofInputIdentityV3,
) -> Digest {
    let mut digest = Sha256::new();
    digest.update(INPUT_BINDING_DOMAIN_V3);
    digest.update(challenge);
    for identity in compiler_inputs {
        digest.update(identity.sha256());
        digest.update(identity.byte_len().to_le_bytes());
    }
    digest.update(proof_binding_sha256);
    digest.update(proof_binding_byte_len.to_le_bytes());
    digest.update(kir_binding.canonical_kir_identity);
    digest.update(kir_binding.semantic_projection_identity);
    digest.update(kir_binding.semantic_projection_byte_len.to_le_bytes());
    digest.update(source_identity.as_bytes());
    Digest::from_bytes(digest.finalize().into())
}

/// Linear receipt from exact retained execution of one request-bound scalar proof input.
#[derive(Debug)]
#[must_use = "request-bound Verus evidence still requires source-to-machine refinement"]
pub struct AuthenticatedScalarGemmWorkerV3ProofV3 {
    input: ScalarGemmWorkerV3ProofInputV3,
    runtime_closure_identity: Digest,
    output_identity: Digest,
    identity: Digest,
}

impl AuthenticatedScalarGemmWorkerV3ProofV3 {
    pub const fn input(&self) -> &ScalarGemmWorkerV3ProofInputV3 {
        &self.input
    }

    pub const fn runtime_closure_identity(&self) -> Digest {
        self.runtime_closure_identity
    }

    pub const fn output_identity(&self) -> Digest {
        self.output_identity
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub const fn authenticates_retained_verus_execution(&self) -> bool {
        true
    }

    pub const fn binds_worker_v3_challenge(&self) -> bool {
        true
    }

    pub const fn establishes_exact_scalar_gemm_kir_profile(&self) -> bool {
        true
    }

    /// Retained Verus checked equality of the generated and reviewed exhaustive projections.
    pub const fn authenticates_exact_decoded_kir_projection(&self) -> bool {
        true
    }

    /// The decoded KIR graph is not yet projected into a formal operational semantics.
    pub const fn establishes_kir_to_integer_model_refinement(&self) -> bool {
        false
    }

    /// The integer model is not a proof that ordinary Rust/MIR produced the retained KIR.
    pub const fn establishes_source_to_kir_refinement(&self) -> bool {
        false
    }

    pub const fn establishes_rust_or_f32_semantics(&self) -> bool {
        false
    }

    pub const fn establishes_emitted_machine_refinement(&self) -> bool {
        false
    }

    pub const fn can_enter_worker_v3_gate(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Executes one request-bound generated scalar proof through the retained runtime closure.
pub fn execute_scalar_gemm_worker_v3_proof_v3(
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
    input: ScalarGemmWorkerV3ProofInputV3,
    timeout_seconds: u32,
) -> Result<AuthenticatedScalarGemmWorkerV3ProofV3, ScalarGemmWorkerV3ProofErrorV3> {
    if timeout_seconds == 0 || timeout_seconds > MAX_SCALAR_GEMM_VERUS_TIMEOUT_SECONDS_V2 {
        return Err(ScalarGemmWorkerV3ProofErrorV3::new(
            ScalarGemmWorkerV3ProofErrorKindV3::InvalidTimeout,
        ));
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(u64::from(timeout_seconds)))
        .ok_or_else(|| {
            ScalarGemmWorkerV3ProofErrorV3::new(ScalarGemmWorkerV3ProofErrorKindV3::InvalidTimeout)
        })?;
    let observed = runtime
        .execute_generated_rust_verify(
            &input.source,
            deadline,
            MAX_SCALAR_GEMM_VERUS_OUTPUT_BYTES_V2,
        )
        .map_err(ScalarGemmWorkerV3ProofErrorV3::runtime)?;
    validate_exact_output(&observed)?;
    if Instant::now() >= deadline {
        return Err(ScalarGemmWorkerV3ProofErrorV3::new(
            ScalarGemmWorkerV3ProofErrorKindV3::TimedOut,
        ));
    }
    let runtime_closure_identity = Digest::from_bytes(runtime.identity().as_bytes());
    let output_identity = output_identity(&observed);
    let identity = execution_identity(&input, runtime_closure_identity, output_identity);
    Ok(AuthenticatedScalarGemmWorkerV3ProofV3 {
        input,
        runtime_closure_identity,
        output_identity,
        identity,
    })
}

fn validate_exact_output(
    observed: &GeneralGemmRuntimeProcessOutputV2,
) -> Result<(), ScalarGemmWorkerV3ProofErrorV3> {
    if observed.exit_code != Some(0)
        || observed.signal.is_some()
        || observed.stdout != EXPECTED_STDOUT
        || !observed.stderr.is_empty()
    {
        return Err(ScalarGemmWorkerV3ProofErrorV3::new(
            ScalarGemmWorkerV3ProofErrorKindV3::UnexpectedProofResult,
        ));
    }
    Ok(())
}

fn output_identity(observed: &GeneralGemmRuntimeProcessOutputV2) -> Digest {
    let mut digest = Sha256::new();
    digest.update(OUTPUT_IDENTITY_DOMAIN_V3);
    digest.update(observed.exit_code.unwrap_or(-1).to_le_bytes());
    digest.update(observed.signal.unwrap_or(0).to_le_bytes());
    put_blob(&mut digest, &observed.stdout);
    put_blob(&mut digest, &observed.stderr);
    Digest::from_bytes(digest.finalize().into())
}

fn execution_identity(
    input: &ScalarGemmWorkerV3ProofInputV3,
    runtime: Digest,
    output: Digest,
) -> Digest {
    let mut digest = Sha256::new();
    digest.update(EXECUTION_IDENTITY_DOMAIN_V3);
    digest.update(input.binding_identity.as_bytes());
    digest.update(input.source.identity().as_bytes());
    digest.update(runtime.as_bytes());
    digest.update(output.as_bytes());
    Digest::from_bytes(digest.finalize().into())
}

fn put_blob(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

/// Request-bound scalar proof-input generation failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum ScalarGemmWorkerV3ProofInputErrorV3 {
    ZeroChallenge,
    AssociationSubstitution,
    Formatting,
    SemanticProjection(ScalarGemmSemanticProjectionErrorV1),
    GeneratedSource(GeneratedVerusProofInputErrorV3),
}

impl fmt::Display for ScalarGemmWorkerV3ProofInputErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "scalar Worker V3 proof input rejected: {self:?}")
    }
}

impl Error for ScalarGemmWorkerV3ProofInputErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticProjection(error) => Some(error),
            Self::GeneratedSource(error) => Some(error),
            _ => None,
        }
    }
}

/// Stable request-bound proof execution failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarGemmWorkerV3ProofErrorKindV3 {
    InvalidTimeout,
    TimedOut,
    OutputTooLarge,
    UnexpectedProofResult,
    RuntimeClosure,
}

/// Failure to execute one request-bound generated scalar proof.
#[derive(Debug)]
pub struct ScalarGemmWorkerV3ProofErrorV3 {
    kind: ScalarGemmWorkerV3ProofErrorKindV3,
    runtime: Option<GeneralGemmRuntimeClosureErrorV2>,
}

impl ScalarGemmWorkerV3ProofErrorV3 {
    pub const fn kind(&self) -> ScalarGemmWorkerV3ProofErrorKindV3 {
        self.kind
    }

    fn new(kind: ScalarGemmWorkerV3ProofErrorKindV3) -> Self {
        Self {
            kind,
            runtime: None,
        }
    }

    fn runtime(error: GeneralGemmRuntimeClosureErrorV2) -> Self {
        let kind = match error.kind() {
            GeneralGemmRuntimeClosureErrorKindV2::TimedOut => {
                ScalarGemmWorkerV3ProofErrorKindV3::TimedOut
            }
            GeneralGemmRuntimeClosureErrorKindV2::OutputTooLarge => {
                ScalarGemmWorkerV3ProofErrorKindV3::OutputTooLarge
            }
            _ => ScalarGemmWorkerV3ProofErrorKindV3::RuntimeClosure,
        };
        Self {
            kind,
            runtime: Some(error),
        }
    }
}

impl fmt::Display for ScalarGemmWorkerV3ProofErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "scalar Worker V3 retained proof execution failed: {:?}",
            self.kind
        )?;
        if let Some(runtime) = &self.runtime {
            write!(formatter, ": {runtime}")?;
        }
        Ok(())
    }
}

impl Error for ScalarGemmWorkerV3ProofErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.runtime
            .as_ref()
            .map(|error| error as &(dyn Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::{Command, Output};

    use fe2o3_compiler_lineage::{
        InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
        InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
        InertProofBindingAssociationInputsV3, InertProofBindingAssociationV3,
        InertProofBindingReceiptV3,
    };
    use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrV5, scalar_gemm_v1_module};

    use super::*;
    use crate::{
        validate_compiler_proof_binding_association_v3, validate_scalar_gemm_compiler_kir_v3,
    };

    fn content(sha256: &[u8; 32], byte_len: u64) -> InertLineageContentIdentityV3 {
        InertLineageContentIdentityV3::new(*sha256, byte_len).unwrap()
    }

    fn validated(
        seed: u8,
    ) -> (
        ValidatedCompilerProofBindingAssociationV3,
        ValidatedScalarGemmCompilerKirV3,
    ) {
        let semantic_mir =
            InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(vec![b's', seed]).unwrap();
        let middle_end =
            InertMiddleEndReceiptV3::from_canonical_preimage(vec![b'm', seed]).unwrap();
        let kernel_ir = InertKernelIrReceiptV3::from_canonical_preimage(
            VerifiedCanonicalKernelIrV5::from_module(scalar_gemm_v1_module())
                .unwrap()
                .into_canonical_bytes(),
        )
        .unwrap();
        let correspondence =
            InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(vec![b'c', seed])
                .unwrap();
        let formal_memory =
            InertFormalMemoryReceiptV3::from_canonical_preimage(vec![b'f', seed]).unwrap();
        let association =
            InertProofBindingAssociationV3::new(InertProofBindingAssociationInputsV3::new(
                content(
                    semantic_mir.identity().sha256(),
                    semantic_mir.identity().byte_len(),
                ),
                content(
                    middle_end.identity().sha256(),
                    middle_end.identity().byte_len(),
                ),
                content(
                    kernel_ir.identity().sha256(),
                    kernel_ir.identity().byte_len(),
                ),
                content(
                    correspondence.identity().sha256(),
                    correspondence.identity().byte_len(),
                ),
                content(
                    formal_memory.identity().sha256(),
                    formal_memory.identity().byte_len(),
                ),
            ))
            .unwrap();
        let proof_binding = InertProofBindingReceiptV3::from_canonical_preimage(
            association.canonical_bytes().to_vec(),
        )
        .unwrap();
        let association = validate_compiler_proof_binding_association_v3(
            &proof_binding,
            &semantic_mir,
            &middle_end,
            &kernel_ir,
            &correspondence,
            &formal_memory,
        )
        .unwrap();
        let scalar_kir = validate_scalar_gemm_compiler_kir_v3(&association, &kernel_ir).unwrap();
        (association, scalar_kir)
    }

    fn input(challenge: [u8; 32]) -> ScalarGemmWorkerV3ProofInputV3 {
        let (association, scalar_kir) = validated(1);
        build_scalar_gemm_worker_v3_proof_input_v3(challenge, &association, &scalar_kir).unwrap()
    }

    fn output(
        exit_code: Option<i32>,
        signal: Option<i32>,
        stdout: &[u8],
        stderr: &[u8],
    ) -> GeneralGemmRuntimeProcessOutputV2 {
        GeneralGemmRuntimeProcessOutputV2 {
            exit_code,
            signal,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        }
    }

    fn run_pinned_verus(source: &[u8], suffix: &str) -> Output {
        let verus = std::env::var_os("FE2O3_TEST_VERUS").expect("FE2O3_TEST_VERUS is not set");
        let path = std::env::temp_dir().join(format!(
            "fe2o3-scalar-worker-v3-proof-{}-{suffix}.rs",
            std::process::id()
        ));
        fs::write(&path, source).unwrap();
        let output = Command::new(verus)
            .arg(&path)
            .args([
                "--crate-type",
                "lib",
                "--triggers-mode",
                "silent",
                "--no-cheating",
                "--num-threads",
                "1",
            ])
            .output()
            .unwrap();
        fs::remove_file(path).unwrap();
        output
    }

    fn substitute_first_generated_projection_byte(source: &mut [u8]) {
        const MARKER: &[u8] = b"FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1";
        let marker = source
            .windows(MARKER.len())
            .position(|window| window == MARKER)
            .expect("generated projection marker is absent");
        let first_byte = source[marker..]
            .windows(b"= [\n    1,".len())
            .position(|window| window == b"= [\n    1,")
            .map(|offset| marker + offset + b"= [\n    ".len())
            .expect("generated projection first byte is absent");
        assert_eq!(source[first_byte], b'1');
        source[first_byte] = b'0';
    }

    #[test]
    fn generated_source_binds_challenge_and_every_compiler_axis() {
        let first = input([0x11; 32]);
        assert!(first.binds_worker_v3_challenge());
        assert!(!first.authenticates_verus_execution());
        assert!(!first.establishes_source_to_kir_refinement());
        assert!(first.includes_reviewed_kir_integer_profile_equations());
        assert!(first.binds_exhaustive_decoded_kir_projection());
        assert_eq!(first.semantic_projection_byte_len(), 2_927);
        assert_ne!(first.semantic_projection_identity(), [0; 32]);
        assert!(!first.grants_artifact_or_runtime_authority());
        let source = std::str::from_utf8(first.canonical_source()).unwrap();
        for name in [
            "FE2O3_WORKER_V3_CHALLENGE_V3",
            "FE2O3_SEMANTIC_MIR_SHA256_V3",
            "FE2O3_MIDDLE_END_SHA256_V3",
            "FE2O3_KERNEL_IR_SHA256_V3",
            "FE2O3_MIR_TO_KIR_CORRESPONDENCE_SHA256_V3",
            "FE2O3_FORMAL_MEMORY_SHA256_V3",
            "FE2O3_PROOF_BINDING_SHA256_V3",
            "FE2O3_CANONICAL_KIR_IDENTITY_V5",
            "FE2O3_SCALAR_KIR_SEMANTIC_PROJECTION_IDENTITY_V1",
            "FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1",
            "FE2O3_REVIEWED_SCALAR_KIR_PROJECTION_V1",
        ] {
            assert!(source.contains(name), "generated source omitted {name}");
        }
        assert!(source.contains(std::str::from_utf8(SOURCE_MODEL).unwrap()));
        assert!(source.ends_with(std::str::from_utf8(KIR_INTEGER_REFINEMENT).unwrap()));
        let challenge_substitution = input([0x12; 32]);
        assert_ne!(
            first.source_identity(),
            challenge_substitution.source_identity()
        );
        assert_ne!(
            first.binding_identity(),
            challenge_substitution.binding_identity()
        );

        let (other_association, other_scalar) = validated(2);
        let compiler_substitution = build_scalar_gemm_worker_v3_proof_input_v3(
            [0x11; 32],
            &other_association,
            &other_scalar,
        )
        .unwrap();
        for index in [0, 1, 3, 4] {
            assert_ne!(
                first.compiler_inputs()[index],
                compiler_substitution.compiler_inputs()[index]
            );
        }
        assert_eq!(
            first.compiler_inputs()[2],
            compiler_substitution.compiler_inputs()[2]
        );
        assert_ne!(
            first.source_identity(),
            compiler_substitution.source_identity()
        );
        assert_ne!(
            first.binding_identity(),
            compiler_substitution.binding_identity()
        );
    }

    #[test]
    fn zero_challenge_and_mixed_associations_fail_closed() {
        let (first_association, first_scalar) = validated(1);
        assert!(matches!(
            build_scalar_gemm_worker_v3_proof_input_v3([0; 32], &first_association, &first_scalar),
            Err(ScalarGemmWorkerV3ProofInputErrorV3::ZeroChallenge)
        ));
        let (second_association, _) = validated(2);
        assert!(matches!(
            build_scalar_gemm_worker_v3_proof_input_v3([1; 32], &second_association, &first_scalar),
            Err(ScalarGemmWorkerV3ProofInputErrorV3::AssociationSubstitution)
        ));
    }

    #[test]
    fn exact_process_result_is_required_and_identity_bound() {
        let exact = output(Some(0), None, EXPECTED_STDOUT, b"");
        validate_exact_output(&exact).unwrap();
        let exact_identity = output_identity(&exact);
        for substituted in [
            output(Some(1), None, EXPECTED_STDOUT, b""),
            output(None, None, EXPECTED_STDOUT, b""),
            output(Some(0), Some(9), EXPECTED_STDOUT, b""),
            output(
                Some(0),
                None,
                b"verification results:: 14 verified, 0 errors\n",
                b"",
            ),
            output(Some(0), None, EXPECTED_STDOUT, b"warning\n"),
        ] {
            assert_eq!(
                validate_exact_output(&substituted).unwrap_err().kind(),
                ScalarGemmWorkerV3ProofErrorKindV3::UnexpectedProofResult
            );
            assert_ne!(output_identity(&substituted), exact_identity);
        }
    }

    #[test]
    fn request_bound_execution_receipt_does_not_overclaim_refinement() {
        let receipt = AuthenticatedScalarGemmWorkerV3ProofV3 {
            input: input([0x33; 32]),
            runtime_closure_identity: Digest::from_bytes([1; 32]),
            output_identity: Digest::from_bytes([2; 32]),
            identity: Digest::from_bytes([3; 32]),
        };
        assert!(receipt.authenticates_retained_verus_execution());
        assert!(receipt.binds_worker_v3_challenge());
        assert!(receipt.establishes_exact_scalar_gemm_kir_profile());
        assert!(receipt.authenticates_exact_decoded_kir_projection());
        assert!(!receipt.establishes_kir_to_integer_model_refinement());
        assert!(!receipt.establishes_source_to_kir_refinement());
        assert!(!receipt.establishes_rust_or_f32_semantics());
        assert!(!receipt.establishes_emitted_machine_refinement());
        assert!(!receipt.can_enter_worker_v3_gate());
        assert!(!receipt.grants_artifact_or_runtime_authority());
    }

    #[test]
    #[ignore = "requires FE2O3_TEST_VERUS pointing to the pinned Verus launcher"]
    fn generated_request_bound_source_verifies_with_pinned_verus() {
        let source = input([0x44; 32]);
        let output = run_pinned_verus(source.canonical_source(), "positive");
        assert_eq!(
            output.status.code(),
            Some(0),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, EXPECTED_STDOUT);
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    #[ignore = "requires FE2O3_TEST_VERUS pointing to the pinned Verus launcher"]
    fn one_byte_projection_substitution_fails_verus() {
        let mut source = input([0x45; 32]).canonical_source().to_vec();
        substitute_first_generated_projection_byte(&mut source);
        let output = run_pinned_verus(&source, "projection-substitution");
        assert_ne!(
            output.status.code(),
            Some(0),
            "mutated projection unexpectedly verified: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("generated_scalar_kir_projection_is_exact_v1")
                || stderr.contains("assertion failed"),
            "unexpected verifier failure:\n{stderr}"
        );
    }
}
