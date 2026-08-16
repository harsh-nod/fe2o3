//! Exact direct Worker V2 validation for the row-softmax V1 profile.
//!
//! The profile starts from an out-of-band-pinned rustc handoff, then checks the
//! measured worker exchange and the existing structural HSACO boundary. It
//! does not assign mathematical meaning to OCML or authenticate the origin of
//! the handoff pin.

use std::{error::Error, fmt};

#[cfg(test)]
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SECTION_NAME_V1,
    MAX_ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_BYTES_V1 as MAX_FRONTEND_AUTHORITY_TRANSCRIPT_BYTES,
    ROW_SOFTMAX_AUTHORITY_SECTION_NAME_V1 as FRONTEND_AUTHORITY_SECTION,
    ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1 as FRONTEND_AUTHORITY_TRANSCRIPT_SECTION,
    ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_BYTES_V1 as EXPONENTIAL_BOUNDARY_BYTES,
    ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_SECTION_NAME_V1 as EXPONENTIAL_BOUNDARY_SECTION,
};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1, CompilerModuleHandoffErrorV2,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, ROW_SOFTMAX_AUTHORITY_BYTES_V1 as FRONTEND_AUTHORITY_BYTES,
    decode_row_softmax_compiler_sections_v1,
};
use fe2o3_kernel_descriptor::{
    CanonicalCodeObjectDigest, CodeObjectVersion, ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
    ROW_SOFTMAX_V1_ENTRY_NAME, RowSoftmaxV1StructuralDescriptorErrorV1,
    RowSoftmaxV1StructuralDescriptorExpectationV1, admit_row_softmax_v1_structural_descriptor_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertDecodedWorkerExchangeV2, InertFirstBuildWorkerV2EvidenceV1,
    InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1, WorkerCompilerFfiEnvelopeIdentityV2,
    WorkerDeviceLibraryProviderEvidenceV1, WorkerInputKindV1, WorkerOptimizationLevelV1,
    WorkerOptionsV1, WorkerProtocolError, WorkerRequestV2, WorkerResponseV2, WorkerStageV1,
    inspect_row_softmax_v1_structural_worker_v2_hsaco_v1,
    row_softmax_authority::{
        RowSoftmaxV1AuthorityPolicyV1, validate_row_softmax_v1_authority_transcript,
    },
};

const TARGET: &str = "gfx942:xnack-";
/// Exact reviewed upstream LLVM build admitted by the protected row-softmax profile.
pub const ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1: &str =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";
const OCML_EXP_F32: &str = "__ocml_exp_f32";
const MEASURED_OCML_PROVIDER_FILE_COUNT: usize = 4;
const OCML_PROVIDER_IDENTITY: &str = "gfx942-ocml-v1";
const OCML_PROVIDER_BASENAMES: [&str; MEASURED_OCML_PROVIDER_FILE_COUNT] = [
    "ocml.bc",
    "oclc_isa_version_942.bc",
    "oclc_unsafe_math_off.bc",
    "oclc_finite_only_off.bc",
];
/// Exact Worker Complete diagnostic for the structural row profile.
///
/// This records transcript digest consistency and exact structural checks. It
/// explicitly does not authenticate descriptor source provenance; that remains
/// a downstream, attempt-scoped admission boundary.
pub const ROW_SOFTMAX_V1_WORKER_COMPLETE_DIAGNOSTIC_V1: &str = concat!(
    "post_link.check=row_softmax_v1_profile status=ok ",
    "profile_identity=row-softmax-v1-gfx942-cov6-llvm22-v1 ",
    "llvm_build_identity=upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1 ",
    "llvm_layout=e-m%3Ae-p%3A64%3A64-p1%3A64%3A64-p2%3A32%3A32-p3%3A32%3A32-",
    "p4%3A64%3A64-p5%3A32%3A32-p6%3A32%3A32-p7%3A160%3A256%3A256%3A32-",
    "p8%3A128%3A128%3A128%3A48-p9%3A192%3A256%3A256%3A32-i64%3A64-",
    "v16%3A16-v24%3A32-v32%3A32-v48%3A64-v96%3A128-v192%3A256-",
    "v256%3A256-v512%3A512-v1024%3A1024-v2048%3A2048-n32%3A64-S32-A5-G1-ni%3A7%3A8%3A9 ",
    "abi_checks=exact descriptor_checks=section-envelope-and-byte-identity ",
    "transcript=sha256-consistency-only ",
    "descriptor_source_authentication=outside-worker-complete",
);
const SUCCESS_DIAGNOSTICS: [&str; 7] = [
    "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[__ocml_exp_f32] files=4",
    "post_link.check=exports status=ok symbols=[__ocml_exp_f32,row_softmax_v1,row_softmax_v1.kd]",
    "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-",
    ROW_SOFTMAX_V1_WORKER_COMPLETE_DIAGNOSTIC_V1,
    "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c",
    "post_link.check=unresolved status=ok symbols=[]",
    "post_link.kernel name=row_softmax_v1 symbol=row_softmax_v1.kd kernarg_size=288 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]",
];
#[cfg(test)]
const EXCHANGE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/ROW-SOFTMAX-V1/DIRECT-WORKER-EXCHANGE/V1\0";
const DUAL_EXCHANGE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/ROW-SOFTMAX-V1/DIRECT-WORKER-DUAL-EXCHANGE/V1\0";

/// Independent pins for the worker-owned gfx942 OCML provider closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1OcmlProviderPinsV1 {
    file_sha256: [[u8; 32]; MEASURED_OCML_PROVIDER_FILE_COUNT],
    manifest_identity: [u8; 32],
}

impl RowSoftmaxV1OcmlProviderPinsV1 {
    pub fn new(
        file_sha256: [[u8; 32]; MEASURED_OCML_PROVIDER_FILE_COUNT],
        manifest_identity: [u8; 32],
    ) -> Result<Self, RowSoftmaxV1DirectWorkerErrorV1> {
        if file_sha256.iter().any(|digest| digest == &[0; 32]) {
            return Err(profile_mismatch("OCML provider file pins"));
        }
        if manifest_identity == [0; 32] {
            return Err(profile_mismatch("OCML provider manifest pin"));
        }
        Ok(Self {
            file_sha256,
            manifest_identity,
        })
    }

    pub const fn file_sha256(&self) -> &[[u8; 32]; MEASURED_OCML_PROVIDER_FILE_COUNT] {
        &self.file_sha256
    }

    pub const fn manifest_identity(&self) -> &[u8; 32] {
        &self.manifest_identity
    }
}

/// Independent pins for the executable, build identities, and provider closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1DirectWorkerPinsV1 {
    executable: ContentIdentityV1,
    worker_build_identity_sha256: [u8; 32],
    llvm_build_identity_sha256: [u8; 32],
    provider: RowSoftmaxV1OcmlProviderPinsV1,
}

impl RowSoftmaxV1DirectWorkerPinsV1 {
    pub fn new(
        executable: ContentIdentityV1,
        worker_build_identity: &str,
        llvm_build_identity: &str,
        provider: RowSoftmaxV1OcmlProviderPinsV1,
    ) -> Result<Self, RowSoftmaxV1DirectWorkerErrorV1> {
        if executable.byte_len() == 0 || executable.sha256() == &[0; 32] {
            return Err(profile_mismatch("worker executable pin"));
        }
        validate_build_identity_pin(worker_build_identity, "worker build identity pin")?;
        validate_build_identity_pin(llvm_build_identity, "LLVM build identity pin")?;
        if llvm_build_identity != ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1 {
            return Err(profile_mismatch("upstream LLVM 22.1.8 build identity pin"));
        }
        Ok(Self {
            executable,
            worker_build_identity_sha256: Sha256::digest(worker_build_identity.as_bytes()).into(),
            llvm_build_identity_sha256: Sha256::digest(llvm_build_identity.as_bytes()).into(),
            provider,
        })
    }

    pub const fn executable(&self) -> ContentIdentityV1 {
        self.executable
    }

    pub const fn provider(&self) -> RowSoftmaxV1OcmlProviderPinsV1 {
        self.provider
    }
}

/// Exact out-of-band pins needed to admit the row-softmax direct-worker path.
///
/// Construction validates the complete canonical handoff digest, exact row
/// compiler descriptor, and embedded frontend commitment. The pin source is
/// still external to this type and is not authenticated by construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1DirectWorkerExpectationV1 {
    handoff_sha256: [u8; 32],
    frontend_authority_commitment: [u8; FRONTEND_AUTHORITY_BYTES],
    authority_policy: Option<RowSoftmaxV1AuthorityPolicyV1>,
    descriptor: RowSoftmaxV1StructuralDescriptorExpectationV1,
    worker: RowSoftmaxV1DirectWorkerPinsV1,
}

impl RowSoftmaxV1DirectWorkerExpectationV1 {
    /// Creates a row profile from one independently pinned rustc handoff.
    pub fn from_pinned_rustc_handoff(
        handoff: &CompilerModuleHandoffV2,
        expected_handoff_sha256: [u8; 32],
        expected_frontend_authority_commitment: [u8; FRONTEND_AUTHORITY_BYTES],
        expected_authority_policy: RowSoftmaxV1AuthorityPolicyV1,
        expected_worker: RowSoftmaxV1DirectWorkerPinsV1,
    ) -> Result<Self, RowSoftmaxV1DirectWorkerErrorV1> {
        Self::from_pinned_rustc_handoff_inner(
            handoff,
            expected_handoff_sha256,
            expected_frontend_authority_commitment,
            Some(expected_authority_policy),
            expected_worker,
        )
    }

    fn from_pinned_rustc_handoff_inner(
        handoff: &CompilerModuleHandoffV2,
        expected_handoff_sha256: [u8; 32],
        expected_frontend_authority_commitment: [u8; FRONTEND_AUTHORITY_BYTES],
        expected_authority_policy: Option<RowSoftmaxV1AuthorityPolicyV1>,
        expected_worker: RowSoftmaxV1DirectWorkerPinsV1,
    ) -> Result<Self, RowSoftmaxV1DirectWorkerErrorV1> {
        if expected_handoff_sha256 == [0; 32]
            || handoff.identity().sha256() != &expected_handoff_sha256
        {
            return Err(profile_mismatch("pinned rustc handoff identity"));
        }
        if expected_frontend_authority_commitment == [0; FRONTEND_AUTHORITY_BYTES] {
            return Err(profile_mismatch("frontend-authority commitment"));
        }
        let descriptor = validate_handoff_profile(
            handoff,
            &expected_frontend_authority_commitment,
            expected_authority_policy,
        )?;
        Ok(Self {
            handoff_sha256: expected_handoff_sha256,
            frontend_authority_commitment: expected_frontend_authority_commitment,
            authority_policy: expected_authority_policy,
            descriptor,
            worker: expected_worker,
        })
    }

    #[cfg(test)]
    fn from_pinned_rustc_handoff_for_test(
        handoff: &CompilerModuleHandoffV2,
        expected_handoff_sha256: [u8; 32],
        expected_frontend_authority_commitment: [u8; FRONTEND_AUTHORITY_BYTES],
        expected_worker: RowSoftmaxV1DirectWorkerPinsV1,
    ) -> Result<Self, RowSoftmaxV1DirectWorkerErrorV1> {
        Self::from_pinned_rustc_handoff_inner(
            handoff,
            expected_handoff_sha256,
            expected_frontend_authority_commitment,
            None,
            expected_worker,
        )
    }

    #[cfg(test)]
    fn with_authority_policy_for_test(
        mut self,
        authority_policy: RowSoftmaxV1AuthorityPolicyV1,
    ) -> Self {
        self.authority_policy = Some(authority_policy);
        self
    }

    pub const fn handoff_sha256(&self) -> &[u8; 32] {
        &self.handoff_sha256
    }

    pub const fn frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.frontend_authority_commitment
    }

    const fn authority_policy(&self) -> Option<RowSoftmaxV1AuthorityPolicyV1> {
        self.authority_policy
    }

    pub const fn descriptor_expectation(self) -> RowSoftmaxV1StructuralDescriptorExpectationV1 {
        self.descriptor
    }

    pub const fn worker_pins(self) -> RowSoftmaxV1DirectWorkerPinsV1 {
        self.worker
    }

    pub const fn authenticates_pin_origin(&self) -> bool {
        false
    }

    pub const fn proves_exp_math_accuracy(&self) -> bool {
        false
    }

    pub const fn proves_functional_softmax(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn proves_no_comgr_linkage(&self) -> bool {
        false
    }

    pub const fn no_comgr_requires_measured_worker_build_manifest(&self) -> bool {
        true
    }
}

/// Identity of one exact row-softmax request/response exchange.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RowSoftmaxV1DirectWorkerExchangeIdentityV1([u8; 32]);

impl RowSoftmaxV1DirectWorkerExchangeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert evidence for the exact row handoff and measured OCML worker exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedRowSoftmaxV1DirectWorkerExchangeV1 {
    identity: RowSoftmaxV1DirectWorkerExchangeIdentityV1,
    compiler_module: ContentIdentityV1,
    linked_output: ContentIdentityV1,
    frontend_authority_commitment: [u8; FRONTEND_AUTHORITY_BYTES],
    provider_manifest_identity: [u8; 32],
}

impl ValidatedRowSoftmaxV1DirectWorkerExchangeV1 {
    pub const fn identity(&self) -> RowSoftmaxV1DirectWorkerExchangeIdentityV1 {
        self.identity
    }

    pub const fn compiler_module_identity(&self) -> ContentIdentityV1 {
        self.compiler_module
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.linked_output
    }

    pub const fn embedded_frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.frontend_authority_commitment
    }

    pub const fn measured_gfx942_ocml_provider_closure_was_checked(&self) -> bool {
        true
    }

    pub const fn measured_ocml_provider_manifest_identity(&self) -> &[u8; 32] {
        &self.provider_manifest_identity
    }

    pub const fn measured_ocml_provider_file_count(&self) -> usize {
        MEASURED_OCML_PROVIDER_FILE_COUNT
    }

    pub const fn requested_ocml_import(&self) -> &'static str {
        OCML_EXP_F32
    }

    pub const fn authenticates_frontend_origin(&self) -> bool {
        false
    }

    pub const fn proves_exp_math_accuracy(&self) -> bool {
        false
    }

    pub const fn proves_functional_softmax(&self) -> bool {
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

    pub const fn proves_no_comgr_linkage(&self) -> bool {
        false
    }

    pub const fn no_comgr_requires_measured_worker_build_manifest(&self) -> bool {
        true
    }
}

/// Exact row Worker V2 exchange joined to the existing structural HSACO check.
#[derive(Debug)]
pub struct InspectedRowSoftmaxV1DirectWorkerHsacoV1 {
    exchange: ValidatedRowSoftmaxV1DirectWorkerExchangeV1,
    structural: InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1,
}

impl InspectedRowSoftmaxV1DirectWorkerHsacoV1 {
    pub const fn exchange(&self) -> ValidatedRowSoftmaxV1DirectWorkerExchangeV1 {
        self.exchange
    }

    pub const fn structural(&self) -> &InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1 {
        &self.structural
    }

    /// Transfers the already admitted structural capability into canonical finalization.
    pub fn into_structural(self) -> InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1 {
        self.structural
    }

    pub const fn proves_exp_math_accuracy(&self) -> bool {
        false
    }

    pub const fn proves_functional_softmax(&self) -> bool {
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
pub enum RowSoftmaxV1DirectWorkerErrorV1 {
    Handoff(CompilerModuleHandoffErrorV2),
    WorkerProtocol(WorkerProtocolError),
    Descriptor(RowSoftmaxV1StructuralDescriptorErrorV1),
    ProfileMismatch(&'static str),
    Structural(crate::RowSoftmaxV1StructuralArtifactErrorV1),
}

impl fmt::Display for RowSoftmaxV1DirectWorkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(formatter, "invalid row-softmax handoff: {error}"),
            Self::WorkerProtocol(error) => {
                write!(formatter, "invalid row-softmax Worker V2 exchange: {error}")
            }
            Self::Descriptor(error) => {
                write!(
                    formatter,
                    "invalid row-softmax compiler descriptor: {error}"
                )
            }
            Self::ProfileMismatch(field) => {
                write!(
                    formatter,
                    "row-softmax direct-worker profile mismatch: {field}"
                )
            }
            Self::Structural(error) => {
                write!(
                    formatter,
                    "row-softmax structural HSACO admission failed: {error}"
                )
            }
        }
    }
}

impl Error for RowSoftmaxV1DirectWorkerErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::WorkerProtocol(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::Structural(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

/// Validates V3 response integrity and transcript consistency, the handoff, OCML import, and
/// provider evidence.
pub fn validate_row_softmax_v1_direct_worker_exchange_v1(
    source: &InertFirstBuildWorkerV2EvidenceV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<ValidatedRowSoftmaxV1DirectWorkerExchangeV1, RowSoftmaxV1DirectWorkerErrorV1> {
    validate_evidence_attempt_binding(source.attempt(), expected)?;
    if source.handoff_identity().as_bytes() != expected.handoff_sha256() {
        return Err(profile_mismatch("consumed rustc handoff identity"));
    }
    let bootstrap = InertDecodedWorkerExchangeV2::decode(
        source.bootstrap_request_bytes(),
        source.bootstrap().response().canonical_bytes(),
    )
    .map_err(RowSoftmaxV1DirectWorkerErrorV1::WorkerProtocol)?;
    let replay = InertDecodedWorkerExchangeV2::decode(
        source.authorized_request_bytes(),
        source.authorized().response().canonical_bytes(),
    )
    .map_err(RowSoftmaxV1DirectWorkerErrorV1::WorkerProtocol)?;
    let validated = validate_dual_exchange_parts(
        &bootstrap,
        &replay,
        source.compiler_envelope(),
        source.symbol_manifest(),
        expected,
    )?;

    if source.plan().target() != replay.request().target() {
        return Err(profile_mismatch("link-plan target"));
    }
    if source.worker_measurement().executable() != replay.request().worker_executable()
        || source.worker_measurement().worker_build_identity()
            != replay.request().worker_build_identity()
        || source.worker_measurement().llvm_build_identity()
            != replay.request().llvm_build_identity()
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

fn validate_evidence_attempt_binding(
    source_attempt: fe2o3_artifact_transaction::BuildAttempt,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if let Some(policy) = expected.authority_policy()
        && source_attempt != policy.attempt()
    {
        return Err(profile_mismatch("consumed Worker evidence build attempt"));
    }
    Ok(())
}

/// Validates the exact exchange, then consumes it through row structural admission.
pub fn inspect_row_softmax_v1_direct_worker_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<InspectedRowSoftmaxV1DirectWorkerHsacoV1, RowSoftmaxV1DirectWorkerErrorV1> {
    let exchange = validate_row_softmax_v1_direct_worker_exchange_v1(&source, expected)?;
    let structural = inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
        source,
        expected.descriptor_expectation(),
    )
    .map_err(RowSoftmaxV1DirectWorkerErrorV1::Structural)?;
    Ok(InspectedRowSoftmaxV1DirectWorkerHsacoV1 {
        exchange,
        structural,
    })
}

#[cfg(test)]
fn validate_exchange_parts(
    exchange: &InertDecodedWorkerExchangeV2,
    envelope: &CompilerFfiEnvelopeV1,
    manifest: &CompilerModuleSymbolManifestV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<ValidatedRowSoftmaxV1DirectWorkerExchangeV1, RowSoftmaxV1DirectWorkerErrorV1> {
    let linked_output =
        validate_exchange_parts_with_output_policy(exchange, envelope, manifest, expected, true)?;
    Ok(ValidatedRowSoftmaxV1DirectWorkerExchangeV1 {
        identity: calculate_exchange_identity(exchange.request(), exchange.response()),
        compiler_module: exchange.request().compiler_module().identity(),
        linked_output,
        frontend_authority_commitment: expected.frontend_authority_commitment,
        provider_manifest_identity: *expected.worker.provider.manifest_identity(),
    })
}

fn validate_dual_exchange_parts(
    bootstrap: &InertDecodedWorkerExchangeV2,
    replay: &InertDecodedWorkerExchangeV2,
    envelope: &CompilerFfiEnvelopeV1,
    manifest: &CompilerModuleSymbolManifestV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<ValidatedRowSoftmaxV1DirectWorkerExchangeV1, RowSoftmaxV1DirectWorkerErrorV1> {
    validate_matching_requests(bootstrap.request(), replay.request())?;
    validate_matching_responses(bootstrap.response(), replay.response())?;
    let bootstrap_output =
        validate_exchange_parts_with_output_policy(bootstrap, envelope, manifest, expected, false)?;
    let replay_output =
        validate_exchange_parts_with_output_policy(replay, envelope, manifest, expected, true)?;
    if bootstrap_output != replay_output {
        return Err(profile_mismatch("bootstrap/replay linked output identity"));
    }
    Ok(ValidatedRowSoftmaxV1DirectWorkerExchangeV1 {
        identity: calculate_dual_exchange_identity(bootstrap, replay),
        compiler_module: replay.request().compiler_module().identity(),
        linked_output: replay_output,
        frontend_authority_commitment: expected.frontend_authority_commitment,
        provider_manifest_identity: *expected.worker.provider.manifest_identity(),
    })
}

fn validate_exchange_parts_with_output_policy(
    exchange: &InertDecodedWorkerExchangeV2,
    envelope: &CompilerFfiEnvelopeV1,
    manifest: &CompilerModuleSymbolManifestV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
    require_exact_output_bound: bool,
) -> Result<ContentIdentityV1, RowSoftmaxV1DirectWorkerErrorV1> {
    let request = exchange.request();
    validate_request(request, envelope, manifest, expected)?;
    validate_response(
        request,
        exchange.response(),
        expected,
        require_exact_output_bound,
    )?;
    let output = exchange
        .response()
        .output()
        .ok_or_else(|| profile_mismatch("completed response output"))?;
    Ok(output.identity())
}

fn validate_matching_requests(
    bootstrap: &WorkerRequestV2,
    replay: &WorkerRequestV2,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if bootstrap.request_id() == replay.request_id()
        || bootstrap.identity() == replay.identity()
        || bootstrap.llvm_build_identity() != replay.llvm_build_identity()
        || bootstrap.worker_build_identity() != replay.worker_build_identity()
        || bootstrap.worker_executable() != replay.worker_executable()
        || bootstrap.target() != replay.target()
        || bootstrap.code_object_version() != replay.code_object_version()
        || bootstrap.options() != replay.options()
        || bootstrap.compiler_envelope_identity() != replay.compiler_envelope_identity()
        || bootstrap.compiler_module() != replay.compiler_module()
        || bootstrap.external_providers() != replay.external_providers()
        || bootstrap.import_symbols() != replay.import_symbols()
        || bootstrap.export_symbols() != replay.export_symbols()
        || bootstrap.final_symbols() != replay.final_symbols()
        || bootstrap.output_constraints().max_bytes() < replay.output_constraints().max_bytes()
    {
        return Err(profile_mismatch("bootstrap/replay request closure"));
    }
    Ok(())
}

fn validate_matching_responses(
    bootstrap: &WorkerResponseV2,
    replay: &WorkerResponseV2,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    let bootstrap_provider = bootstrap
        .device_library_provider()
        .ok_or_else(|| profile_mismatch("bootstrap structured OCML provider evidence"))?;
    let replay_provider = replay
        .device_library_provider()
        .ok_or_else(|| profile_mismatch("replay structured OCML provider evidence"))?;
    let bootstrap_identity = bootstrap
        .response_identity()
        .ok_or_else(|| profile_mismatch("bootstrap response-integrity identity"))?;
    let replay_identity = replay
        .response_identity()
        .ok_or_else(|| profile_mismatch("replay response-integrity identity"))?;
    if bootstrap_provider != replay_provider {
        return Err(profile_mismatch(
            "bootstrap/replay ordered OCML provider closure",
        ));
    }
    let bootstrap_output = bootstrap
        .output()
        .ok_or_else(|| profile_mismatch("bootstrap completed response output"))?;
    let replay_output = replay
        .output()
        .ok_or_else(|| profile_mismatch("replay completed response output"))?;
    if bootstrap_identity == replay_identity
        || bootstrap.compiler_envelope_identity() != replay.compiler_envelope_identity()
        || bootstrap.worker_build_identity() != replay.worker_build_identity()
        || bootstrap.stage() != replay.stage()
        || bootstrap.diagnostics() != replay.diagnostics()
        || bootstrap_output.identity() != replay_output.identity()
        || bootstrap_output.bytes() != replay_output.bytes()
    {
        return Err(profile_mismatch("bootstrap/replay response closure"));
    }
    Ok(())
}

fn validate_request(
    request: &WorkerRequestV2,
    envelope: &CompilerFfiEnvelopeV1,
    manifest: &CompilerModuleSymbolManifestV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if request.target().to_string() != TARGET {
        return Err(profile_mismatch("request target"));
    }
    if request.code_object_version() != CodeObjectVersion::V6 {
        return Err(profile_mismatch("requested code-object version"));
    }
    if request.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O0, true, true) {
        return Err(profile_mismatch("worker options"));
    }
    let worker_build_identity_sha256: [u8; 32] =
        Sha256::digest(request.worker_build_identity().as_bytes()).into();
    let llvm_build_identity_sha256: [u8; 32] =
        Sha256::digest(request.llvm_build_identity().as_bytes()).into();
    if request.worker_executable() != expected.worker.executable
        || worker_build_identity_sha256 != expected.worker.worker_build_identity_sha256
        || llvm_build_identity_sha256 != expected.worker.llvm_build_identity_sha256
    {
        return Err(profile_mismatch("independently pinned worker identity"));
    }
    if request.compiler_envelope_identity()
        != WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(envelope.identity())
    {
        return Err(profile_mismatch("request compiler-envelope identity"));
    }
    if request.compiler_module().kind() != WorkerInputKindV1::LlvmTextIr {
        return Err(profile_mismatch("compiler-module input kind"));
    }
    if !request.external_providers().is_empty() {
        return Err(profile_mismatch("request-side external provider closure"));
    }
    if request.import_symbols() != [OCML_EXP_F32] || !request.export_symbols().is_empty() {
        return Err(profile_mismatch("device FFI symbol closure"));
    }
    if request.final_symbols()
        != [
            OCML_EXP_F32,
            ROW_SOFTMAX_V1_ENTRY_NAME,
            ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
        ]
    {
        return Err(profile_mismatch("final symbol closure"));
    }

    validate_manifest(manifest)?;
    validate_envelope(envelope)?;
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        envelope.target(),
        envelope.code_object_version(),
        envelope.clone(),
        manifest.clone(),
        request.compiler_module().bytes(),
    )
    .map_err(RowSoftmaxV1DirectWorkerErrorV1::Handoff)?;
    if handoff.identity().sha256() != expected.handoff_sha256() {
        return Err(profile_mismatch("pinned rustc handoff identity"));
    }
    let descriptor = validate_handoff_profile(
        &handoff,
        expected.frontend_authority_commitment(),
        expected.authority_policy(),
    )?;
    if descriptor != expected.descriptor_expectation() {
        return Err(profile_mismatch("compiler descriptor expectation"));
    }
    Ok(())
}

fn validate_handoff_profile(
    handoff: &CompilerModuleHandoffV2,
    expected_frontend_authority: &[u8; FRONTEND_AUTHORITY_BYTES],
    expected_authority_policy: Option<RowSoftmaxV1AuthorityPolicyV1>,
) -> Result<RowSoftmaxV1StructuralDescriptorExpectationV1, RowSoftmaxV1DirectWorkerErrorV1> {
    if handoff.kind() != CompilerModuleKindV1::LlvmTextIr {
        return Err(profile_mismatch("rustc handoff module kind"));
    }
    if handoff.target().to_string() != TARGET {
        return Err(profile_mismatch("rustc handoff target"));
    }
    if handoff.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6 {
        return Err(profile_mismatch("rustc handoff code-object version"));
    }
    validate_envelope(handoff.envelope())?;
    validate_manifest(handoff.symbol_manifest())?;

    let sections = decode_row_softmax_compiler_sections_v1(handoff.module_bytes())
        .map_err(|error| profile_mismatch(error.profile_field()))?;
    let authority = *sections.authority();
    if authority.as_slice() != expected_frontend_authority
        || <[u8; 32]>::from(Sha256::digest(sections.authority_transcript())) != authority
    {
        return Err(profile_mismatch("frontend-authority commitment"));
    }
    let directional = handoff.envelope().directional_symbols();
    if directional.import_semantic_identities().collect::<Vec<_>>()
        != [sections.exponential_boundary()]
    {
        return Err(profile_mismatch("exponential-boundary semantic identity"));
    }
    let source = CompilerDescriptorSourceV1::decode(sections.descriptor())
        .map_err(|_| profile_mismatch("compiler descriptor source"))?;
    if let Some(policy) = expected_authority_policy {
        validate_row_softmax_v1_authority_transcript(
            sections.authority_transcript(),
            *source.identity().sha256(),
            *sections.exponential_boundary(),
            policy,
        )
        .map_err(|_| profile_mismatch("frontend-authority policy"))?;
    }
    let table = source.table();
    if table.canonical_code_object_digest() != CanonicalCodeObjectDigest::from_bytes([0; 32])
        || table.compiler().name().as_str() != "rustc-codegen-fe2o3"
        || table.producer().name().as_str() != "rustc-codegen-fe2o3-worker-v2"
        || table.producer().version().as_str() != "typed-row-softmax-gfx942-cov6-v1"
    {
        return Err(profile_mismatch("compiler descriptor producer profile"));
    }
    let [kernel] = table.kernels() else {
        return Err(profile_mismatch("compiler descriptor kernel closure"));
    };
    let expected = RowSoftmaxV1StructuralDescriptorExpectationV1::new(
        kernel.kernel_id(),
        kernel.source_evidence(),
        kernel.executable_ir_evidence(),
    )
    .map_err(RowSoftmaxV1DirectWorkerErrorV1::Descriptor)?;
    admit_row_softmax_v1_structural_descriptor_v1(table, expected)
        .map_err(RowSoftmaxV1DirectWorkerErrorV1::Descriptor)?;
    Ok(expected)
}

fn validate_envelope(
    envelope: &CompilerFfiEnvelopeV1,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if envelope.target().to_string() != TARGET
        || envelope.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
    {
        return Err(profile_mismatch("compiler FFI envelope target"));
    }
    let directional = envelope.directional_symbols();
    if directional.imports().collect::<Vec<_>>() != [OCML_EXP_F32]
        || directional.exports().next().is_some()
    {
        return Err(profile_mismatch("compiler FFI envelope symbol closure"));
    }
    Ok(())
}

fn validate_manifest(
    manifest: &CompilerModuleSymbolManifestV1,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    let expected = [
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            ROW_SOFTMAX_V1_ENTRY_NAME,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
        ),
        (
            CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
            OCML_EXP_F32,
        ),
    ];
    if manifest.entries().collect::<Vec<_>>() != expected {
        return Err(profile_mismatch("compiler symbol manifest"));
    }
    Ok(())
}

fn validate_response(
    request: &WorkerRequestV2,
    response: &WorkerResponseV2,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
    require_exact_output_bound: bool,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if !response.binds_request(request) {
        return Err(profile_mismatch("response request binding"));
    }
    if response.worker_build_identity() != request.worker_build_identity() {
        return Err(profile_mismatch("response worker identity"));
    }
    if response.stage() != WorkerStageV1::Complete {
        return Err(profile_mismatch("response completion stage"));
    }
    let provider = response
        .device_library_provider()
        .ok_or_else(|| profile_mismatch("structured OCML provider evidence"))?;
    validate_provider_evidence(provider, expected.worker.provider)?;
    if response.response_identity().is_none() {
        return Err(profile_mismatch("provider response-integrity identity"));
    }
    if response.diagnostics().len() != SUCCESS_DIAGNOSTICS.len()
        || response
            .diagnostics()
            .iter()
            .zip(SUCCESS_DIAGNOSTICS)
            .any(|(actual, expected)| actual != expected)
    {
        return Err(profile_mismatch("measured OCML and post-link diagnostics"));
    }
    let output = response
        .output()
        .ok_or_else(|| profile_mismatch("completed response output"))?;
    if output.request_identity() != request.identity()
        || output.compiler_envelope_identity() != request.compiler_envelope_identity()
        || !output.identity().matches(output.bytes())
        || (require_exact_output_bound
            && output.identity().byte_len() != request.output_constraints().max_bytes())
        || output.identity().byte_len() > request.output_constraints().max_bytes()
    {
        return Err(profile_mismatch("response output binding"));
    }
    Ok(())
}

fn validate_provider_evidence(
    evidence: &WorkerDeviceLibraryProviderEvidenceV1,
    expected: RowSoftmaxV1OcmlProviderPinsV1,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if evidence.provider_identity() != OCML_PROVIDER_IDENTITY
        || evidence.target().to_string() != TARGET
        || evidence.code_object_version() != CodeObjectVersion::V6
        || evidence.import_symbols() != [OCML_EXP_F32]
        || evidence.manifest_identity() != expected.manifest_identity()
        || evidence.files().len() != MEASURED_OCML_PROVIDER_FILE_COUNT
    {
        return Err(profile_mismatch("structured OCML provider closure"));
    }
    for (index, file) in evidence.files().iter().enumerate() {
        if file.basename() != OCML_PROVIDER_BASENAMES[index]
            || file.sha256() != &expected.file_sha256[index]
        {
            return Err(profile_mismatch("ordered OCML provider file pins"));
        }
    }
    Ok(())
}

fn validate_build_identity_pin(
    value: &str,
    field: &'static str,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if value.is_empty()
        || value.len() > crate::MAX_WORKER_TOOLCHAIN_ID_BYTES
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(profile_mismatch(field));
    }
    Ok(())
}

#[cfg(test)]
fn decode_bound_sections(
    module: &[u8],
) -> Result<
    fe2o3_compiler_ffi::DecodedRowSoftmaxCompilerSectionsV1,
    fe2o3_compiler_ffi::RowSoftmaxCompilerSectionsErrorV1,
> {
    decode_row_softmax_compiler_sections_v1(module)
}

#[cfg(test)]
fn positions(bytes: &[u8], needle: &[u8]) -> Vec<usize> {
    bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect()
}

#[cfg(test)]
fn module_assembly_section_header(section: &str) -> String {
    format!("module asm \".section {section},\\22\\22,@progbits\"\n")
}

#[cfg(test)]
fn calculate_exchange_identity(
    request: &WorkerRequestV2,
    response: &WorkerResponseV2,
) -> RowSoftmaxV1DirectWorkerExchangeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(EXCHANGE_IDENTITY_DOMAIN_V1);
    digest.update((request.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(request.canonical_bytes());
    digest.update((response.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(response.canonical_bytes());
    RowSoftmaxV1DirectWorkerExchangeIdentityV1(digest.finalize().into())
}

fn calculate_dual_exchange_identity(
    bootstrap: &InertDecodedWorkerExchangeV2,
    replay: &InertDecodedWorkerExchangeV2,
) -> RowSoftmaxV1DirectWorkerExchangeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(DUAL_EXCHANGE_IDENTITY_DOMAIN_V1);
    for exchange in [bootstrap, replay] {
        digest.update((exchange.request().canonical_bytes().len() as u64).to_le_bytes());
        digest.update(exchange.request().canonical_bytes());
        digest.update((exchange.response().canonical_bytes().len() as u64).to_le_bytes());
        digest.update(exchange.response().canonical_bytes());
    }
    RowSoftmaxV1DirectWorkerExchangeIdentityV1(digest.finalize().into())
}

const fn profile_mismatch(field: &'static str) -> RowSoftmaxV1DirectWorkerErrorV1 {
    RowSoftmaxV1DirectWorkerErrorV1::ProfileMismatch(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        RowSoftmaxV1CompilerClosurePolicyV1, RowSoftmaxV1ProviderManifestV1,
        WORKER_REQUEST_MAGIC_V1, WORKER_RESPONSE_MAGIC_V2, WORKER_RESPONSE_MAGIC_V3,
        WorkerEvidenceClassV1, WorkerInputV1, WorkerOutputConstraintsV1, WorkerRequestV1,
        worker_protocol_v2::SealedWorkerRequestV2Parts,
    };
    use fe2o3_artifact_transaction::BuildAttempt;
    use fe2o3_compiler_ffi::{
        CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
        CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
        CompilerModuleSymbolManifestV1, DeviceTargetV1 as CompilerDeviceTargetV1,
    };
    use fe2o3_kernel_descriptor::{
        AccessMode, BlockSizeV1, BuildEvidenceV1, CapabilityV1, CompilerIdentityV1,
        DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DimensionsV1,
        EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1, KernelId,
        LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1, SourceTypeDescriptorV1,
        SourceTypeRecordV1, Text, ValidName, encode_device_descriptor_table_v1,
    };
    use reserved_fe2o3_symbols::{
        DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
        derive_device_ffi_contract_id_v1,
    };

    const OUTPUT: &[u8] = b"linked-row";
    const AUTHORITY_TRANSCRIPT: &[u8] = b"row-softmax-authority-transcript-test-v1";
    const AUTHORITY: [u8; FRONTEND_AUTHORITY_BYTES] = [
        0xd9, 0x8f, 0xe9, 0xd4, 0xe6, 0xd1, 0xa2, 0x4b, 0x12, 0xf1, 0x08, 0x61, 0xc3, 0x19, 0xb5,
        0xf1, 0x0c, 0xb1, 0x5b, 0x31, 0x63, 0xb4, 0x97, 0x92, 0xce, 0xf7, 0xee, 0x92, 0xbe, 0xae,
        0x86, 0x8f,
    ];
    const EXPONENTIAL_BOUNDARY: [u8; EXPONENTIAL_BOUNDARY_BYTES] =
        [0x91; EXPONENTIAL_BOUNDARY_BYTES];
    const OCML_ABI: &str = "C(f32[size=4,align=4])->f32[size=4,align=4]";
    const PROVIDER_DIGESTS: [[u8; 32]; MEASURED_OCML_PROVIDER_FILE_COUNT] =
        [[0x41; 32], [0x42; 32], [0x43; 32], [0x44; 32]];
    const PROVIDER_MANIFEST_DOMAIN: &[u8] = b"FE2O3/DEVICE-LIBRARY-PROVIDER-MANIFEST/V1\0";
    const RESPONSE_DOMAIN: &[u8] = b"FE2O3/DIRECT-LLVM-WORKER-RESPONSE/V3\0";

    fn exact_handoff() -> CompilerModuleHandoffV2 {
        handoff_with(exact_descriptor_source().canonical_bytes(), &AUTHORITY, b"")
    }

    fn handoff_with(
        descriptor: &[u8],
        authority: &[u8],
        extra_text: &[u8],
    ) -> CompilerModuleHandoffV2 {
        let mut module = row_module_prefix(extra_text);
        append_module_assembly_section(
            &mut module,
            COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            descriptor,
        );
        append_module_assembly_section(
            &mut module,
            FRONTEND_AUTHORITY_TRANSCRIPT_SECTION,
            AUTHORITY_TRANSCRIPT,
        );
        append_module_assembly_section(&mut module, FRONTEND_AUTHORITY_SECTION, authority);
        append_module_assembly_section(
            &mut module,
            EXPONENTIAL_BOUNDARY_SECTION,
            &EXPONENTIAL_BOUNDARY,
        );
        handoff_from_module(&module)
    }

    fn row_module_prefix(extra_text: &[u8]) -> Vec<u8> {
        let mut module = br#"; ModuleID = 'row-softmax-v1-test'
target triple = "amdgcn-amd-amdhsa"

declare float @__ocml_exp_f32(float)
define amdgpu_kernel void @row_softmax_v1(ptr %input, i64 %input_len, ptr %output, i64 %output_len) {
entry:
  ret void
}
@row_softmax_v1.kd = external addrspace(1) global i8
"#
        .to_vec();
        module.extend_from_slice(extra_text);
        module
    }

    fn handoff_from_module(module: &[u8]) -> CompilerModuleHandoffV2 {
        let envelope = exact_envelope();
        CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            compiler_target(),
            CompilerCodeObjectVersion::V6,
            envelope,
            exact_manifest(),
            module,
        )
        .unwrap()
    }

    fn row_module_with_sections(sections: &[(&str, &[u8])]) -> Vec<u8> {
        let mut module = row_module_prefix(b"");
        for (name, bytes) in sections {
            append_module_assembly_section(&mut module, name, bytes);
        }
        module
    }

    fn exact_expectation(
        handoff: &CompilerModuleHandoffV2,
    ) -> RowSoftmaxV1DirectWorkerExpectationV1 {
        RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff_for_test(
            handoff,
            *handoff.identity().sha256(),
            AUTHORITY,
            exact_worker_pins(),
        )
        .unwrap()
    }

    fn attempt(generation: u64, session: u8, invocation: u8) -> BuildAttempt {
        BuildAttempt::from_env_value(&format!(
            "{generation}:{}:{}",
            format!("{session:02x}").repeat(16),
            format!("{invocation:02x}").repeat(32),
        ))
        .unwrap()
    }

    fn authority_policy(attempt: BuildAttempt) -> RowSoftmaxV1AuthorityPolicyV1 {
        let definitions = std::array::from_fn(|index| [u8::try_from(index + 1).unwrap(); 16]);
        let sources = [
            [0x31; 32], [0x32; 32], [0x32; 32], [0x32; 32], [0x31; 32], [0x33; 32], [0x33; 32],
            [0x33; 32],
        ];
        let provider =
            RowSoftmaxV1ProviderManifestV1::new(7, [0x41; 16], definitions, sources).unwrap();
        let compiler_closure = RowSoftmaxV1CompilerClosurePolicyV1::new(
            [0x43; 32], [0x44; 32], [0x45; 32], [0x46; 32],
        )
        .unwrap();
        RowSoftmaxV1AuthorityPolicyV1::new(provider, attempt, [0x42; 32], compiler_closure).unwrap()
    }

    fn exact_worker_pins() -> RowSoftmaxV1DirectWorkerPinsV1 {
        let provider = provider_preimage(
            TARGET,
            CodeObjectVersion::V6,
            &[OCML_EXP_F32],
            &exact_provider_files(),
        );
        RowSoftmaxV1DirectWorkerPinsV1::new(
            ContentIdentityV1::from_parts([0x22; 32], 4096),
            "fe2o3-direct-llvm-lld-worker-v2-row",
            ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
            RowSoftmaxV1OcmlProviderPinsV1::new(
                PROVIDER_DIGESTS,
                calculate_test_identity(PROVIDER_MANIFEST_DOMAIN, &provider),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn exact_provider_files() -> Vec<(&'static str, [u8; 32])> {
        OCML_PROVIDER_BASENAMES
            .into_iter()
            .zip(PROVIDER_DIGESTS)
            .collect()
    }

    fn exact_envelope() -> CompilerFfiEnvelopeV1 {
        let semantic_identity = EXPONENTIAL_BOUNDARY;
        let semantic_text = lower_hex(&semantic_identity);
        let fields = DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            symbol: OCML_EXP_F32,
            calling_convention: "C",
            code_object_version: 6,
            target: TARGET,
            physical_abi: OCML_ABI,
            effects: "none",
            semantic_identity: &semantic_text,
        };
        let contract = CompilerFfiContractV1::new(
            derive_device_ffi_contract_id_v1(fields),
            DeviceFfiDirectionV1::Import,
            CompilerFfiLinkRoleV1::RequiresExternalDefinition,
            compiler_target(),
            CompilerCodeObjectVersion::V6,
            CompilerFfiSourceOwnerV1::new(
                "rustc-codegen-fe2o3",
                "rustc-codegen-fe2o3::row_softmax_v1::__ocml_exp_f32",
                [0x92; 16],
                "_RNvNtCs1234_21rustc_codegen_fe2o321row_softmax_v1_14__ocml_exp_f32",
            )
            .unwrap(),
            OCML_EXP_F32,
            OCML_ABI,
            "none",
            semantic_identity,
        )
        .unwrap();
        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(compiler_target(), CompilerCodeObjectVersion::V6, 1)
                .unwrap();
        builder.push(contract).unwrap();
        builder.finish().unwrap()
    }

    fn exact_manifest() -> CompilerModuleSymbolManifestV1 {
        CompilerModuleSymbolManifestV1::new([
            (
                CompilerModuleSymbolRoleV1::KernelEntry,
                ROW_SOFTMAX_V1_ENTRY_NAME,
            ),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
            ),
            (
                CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
                OCML_EXP_F32,
            ),
        ])
        .unwrap()
    }

    fn request(
        handoff: &CompilerModuleHandoffV2,
        request_id: u8,
        target: fe2o3_kernel_descriptor::DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        imports: Vec<String>,
        providers: Vec<WorkerInputV1>,
        final_symbols: Vec<String>,
    ) -> WorkerRequestV2 {
        request_with_output_bound(
            handoff,
            request_id,
            target,
            code_object_version,
            imports,
            providers,
            final_symbols,
            OUTPUT.len() as u64,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn request_with_output_bound(
        handoff: &CompilerModuleHandoffV2,
        request_id: u8,
        target: fe2o3_kernel_descriptor::DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        imports: Vec<String>,
        providers: Vec<WorkerInputV1>,
        final_symbols: Vec<String>,
        output_bound: u64,
    ) -> WorkerRequestV2 {
        WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
            request_id: [request_id; 32],
            llvm_build_identity: ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1.to_owned(),
            worker_build_identity: "fe2o3-direct-llvm-lld-worker-v2-row".to_owned(),
            worker_executable: ContentIdentityV1::from_parts([0x22; 32], 4096),
            target,
            code_object_version,
            options: WorkerOptionsV1::new(WorkerOptimizationLevelV1::O0, true, true),
            compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(
                handoff.envelope().identity(),
            ),
            compiler_module: WorkerInputV1::new(
                WorkerInputKindV1::LlvmTextIr,
                handoff.module_bytes().to_vec(),
            )
            .unwrap(),
            external_providers: providers,
            import_symbols: imports,
            export_symbols: Vec::new(),
            final_symbols,
            output: WorkerOutputConstraintsV1::new(output_bound).unwrap(),
        })
        .unwrap()
    }

    fn exact_request(handoff: &CompilerModuleHandoffV2, request_id: u8) -> WorkerRequestV2 {
        request(
            handoff,
            request_id,
            descriptor_target(),
            CodeObjectVersion::V6,
            vec![OCML_EXP_F32.to_owned()],
            Vec::new(),
            exact_final_symbols(),
        )
    }

    fn bootstrap_request(handoff: &CompilerModuleHandoffV2, request_id: u8) -> WorkerRequestV2 {
        request_with_output_bound(
            handoff,
            request_id,
            descriptor_target(),
            CodeObjectVersion::V6,
            vec![OCML_EXP_F32.to_owned()],
            Vec::new(),
            exact_final_symbols(),
            4096,
        )
    }

    #[test]
    fn legacy_v1_first_build_request_cannot_enter_row_softmax_v2_admission() {
        let handoff = exact_handoff();
        let legacy = WorkerRequestV1::new(
            [0x6a; 32],
            ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
            descriptor_target(),
            CodeObjectVersion::V6,
            WorkerOptionsV1::new(WorkerOptimizationLevelV1::O0, true, true),
            vec![
                WorkerInputV1::new(
                    WorkerInputKindV1::LlvmTextIr,
                    handoff.module_bytes().to_vec(),
                )
                .unwrap(),
            ],
            exact_final_symbols(),
            exact_final_symbols(),
            WorkerOutputConstraintsV1::new(4096).unwrap(),
        )
        .unwrap();

        assert_eq!(legacy.evidence_class(), WorkerEvidenceClassV1::GenericLink);
        assert!(
            legacy
                .canonical_bytes()
                .starts_with(WORKER_REQUEST_MAGIC_V1)
        );
        assert!(matches!(
            InertDecodedWorkerExchangeV2::decode(legacy.canonical_bytes(), b""),
            Err(WorkerProtocolError::BadMagic)
        ));
    }

    fn exact_final_symbols() -> Vec<String> {
        [
            OCML_EXP_F32,
            ROW_SOFTMAX_V1_ENTRY_NAME,
            ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
        ]
        .map(str::to_owned)
        .to_vec()
    }

    fn response(request: &WorkerRequestV2, diagnostics: &[&str]) -> Vec<u8> {
        response_with_provider_files(request, diagnostics, &exact_provider_files())
    }

    fn response_without_provider(request: &WorkerRequestV2, diagnostics: &[&str]) -> Vec<u8> {
        response_prefix(request, diagnostics, WORKER_RESPONSE_MAGIC_V2)
    }

    fn response_with_provider_files(
        request: &WorkerRequestV2,
        diagnostics: &[&str],
        files: &[(&str, [u8; 32])],
    ) -> Vec<u8> {
        let mut encoded = response_prefix(request, diagnostics, WORKER_RESPONSE_MAGIC_V3);
        let mut provider = provider_preimage(TARGET, CodeObjectVersion::V6, &[OCML_EXP_F32], files);
        let manifest_identity = calculate_test_identity(PROVIDER_MANIFEST_DOMAIN, &provider);
        provider.extend_from_slice(&manifest_identity);
        push_field(&mut encoded, 8, &provider);
        let response_identity = calculate_test_identity(RESPONSE_DOMAIN, &encoded);
        push_field(&mut encoded, 9, &response_identity);
        encoded
    }

    fn response_prefix(
        request: &WorkerRequestV2,
        diagnostics: &[&str],
        magic: &[u8; 8],
    ) -> Vec<u8> {
        let mut encoded = magic.to_vec();
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
        let output_identity = ContentIdentityV1::calculate(OUTPUT);
        let mut output = vec![1];
        output.extend_from_slice(output_identity.sha256());
        output.extend_from_slice(&output_identity.byte_len().to_le_bytes());
        output.extend_from_slice(OUTPUT);
        push_field(&mut encoded, 7, &output);
        encoded
    }

    fn provider_preimage(
        target: &str,
        code_object_version: CodeObjectVersion,
        imports: &[&str],
        files: &[(&str, [u8; 32])],
    ) -> Vec<u8> {
        let mut encoded = Vec::new();
        for value in [OCML_PROVIDER_IDENTITY, target] {
            encoded.extend_from_slice(&(value.len() as u32).to_le_bytes());
            encoded.extend_from_slice(value.as_bytes());
        }
        encoded.push(match code_object_version {
            CodeObjectVersion::V4 => 4,
            CodeObjectVersion::V5 => 5,
            CodeObjectVersion::V6 => 6,
        });
        encoded.extend_from_slice(&(imports.len() as u32).to_le_bytes());
        for import in imports {
            encoded.extend_from_slice(&(import.len() as u32).to_le_bytes());
            encoded.extend_from_slice(import.as_bytes());
        }
        encoded.extend_from_slice(&(files.len() as u32).to_le_bytes());
        for (basename, digest) in files {
            encoded.extend_from_slice(&(basename.len() as u32).to_le_bytes());
            encoded.extend_from_slice(basename.as_bytes());
            encoded.extend_from_slice(digest);
        }
        encoded
    }

    fn calculate_test_identity(domain: &[u8], preimage: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(domain);
        digest.update((preimage.len() as u64).to_le_bytes());
        digest.update(preimage);
        digest.finalize().into()
    }

    fn validate(
        handoff: &CompilerModuleHandoffV2,
        request: &WorkerRequestV2,
        expected: RowSoftmaxV1DirectWorkerExpectationV1,
        diagnostics: &[&str],
    ) -> Result<ValidatedRowSoftmaxV1DirectWorkerExchangeV1, RowSoftmaxV1DirectWorkerErrorV1> {
        let exchange = InertDecodedWorkerExchangeV2::decode(
            request.canonical_bytes(),
            &response(request, diagnostics),
        )
        .map_err(RowSoftmaxV1DirectWorkerErrorV1::WorkerProtocol)?;
        validate_exchange_parts(
            &exchange,
            handoff.envelope(),
            handoff.symbol_manifest(),
            expected,
        )
    }

    fn validate_dual(
        handoff: &CompilerModuleHandoffV2,
        bootstrap_request: &WorkerRequestV2,
        bootstrap_response: &[u8],
        replay_request: &WorkerRequestV2,
        replay_response: &[u8],
        expected: RowSoftmaxV1DirectWorkerExpectationV1,
    ) -> Result<ValidatedRowSoftmaxV1DirectWorkerExchangeV1, RowSoftmaxV1DirectWorkerErrorV1> {
        let bootstrap = InertDecodedWorkerExchangeV2::decode(
            bootstrap_request.canonical_bytes(),
            bootstrap_response,
        )
        .map_err(RowSoftmaxV1DirectWorkerErrorV1::WorkerProtocol)?;
        let replay =
            InertDecodedWorkerExchangeV2::decode(replay_request.canonical_bytes(), replay_response)
                .map_err(RowSoftmaxV1DirectWorkerErrorV1::WorkerProtocol)?;
        validate_dual_exchange_parts(
            &bootstrap,
            &replay,
            handoff.envelope(),
            handoff.symbol_manifest(),
            expected,
        )
    }

    fn success_diagnostics() -> Vec<&'static str> {
        SUCCESS_DIAGNOSTICS.to_vec()
    }

    fn exact_descriptor_source() -> CompilerDescriptorSourceV1 {
        let input_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(
            fe2o3_kernel_descriptor::ScalarTypeV1::F32,
        ));
        let input_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(
            fe2o3_kernel_descriptor::ScalarTypeV1::F32,
        ));
        let output_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(
            fe2o3_kernel_descriptor::ScalarTypeV1::F32,
        ));
        let output_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(
            fe2o3_kernel_descriptor::ScalarTypeV1::F32,
        ));
        let kernel = KernelDescriptorV1::new(
            KernelId::from_bytes([0x81; 32]),
            name(ROW_SOFTMAX_V1_ENTRY_NAME),
            name(ROW_SOFTMAX_V1_ENTRY_NAME),
            name(ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL),
            evidence(0x82, 0x83),
            evidence(0x84, 0x85),
            vec![CapabilityV1::AmdWave],
            KernelAbiLayoutV1::new(32, 288, 8).unwrap(),
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
                DimensionsV1::new(1, 1, 1).unwrap(),
                64,
                0,
                0,
            )
            .unwrap(),
            vec![
                LogicalArgumentV1::shared_slice(0, name("input"), &input_source, &input_layout, 0)
                    .unwrap(),
                LogicalArgumentV1::disjoint_slice(
                    1,
                    name("output"),
                    &output_source,
                    &output_layout,
                    AccessMode::ReadWrite,
                    16,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        CompilerDescriptorSourceV1::new(
            DeviceDescriptorTableV1::new(
                CanonicalCodeObjectDigest::from_bytes([0; 32]),
                CodeObjectVersion::V6,
                CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("0.1.0"), [0; 20]),
                ProducerIdentityV1::new(
                    text("rustc-codegen-fe2o3-worker-v2"),
                    text("typed-row-softmax-gfx942-cov6-v1"),
                ),
                descriptor_target(),
                vec![input_source, output_source],
                vec![input_layout, output_layout],
                vec![kernel],
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn append_module_assembly_section(module: &mut Vec<u8>, section: &str, bytes: &[u8]) {
        append_module_assembly_section_with_chunk_width(module, section, bytes, 16);
    }

    fn append_module_assembly_section_with_chunk_width(
        module: &mut Vec<u8>,
        section: &str,
        bytes: &[u8],
        chunk_width: usize,
    ) {
        module.extend_from_slice(module_assembly_section_header(section).as_bytes());
        module.extend_from_slice(b"module asm \".balign 8\"\n");
        for chunk in bytes.chunks(chunk_width) {
            module.extend_from_slice(b"module asm \".byte ");
            for (index, byte) in chunk.iter().copied().enumerate() {
                if index != 0 {
                    module.extend_from_slice(b", ");
                }
                module.extend_from_slice(format!("0x{byte:02x}").as_bytes());
            }
            module.extend_from_slice(b"\"\n");
        }
    }

    fn descriptor_target() -> fe2o3_kernel_descriptor::DeviceTargetV1 {
        fe2o3_kernel_descriptor::DeviceTargetV1::parse(TARGET).unwrap()
    }

    fn compiler_target() -> CompilerDeviceTargetV1 {
        CompilerDeviceTargetV1::parse(TARGET).unwrap()
    }

    fn evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([identity; 32]),
            EvidenceDigest::from_sha256_bytes([digest; 32]),
        )
    }

    fn name(value: &str) -> ValidName {
        ValidName::new(value).unwrap()
    }

    fn text(value: &str) -> Text {
        Text::new(value).unwrap()
    }

    #[test]
    fn production_four_section_suffix_is_admitted() {
        let handoff = exact_handoff();
        let decoded = decode_bound_sections(handoff.module_bytes()).unwrap();

        assert_eq!(
            decoded.descriptor(),
            exact_descriptor_source().canonical_bytes()
        );
        assert_eq!(decoded.authority_transcript(), AUTHORITY_TRANSCRIPT);
        assert_eq!(decoded.authority(), &AUTHORITY);
        assert_eq!(decoded.exponential_boundary(), &EXPONENTIAL_BOUNDARY);
    }

    #[test]
    fn authority_transcript_and_exponential_commitments_are_cross_checked() {
        let descriptor = exact_descriptor_source().canonical_bytes().to_vec();
        let changed_transcript = b"row-softmax-authority-transcript-test-v2";
        let transcript_mismatch = handoff_from_module(&row_module_with_sections(&[
            (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor.as_slice()),
            (FRONTEND_AUTHORITY_TRANSCRIPT_SECTION, changed_transcript),
            (FRONTEND_AUTHORITY_SECTION, AUTHORITY.as_slice()),
            (
                EXPONENTIAL_BOUNDARY_SECTION,
                EXPONENTIAL_BOUNDARY.as_slice(),
            ),
        ]));
        assert!(matches!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff_for_test(
                &transcript_mismatch,
                *transcript_mismatch.identity().sha256(),
                AUTHORITY,
                exact_worker_pins(),
            ),
            Err(RowSoftmaxV1DirectWorkerErrorV1::ProfileMismatch(
                "frontend-authority commitment"
            ))
        ));

        let changed_exponential = [0x92; EXPONENTIAL_BOUNDARY_BYTES];
        let exponential_mismatch = handoff_from_module(&row_module_with_sections(&[
            (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor.as_slice()),
            (FRONTEND_AUTHORITY_TRANSCRIPT_SECTION, AUTHORITY_TRANSCRIPT),
            (FRONTEND_AUTHORITY_SECTION, AUTHORITY.as_slice()),
            (EXPONENTIAL_BOUNDARY_SECTION, changed_exponential.as_slice()),
        ]));
        assert!(matches!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff_for_test(
                &exponential_mismatch,
                *exponential_mismatch.identity().sha256(),
                AUTHORITY,
                exact_worker_pins(),
            ),
            Err(RowSoftmaxV1DirectWorkerErrorV1::ProfileMismatch(
                "exponential-boundary semantic identity"
            ))
        ));

        let oversized = vec![0x55; MAX_FRONTEND_AUTHORITY_TRANSCRIPT_BYTES + 1];
        assert!(
            decode_bound_sections(&row_module_with_sections(&[
                (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor.as_slice()),
                (FRONTEND_AUTHORITY_TRANSCRIPT_SECTION, oversized.as_slice(),),
                (FRONTEND_AUTHORITY_SECTION, AUTHORITY.as_slice()),
                (
                    EXPONENTIAL_BOUNDARY_SECTION,
                    EXPONENTIAL_BOUNDARY.as_slice(),
                ),
            ]))
            .is_err()
        );
    }

    #[test]
    fn row_section_closure_rejects_missing_reordered_duplicate_and_trailing_sections() {
        let descriptor = exact_descriptor_source().canonical_bytes().to_vec();
        let exact = [
            (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor.as_slice()),
            (FRONTEND_AUTHORITY_TRANSCRIPT_SECTION, AUTHORITY_TRANSCRIPT),
            (FRONTEND_AUTHORITY_SECTION, AUTHORITY.as_slice()),
            (
                EXPONENTIAL_BOUNDARY_SECTION,
                EXPONENTIAL_BOUNDARY.as_slice(),
            ),
        ];
        assert!(decode_bound_sections(&row_module_with_sections(&exact)).is_ok());

        let missing = row_module_with_sections(&exact[..3]);
        let reordered = row_module_with_sections(&[exact[1], exact[0], exact[2], exact[3]]);
        let duplicate =
            row_module_with_sections(&[exact[0], exact[1], exact[1], exact[2], exact[3]]);
        let mut trailing_section = row_module_with_sections(&exact);
        append_module_assembly_section(&mut trailing_section, ".fe2o3.unreviewed.v1", &[0x42]);
        let mut trailing_text = row_module_with_sections(&exact);
        trailing_text.extend_from_slice(b"define void @trailing() { ret void }\n");

        for (name, module) in [
            ("missing", missing),
            ("reordered", reordered),
            ("duplicate", duplicate),
            ("trailing section", trailing_section),
            ("trailing text", trailing_text),
        ] {
            assert!(
                decode_bound_sections(&module).is_err(),
                "accepted {name} row-softmax section closure"
            );
        }
    }

    #[test]
    fn row_section_closure_rejects_noncanonical_chunks_and_commitment_sizes() {
        let descriptor = exact_descriptor_source().canonical_bytes().to_vec();
        for (name, authority, exponential) in [
            (
                "short authority",
                AUTHORITY[..31].to_vec(),
                EXPONENTIAL_BOUNDARY.to_vec(),
            ),
            (
                "long authority",
                [AUTHORITY.as_slice(), &[0x01]].concat(),
                EXPONENTIAL_BOUNDARY.to_vec(),
            ),
            (
                "short exponential",
                AUTHORITY.to_vec(),
                EXPONENTIAL_BOUNDARY[..31].to_vec(),
            ),
            (
                "long exponential",
                AUTHORITY.to_vec(),
                [EXPONENTIAL_BOUNDARY.as_slice(), &[0x01]].concat(),
            ),
        ] {
            let module = row_module_with_sections(&[
                (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor.as_slice()),
                (FRONTEND_AUTHORITY_TRANSCRIPT_SECTION, AUTHORITY_TRANSCRIPT),
                (FRONTEND_AUTHORITY_SECTION, authority.as_slice()),
                (EXPONENTIAL_BOUNDARY_SECTION, exponential.as_slice()),
            ]);
            assert!(
                decode_bound_sections(&module).is_err(),
                "accepted {name} commitment"
            );
        }

        let mut short_chunks = row_module_prefix(b"");
        append_module_assembly_section_with_chunk_width(
            &mut short_chunks,
            COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            &descriptor,
            8,
        );
        append_module_assembly_section(
            &mut short_chunks,
            FRONTEND_AUTHORITY_TRANSCRIPT_SECTION,
            AUTHORITY_TRANSCRIPT,
        );
        append_module_assembly_section(&mut short_chunks, FRONTEND_AUTHORITY_SECTION, &AUTHORITY);
        append_module_assembly_section(
            &mut short_chunks,
            EXPONENTIAL_BOUNDARY_SECTION,
            &EXPONENTIAL_BOUNDARY,
        );

        let mut oversized_chunk = row_module_prefix(b"");
        append_module_assembly_section_with_chunk_width(
            &mut oversized_chunk,
            COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            &descriptor,
            17,
        );
        append_module_assembly_section(
            &mut oversized_chunk,
            FRONTEND_AUTHORITY_TRANSCRIPT_SECTION,
            AUTHORITY_TRANSCRIPT,
        );
        append_module_assembly_section(
            &mut oversized_chunk,
            FRONTEND_AUTHORITY_SECTION,
            &AUTHORITY,
        );
        append_module_assembly_section(
            &mut oversized_chunk,
            EXPONENTIAL_BOUNDARY_SECTION,
            &EXPONENTIAL_BOUNDARY,
        );

        let exact = row_module_with_sections(&[
            (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor.as_slice()),
            (FRONTEND_AUTHORITY_TRANSCRIPT_SECTION, AUTHORITY_TRANSCRIPT),
            (FRONTEND_AUTHORITY_SECTION, AUTHORITY.as_slice()),
            (
                EXPONENTIAL_BOUNDARY_SECTION,
                EXPONENTIAL_BOUNDARY.as_slice(),
            ),
        ]);
        let mut uppercase_hex = exact.clone();
        let authority_position = positions(
            &uppercase_hex,
            module_assembly_section_header(FRONTEND_AUTHORITY_SECTION).as_bytes(),
        )[0];
        let hexadecimal = uppercase_hex[authority_position..]
            .windows(4)
            .position(|window| window == b"0xd9")
            .unwrap()
            + authority_position;
        uppercase_hex[hexadecimal + 2] = b'D';
        let mut missing_final_newline = exact;
        assert_eq!(missing_final_newline.pop(), Some(b'\n'));

        for (name, module) in [
            ("short chunks", short_chunks),
            ("oversized chunk", oversized_chunk),
            ("uppercase hexadecimal", uppercase_hex),
            ("missing final newline", missing_final_newline),
        ] {
            assert!(
                decode_bound_sections(&module).is_err(),
                "accepted {name} encoding"
            );
        }
    }

    fn lower_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn push_field(encoded: &mut Vec<u8>, tag: u16, bytes: &[u8]) {
        encoded.extend_from_slice(&tag.to_le_bytes());
        encoded.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        encoded.extend_from_slice(bytes);
    }

    #[test]
    fn exact_pinned_row_exchange_is_admitted_without_math_or_execution_authority() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let request = exact_request(&handoff, 0x11);
        let validated = validate(&handoff, &request, expected, &success_diagnostics()).unwrap();

        assert_ne!(validated.identity().as_bytes(), &[0; 32]);
        assert_eq!(
            validated.compiler_module_identity(),
            request.compiler_module().identity()
        );
        assert_eq!(
            validated.linked_output_identity(),
            ContentIdentityV1::calculate(OUTPUT)
        );
        assert_eq!(
            validated.embedded_frontend_authority_commitment(),
            &AUTHORITY
        );
        assert!(validated.measured_gfx942_ocml_provider_closure_was_checked());
        assert_eq!(validated.measured_ocml_provider_file_count(), 4);
        assert_eq!(
            validated.measured_ocml_provider_manifest_identity(),
            exact_worker_pins().provider().manifest_identity()
        );
        assert_eq!(validated.requested_ocml_import(), OCML_EXP_F32);
        assert!(!validated.authenticates_frontend_origin());
        assert!(!validated.proves_exp_math_accuracy());
        assert!(!validated.proves_functional_softmax());
        assert!(!validated.grants_publication_authority());
        assert!(!validated.grants_load_authority());
        assert!(!validated.grants_launch_authority());
        assert!(!validated.proves_no_comgr_linkage());
        assert!(validated.no_comgr_requires_measured_worker_build_manifest());
    }

    #[test]
    fn exact_handoff_and_matching_evidence_attempt_are_admitted() {
        let handoff = exact_handoff();
        let attempt_a = attempt(7, 0x51, 0x52);
        let expected =
            exact_expectation(&handoff).with_authority_policy_for_test(authority_policy(attempt_a));

        assert_eq!(expected.handoff_sha256(), handoff.identity().sha256());
        validate_evidence_attempt_binding(attempt_a, expected).unwrap();
    }

    #[test]
    fn exact_handoff_from_attempt_a_cannot_be_republished_with_evidence_attempt_b() {
        let handoff = exact_handoff();
        let canonical_handoff = handoff.canonical_bytes().to_vec();
        let attempt_a = attempt(7, 0x51, 0x52);
        let attempt_b = attempt(8, 0x61, 0x62);
        let expected =
            exact_expectation(&handoff).with_authority_policy_for_test(authority_policy(attempt_a));

        assert_eq!(expected.handoff_sha256(), handoff.identity().sha256());
        assert_eq!(canonical_handoff, handoff.canonical_bytes());
        assert!(matches!(
            validate_evidence_attempt_binding(attempt_b, expected),
            Err(RowSoftmaxV1DirectWorkerErrorV1::ProfileMismatch(
                "consumed Worker evidence build attempt"
            ))
        ));
    }

    #[test]
    fn dual_v3_bootstrap_and_exact_replay_are_admitted_as_one_exchange() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let bootstrap_request = bootstrap_request(&handoff, 0x12);
        let replay_request = exact_request(&handoff, 0x13);
        let validated = validate_dual(
            &handoff,
            &bootstrap_request,
            &response(&bootstrap_request, &success_diagnostics()),
            &replay_request,
            &response(&replay_request, &success_diagnostics()),
            expected,
        )
        .unwrap();

        assert_ne!(validated.identity().as_bytes(), &[0; 32]);
        assert_eq!(
            validated.linked_output_identity(),
            ContentIdentityV1::calculate(OUTPUT)
        );
    }

    #[test]
    fn bootstrap_legacy_v2_response_is_rejected_even_when_replay_is_v3() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let bootstrap_request = bootstrap_request(&handoff, 0x14);
        let replay_request = exact_request(&handoff, 0x15);
        let error = validate_dual(
            &handoff,
            &bootstrap_request,
            &response_without_provider(&bootstrap_request, &success_diagnostics()),
            &replay_request,
            &response(&replay_request, &success_diagnostics()),
            expected,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RowSoftmaxV1DirectWorkerErrorV1::ProfileMismatch(
                "bootstrap structured OCML provider evidence"
            )
        ));
    }

    #[test]
    fn identical_outputs_cannot_hide_bootstrap_replay_provider_substitution() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let bootstrap_request = bootstrap_request(&handoff, 0x16);
        let replay_request = exact_request(&handoff, 0x17);
        let mut reordered = exact_provider_files();
        reordered.swap(0, 1);
        let error = validate_dual(
            &handoff,
            &bootstrap_request,
            &response_with_provider_files(&bootstrap_request, &success_diagnostics(), &reordered),
            &replay_request,
            &response(&replay_request, &success_diagnostics()),
            expected,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RowSoftmaxV1DirectWorkerErrorV1::ProfileMismatch(
                "bootstrap/replay ordered OCML provider closure"
            )
        ));
    }

    #[test]
    fn substituted_bootstrap_response_identity_is_rejected_before_row_admission() {
        let handoff = exact_handoff();
        let bootstrap_request = bootstrap_request(&handoff, 0x18);
        let mut substituted = response(&bootstrap_request, &success_diagnostics());
        *substituted.last_mut().unwrap() ^= 1;

        assert!(matches!(
            InertDecodedWorkerExchangeV2::decode(bootstrap_request.canonical_bytes(), &substituted),
            Err(WorkerProtocolError::ResponseIdentityMismatch)
        ));
    }

    #[test]
    fn missing_extra_ocml_imports_and_request_providers_fail_closed() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let provider = WorkerInputV1::new(
            WorkerInputKindV1::LlvmBitcode,
            b"request-side-ocml-provider".to_vec(),
        )
        .unwrap();
        let cases = [
            request(
                &handoff,
                0x21,
                descriptor_target(),
                CodeObjectVersion::V6,
                Vec::new(),
                Vec::new(),
                exact_final_symbols(),
            ),
            request(
                &handoff,
                0x22,
                descriptor_target(),
                CodeObjectVersion::V6,
                vec!["__ocml_cos_f32".to_owned(), OCML_EXP_F32.to_owned()],
                Vec::new(),
                vec![
                    "__ocml_cos_f32".to_owned(),
                    OCML_EXP_F32.to_owned(),
                    ROW_SOFTMAX_V1_ENTRY_NAME.to_owned(),
                    ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL.to_owned(),
                ],
            ),
            request(
                &handoff,
                0x23,
                descriptor_target(),
                CodeObjectVersion::V6,
                vec![OCML_EXP_F32.to_owned()],
                vec![provider],
                exact_final_symbols(),
            ),
        ];
        for request in cases {
            assert!(
                validate(&handoff, &request, expected, &success_diagnostics()).is_err(),
                "accepted substituted OCML closure: {request:?}"
            );
        }
    }

    #[test]
    fn target_cov_symbol_and_provider_diagnostic_substitutions_fail_closed() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let requests = [
            request(
                &handoff,
                0x31,
                fe2o3_kernel_descriptor::DeviceTargetV1::parse("gfx942").unwrap(),
                CodeObjectVersion::V6,
                vec![OCML_EXP_F32.to_owned()],
                Vec::new(),
                exact_final_symbols(),
            ),
            request(
                &handoff,
                0x32,
                descriptor_target(),
                CodeObjectVersion::V5,
                vec![OCML_EXP_F32.to_owned()],
                Vec::new(),
                exact_final_symbols(),
            ),
            request(
                &handoff,
                0x33,
                descriptor_target(),
                CodeObjectVersion::V6,
                vec![OCML_EXP_F32.to_owned()],
                Vec::new(),
                vec![OCML_EXP_F32.to_owned(), "row_softmax_alias".to_owned()],
            ),
        ];
        for request in requests {
            assert!(
                validate(&handoff, &request, expected, &success_diagnostics()).is_err(),
                "accepted substituted row request: {request:?}"
            );
        }

        let request = exact_request(&handoff, 0x34);
        for diagnostic in [
            "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[] files=4",
            "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[__ocml_exp_f32] files=5",
        ] {
            let mut diagnostics = success_diagnostics();
            diagnostics[0] = diagnostic;
            assert!(validate(&handoff, &request, expected, &diagnostics).is_err());
        }
        let mut diagnostics = success_diagnostics();
        diagnostics[3] = "post_link.check=row_softmax_v1_profile status=ok profile_identity=row-softmax-v1-gfx942-cov6-llvm22-v1 descriptor_source_authentication=inside-worker-complete";
        assert!(
            validate(&handoff, &request, expected, &diagnostics).is_err(),
            "accepted a row diagnostic that moved descriptor-source authentication into Worker Complete",
        );
    }

    #[test]
    fn structured_provider_order_substitution_and_diagnostics_only_fail_closed() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let request = exact_request(&handoff, 0x35);

        let diagnostics_only = response_without_provider(&request, &success_diagnostics());
        let diagnostics_only_exchange =
            InertDecodedWorkerExchangeV2::decode(request.canonical_bytes(), &diagnostics_only)
                .unwrap();
        assert!(
            validate_exchange_parts(
                &diagnostics_only_exchange,
                handoff.envelope(),
                handoff.symbol_manifest(),
                expected,
            )
            .is_err()
        );

        let mut reordered = exact_provider_files();
        reordered.swap(0, 1);
        let reordered_response =
            response_with_provider_files(&request, &success_diagnostics(), &reordered);
        let reordered_exchange =
            InertDecodedWorkerExchangeV2::decode(request.canonical_bytes(), &reordered_response)
                .unwrap();
        assert!(
            validate_exchange_parts(
                &reordered_exchange,
                handoff.envelope(),
                handoff.symbol_manifest(),
                expected,
            )
            .is_err()
        );

        let mut substituted = exact_provider_files();
        substituted[2].1[0] ^= 1;
        let substituted_response =
            response_with_provider_files(&request, &success_diagnostics(), &substituted);
        let substituted_exchange =
            InertDecodedWorkerExchangeV2::decode(request.canonical_bytes(), &substituted_response)
                .unwrap();
        assert!(
            validate_exchange_parts(
                &substituted_exchange,
                handoff.envelope(),
                handoff.symbol_manifest(),
                expected,
            )
            .is_err()
        );

        let mut missing = exact_provider_files();
        missing.pop();
        let mut extra = exact_provider_files();
        extra.push(("unmeasured-extra.bc", [0x66; 32]));
        for files in [missing, extra] {
            let response = response_with_provider_files(&request, &success_diagnostics(), &files);
            let exchange =
                InertDecodedWorkerExchangeV2::decode(request.canonical_bytes(), &response).unwrap();
            assert!(
                validate_exchange_parts(
                    &exchange,
                    handoff.envelope(),
                    handoff.symbol_manifest(),
                    expected,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn protocol_compatible_fake_worker_cannot_replace_independent_pins() {
        let handoff = exact_handoff();
        let request = exact_request(&handoff, 0x36);
        let fake_worker = RowSoftmaxV1DirectWorkerPinsV1::new(
            ContentIdentityV1::from_parts([0x77; 32], 4096),
            "untrusted-protocol-compatible-worker",
            ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
            exact_worker_pins().provider(),
        )
        .unwrap();
        let expected = RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff_for_test(
            &handoff,
            *handoff.identity().sha256(),
            AUTHORITY,
            fake_worker,
        )
        .unwrap();
        assert!(validate(&handoff, &request, expected, &success_diagnostics()).is_err());
    }

    #[test]
    fn descriptor_authority_and_arbitrary_semantic_text_cannot_replace_the_pin() {
        let exact = exact_handoff();
        let expected = exact_expectation(&exact);

        let mut descriptor = exact_descriptor_source().canonical_bytes().to_vec();
        let symbol = descriptor
            .windows(ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL.len())
            .position(|window| window == ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL.as_bytes())
            .unwrap();
        descriptor[symbol] = b's';
        let wrong_descriptor = handoff_with(&descriptor, &AUTHORITY, b"");
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff_for_test(
                &wrong_descriptor,
                *wrong_descriptor.identity().sha256(),
                AUTHORITY,
                exact_worker_pins(),
            )
            .is_err()
        );

        let wrong_authority = handoff_with(
            exact_descriptor_source().canonical_bytes(),
            &[0xb6; FRONTEND_AUTHORITY_BYTES],
            b"",
        );
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff_for_test(
                &wrong_authority,
                *wrong_authority.identity().sha256(),
                AUTHORITY,
                exact_worker_pins(),
            )
            .is_err()
        );

        let overclaim = handoff_with(
            exact_descriptor_source().canonical_bytes(),
            &AUTHORITY,
            b"; arbitrary text claims exact exp and softmax semantics\n",
        );
        let request = exact_request(&overclaim, 0x41);
        assert!(validate(&overclaim, &request, expected, &success_diagnostics()).is_err());
        assert!(!expected.proves_exp_math_accuracy());
        assert!(!expected.proves_functional_softmax());
    }

    #[test]
    fn cross_request_output_replay_is_rejected_before_profile_admission() {
        let handoff = exact_handoff();
        let first = exact_request(&handoff, 0x51);
        let second = exact_request(&handoff, 0x52);
        let replay = response(&first, &success_diagnostics());
        assert!(InertDecodedWorkerExchangeV2::decode(second.canonical_bytes(), &replay).is_err());
    }

    #[test]
    fn handoff_pin_and_authority_must_be_nonzero_and_exact() {
        let handoff = exact_handoff();
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff_for_test(
                &handoff,
                [0; 32],
                AUTHORITY,
                exact_worker_pins(),
            )
            .is_err()
        );
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff_for_test(
                &handoff,
                *handoff.identity().sha256(),
                [0; FRONTEND_AUTHORITY_BYTES],
                exact_worker_pins(),
            )
            .is_err()
        );
        let mut wrong = *handoff.identity().sha256();
        wrong[0] ^= 1;
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff_for_test(
                &handoff,
                wrong,
                AUTHORITY,
                exact_worker_pins(),
            )
            .is_err()
        );
    }

    #[test]
    fn worker_and_provider_pins_must_be_independent_nonzero_values() {
        assert!(RowSoftmaxV1OcmlProviderPinsV1::new([[0; 32]; 4], [0x55; 32]).is_err());
        assert!(RowSoftmaxV1OcmlProviderPinsV1::new(PROVIDER_DIGESTS, [0; 32]).is_err());

        let provider = exact_worker_pins().provider();
        assert!(
            RowSoftmaxV1DirectWorkerPinsV1::new(
                ContentIdentityV1::from_parts([0; 32], 4096),
                "worker",
                ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
                provider,
            )
            .is_err()
        );
        assert!(
            RowSoftmaxV1DirectWorkerPinsV1::new(
                ContentIdentityV1::from_parts([0x22; 32], 4096),
                "",
                ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
                provider,
            )
            .is_err()
        );
        assert!(
            RowSoftmaxV1DirectWorkerPinsV1::new(
                ContentIdentityV1::from_parts([0x22; 32], 4096),
                "worker",
                "llvm\nforged",
                provider,
            )
            .is_err()
        );
    }

    #[test]
    fn row_softmax_worker_rejects_rocm_llvm_identity_drift() {
        let provider = exact_worker_pins().provider();
        assert!(
            RowSoftmaxV1DirectWorkerPinsV1::new(
                ContentIdentityV1::from_parts([0x22; 32], 4096),
                "fe2o3-direct-llvm-lld-worker-v2-row",
                ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
                provider,
            )
            .is_ok()
        );
        let rocm_drift = RowSoftmaxV1DirectWorkerPinsV1::new(
            ContentIdentityV1::from_parts([0x22; 32], 4096),
            "fe2o3-direct-llvm-lld-worker-v2-row",
            "7.2.4",
            provider,
        );
        assert!(matches!(
            rocm_drift,
            Err(RowSoftmaxV1DirectWorkerErrorV1::ProfileMismatch(
                "upstream LLVM 22.1.8 build identity pin"
            ))
        ));
    }

    #[test]
    fn descriptor_encoding_is_canonical() {
        let source = exact_descriptor_source();
        let encoded = encode_device_descriptor_table_v1(source.table()).unwrap();
        assert!(!encoded.is_empty());
    }
}
