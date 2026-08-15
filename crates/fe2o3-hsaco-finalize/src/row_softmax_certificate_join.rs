//! Exact certificate, compiler, Worker V2, and finalizer join for row-softmax V1.

use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::CodeObjectVersion;
use fe2o3_verifier::{
    AuthenticatedRowSoftmaxVerificationCertificateIdentityV1,
    AuthenticatedRowSoftmaxVerificationCertificateV1,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, FinalizedRowSoftmaxV1StructuralHsacoV1, FinalizedWorkerV2HsacoIdentityV1,
    InspectedRowSoftmaxV1DirectWorkerHsacoV1, RowSoftmaxV1DirectWorkerExchangeIdentityV1,
    RowSoftmaxV1StructuralArtifactErrorV1, ValidatedRowSoftmaxV1DirectWorkerExchangeV1,
    finalize_row_softmax_v1_structural_worker_v2_hsaco_v1,
};

const ADMISSION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/ROW-SOFTMAX/PROTECTED-ARTIFACT-ADMISSION/V1\0";
const TARGET: &str = "gfx942:xnack-";
const COMPILER_PROFILE: &str = "fe2o3.manifest-derived-scalar-slice.v1";
const KERNEL_IR_PROFILE: &str = "fe2o3::row_softmax_v1;fixed-row-64;wg64;cov6";
const OCML_EXP_F32: &str = "__ocml_exp_f32";
const SOURCE_SHA256: &str = "9551d13970d1e6d577a6b058eb3ef9b389a2bb20544e6977291379b3f68b866c";
const PORTABLE_MIR_SHA256: &str =
    "cb10b6fac6475435e45a6f9166739c9e26bae17031105791abf3f440b004d4dd";
const COMPILER_SEMANTICS_SHA256: &str =
    "3132d86d229a3977ed9c5283c241c4f6c85aff23c1d177fb0d23c0743279f0a4";
const NUMERICAL_POLICY_SHA256: &str =
    "367b11f440d884cc1ecafd3b88fbf209c819acae09c21177718fd720fe9b18ad";
const PROOF_SOURCE_SHA256: &str =
    "cacf81e02eb071cc29b1124811e911097fd62e7d29556dda8380418a631f5db5";
const KERNEL_IR_SHA256: &str = "1e1b14c6842ffd09103eb55eb39b1bcae9c0da81597fed6186767562337230e6";
const LLVM_BODY_SHA256: &str = "0a3313675344437bc7b894ad2f4dadb38107d90296a9665b763234acd2405acc";
const VERUS_EXECUTABLE_SHA256: &str =
    "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd";
const SOLVER_EXECUTABLE_SHA256: &str =
    "e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765";

/// Stable identity of the complete protected admission join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedRowSoftmaxV1AdmissionIdentityV1([u8; 32]);

impl ProtectedRowSoftmaxV1AdmissionIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Linear, inert result of the exact certificate-to-artifact join.
///
/// The retained finalized artifact is intentionally inaccessible. This type
/// exposes identities only and provides no bytes, handles, publication, load,
/// or launch operation.
#[must_use = "protected row-softmax admission must enter the exact host path"]
pub struct PreparedProtectedRowSoftmaxV1AdmissionV1 {
    identity: ProtectedRowSoftmaxV1AdmissionIdentityV1,
    certificate: AuthenticatedRowSoftmaxVerificationCertificateV1,
    exchange: ValidatedRowSoftmaxV1DirectWorkerExchangeV1,
    finalized: FinalizedRowSoftmaxV1StructuralHsacoV1,
}

impl fmt::Debug for PreparedProtectedRowSoftmaxV1AdmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedProtectedRowSoftmaxV1AdmissionV1")
            .field("identity", &self.identity)
            .field("certificate", &self.certificate.identity())
            .field("exchange", &self.exchange.identity())
            .field("finalized", &self.finalized.identity())
            .finish_non_exhaustive()
    }
}

impl PreparedProtectedRowSoftmaxV1AdmissionV1 {
    pub const fn identity(&self) -> ProtectedRowSoftmaxV1AdmissionIdentityV1 {
        self.identity
    }

    pub const fn certificate_identity(
        &self,
    ) -> AuthenticatedRowSoftmaxVerificationCertificateIdentityV1 {
        self.certificate.identity()
    }

    pub const fn worker_exchange_identity(&self) -> RowSoftmaxV1DirectWorkerExchangeIdentityV1 {
        self.exchange.identity()
    }

    pub const fn finalized_artifact_identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.finalized.identity()
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized.finalized_output_identity()
    }

    pub const fn target(&self) -> &'static str {
        TARGET
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        CodeObjectVersion::V6
    }

    pub const fn row_elements(&self) -> u32 {
        64
    }

    pub const fn workgroup_size(&self) -> [u32; 3] {
        [64, 1, 1]
    }

    pub const fn grid_size(&self) -> [u32; 3] {
        [1, 1, 1]
    }

    pub const fn explicit_kernarg_bytes(&self) -> u32 {
        32
    }

    pub const fn implicit_kernarg_bytes(&self) -> u32 {
        256
    }

    pub const fn total_kernarg_bytes(&self) -> u32 {
        288
    }

    pub const fn kernarg_alignment(&self) -> u32 {
        8
    }

    pub const fn static_lds_bytes(&self) -> u32 {
        0
    }

    pub const fn private_segment_bytes(&self) -> u32 {
        0
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_ocml_or_ieee_error_bound(&self) -> bool {
        false
    }

    pub const fn proves_source_to_machine_refinement(&self) -> bool {
        false
    }

    pub const fn proves_execution(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Rejection before exact host authority can exist.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedRowSoftmaxV1AdmissionErrorV1 {
    CertificateField(&'static str),
    CompilerArtifactField(&'static str),
    Finalization(RowSoftmaxV1StructuralArtifactErrorV1),
}

impl fmt::Display for ProtectedRowSoftmaxV1AdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CertificateField(field) => {
                write!(formatter, "row-softmax certificate {field} drifted")
            }
            Self::CompilerArtifactField(field) => {
                write!(formatter, "row-softmax compiler/artifact {field} drifted")
            }
            Self::Finalization(error) => {
                write!(
                    formatter,
                    "row-softmax protected finalization failed: {error}"
                )
            }
        }
    }
}

impl Error for ProtectedRowSoftmaxV1AdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Finalization(error) => Some(error),
            Self::CertificateField(_) | Self::CompilerArtifactField(_) => None,
        }
    }
}

/// Consumes the exact proof witness and direct-worker inspection into one inert
/// protected artifact admission.
pub fn prepare_protected_row_softmax_v1_admission_v1(
    certificate: AuthenticatedRowSoftmaxVerificationCertificateV1,
    inspected: InspectedRowSoftmaxV1DirectWorkerHsacoV1,
) -> Result<PreparedProtectedRowSoftmaxV1AdmissionV1, ProtectedRowSoftmaxV1AdmissionErrorV1> {
    validate_certificate(&certificate)?;

    let exchange = inspected.exchange();
    let structural = inspected.structural();
    let raw_identity = ContentIdentityV1::calculate(structural.exact_bytes());
    if raw_identity != exchange.linked_output_identity() {
        return Err(artifact_mismatch("linked output identity"));
    }
    let descriptor = structural.descriptor_admission();
    let pre_facts = JoinFactsV1 {
        certificate_source: certificate.attributed_source_sha256(),
        certificate_portable_mir: certificate.portable_mir_sha256(),
        certificate_compiler_semantics: certificate.compiler_semantics_sha256(),
        certificate_numerical_policy: certificate.numerical_policy_sha256(),
        certificate_proof: certificate.proof_source_sha256(),
        certificate_kernel_ir: certificate.kernel_ir_sha256(),
        certificate_llvm_body: certificate.llvm_body_sha256(),
        certificate_verus: certificate.verus_executable_sha256(),
        certificate_solver: certificate.solver_executable_sha256(),
        target: structural.target().to_string(),
        code_object_version: structural.code_object_version(),
        row_elements: certificate.row_elements(),
        workgroup: descriptor.workgroup_size(),
        grid: descriptor.max_grid_size(),
        explicit_kernarg_bytes: descriptor.explicit_kernarg_bytes(),
        implicit_kernarg_bytes: descriptor.implicit_kernarg_bytes(),
        total_kernarg_bytes: descriptor.total_kernarg_bytes(),
        kernarg_alignment: 8,
        wavefront_size: 64,
        static_lds_bytes: 0,
        private_segment_bytes: 0,
        exchange_identity: *exchange.identity().as_bytes(),
        compiler_module: exchange.compiler_module_identity(),
        linked_output: exchange.linked_output_identity(),
        raw_output: raw_identity,
        frontend_authority: *exchange.embedded_frontend_authority_commitment(),
        ocml_provider: *exchange.measured_ocml_provider_manifest_identity(),
        ocml_file_count: exchange.measured_ocml_provider_file_count(),
        ocml_import: exchange.requested_ocml_import(),
        descriptor_kernel: *descriptor.kernel_id().as_bytes(),
        descriptor_digest: *descriptor.descriptor_digest().as_bytes(),
    };
    validate_join_facts(&pre_facts)?;

    let finalized =
        finalize_row_softmax_v1_structural_worker_v2_hsaco_v1(inspected.into_structural())
            .map_err(ProtectedRowSoftmaxV1AdmissionErrorV1::Finalization)?;
    if finalized.raw_output_identity() != exchange.linked_output_identity()
        || !finalized
            .finalized_output_identity()
            .matches(finalized.exact_finalized_bytes())
        || finalized.descriptor_admission() != descriptor
    {
        return Err(artifact_mismatch("finalized identity closure"));
    }

    let identity = calculate_admission_identity(&certificate, exchange, &finalized, &pre_facts);
    Ok(PreparedProtectedRowSoftmaxV1AdmissionV1 {
        identity,
        certificate,
        exchange,
        finalized,
    })
}

fn validate_certificate(
    certificate: &AuthenticatedRowSoftmaxVerificationCertificateV1,
) -> Result<(), ProtectedRowSoftmaxV1AdmissionErrorV1> {
    for (field, actual, expected) in [
        (
            "attributed source",
            certificate.attributed_source_sha256(),
            pinned_sha256(SOURCE_SHA256),
        ),
        (
            "portable MIR",
            certificate.portable_mir_sha256(),
            pinned_sha256(PORTABLE_MIR_SHA256),
        ),
        (
            "compiler semantics",
            certificate.compiler_semantics_sha256(),
            pinned_sha256(COMPILER_SEMANTICS_SHA256),
        ),
        (
            "numerical policy",
            certificate.numerical_policy_sha256(),
            pinned_sha256(NUMERICAL_POLICY_SHA256),
        ),
        (
            "proof source",
            certificate.proof_source_sha256(),
            pinned_sha256(PROOF_SOURCE_SHA256),
        ),
        (
            "Kernel IR",
            certificate.kernel_ir_sha256(),
            pinned_sha256(KERNEL_IR_SHA256),
        ),
        (
            "LLVM body",
            certificate.llvm_body_sha256(),
            pinned_sha256(LLVM_BODY_SHA256),
        ),
        (
            "Verus executable",
            certificate.verus_executable_sha256(),
            pinned_sha256(VERUS_EXECUTABLE_SHA256),
        ),
        (
            "solver executable",
            certificate.solver_executable_sha256(),
            pinned_sha256(SOLVER_EXECUTABLE_SHA256),
        ),
    ] {
        if actual != expected {
            return Err(ProtectedRowSoftmaxV1AdmissionErrorV1::CertificateField(
                field,
            ));
        }
    }
    if certificate.target() != TARGET
        || certificate.row_elements() != 64
        || certificate.compiler_profile() != COMPILER_PROFILE
        || certificate.kernel_ir_profile() != KERNEL_IR_PROFILE
    {
        return Err(ProtectedRowSoftmaxV1AdmissionErrorV1::CertificateField(
            "profile",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct JoinFactsV1 {
    certificate_source: [u8; 32],
    certificate_portable_mir: [u8; 32],
    certificate_compiler_semantics: [u8; 32],
    certificate_numerical_policy: [u8; 32],
    certificate_proof: [u8; 32],
    certificate_kernel_ir: [u8; 32],
    certificate_llvm_body: [u8; 32],
    certificate_verus: [u8; 32],
    certificate_solver: [u8; 32],
    target: String,
    code_object_version: CodeObjectVersion,
    row_elements: u32,
    workgroup: [u32; 3],
    grid: [u32; 3],
    explicit_kernarg_bytes: u32,
    implicit_kernarg_bytes: u32,
    total_kernarg_bytes: u32,
    kernarg_alignment: u32,
    wavefront_size: u32,
    static_lds_bytes: u32,
    private_segment_bytes: u32,
    exchange_identity: [u8; 32],
    compiler_module: ContentIdentityV1,
    linked_output: ContentIdentityV1,
    raw_output: ContentIdentityV1,
    frontend_authority: [u8; 32],
    ocml_provider: [u8; 32],
    ocml_file_count: usize,
    ocml_import: &'static str,
    descriptor_kernel: [u8; 32],
    descriptor_digest: [u8; 32],
}

fn validate_join_facts(facts: &JoinFactsV1) -> Result<(), ProtectedRowSoftmaxV1AdmissionErrorV1> {
    for (field, actual, expected) in [
        (
            "attributed source",
            facts.certificate_source,
            pinned_sha256(SOURCE_SHA256),
        ),
        (
            "portable MIR",
            facts.certificate_portable_mir,
            pinned_sha256(PORTABLE_MIR_SHA256),
        ),
        (
            "compiler semantics",
            facts.certificate_compiler_semantics,
            pinned_sha256(COMPILER_SEMANTICS_SHA256),
        ),
        (
            "numerical policy",
            facts.certificate_numerical_policy,
            pinned_sha256(NUMERICAL_POLICY_SHA256),
        ),
        (
            "proof source",
            facts.certificate_proof,
            pinned_sha256(PROOF_SOURCE_SHA256),
        ),
        (
            "Kernel IR",
            facts.certificate_kernel_ir,
            pinned_sha256(KERNEL_IR_SHA256),
        ),
        (
            "LLVM body",
            facts.certificate_llvm_body,
            pinned_sha256(LLVM_BODY_SHA256),
        ),
        (
            "Verus executable",
            facts.certificate_verus,
            pinned_sha256(VERUS_EXECUTABLE_SHA256),
        ),
        (
            "solver executable",
            facts.certificate_solver,
            pinned_sha256(SOLVER_EXECUTABLE_SHA256),
        ),
    ] {
        if actual != expected {
            return Err(artifact_mismatch(field));
        }
    }
    if facts.target != TARGET || facts.code_object_version != CodeObjectVersion::V6 {
        return Err(artifact_mismatch("target/code-object profile"));
    }
    if facts.row_elements != 64 || facts.workgroup != [64, 1, 1] || facts.grid != [1, 1, 1] {
        return Err(artifact_mismatch("specialization/launch profile"));
    }
    if facts.explicit_kernarg_bytes != 32
        || facts.implicit_kernarg_bytes != 256
        || facts.total_kernarg_bytes != 288
        || facts.kernarg_alignment != 8
    {
        return Err(artifact_mismatch("ABI profile"));
    }
    if facts.wavefront_size != 64 || facts.static_lds_bytes != 0 || facts.private_segment_bytes != 0
    {
        return Err(artifact_mismatch("resource profile"));
    }
    for (field, digest) in [
        ("exchange identity", facts.exchange_identity),
        ("frontend authority", facts.frontend_authority),
        ("OCML provider", facts.ocml_provider),
        ("descriptor kernel", facts.descriptor_kernel),
        ("descriptor digest", facts.descriptor_digest),
    ] {
        if digest == [0; 32] {
            return Err(artifact_mismatch(field));
        }
    }
    for (field, identity) in [
        ("compiler module", facts.compiler_module),
        ("linked output", facts.linked_output),
        ("raw output", facts.raw_output),
    ] {
        if identity.byte_len() == 0 || identity.sha256() == &[0; 32] {
            return Err(artifact_mismatch(field));
        }
    }
    if facts.linked_output != facts.raw_output {
        return Err(artifact_mismatch("linked/raw output correspondence"));
    }
    if facts.ocml_file_count != 4 || facts.ocml_import != OCML_EXP_F32 {
        return Err(artifact_mismatch("OCML closure"));
    }
    Ok(())
}

fn calculate_admission_identity(
    certificate: &AuthenticatedRowSoftmaxVerificationCertificateV1,
    exchange: ValidatedRowSoftmaxV1DirectWorkerExchangeV1,
    finalized: &FinalizedRowSoftmaxV1StructuralHsacoV1,
    facts: &JoinFactsV1,
) -> ProtectedRowSoftmaxV1AdmissionIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(ADMISSION_IDENTITY_DOMAIN_V1);
    digest.update(certificate.identity().as_bytes());
    for identity in [
        facts.certificate_source,
        facts.certificate_portable_mir,
        facts.certificate_compiler_semantics,
        facts.certificate_numerical_policy,
        facts.certificate_proof,
        facts.certificate_kernel_ir,
        facts.certificate_llvm_body,
        facts.certificate_verus,
        facts.certificate_solver,
    ] {
        digest.update(identity);
    }
    digest.update(exchange.identity().as_bytes());
    hash_content(&mut digest, exchange.compiler_module_identity());
    hash_content(&mut digest, exchange.linked_output_identity());
    digest.update(exchange.embedded_frontend_authority_commitment());
    digest.update(exchange.measured_ocml_provider_manifest_identity());
    digest.update(finalized.identity().as_bytes());
    hash_content(&mut digest, finalized.raw_output_identity());
    hash_content(&mut digest, finalized.finalized_output_identity());
    digest.update(facts.descriptor_kernel);
    digest.update(facts.descriptor_digest);
    digest.update(TARGET.as_bytes());
    digest.update(6_u16.to_le_bytes());
    digest.update(64_u32.to_le_bytes());
    digest.update(
        [64_u32, 1, 1]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    digest.update(
        [1_u32, 1, 1]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    for value in [32_u32, 256, 288, 8, 64, 0, 0] {
        digest.update(value.to_le_bytes());
    }
    ProtectedRowSoftmaxV1AdmissionIdentityV1(digest.finalize().into())
}

fn hash_content(digest: &mut Sha256, identity: ContentIdentityV1) {
    digest.update(identity.byte_len().to_le_bytes());
    digest.update(identity.sha256());
}

fn pinned_sha256(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    std::array::from_fn(|index| {
        (hex_nibble(bytes[index * 2]) << 4) | hex_nibble(bytes[index * 2 + 1])
    })
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("pinned SHA-256 is lowercase hexadecimal"),
    }
}

const fn artifact_mismatch(field: &'static str) -> ProtectedRowSoftmaxV1AdmissionErrorV1 {
    ProtectedRowSoftmaxV1AdmissionErrorV1::CompilerArtifactField(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(byte: u8) -> ContentIdentityV1 {
        ContentIdentityV1::from_parts([byte; 32], 64)
    }

    fn canonical_facts() -> JoinFactsV1 {
        JoinFactsV1 {
            certificate_source: pinned_sha256(SOURCE_SHA256),
            certificate_portable_mir: pinned_sha256(PORTABLE_MIR_SHA256),
            certificate_compiler_semantics: pinned_sha256(COMPILER_SEMANTICS_SHA256),
            certificate_numerical_policy: pinned_sha256(NUMERICAL_POLICY_SHA256),
            certificate_proof: pinned_sha256(PROOF_SOURCE_SHA256),
            certificate_kernel_ir: pinned_sha256(KERNEL_IR_SHA256),
            certificate_llvm_body: pinned_sha256(LLVM_BODY_SHA256),
            certificate_verus: pinned_sha256(VERUS_EXECUTABLE_SHA256),
            certificate_solver: pinned_sha256(SOLVER_EXECUTABLE_SHA256),
            target: TARGET.to_owned(),
            code_object_version: CodeObjectVersion::V6,
            row_elements: 64,
            workgroup: [64, 1, 1],
            grid: [1, 1, 1],
            explicit_kernarg_bytes: 32,
            implicit_kernarg_bytes: 256,
            total_kernarg_bytes: 288,
            kernarg_alignment: 8,
            wavefront_size: 64,
            static_lds_bytes: 0,
            private_segment_bytes: 0,
            exchange_identity: [1; 32],
            compiler_module: content(2),
            linked_output: content(3),
            raw_output: content(3),
            frontend_authority: [4; 32],
            ocml_provider: [5; 32],
            ocml_file_count: 4,
            ocml_import: OCML_EXP_F32,
            descriptor_kernel: [6; 32],
            descriptor_digest: [7; 32],
        }
    }

    #[test]
    fn exact_join_facts_are_accepted() {
        validate_join_facts(&canonical_facts()).unwrap();
    }

    #[test]
    fn every_join_field_is_independently_rejected() {
        let mutations: &[fn(&mut JoinFactsV1)] = &[
            |v| v.certificate_source[0] ^= 1,
            |v| v.certificate_portable_mir[0] ^= 1,
            |v| v.certificate_compiler_semantics[0] ^= 1,
            |v| v.certificate_numerical_policy[0] ^= 1,
            |v| v.certificate_proof[0] ^= 1,
            |v| v.certificate_kernel_ir[0] ^= 1,
            |v| v.certificate_llvm_body[0] ^= 1,
            |v| v.certificate_verus[0] ^= 1,
            |v| v.certificate_solver[0] ^= 1,
            |v| v.target = "gfx942:xnack+".to_owned(),
            |v| v.code_object_version = CodeObjectVersion::V5,
            |v| v.row_elements = 63,
            |v| v.workgroup = [32, 1, 1],
            |v| v.grid = [2, 1, 1],
            |v| v.explicit_kernarg_bytes = 31,
            |v| v.implicit_kernarg_bytes = 255,
            |v| v.total_kernarg_bytes = 287,
            |v| v.kernarg_alignment = 4,
            |v| v.wavefront_size = 32,
            |v| v.static_lds_bytes = 4,
            |v| v.private_segment_bytes = 4,
            |v| v.exchange_identity = [0; 32],
            |v| v.compiler_module = ContentIdentityV1::from_parts([0; 32], 0),
            |v| v.linked_output = content(8),
            |v| v.raw_output = ContentIdentityV1::from_parts([0; 32], 0),
            |v| v.frontend_authority = [0; 32],
            |v| v.ocml_provider = [0; 32],
            |v| v.ocml_file_count = 3,
            |v| v.ocml_import = "__ocml_exp_f64",
            |v| v.descriptor_kernel = [0; 32],
            |v| v.descriptor_digest = [0; 32],
        ];
        for (index, mutate) in mutations.iter().enumerate() {
            let mut facts = canonical_facts();
            mutate(&mut facts);
            assert!(
                validate_join_facts(&facts).is_err(),
                "mutation {index} escaped admission"
            );
        }
    }
}
