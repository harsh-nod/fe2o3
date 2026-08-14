//! Exact Worker V2 validation for the scalar GEMM V1 profile.
//!
//! This module starts at an already sealed Worker V2 exchange. It does not
//! authenticate the rustc frontend that selected the canonical Kernel IR.

use std::{error::Error, fmt};

use dialect_amdgcn::{ScalarGemmLoweringErrorV1, lower_scalar_gemm_v1_to_gfx942_llvm_ir};
use fe2o3_compiler_ffi::{
    CompilerFfiEnvelopeV1, CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use fe2o3_kernel_ir::{
    SCALAR_GEMM_V1_KERNEL_ID, ScalarGemmTargetRequirementsV1, scalar_gemm_v1_module,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertDecodedWorkerExchangeV2, InertFirstBuildWorkerV2EvidenceV1,
    InspectedRawWorkerV2HsacoV1, WorkerCompilerFfiEnvelopeIdentityV2, WorkerInputKindV1,
    WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerProtocolError, WorkerRequestV2,
    WorkerResponseV2, WorkerStageV1, WorkerV2RawHsacoInspectionError,
    inspect_worker_v2_raw_hsaco_v1,
};

const SCALAR_GEMM_V1_TARGET: &str = "gfx942:xnack-";
const SCALAR_GEMM_V1_DESCRIPTOR: &str = "scalar_gemm_v1.kd";
const SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES: u64 = 64;
const SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES: u64 = 320;
const SCALAR_GEMM_V1_SUCCESS_DIAGNOSTICS: [&str; 5] = [
    "post_link.check=exports status=ok symbols=[scalar_gemm_v1,scalar_gemm_v1.kd]",
    "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-",
    "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c",
    "post_link.check=unresolved status=ok symbols=[]",
    "post_link.kernel name=scalar_gemm_v1 symbol=scalar_gemm_v1.kd kernarg_size=320 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=256 reqd_workgroup_size=[256,1,1]",
];
const EXCHANGE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SCALAR-GEMM-V1/WORKER-EXCHANGE/V1\0";

/// Identity of one exact request/response exchange admitted by the scalar GEMM V1 worker profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarGemmV1WorkerExchangeIdentityV1([u8; 32]);

impl ScalarGemmV1WorkerExchangeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert validation of the exact scalar GEMM V1 Worker V2 request and response.
///
/// This proves that the request contains the canonical lowering and that the
/// response is bound to that request. It does not inspect the output as a code
/// object and does not authenticate the frontend that selected the Kernel IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedScalarGemmV1WorkerExchangeV1 {
    identity: ScalarGemmV1WorkerExchangeIdentityV1,
    compiler_module: ContentIdentityV1,
    linked_output: ContentIdentityV1,
}

impl ValidatedScalarGemmV1WorkerExchangeV1 {
    pub const fn identity(&self) -> ScalarGemmV1WorkerExchangeIdentityV1 {
        self.identity
    }

    pub const fn compiler_module_identity(&self) -> ContentIdentityV1 {
        self.compiler_module
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.linked_output
    }

    pub const fn requested_code_object_version(&self) -> CodeObjectVersion {
        CodeObjectVersion::V6
    }

    pub const fn code_object_version_was_inspected(&self) -> bool {
        false
    }

    pub const fn authenticates_frontend_origin(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Inert scalar GEMM V1 evidence whose exact Worker V2 output passed raw-HSACO inspection.
///
/// COV6, `gfx942:xnack-`, the one exact kernel/descriptor pair, workgroup size,
/// and wavefront size have been observed in the raw artifact. This still grants
/// no publication, loading, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InspectedScalarGemmV1WorkerV2HsacoV1 {
    exchange: ValidatedScalarGemmV1WorkerExchangeV1,
    raw: InspectedRawWorkerV2HsacoV1,
}

impl InspectedScalarGemmV1WorkerV2HsacoV1 {
    pub const fn exchange(&self) -> ValidatedScalarGemmV1WorkerExchangeV1 {
        self.exchange
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub const fn code_object_version_was_inspected(&self) -> bool {
        true
    }

    pub fn exact_bytes(&self) -> &[u8] {
        self.raw.exact_bytes()
    }

    pub const fn raw_inspection(&self) -> &InspectedRawWorkerV2HsacoV1 {
        &self.raw
    }

    /// Consumes exact scalar-profile inspection and transfers its retained raw-HSACO lineage.
    ///
    /// This is the only ownership bridge into generic Worker V2 finalization. It does not create
    /// authority from bytes, and the consumed scalar capability cannot be reused.
    pub fn into_raw(self) -> InspectedRawWorkerV2HsacoV1 {
        self.raw
    }

    pub const fn authenticates_frontend_origin(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ScalarGemmV1WorkerValidationErrorV1 {
    CanonicalLowering(ScalarGemmLoweringErrorV1),
    WorkerProtocol(WorkerProtocolError),
    ProfileMismatch(&'static str),
    RawHsaco(WorkerV2RawHsacoInspectionError),
}

impl fmt::Display for ScalarGemmV1WorkerValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CanonicalLowering(error) => {
                write!(
                    formatter,
                    "canonical scalar GEMM V1 lowering failed: {error}"
                )
            }
            Self::WorkerProtocol(error) => {
                write!(
                    formatter,
                    "scalar GEMM V1 Worker V2 exchange is invalid: {error}"
                )
            }
            Self::ProfileMismatch(field) => {
                write!(
                    formatter,
                    "scalar GEMM V1 Worker V2 profile mismatch: {field}"
                )
            }
            Self::RawHsaco(error) => {
                write!(
                    formatter,
                    "scalar GEMM V1 raw-HSACO inspection failed: {error}"
                )
            }
        }
    }
}

impl Error for ScalarGemmV1WorkerValidationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CanonicalLowering(error) => Some(error),
            Self::WorkerProtocol(error) => Some(error),
            Self::RawHsaco(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

/// Validates the exact scalar GEMM V1 module and sealed Worker V2 exchange.
///
/// The canonical Kernel IR and LLVM input are reconstructed internally. No
/// symbol, target, LLVM module, provider, option, or output identity is accepted
/// from the caller. The retained first-build evidence remains inert.
pub fn validate_scalar_gemm_v1_worker_exchange_v1(
    source: &InertFirstBuildWorkerV2EvidenceV1,
) -> Result<ValidatedScalarGemmV1WorkerExchangeV1, ScalarGemmV1WorkerValidationErrorV1> {
    let exchange = InertDecodedWorkerExchangeV2::decode(
        source.authorized_request_bytes(),
        source.authorized().response().canonical_bytes(),
    )
    .map_err(ScalarGemmV1WorkerValidationErrorV1::WorkerProtocol)?;
    let expected_envelope = exact_compiler_envelope()?;
    let expected_manifest = exact_symbol_manifest()?;
    let validated = validate_exchange_parts(&exchange, &expected_envelope, &expected_manifest)?;

    if source.compiler_envelope() != &expected_envelope {
        return Err(profile_mismatch("compiler FFI envelope"));
    }
    if source.symbol_manifest() != &expected_manifest {
        return Err(profile_mismatch("compiler symbol manifest"));
    }
    if source.plan().target() != exchange.request().target() {
        return Err(profile_mismatch("link-plan target"));
    }
    if source.worker_measurement().executable() != exchange.request().worker_executable()
        || source.worker_measurement().worker_build_identity()
            != exchange.request().worker_build_identity()
        || source.worker_measurement().llvm_build_identity()
            != exchange.request().llvm_build_identity()
    {
        return Err(profile_mismatch("measured Worker V2 identity"));
    }
    if source.output_identity() != validated.linked_output_identity()
        || !source.output_identity().matches(source.output_bytes())
    {
        return Err(profile_mismatch("linked output identity"));
    }
    Ok(validated)
}

/// Validates the exact exchange, then consumes it through raw-HSACO inspection.
///
/// This is the first scalar GEMM V1 API in this module that reports COV6 as an
/// observed artifact property. It deliberately exposes no HSA launch operation.
pub fn inspect_scalar_gemm_v1_worker_v2_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
) -> Result<InspectedScalarGemmV1WorkerV2HsacoV1, ScalarGemmV1WorkerValidationErrorV1> {
    let exchange = validate_scalar_gemm_v1_worker_exchange_v1(&source)?;
    let raw = inspect_worker_v2_raw_hsaco_v1(source)
        .map_err(ScalarGemmV1WorkerValidationErrorV1::RawHsaco)?;
    let expected_target = exact_target();
    if raw.target() != expected_target {
        return Err(profile_mismatch("inspected target"));
    }
    if raw.code_object_version() != CodeObjectVersion::V6 {
        return Err(profile_mismatch("inspected code-object version"));
    }
    if raw.policy().observed_kernels().len() != 1
        || raw.policy().observed_kernels()[0].entry() != SCALAR_GEMM_V1_KERNEL_ID
        || raw.policy().observed_kernels()[0].descriptor() != SCALAR_GEMM_V1_DESCRIPTOR
    {
        return Err(profile_mismatch("inspected kernel symbol pair"));
    }
    validate_scalar_gemm_v1_kernarg_layout(raw.exact_bytes())?;
    Ok(InspectedScalarGemmV1WorkerV2HsacoV1 { exchange, raw })
}

fn validate_scalar_gemm_v1_kernarg_layout(
    bytes: &[u8],
) -> Result<(), ScalarGemmV1WorkerValidationErrorV1> {
    let inspected = fe2o3_hsaco::inspect(bytes)
        .map_err(|_| profile_mismatch("inspected scalar GEMM metadata"))?;
    let [kernel] = inspected.kernels() else {
        return Err(profile_mismatch("inspected scalar GEMM kernel count"));
    };
    if kernel.kernarg_segment_size() != SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES
        || kernel.kernarg_segment_alignment() != 8
        || kernel.implicit_argument_offset() != Some(SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES)
        || kernel.implicit_argument_size()
            != SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES - SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES
    {
        return Err(profile_mismatch("inspected scalar GEMM kernarg span"));
    }

    const EXPLICIT_FIELDS: [(u64, u64); 9] = [
        (0, 8),
        (8, 8),
        (16, 8),
        (24, 8),
        (32, 8),
        (40, 8),
        (48, 4),
        (52, 4),
        (56, 4),
    ];
    let actual_fields = kernel
        .explicit_arguments()
        .iter()
        .map(|argument| (argument.offset(), argument.size()))
        .collect::<Vec<_>>();
    if actual_fields != EXPLICIT_FIELDS {
        return Err(profile_mismatch("inspected scalar GEMM explicit ABI"));
    }
    Ok(())
}

fn validate_exchange_parts(
    exchange: &InertDecodedWorkerExchangeV2,
    expected_envelope: &CompilerFfiEnvelopeV1,
    expected_manifest: &CompilerModuleSymbolManifestV1,
) -> Result<ValidatedScalarGemmV1WorkerExchangeV1, ScalarGemmV1WorkerValidationErrorV1> {
    let request = exchange.request();
    let response = exchange.response();
    validate_request(request, expected_envelope, expected_manifest)?;
    validate_response(request, response)?;
    let output = response
        .output()
        .ok_or_else(|| profile_mismatch("completed response output"))?;
    let identity = calculate_exchange_identity(request, response);
    Ok(ValidatedScalarGemmV1WorkerExchangeV1 {
        identity,
        compiler_module: request.compiler_module().identity(),
        linked_output: output.identity(),
    })
}

fn validate_request(
    request: &WorkerRequestV2,
    expected_envelope: &CompilerFfiEnvelopeV1,
    expected_manifest: &CompilerModuleSymbolManifestV1,
) -> Result<(), ScalarGemmV1WorkerValidationErrorV1> {
    let canonical = lower_scalar_gemm_v1_to_gfx942_llvm_ir(
        &scalar_gemm_v1_module(),
        ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6(),
    )
    .map_err(ScalarGemmV1WorkerValidationErrorV1::CanonicalLowering)?;
    if request.target() != exact_target() {
        return Err(profile_mismatch("request target"));
    }
    if request.code_object_version() != CodeObjectVersion::V6 {
        return Err(profile_mismatch("requested code-object version"));
    }
    if request.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O0, true, true) {
        return Err(profile_mismatch("worker options"));
    }
    if request.compiler_envelope_identity()
        != WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(expected_envelope.identity())
    {
        return Err(profile_mismatch("request compiler-envelope identity"));
    }
    if expected_manifest != &exact_symbol_manifest()? {
        return Err(profile_mismatch("expected compiler symbol manifest"));
    }
    if request.compiler_module().kind() != WorkerInputKindV1::LlvmTextIr {
        return Err(profile_mismatch("compiler-module input kind"));
    }
    if request.compiler_module().bytes() != canonical.as_str().as_bytes() {
        return Err(profile_mismatch("canonical compiler-module bytes"));
    }
    if !request.external_providers().is_empty() {
        return Err(profile_mismatch("external provider closure"));
    }
    if !request.import_symbols().is_empty() || !request.export_symbols().is_empty() {
        return Err(profile_mismatch("device FFI symbol closure"));
    }
    if request.final_symbols() != [SCALAR_GEMM_V1_KERNEL_ID, SCALAR_GEMM_V1_DESCRIPTOR] {
        return Err(profile_mismatch("final symbol closure"));
    }
    Ok(())
}

fn validate_response(
    request: &WorkerRequestV2,
    response: &WorkerResponseV2,
) -> Result<(), ScalarGemmV1WorkerValidationErrorV1> {
    if !response.binds_request(request) {
        return Err(profile_mismatch("response request binding"));
    }
    if response.stage() != WorkerStageV1::Complete {
        return Err(profile_mismatch("response completion stage"));
    }
    if response.diagnostics().len() != SCALAR_GEMM_V1_SUCCESS_DIAGNOSTICS.len()
        || response
            .diagnostics()
            .iter()
            .zip(SCALAR_GEMM_V1_SUCCESS_DIAGNOSTICS)
            .any(|(actual, expected)| actual != expected)
    {
        return Err(profile_mismatch("completed response post-link diagnostics"));
    }
    let output = response
        .output()
        .ok_or_else(|| profile_mismatch("completed response output"))?;
    if output.request_identity() != request.identity()
        || output.compiler_envelope_identity() != request.compiler_envelope_identity()
        || !output.identity().matches(output.bytes())
        || output.identity().byte_len() != request.output_constraints().max_bytes()
    {
        return Err(profile_mismatch("response output binding"));
    }
    Ok(())
}

fn exact_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(SCALAR_GEMM_V1_TARGET).expect("fixed scalar GEMM V1 target is valid")
}

fn exact_compiler_envelope() -> Result<CompilerFfiEnvelopeV1, ScalarGemmV1WorkerValidationErrorV1> {
    CompilerFfiEnvelopeV1::for_module_without_device_ffi(exact_target(), CodeObjectVersion::V6)
        .map_err(|_| profile_mismatch("internal empty compiler FFI envelope"))
}

fn exact_symbol_manifest()
-> Result<CompilerModuleSymbolManifestV1, ScalarGemmV1WorkerValidationErrorV1> {
    CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            SCALAR_GEMM_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            SCALAR_GEMM_V1_DESCRIPTOR,
        ),
    ])
    .map_err(|_| profile_mismatch("internal compiler symbol manifest"))
}

fn calculate_exchange_identity(
    request: &WorkerRequestV2,
    response: &WorkerResponseV2,
) -> ScalarGemmV1WorkerExchangeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(EXCHANGE_IDENTITY_DOMAIN_V1);
    digest.update((request.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(request.canonical_bytes());
    digest.update((response.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(response.canonical_bytes());
    ScalarGemmV1WorkerExchangeIdentityV1(digest.finalize().into())
}

fn profile_mismatch(field: &'static str) -> ScalarGemmV1WorkerValidationErrorV1 {
    ScalarGemmV1WorkerValidationErrorV1::ProfileMismatch(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WORKER_RESPONSE_MAGIC_V2, WorkerInputV1, WorkerOutputConstraintsV1,
        worker_protocol_v2::SealedWorkerRequestV2Parts,
    };

    fn request_with(
        module: Vec<u8>,
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        options: WorkerOptionsV1,
        external_providers: Vec<WorkerInputV1>,
        final_symbols: Vec<String>,
        compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2,
    ) -> WorkerRequestV2 {
        WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
            request_id: [0x11; 32],
            llvm_build_identity: "upstream-llvm-22".to_owned(),
            worker_build_identity: "fe2o3-direct-llvm-lld-worker-v2".to_owned(),
            worker_executable: ContentIdentityV1::from_parts([0x22; 32], 4096),
            target,
            code_object_version,
            options,
            compiler_envelope,
            compiler_module: WorkerInputV1::new(WorkerInputKindV1::LlvmTextIr, module).unwrap(),
            external_providers,
            import_symbols: Vec::new(),
            export_symbols: Vec::new(),
            final_symbols,
            output: WorkerOutputConstraintsV1::new(b"linked-cov6".len() as u64).unwrap(),
        })
        .unwrap()
    }

    fn exact_request() -> WorkerRequestV2 {
        let canonical = lower_scalar_gemm_v1_to_gfx942_llvm_ir(
            &scalar_gemm_v1_module(),
            ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6(),
        )
        .unwrap();
        let envelope = exact_compiler_envelope().unwrap();
        request_with(
            canonical.as_str().as_bytes().to_vec(),
            exact_target(),
            CodeObjectVersion::V6,
            WorkerOptionsV1::new(WorkerOptimizationLevelV1::O0, true, true),
            Vec::new(),
            vec![
                SCALAR_GEMM_V1_KERNEL_ID.to_owned(),
                SCALAR_GEMM_V1_DESCRIPTOR.to_owned(),
            ],
            WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(envelope.identity()),
        )
    }

    fn response(request: &WorkerRequestV2, diagnostics: &[&str]) -> Vec<u8> {
        let mut encoded = WORKER_RESPONSE_MAGIC_V2.to_vec();
        push_field(&mut encoded, 1, request.request_id());
        push_field(&mut encoded, 2, request.identity());
        push_field(
            &mut encoded,
            3,
            &request.compiler_envelope_identity().as_bytes(),
        );
        push_field(&mut encoded, 4, request.worker_build_identity().as_bytes());
        push_field(&mut encoded, 5, &[WorkerStageV1::Complete as u8]);
        let mut diagnostic_bytes = Vec::new();
        diagnostic_bytes.extend_from_slice(&(diagnostics.len() as u32).to_le_bytes());
        for diagnostic in diagnostics {
            diagnostic_bytes.extend_from_slice(&(diagnostic.len() as u32).to_le_bytes());
            diagnostic_bytes.extend_from_slice(diagnostic.as_bytes());
        }
        push_field(&mut encoded, 6, &diagnostic_bytes);
        let output_bytes = b"linked-cov6";
        let output_identity = ContentIdentityV1::calculate(output_bytes);
        let mut output = vec![1];
        output.extend_from_slice(output_identity.sha256());
        output.extend_from_slice(&output_identity.byte_len().to_le_bytes());
        output.extend_from_slice(output_bytes);
        push_field(&mut encoded, 7, &output);
        encoded
    }

    fn exchange(request: &WorkerRequestV2, diagnostics: &[&str]) -> InertDecodedWorkerExchangeV2 {
        InertDecodedWorkerExchangeV2::decode(
            request.canonical_bytes(),
            &response(request, diagnostics),
        )
        .unwrap()
    }

    fn success_diagnostics() -> Vec<&'static str> {
        SCALAR_GEMM_V1_SUCCESS_DIAGNOSTICS.to_vec()
    }

    fn push_field(encoded: &mut Vec<u8>, tag: u16, bytes: &[u8]) {
        encoded.extend_from_slice(&tag.to_le_bytes());
        encoded.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        encoded.extend_from_slice(bytes);
    }

    #[test]
    fn exact_canonical_request_and_bound_response_are_admitted_inertly() {
        let request = exact_request();
        let exchange = exchange(&request, &success_diagnostics());
        let validated = validate_exchange_parts(
            &exchange,
            &exact_compiler_envelope().unwrap(),
            &exact_symbol_manifest().unwrap(),
        )
        .unwrap();
        assert_ne!(validated.identity().as_bytes(), &[0; 32]);
        assert_eq!(
            validated.compiler_module_identity(),
            request.compiler_module().identity()
        );
        assert!(!validated.code_object_version_was_inspected());
        assert!(!validated.authenticates_frontend_origin());
        assert!(!validated.grants_publication_authority());
        assert!(!validated.grants_load_authority());
        assert!(!validated.grants_launch_authority());
    }

    #[test]
    fn near_match_and_untrusted_module_identities_cannot_enter_the_profile() {
        let exact = exact_request();
        let mut near_match = exact.compiler_module().bytes().to_vec();
        near_match.extend_from_slice(b"; same symbol, untrusted extra text\n");
        let request = request_with(
            near_match,
            exact_target(),
            CodeObjectVersion::V6,
            exact.options(),
            Vec::new(),
            exact.final_symbols().to_vec(),
            exact.compiler_envelope_identity(),
        );
        let exchange = exchange(&request, &success_diagnostics());
        assert!(matches!(
            validate_exchange_parts(
                &exchange,
                &exact_compiler_envelope().unwrap(),
                &exact_symbol_manifest().unwrap(),
            ),
            Err(ScalarGemmV1WorkerValidationErrorV1::ProfileMismatch(
                "canonical compiler-module bytes"
            ))
        ));
    }

    #[test]
    fn every_request_profile_substitution_fails_closed() {
        let exact = exact_request();
        let canonical = exact.compiler_module().bytes().to_vec();
        let expected_envelope = exact_compiler_envelope().unwrap();
        let expected_manifest = exact_symbol_manifest().unwrap();
        let provider = WorkerInputV1::new(
            WorkerInputKindV1::LlvmBitcode,
            b"untrusted-provider".to_vec(),
        )
        .unwrap();
        let cases = [
            request_with(
                canonical.clone(),
                DeviceTargetV1::parse("gfx942").unwrap(),
                CodeObjectVersion::V6,
                exact.options(),
                Vec::new(),
                exact.final_symbols().to_vec(),
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical.clone(),
                exact_target(),
                CodeObjectVersion::V5,
                exact.options(),
                Vec::new(),
                exact.final_symbols().to_vec(),
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical.clone(),
                exact_target(),
                CodeObjectVersion::V6,
                WorkerOptionsV1::new(WorkerOptimizationLevelV1::O1, true, true),
                Vec::new(),
                exact.final_symbols().to_vec(),
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical.clone(),
                exact_target(),
                CodeObjectVersion::V6,
                exact.options(),
                vec![provider],
                exact.final_symbols().to_vec(),
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical.clone(),
                exact_target(),
                CodeObjectVersion::V6,
                exact.options(),
                Vec::new(),
                vec!["scalar_gemm_v1_alias".to_owned()],
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical,
                exact_target(),
                CodeObjectVersion::V6,
                exact.options(),
                Vec::new(),
                exact.final_symbols().to_vec(),
                WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(
                    CompilerFfiEnvelopeV1::for_module_without_device_ffi(
                        DeviceTargetV1::parse("gfx942").unwrap(),
                        CodeObjectVersion::V6,
                    )
                    .unwrap()
                    .identity(),
                ),
            ),
        ];
        for request in cases {
            let exchange = exchange(&request, &success_diagnostics());
            assert!(
                validate_exchange_parts(&exchange, &expected_envelope, &expected_manifest).is_err(),
                "accepted substituted request: {request:?}"
            );
        }
    }

    #[test]
    fn response_diagnostics_and_cross_request_replay_fail_closed() {
        let request = exact_request();
        let with_diagnostic = exchange(&request, &["unexpected warning"]);
        assert!(matches!(
            validate_exchange_parts(
                &with_diagnostic,
                &exact_compiler_envelope().unwrap(),
                &exact_symbol_manifest().unwrap(),
            ),
            Err(ScalarGemmV1WorkerValidationErrorV1::ProfileMismatch(
                "completed response post-link diagnostics"
            ))
        ));

        let mut replay = response(&request, &success_diagnostics());
        replay[14] ^= 1;
        assert!(InertDecodedWorkerExchangeV2::decode(request.canonical_bytes(), &replay).is_err());
    }
}
