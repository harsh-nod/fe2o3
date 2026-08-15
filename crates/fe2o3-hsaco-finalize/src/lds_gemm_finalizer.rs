//! One-shot direct-LLVM finalization for an admitted exact LDS GEMM import.
//!
//! The API in this module has no `llc`, `ld.lld`, shell, or COMGR escape hatch.
//! It consumes the admitted compiler import and executes the existing measured
//! Worker V2 protocol, whose native implementation uses the LLVM target-machine
//! and LLD library APIs. The returned receipt is inert and deliberately not
//! `Clone`; it grants no publication, load, or launch authority.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_artifact_transaction::{
    CompilerModuleHandoffIdentityV1, ConsumedCompilerModuleHandoffV1,
};
use fe2o3_compiler_ffi::{
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffIdentityV2, CompilerModuleIdentityV1,
    CompilerModuleSymbolManifestIdentityV1,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion as InspectedCodeObjectVersion,
    ExplicitArgument, ExplicitValueKind, ExplicitValueType, InspectedHsaco, KernelKind,
    MAX_HSACO_BYTES,
};
use fe2o3_kernel_descriptor::{CanonicalCodeObjectDigest, CodeObjectVersion};
use fe2o3_kernel_ir::{TILED_GEMM_LDS_V1_KERNEL_ID, TILED_GEMM_LDS_V1_STATIC_LDS_BYTES};
use object::{Object, ObjectSymbol, SymbolKind, SymbolSection};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, ExactLdsGemmContractV1, ExactLdsGemmProfileIdentityV1, FinalizationError,
    FinalizedHsaco, FirstBuildWorkerV2Error, FirstBuildWorkerV2IdentityV1,
    InertDecodedWorkerExchangeV2, InertFirstBuildWorkerV2EvidenceV1,
    InspectedExactLdsGemmCompilerImportIdentityV1, InspectedExactLdsGemmCompilerImportV1,
    LinkOptionV1, LinkPlanError, LinkPlanIdentityV1, PinnedWorkerV1, WorkerExecutionLimitsV1,
    WorkerInputKindV1, WorkerMeasurementV1, WorkerOptimizationLevelV1, WorkerOptionsV1,
    WorkerOutputConstraintsV1, WorkerProtocolError, execute_reproducible_first_build_worker_v2,
    finalize_unfinalized, verify_finalized,
};

const EXACT_TARGET: &str = "gfx942:xnack-";
const EXACT_DESCRIPTOR_SYMBOL: &str = "tiled_gemm_lds_v1.kd";
const EXACT_WORKGROUP: [u32; 3] = [64, 1, 1];
const EXACT_MAX_FLAT_WORKGROUP_SIZE: u32 = 64;
const EXACT_WAVEFRONT_SIZE: u32 = 64;
const EXACT_EXPLICIT_KERNARG_BYTES: u64 = 48;
const EXACT_COMPLETE_KERNARG_BYTES: u64 = 304;
const EXACT_IMPLICIT_KERNARG_BYTES: u64 = 256;
const EXACT_KERNARG_ALIGNMENT: u64 = 8;
const RECEIPT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/EXACT-LDS-GEMM/DIRECT-LLVM-FINALIZATION/V1\0";

/// Stable identity of one complete exact-import-to-finalized-HSACO transition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedExactLdsGemmHsacoIdentityV1([u8; 32]);

impl FinalizedExactLdsGemmHsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Finalized Slice 1 HSACO with the complete one-shot compiler and worker lineage retained.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::FinalizedExactLdsGemmHsacoV1;
///
/// fn replay(value: FinalizedExactLdsGemmHsacoV1) {
///     let _second = value.clone();
/// }
/// ```
#[derive(Debug)]
pub struct FinalizedExactLdsGemmHsacoV1 {
    identity: FinalizedExactLdsGemmHsacoIdentityV1,
    transactional_handoff: CompilerModuleHandoffIdentityV1,
    request: ContentIdentityV1,
    response: ContentIdentityV1,
    raw_output: ContentIdentityV1,
    finalized_output: ContentIdentityV1,
    descriptor_digest: CanonicalCodeObjectDigest,
    import: InspectedExactLdsGemmCompilerImportV1,
    worker_evidence: InertFirstBuildWorkerV2EvidenceV1,
    finalized: FinalizedHsaco,
}

impl FinalizedExactLdsGemmHsacoV1 {
    pub const fn identity(&self) -> FinalizedExactLdsGemmHsacoIdentityV1 {
        self.identity
    }

    pub const fn import_identity(&self) -> InspectedExactLdsGemmCompilerImportIdentityV1 {
        self.import.identity()
    }

    pub const fn profile_identity(&self) -> ExactLdsGemmProfileIdentityV1 {
        self.import.contract().identity()
    }

    pub const fn compiler_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.import.handoff().identity()
    }

    pub const fn compiler_module_identity(&self) -> CompilerModuleIdentityV1 {
        self.import.handoff().module_identity()
    }

    pub const fn symbol_manifest_identity(&self) -> CompilerModuleSymbolManifestIdentityV1 {
        self.import.handoff().symbol_manifest().identity()
    }

    pub const fn transactional_handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.transactional_handoff
    }

    pub const fn worker_evidence_identity(&self) -> FirstBuildWorkerV2IdentityV1 {
        self.worker_evidence.identity()
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        self.worker_evidence.worker_measurement()
    }

    pub const fn link_plan_identity(&self) -> LinkPlanIdentityV1 {
        self.worker_evidence.link_plan_identity()
    }

    pub const fn request_identity(&self) -> ContentIdentityV1 {
        self.request
    }

    pub const fn response_identity(&self) -> ContentIdentityV1 {
        self.response
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.raw_output
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized_output
    }

    pub const fn canonical_descriptor_digest(&self) -> CanonicalCodeObjectDigest {
        self.descriptor_digest
    }

    pub const fn contract(&self) -> ExactLdsGemmContractV1 {
        self.import.contract()
    }

    pub fn exact_finalized_bytes(&self) -> &[u8] {
        self.finalized.as_bytes()
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_llvm_to_isa_refinement(&self) -> bool {
        false
    }

    pub const fn proves_verus_verification(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
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

/// Failure to turn one exact admitted import into inert finalized code-object evidence.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExactLdsGemmFinalizationErrorV1 {
    TransactionalHandoffMismatch,
    HandoffDecode(CompilerModuleHandoffErrorV2),
    LinkOption(LinkPlanError),
    OutputConstraint(WorkerProtocolError),
    FirstBuild(FirstBuildWorkerV2Error),
    WorkerExchange(WorkerProtocolError),
    WorkerLineage(&'static str),
    RawIdentityMismatch,
    RawFinalization(FinalizationError),
    FinalizedVerification(FinalizationError),
    FinalizedInspectionMismatch,
    DescriptorSourceMismatch,
    ArtifactShape(&'static str),
    Elf(object::Error),
    ElfPolicy(&'static str),
}

impl fmt::Display for ExactLdsGemmFinalizationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransactionalHandoffMismatch => formatter.write_str(
                "transactional handoff bytes differ from the consumed exact LDS GEMM import",
            ),
            Self::HandoffDecode(error) => {
                write!(formatter, "transactional handoff decode failed: {error}")
            }
            Self::LinkOption(error) => {
                write!(formatter, "fixed direct-LLVM option rejected: {error}")
            }
            Self::OutputConstraint(error) => {
                write!(
                    formatter,
                    "fixed direct-LLVM output bound rejected: {error}"
                )
            }
            Self::FirstBuild(error) => {
                write!(formatter, "measured Worker V2 finalization failed: {error}")
            }
            Self::WorkerExchange(error) => {
                write!(formatter, "sealed Worker V2 exchange rejected: {error}")
            }
            Self::WorkerLineage(field) => write!(formatter, "Worker V2 {field} identity drifted"),
            Self::RawIdentityMismatch => formatter.write_str("raw Worker output identity drifted"),
            Self::RawFinalization(error) => {
                write!(
                    formatter,
                    "raw Slice 1 descriptor inspection/finalization failed: {error}"
                )
            }
            Self::FinalizedVerification(error) => {
                write!(
                    formatter,
                    "finalized Slice 1 HSACO verification failed: {error}"
                )
            }
            Self::FinalizedInspectionMismatch => formatter.write_str(
                "independent finalized Slice 1 inspection differs from finalizer inspection",
            ),
            Self::DescriptorSourceMismatch => formatter.write_str(
                "linked zero-digest descriptor table differs from the admitted compiler source",
            ),
            Self::ArtifactShape(field) => write!(formatter, "Slice 1 artifact {field} drifted"),
            Self::Elf(error) => write!(formatter, "Slice 1 ELF symbol inspection failed: {error}"),
            Self::ElfPolicy(reason) => write!(formatter, "Slice 1 ELF policy rejected {reason}"),
        }
    }
}

impl Error for ExactLdsGemmFinalizationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HandoffDecode(error) => Some(error),
            Self::LinkOption(error) => Some(error),
            Self::OutputConstraint(error) | Self::WorkerExchange(error) => Some(error),
            Self::FirstBuild(error) => Some(error),
            Self::RawFinalization(error) | Self::FinalizedVerification(error) => Some(error),
            Self::Elf(error) => Some(error),
            Self::TransactionalHandoffMismatch
            | Self::WorkerLineage(_)
            | Self::RawIdentityMismatch
            | Self::FinalizedInspectionMismatch
            | Self::DescriptorSourceMismatch
            | Self::ArtifactShape(_)
            | Self::ElfPolicy(_) => None,
        }
    }
}

/// Consumes one admitted Slice 1 import and finalizes it through measured Worker V2.
///
/// The additional consumed handoff must contain the exact same canonical V2
/// bytes. It preserves the existing build-attempt and measured-worker protocol
/// instead of creating a second untracked compiler-module route.
pub fn finalize_exact_lds_gemm_compiler_import_v1(
    import: InspectedExactLdsGemmCompilerImportV1,
    consumed: ConsumedCompilerModuleHandoffV1,
    worker: &PinnedWorkerV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<FinalizedExactLdsGemmHsacoV1, ExactLdsGemmFinalizationErrorV1> {
    validate_transactional_handoff(&import, &consumed)?;
    let transactional_handoff = consumed.identity();
    let worker_evidence = execute_reproducible_first_build_worker_v2(
        consumed,
        worker,
        Vec::new(),
        exact_link_options()?,
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64)
            .map_err(ExactLdsGemmFinalizationErrorV1::OutputConstraint)?,
        limits,
    )
    .map_err(ExactLdsGemmFinalizationErrorV1::FirstBuild)?;

    let exchange = validate_worker_lineage(&import, worker, &worker_evidence)?;
    let raw_bytes = worker_evidence.output_bytes();
    let raw_output = ContentIdentityV1::calculate(raw_bytes);
    if raw_output != worker_evidence.output_identity() || !raw_output.matches(raw_bytes) {
        return Err(ExactLdsGemmFinalizationErrorV1::RawIdentityMismatch);
    }

    validate_elf_safety(raw_bytes)?;
    validate_exact_symbol_closure(raw_bytes)?;
    let raw = crate::inspect_unfinalized(raw_bytes)
        .map_err(ExactLdsGemmFinalizationErrorV1::RawFinalization)?;
    if raw.descriptor_table() != import.descriptor_source().table() {
        return Err(ExactLdsGemmFinalizationErrorV1::DescriptorSourceMismatch);
    }
    validate_exact_artifact_shape(raw.hsaco())?;

    let finalized = finalize_unfinalized(raw_bytes)
        .map_err(ExactLdsGemmFinalizationErrorV1::RawFinalization)?;
    let verified = verify_finalized(finalized.as_bytes())
        .map_err(ExactLdsGemmFinalizationErrorV1::FinalizedVerification)?;
    if &verified != finalized.inspection() {
        return Err(ExactLdsGemmFinalizationErrorV1::FinalizedInspectionMismatch);
    }
    validate_elf_safety(finalized.as_bytes())?;
    validate_exact_symbol_closure(finalized.as_bytes())?;
    validate_exact_artifact_shape(verified.hsaco())?;

    let finalized_output = ContentIdentityV1::calculate(finalized.as_bytes());
    let descriptor_digest = verified.digest();
    if !finalized_output.matches(finalized.as_bytes()) || descriptor_digest.as_bytes() == &[0; 32] {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "final content identity or descriptor digest",
        ));
    }
    if contains_forbidden_tool_reference(finalized.as_bytes()) {
        return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
            "a COMGR reference",
        ));
    }

    let request = ContentIdentityV1::calculate(worker_evidence.authorized_request_bytes());
    let response = ContentIdentityV1::calculate(exchange.response().canonical_bytes());
    let identity = calculate_receipt_identity(
        &import,
        transactional_handoff,
        &worker_evidence,
        request,
        response,
        raw_output,
        finalized_output,
        descriptor_digest,
    );
    Ok(FinalizedExactLdsGemmHsacoV1 {
        identity,
        transactional_handoff,
        request,
        response,
        raw_output,
        finalized_output,
        descriptor_digest,
        import,
        worker_evidence,
        finalized,
    })
}

fn exact_link_options() -> Result<Vec<LinkOptionV1>, ExactLdsGemmFinalizationErrorV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| {
        LinkOptionV1::new(name, value).map_err(ExactLdsGemmFinalizationErrorV1::LinkOption)
    })
    .collect()
}

fn validate_transactional_handoff(
    import: &InspectedExactLdsGemmCompilerImportV1,
    consumed: &ConsumedCompilerModuleHandoffV1,
) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    if consumed.bytes() != import.handoff().canonical_bytes() {
        return Err(ExactLdsGemmFinalizationErrorV1::TransactionalHandoffMismatch);
    }
    let decoded = fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(consumed.bytes())
        .map_err(ExactLdsGemmFinalizationErrorV1::HandoffDecode)?;
    if decoded.identity() != import.handoff().identity()
        || decoded.module_identity() != import.handoff().module_identity()
        || decoded.envelope().identity() != import.handoff().envelope().identity()
        || decoded.symbol_manifest().identity() != import.handoff().symbol_manifest().identity()
    {
        return Err(ExactLdsGemmFinalizationErrorV1::TransactionalHandoffMismatch);
    }
    Ok(())
}

fn validate_worker_lineage(
    import: &InspectedExactLdsGemmCompilerImportV1,
    worker: &PinnedWorkerV1,
    evidence: &InertFirstBuildWorkerV2EvidenceV1,
) -> Result<InertDecodedWorkerExchangeV2, ExactLdsGemmFinalizationErrorV1> {
    if evidence.worker_measurement() != worker.measurement() {
        return Err(ExactLdsGemmFinalizationErrorV1::WorkerLineage(
            "measurement",
        ));
    }
    if evidence.compiler_envelope_identity() != import.handoff().envelope().identity() {
        return Err(ExactLdsGemmFinalizationErrorV1::WorkerLineage(
            "compiler envelope",
        ));
    }
    if evidence.manifest_identity() != import.handoff().symbol_manifest().identity() {
        return Err(ExactLdsGemmFinalizationErrorV1::WorkerLineage(
            "symbol manifest",
        ));
    }
    if evidence.plan().target().to_string() != EXACT_TARGET
        || evidence.plan().inputs().len() != 1
        || evidence.plan().inputs()[0].identity()
            != ContentIdentityV1::calculate(import.handoff().module_bytes())
        || evidence.plan().output().identity() != evidence.output_identity()
    {
        return Err(ExactLdsGemmFinalizationErrorV1::WorkerLineage(
            "native link plan",
        ));
    }

    let response = evidence.authorized().response();
    let exchange = InertDecodedWorkerExchangeV2::decode(
        evidence.authorized_request_bytes(),
        response.canonical_bytes(),
    )
    .map_err(ExactLdsGemmFinalizationErrorV1::WorkerExchange)?;
    let request = exchange.request();
    let mismatch = |field| ExactLdsGemmFinalizationErrorV1::WorkerLineage(field);
    if request.target().to_string() != EXACT_TARGET {
        return Err(mismatch("request target"));
    }
    if request.code_object_version() != CodeObjectVersion::V6 {
        return Err(mismatch("request code-object version"));
    }
    if request.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true) {
        return Err(mismatch("request options"));
    }
    if request.llvm_build_identity() != worker.measurement().llvm_build_identity() {
        return Err(mismatch("request LLVM build identity"));
    }
    if request.worker_build_identity() != worker.measurement().worker_build_identity() {
        return Err(mismatch("request Worker build identity"));
    }
    if request.worker_executable() != worker.measurement().executable() {
        return Err(mismatch("request Worker executable"));
    }
    if request.compiler_module().kind() != WorkerInputKindV1::LlvmTextIr {
        return Err(mismatch("request compiler-module kind"));
    }
    if request.compiler_module().bytes() != import.handoff().module_bytes() {
        return Err(mismatch("request compiler-module bytes"));
    }
    if !request.external_providers().is_empty() {
        return Err(mismatch("request external providers"));
    }
    if !request.import_symbols().is_empty() {
        return Err(mismatch("request imports"));
    }
    if !request.export_symbols().is_empty() {
        return Err(mismatch("request exports"));
    }
    if request.final_symbols() != [TILED_GEMM_LDS_V1_KERNEL_ID, EXACT_DESCRIPTOR_SYMBOL] {
        return Err(mismatch("request final symbols"));
    }
    if exchange.response().device_library_provider().is_some() {
        return Err(mismatch("response device-library provider"));
    }
    Ok(exchange)
}

fn validate_exact_artifact_shape(
    hsaco: &InspectedHsaco,
) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    let observed = ObservedArtifactShapeV1::from_hsaco(hsaco)?;
    validate_observed_artifact_shape(&observed)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedArgumentShapeV1 {
    pub(crate) name: Option<String>,
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) alignment: Option<u64>,
    pub(crate) value_kind: ExplicitValueKind,
    pub(crate) value_type: Option<ExplicitValueType>,
    pub(crate) address_space: Option<ArgumentAddressSpace>,
    pub(crate) access: Option<ArgumentAccess>,
    pub(crate) actual_access: Option<ArgumentAccess>,
    pub(crate) pointee_alignment: Option<u64>,
    pub(crate) is_const: Option<bool>,
    pub(crate) is_restrict: Option<bool>,
}

impl From<&ExplicitArgument> for ObservedArgumentShapeV1 {
    fn from(argument: &ExplicitArgument) -> Self {
        Self {
            name: argument.name().map(str::to_owned),
            offset: argument.offset(),
            size: argument.size(),
            alignment: argument.alignment(),
            value_kind: argument.value_kind(),
            value_type: argument.value_type(),
            address_space: argument.address_space(),
            access: argument.access(),
            actual_access: argument.actual_access(),
            pointee_alignment: argument.pointee_alignment(),
            is_const: argument.is_const(),
            is_restrict: argument.is_restrict(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedArtifactShapeV1 {
    pub(crate) target: String,
    pub(crate) code_object_version: InspectedCodeObjectVersion,
    pub(crate) has_printf_metadata: bool,
    pub(crate) entry: String,
    pub(crate) descriptor: String,
    pub(crate) required_workgroup_size: Option<[u32; 3]>,
    pub(crate) max_flat_workgroup_size: u32,
    pub(crate) wavefront_size: u32,
    pub(crate) group_segment_fixed_size: u64,
    pub(crate) private_segment_fixed_size: u64,
    pub(crate) kernarg_segment_size: u64,
    pub(crate) kernarg_segment_alignment: u64,
    pub(crate) implicit_argument_offset: Option<u64>,
    pub(crate) implicit_argument_size: u64,
    pub(crate) kind: KernelKind,
    pub(crate) uses_dynamic_stack: bool,
    pub(crate) sgpr_spill_count: Option<u32>,
    pub(crate) vgpr_spill_count: Option<u32>,
    pub(crate) explicit_arguments: Vec<ObservedArgumentShapeV1>,
}

impl ObservedArtifactShapeV1 {
    fn from_hsaco(hsaco: &InspectedHsaco) -> Result<Self, ExactLdsGemmFinalizationErrorV1> {
        let [kernel] = hsaco.kernels() else {
            return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
                "kernel cardinality",
            ));
        };
        Ok(Self {
            target: hsaco.target().to_string(),
            code_object_version: hsaco.code_object_version(),
            has_printf_metadata: hsaco.has_printf_metadata(),
            entry: kernel.name().to_owned(),
            descriptor: kernel.symbol().to_owned(),
            required_workgroup_size: kernel.required_workgroup_size(),
            max_flat_workgroup_size: kernel.max_flat_workgroup_size(),
            wavefront_size: kernel.wavefront_size(),
            group_segment_fixed_size: kernel.group_segment_fixed_size(),
            private_segment_fixed_size: kernel.private_segment_fixed_size(),
            kernarg_segment_size: kernel.kernarg_segment_size(),
            kernarg_segment_alignment: kernel.kernarg_segment_alignment(),
            implicit_argument_offset: kernel.implicit_argument_offset(),
            implicit_argument_size: kernel.implicit_argument_size(),
            kind: kernel.kind(),
            uses_dynamic_stack: kernel.uses_dynamic_stack(),
            sgpr_spill_count: kernel.sgpr_spill_count(),
            vgpr_spill_count: kernel.vgpr_spill_count(),
            explicit_arguments: kernel
                .explicit_arguments()
                .iter()
                .map(ObservedArgumentShapeV1::from)
                .collect(),
        })
    }
}

pub(crate) fn validate_observed_artifact_shape(
    observed: &ObservedArtifactShapeV1,
) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    if observed.target != EXACT_TARGET {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape("target"));
    }
    if observed.code_object_version != InspectedCodeObjectVersion::V6 {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "code-object version",
        ));
    }
    if observed.has_printf_metadata {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "printf metadata",
        ));
    }
    if observed.entry != TILED_GEMM_LDS_V1_KERNEL_ID
        || observed.descriptor != EXACT_DESCRIPTOR_SYMBOL
    {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "entry/descriptor symbols",
        ));
    }
    if observed.required_workgroup_size != Some(EXACT_WORKGROUP)
        || observed.max_flat_workgroup_size != EXACT_MAX_FLAT_WORKGROUP_SIZE
        || observed.wavefront_size != EXACT_WAVEFRONT_SIZE
    {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "WG64 launch metadata",
        ));
    }
    if observed.group_segment_fixed_size != u64::from(TILED_GEMM_LDS_V1_STATIC_LDS_BYTES)
        || observed.private_segment_fixed_size != 0
        || observed.uses_dynamic_stack
        || observed.sgpr_spill_count != Some(0)
        || observed.vgpr_spill_count != Some(0)
    {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "LDS/private/spill resources",
        ));
    }
    if observed.kernarg_segment_size != EXACT_COMPLETE_KERNARG_BYTES
        || observed.kernarg_segment_alignment != EXACT_KERNARG_ALIGNMENT
        || observed.implicit_argument_offset != Some(EXACT_EXPLICIT_KERNARG_BYTES)
        || observed.implicit_argument_size != EXACT_IMPLICIT_KERNARG_BYTES
    {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "kernarg span",
        ));
    }
    if observed.kind != KernelKind::Normal || observed.explicit_arguments.len() != 6 {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "kernel kind or explicit argument count",
        ));
    }
    for role in 0..3 {
        validate_pointer_argument(&observed.explicit_arguments[role * 2], role)?;
        validate_length_argument(&observed.explicit_arguments[role * 2 + 1], role)?;
    }
    Ok(())
}

fn validate_pointer_argument(
    argument: &ObservedArgumentShapeV1,
    role: usize,
) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    let expected_type = if role < 2 {
        ExplicitValueType::U16
    } else {
        ExplicitValueType::F32
    };
    let expected_access = if role < 2 {
        ArgumentAccess::ReadOnly
    } else {
        ArgumentAccess::ReadWrite
    };
    let expected_pointee_alignment = if role < 2 { 2 } else { 4 };
    let expected_restrict = role == 2;
    if argument.name.as_deref() != Some(&format!("arg{role}.data"))
        || argument.offset != (role as u64) * 16
        || argument.size != 8
        || argument.value_kind != ExplicitValueKind::GlobalBuffer
        || argument.address_space != Some(ArgumentAddressSpace::Global)
    {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "explicit pointer ABI",
        ));
    }
    // Upstream LLVM may omit these schema-level annotations. Source type,
    // access, alignment, and alias facts remain bound by the exact canonical
    // LLVM body and descriptor; emitted metadata may agree or narrow access,
    // but it may not contradict those sources.
    if argument.alignment.is_some_and(|value| value != 8)
        || argument
            .value_type
            .is_some_and(|value| value != expected_type)
        || argument
            .access
            .is_some_and(|value| value != expected_access)
        || argument
            .actual_access
            .is_some_and(|value| !actual_access_is_subset(expected_access, value))
        || argument
            .pointee_alignment
            .is_some_and(|value| value != expected_pointee_alignment)
        || argument.is_const.is_some_and(|value| value != (role < 2))
        || argument
            .is_restrict
            .is_some_and(|value| value != expected_restrict)
    {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "explicit pointer annotations",
        ));
    }
    Ok(())
}

fn validate_length_argument(
    argument: &ObservedArgumentShapeV1,
    role: usize,
) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    if argument.name.as_deref() != Some(&format!("arg{role}.len"))
        || argument.offset != (role as u64) * 16 + 8
        || argument.size != 8
        || argument.value_kind != ExplicitValueKind::ByValue
        || argument.address_space.is_some()
        || argument.access.is_some()
        || argument.actual_access.is_some()
        || argument.pointee_alignment.is_some()
    {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "explicit length ABI",
        ));
    }
    // `.align`, deprecated `.value_type`, and false qualifiers are optional
    // upstream metadata. The exact canonical LLVM body and descriptor bind
    // the source facts even when LLVM does not synthesize physical metadata.
    if argument.alignment.is_some_and(|value| value != 8)
        || argument
            .value_type
            .is_some_and(|value| value != ExplicitValueType::U64)
        || argument.is_const == Some(true)
        || argument.is_restrict == Some(true)
    {
        return Err(ExactLdsGemmFinalizationErrorV1::ArtifactShape(
            "explicit length annotations",
        ));
    }
    Ok(())
}

fn actual_access_is_subset(contract: ArgumentAccess, actual: ArgumentAccess) -> bool {
    matches!(
        (contract, actual),
        (ArgumentAccess::ReadOnly, ArgumentAccess::ReadOnly)
            | (ArgumentAccess::WriteOnly, ArgumentAccess::WriteOnly)
            | (
                ArgumentAccess::ReadWrite,
                ArgumentAccess::ReadOnly | ArgumentAccess::WriteOnly | ArgumentAccess::ReadWrite
            )
    )
}

fn validate_exact_symbol_closure(bytes: &[u8]) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    let file = object::File::parse(bytes).map_err(ExactLdsGemmFinalizationErrorV1::Elf)?;
    let mut observed = BTreeSet::new();
    inspect_symbol_table(file.symbols(), &mut observed)?;
    inspect_symbol_table(file.dynamic_symbols(), &mut observed)?;
    for required in [TILED_GEMM_LDS_V1_KERNEL_ID, EXACT_DESCRIPTOR_SYMBOL] {
        if !observed.contains(required) {
            return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
                "a missing required definition",
            ));
        }
    }
    Ok(())
}

fn inspect_symbol_table<'data, I>(
    symbols: I,
    observed: &mut BTreeSet<String>,
) -> Result<(), ExactLdsGemmFinalizationErrorV1>
where
    I: Iterator<Item = object::Symbol<'data, 'data>>,
{
    let mut table_definitions = BTreeSet::new();
    for symbol in symbols {
        let name = symbol
            .name()
            .map_err(ExactLdsGemmFinalizationErrorV1::Elf)?;
        if name.is_empty() || matches!(symbol.kind(), SymbolKind::Section | SymbolKind::File) {
            continue;
        }
        if symbol.is_undefined() {
            return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
                "an undefined static or dynamic symbol",
            ));
        }
        if symbol.is_definition() && !allowed_defined_symbol(name, &symbol) {
            return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
                "an unexpected static or dynamic definition",
            ));
        }
        if symbol.is_definition() {
            if !table_definitions.insert(name.to_owned()) {
                return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
                    "a duplicate static or dynamic definition",
                ));
            }
            observed.insert(name.to_owned());
        }
    }
    Ok(())
}

fn allowed_defined_symbol(name: &str, symbol: &object::Symbol<'_, '_>) -> bool {
    match name {
        TILED_GEMM_LDS_V1_KERNEL_ID => return symbol.kind() == SymbolKind::Text,
        EXACT_DESCRIPTOR_SYMBOL => return symbol.kind() == SymbolKind::Data,
        "_DYNAMIC" => {
            return symbol.kind() == SymbolKind::Unknown
                && matches!(symbol.section(), SymbolSection::Section(_));
        }
        _ => {}
    }
    if let Some(suffix) = name.strip_prefix("tiled_gemm_lds_v1.") {
        return symbol.kind() == SymbolKind::Unknown
            && symbol.section() == SymbolSection::Absolute
            && matches!(
                suffix,
                "private_seg_size"
                    | "num_vgpr"
                    | "num_agpr"
                    | "numbered_sgpr"
                    | "uses_vcc"
                    | "uses_flat_scratch"
                    | "has_dyn_sized_stack"
                    | "has_recursion"
            );
    }
    symbol.kind() == SymbolKind::Data
        && matches!(
            name,
            "__fe2o3_lds_tiled_gemm_lds_v1_6" | "__fe2o3_lds_tiled_gemm_lds_v1_7"
        )
}

fn validate_elf_safety(bytes: &[u8]) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    const ELF64_HEADER_BYTES: usize = 64;
    const ELF64_SECTION_HEADER_BYTES: usize = 64;
    const SHT_RELA: u32 = 4;
    const SHT_DYNAMIC: u32 = 6;
    const SHT_REL: u32 = 9;
    const ELF64_DYNAMIC_BYTES: usize = 16;
    const DT_NULL: u64 = 0;
    const DT_NEEDED: u64 = 1;

    if bytes.len() < ELF64_HEADER_BYTES
        || bytes.get(..6) != Some(b"\x7fELF\x02\x01")
        || read_u16(bytes, 52)? != ELF64_HEADER_BYTES as u16
        || read_u16(bytes, 58)? != ELF64_SECTION_HEADER_BYTES as u16
    {
        return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
            "a noncanonical ELF64 little-endian header",
        ));
    }
    let table_offset = usize_from_u64(read_u64(bytes, 40)?)?;
    let section_count = usize::from(read_u16(bytes, 60)?);
    if section_count == 0 {
        return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
            "a missing section table",
        ));
    }
    let table_bytes = section_count
        .checked_mul(ELF64_SECTION_HEADER_BYTES)
        .and_then(|size| table_offset.checked_add(size))
        .filter(|end| *end <= bytes.len())
        .ok_or(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
            "an out-of-bounds section table",
        ))?;
    let _ = table_bytes;

    for index in 0..section_count {
        let header = table_offset + index * ELF64_SECTION_HEADER_BYTES;
        let section_type = read_u32(bytes, header + 4)?;
        if matches!(section_type, SHT_REL | SHT_RELA) {
            return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
                "a residual relocation section",
            ));
        }
        if section_type != SHT_DYNAMIC {
            continue;
        }
        let offset = usize_from_u64(read_u64(bytes, header + 24)?)?;
        let size = usize_from_u64(read_u64(bytes, header + 32)?)?;
        let entry_size = usize_from_u64(read_u64(bytes, header + 56)?)?;
        if entry_size != ELF64_DYNAMIC_BYTES || size % entry_size != 0 {
            return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
                "a malformed dynamic section",
            ));
        }
        let end = offset
            .checked_add(size)
            .filter(|end| *end <= bytes.len())
            .ok_or(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
                "an out-of-bounds dynamic section",
            ))?;
        let mut saw_null = false;
        for entry in bytes[offset..end].chunks_exact(entry_size) {
            let tag = u64::from_le_bytes(entry[..8].try_into().expect("fixed dynamic tag"));
            if tag == DT_NEEDED {
                return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
                    "a DT_NEEDED dependency",
                ));
            }
            if tag == DT_NULL {
                saw_null = true;
                break;
            }
        }
        if !saw_null {
            return Err(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
                "an unterminated dynamic section",
            ));
        }
    }
    Ok(())
}

fn contains_forbidden_tool_reference(bytes: &[u8]) -> bool {
    [b"amd_comgr".as_slice(), b"libamd_comgr".as_slice()]
        .iter()
        .any(|needle| bytes.windows(needle.len()).any(|window| window == *needle))
}

#[allow(clippy::too_many_arguments)]
fn calculate_receipt_identity(
    import: &InspectedExactLdsGemmCompilerImportV1,
    transactional_handoff: CompilerModuleHandoffIdentityV1,
    worker: &InertFirstBuildWorkerV2EvidenceV1,
    request: ContentIdentityV1,
    response: ContentIdentityV1,
    raw_output: ContentIdentityV1,
    finalized_output: ContentIdentityV1,
    descriptor_digest: CanonicalCodeObjectDigest,
) -> FinalizedExactLdsGemmHsacoIdentityV1 {
    let handoff = import.handoff();
    let mut digest = Sha256::new();
    hash_field(&mut digest, RECEIPT_IDENTITY_DOMAIN_V1);
    hash_field(&mut digest, import.identity().as_bytes());
    hash_field(&mut digest, import.contract().identity().as_bytes());
    hash_sized_identity(
        &mut digest,
        handoff.identity().sha256(),
        handoff.identity().byte_len(),
    );
    hash_sized_identity(
        &mut digest,
        handoff.module_identity().sha256(),
        handoff.module_identity().byte_len(),
    );
    hash_field(&mut digest, &handoff.envelope().identity().as_bytes());
    let manifest = handoff.symbol_manifest().identity();
    hash_sized_identity(&mut digest, manifest.sha256(), manifest.byte_len());
    hash_field(&mut digest, transactional_handoff.as_bytes());
    hash_worker_measurement(&mut digest, worker.worker_measurement());
    hash_field(&mut digest, worker.identity().as_bytes());
    hash_field(&mut digest, worker.link_plan_identity().as_bytes());
    for identity in [request, response, raw_output, finalized_output] {
        hash_content_identity(&mut digest, identity);
    }
    hash_field(&mut digest, descriptor_digest.as_bytes());
    hash_field(&mut digest, EXACT_TARGET.as_bytes());
    hash_field(&mut digest, &[6]);
    hash_field(&mut digest, TILED_GEMM_LDS_V1_KERNEL_ID.as_bytes());
    hash_field(&mut digest, EXACT_DESCRIPTOR_SYMBOL.as_bytes());
    FinalizedExactLdsGemmHsacoIdentityV1(digest.finalize().into())
}

fn hash_worker_measurement(digest: &mut Sha256, measurement: &WorkerMeasurementV1) {
    hash_content_identity(digest, measurement.executable());
    hash_field(digest, measurement.worker_build_identity().as_bytes());
    hash_field(digest, measurement.llvm_build_identity().as_bytes());
}

fn hash_content_identity(digest: &mut Sha256, identity: ContentIdentityV1) {
    hash_sized_identity(digest, identity.sha256(), identity.byte_len());
}

fn hash_sized_identity(digest: &mut Sha256, hash: &[u8; 32], byte_len: u64) {
    hash_field(digest, hash);
    hash_field(digest, &byte_len.to_le_bytes());
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn usize_from_u64(value: u64) -> Result<usize, ExactLdsGemmFinalizationErrorV1> {
    usize::try_from(value).map_err(|_| {
        ExactLdsGemmFinalizationErrorV1::ElfPolicy("an ELF offset outside the host address space")
    })
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ExactLdsGemmFinalizationErrorV1> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
            "a truncated ELF field",
        ))?;
    Ok(u16::from_le_bytes(value.try_into().expect("fixed u16")))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ExactLdsGemmFinalizationErrorV1> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
            "a truncated ELF field",
        ))?;
    Ok(u32::from_le_bytes(value.try_into().expect("fixed u32")))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ExactLdsGemmFinalizationErrorV1> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or(ExactLdsGemmFinalizationErrorV1::ElfPolicy(
            "a truncated ELF field",
        ))?;
    Ok(u64::from_le_bytes(value.try_into().expect("fixed u64")))
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn exact_observed_artifact_shape_for_test() -> ObservedArtifactShapeV1 {
    let mut arguments = Vec::new();
    for role in 0..3 {
        let (value_type, access, pointee_alignment, is_const) = if role < 2 {
            (ExplicitValueType::U16, ArgumentAccess::ReadOnly, 2, true)
        } else {
            (ExplicitValueType::F32, ArgumentAccess::ReadWrite, 4, false)
        };
        arguments.push(ObservedArgumentShapeV1 {
            name: Some(format!("arg{role}.data")),
            offset: role * 16,
            size: 8,
            alignment: Some(8),
            value_kind: ExplicitValueKind::GlobalBuffer,
            value_type: Some(value_type),
            address_space: Some(ArgumentAddressSpace::Global),
            access: Some(access),
            actual_access: Some(access),
            pointee_alignment: Some(pointee_alignment),
            is_const: Some(is_const),
            is_restrict: Some(role == 2),
        });
        arguments.push(ObservedArgumentShapeV1 {
            name: Some(format!("arg{role}.len")),
            offset: role * 16 + 8,
            size: 8,
            alignment: Some(8),
            value_kind: ExplicitValueKind::ByValue,
            value_type: Some(ExplicitValueType::U64),
            address_space: None,
            access: None,
            actual_access: None,
            pointee_alignment: None,
            is_const: None,
            is_restrict: None,
        });
    }
    ObservedArtifactShapeV1 {
        target: EXACT_TARGET.to_owned(),
        code_object_version: InspectedCodeObjectVersion::V6,
        has_printf_metadata: false,
        entry: TILED_GEMM_LDS_V1_KERNEL_ID.to_owned(),
        descriptor: EXACT_DESCRIPTOR_SYMBOL.to_owned(),
        required_workgroup_size: Some(EXACT_WORKGROUP),
        max_flat_workgroup_size: EXACT_MAX_FLAT_WORKGROUP_SIZE,
        wavefront_size: EXACT_WAVEFRONT_SIZE,
        group_segment_fixed_size: u64::from(TILED_GEMM_LDS_V1_STATIC_LDS_BYTES),
        private_segment_fixed_size: 0,
        kernarg_segment_size: EXACT_COMPLETE_KERNARG_BYTES,
        kernarg_segment_alignment: EXACT_KERNARG_ALIGNMENT,
        implicit_argument_offset: Some(EXACT_EXPLICIT_KERNARG_BYTES),
        implicit_argument_size: EXACT_IMPLICIT_KERNARG_BYTES,
        kind: KernelKind::Normal,
        uses_dynamic_stack: false,
        sgpr_spill_count: Some(0),
        vgpr_spill_count: Some(0),
        explicit_arguments: arguments,
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn validate_elf_safety_for_test(
    bytes: &[u8],
) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    validate_elf_safety(bytes)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn validate_exact_symbol_closure_for_test(
    bytes: &[u8],
) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    validate_exact_symbol_closure(bytes)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn validate_transactional_handoff_for_test(
    import: &InspectedExactLdsGemmCompilerImportV1,
    consumed: &ConsumedCompilerModuleHandoffV1,
) -> Result<(), ExactLdsGemmFinalizationErrorV1> {
    validate_transactional_handoff(import, consumed)
}
