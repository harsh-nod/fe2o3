#![forbid(unsafe_code)]
#![doc = "Exact scalar-add handoff V2 to sealed Worker V2 bridge."]

use core::fmt;

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffIdentityV1, ConsumedCompilerModuleHandoffV1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffIdentityV2, CompilerModuleHandoffV2,
    CompilerModuleKindV1, CompilerModuleSymbolManifestErrorV1,
    CompilerModuleSymbolManifestIdentityV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    CompilerHandoffWorkerRequestV2, ContentIdentityV1, LinkInputKindClosureV1, LinkPlanIdentityV1,
    MultiInputLinkPlanV1, PinnedWorkerV1, WorkerInputKindV1, WorkerMeasurementV1,
    WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerOutputConstraintsV1,
    WorkerRequestConstructionError, WorkerRequestV2,
    construct_worker_request_v2_from_consumed_handoff,
};
use fe2o3_llvm_handoff::{
    AddressSpaceV1, BinaryOperationV2, BlockIdV2, CallingConventionV2, EvidenceV2,
    FloatBinaryOperationV2, FunctionAttributeV1, FunctionAttributeV2, FunctionIdV2, FunctionKindV2,
    Gfx942HandoffV2, Gfx942TargetPolicyV1, HandoffIdentityV2, InstructionKindV2, KernelValueTypeV1,
    ModuleFlagV1, ObligationKindV1, ReturnTypeV2, ScalarTypeV1, TerminatorV2, TypedValueV2,
    ValueIdV2, ValueTypeV2, WorkgroupSizeRangeV1,
};
use fe2o3_llvm_text::{LlvmAssemblySha256V2, SerializeErrorV2, serialize_gfx942_handoff_v2};
use sha2::{Digest as _, Sha256};

/// The only admitted kernel entry symbol.
pub const SCALAR_ADD_KERNEL_SYMBOL_V1: &str = "scalar_add";
/// The descriptor symbol derived for the admitted kernel.
pub const SCALAR_ADD_KERNEL_DESCRIPTOR_SYMBOL_V1: &str = "scalar_add.kd";
/// The only admitted compiler and worker target.
pub const SCALAR_ADD_DEVICE_TARGET_V1: &str = "gfx942:xnack-";

const INPUT_PARAMETER_NAME: &str = "input";
const OUTPUT_PARAMETER_NAME: &str = "output";
const ADDEND_PARAMETER_NAME: &str = "addend";

/// One typed portion of the closed scalar-add handoff profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ScalarAddProfileFieldV1 {
    /// The complete gfx942 target-machine policy.
    Target,
    /// The V1 module flags and named metadata.
    ModuleMetadata,
    /// The empty device-library provider closure.
    ProviderClosure,
    /// The single canonical source origin and preservation obligations.
    SourceEvidence,
    /// The exact single-kernel inventory and symbol.
    KernelInventory,
    /// The exact scalar-add kernel ABI.
    KernelAbi,
    /// The exact strict floating-point and workgroup attributes.
    KernelAttributes,
    /// The absence of globals, intrinsics, and helper functions.
    CompilerClosure,
    /// The exact load/add/store/return body.
    KernelBody,
    /// The evidence attached to the function and every instruction.
    GraphEvidence,
}

/// Failure to prepare the exact scalar-add compiler handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrepareScalarAddWorkerV2ErrorV1 {
    /// A typed handoff field differs from the closed profile.
    Profile(ScalarAddProfileFieldV1),
    /// The typed LLVM serializer rejected the handoff.
    Serialize(SerializeErrorV2),
    /// The serializer artifact did not retain the exact source identity marker.
    MissingEmbeddedSourceIdentity,
    /// The fixed empty compiler-FFI envelope could not be built.
    CompilerEnvelope(CompilerFfiEnvelopeError),
    /// The fixed two-symbol manifest could not be built.
    SymbolManifest(CompilerModuleSymbolManifestErrorV1),
    /// The compiler module handoff could not bind the serialized bytes.
    CompilerHandoff(CompilerModuleHandoffErrorV2),
}

impl fmt::Display for PrepareScalarAddWorkerV2ErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(field) => write!(formatter, "scalar-add profile substituted {field:?}"),
            Self::Serialize(error) => write!(formatter, "LLVM serialization failed: {error}"),
            Self::MissingEmbeddedSourceIdentity => formatter
                .write_str("serialized LLVM assembly omitted the exact handoff identity marker"),
            Self::CompilerEnvelope(error) => {
                write!(
                    formatter,
                    "compiler FFI envelope construction failed: {error}"
                )
            }
            Self::SymbolManifest(error) => {
                write!(
                    formatter,
                    "compiler symbol manifest construction failed: {error}"
                )
            }
            Self::CompilerHandoff(error) => {
                write!(
                    formatter,
                    "compiler module handoff construction failed: {error}"
                )
            }
        }
    }
}

impl std::error::Error for PrepareScalarAddWorkerV2ErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialize(error) => Some(error),
            Self::CompilerEnvelope(error) => Some(error),
            Self::SymbolManifest(error) => Some(error),
            Self::CompilerHandoff(error) => Some(error),
            Self::Profile(_) | Self::MissingEmbeddedSourceIdentity => None,
        }
    }
}

/// Opaque preparation of one exact scalar-add compiler handoff.
///
/// The value owns the canonical compiler handoff and binds it to the typed
/// source identity and serializer-owned LLVM assembly identity. It is inert
/// and confers no compiler, worker, link, publication, load, or launch power.
#[derive(Debug)]
pub struct PreparedScalarAddWorkerV2 {
    source_identity: HandoffIdentityV2,
    assembly_sha256: LlvmAssemblySha256V2,
    assembly_len: u64,
    compiler_handoff_identity: CompilerModuleHandoffIdentityV2,
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    compiler_handoff: CompilerModuleHandoffV2,
}

impl PreparedScalarAddWorkerV2 {
    /// Returns the canonical typed source handoff identity.
    pub const fn source_identity(&self) -> HandoffIdentityV2 {
        self.source_identity
    }

    /// Returns the SHA-256 digest of the exact serializer bytes.
    pub const fn assembly_sha256(&self) -> LlvmAssemblySha256V2 {
        self.assembly_sha256
    }

    /// Returns the exact serializer byte length.
    pub const fn assembly_len(&self) -> u64 {
        self.assembly_len
    }

    /// Returns the serializer bytes as the worker protocol content identity.
    pub fn assembly_content_identity(&self) -> ContentIdentityV1 {
        ContentIdentityV1::from_parts(*self.assembly_sha256.as_bytes(), self.assembly_len)
    }

    /// Returns the identity of the complete canonical compiler handoff.
    pub const fn compiler_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.compiler_handoff_identity
    }

    /// Returns the derived two-symbol manifest identity.
    pub const fn manifest_identity(&self) -> CompilerModuleSymbolManifestIdentityV1 {
        self.manifest_identity
    }

    /// Returns the canonical compiler handoff for attempt-scoped publication.
    pub const fn compiler_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.compiler_handoff
    }

    /// This preparation does not authenticate a compiler executable.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    /// This preparation grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// This preparation grants no worker authority.
    pub const fn grants_worker_authority(&self) -> bool {
        false
    }

    /// This preparation grants no link authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// This preparation grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// This preparation grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// This preparation grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Validates and serializes one exact scalar-add typed handoff.
///
/// No symbol, module byte, target, code-object version, provider, or build
/// identity is supplied by the caller. The compiler handoff is derived only
/// after the complete typed source profile has been checked.
pub fn prepare_scalar_add_worker_v2(
    handoff: Gfx942HandoffV2,
) -> Result<PreparedScalarAddWorkerV2, PrepareScalarAddWorkerV2ErrorV1> {
    validate_scalar_add_profile(&handoff).map_err(PrepareScalarAddWorkerV2ErrorV1::Profile)?;

    let source_identity = handoff.identity();
    let assembly = serialize_gfx942_handoff_v2(&handoff)
        .map_err(PrepareScalarAddWorkerV2ErrorV1::Serialize)?;
    if assembly.source_identity() != source_identity
        || !assembly.has_embedded_source_identity()
        || !has_exact_embedded_source_identity(assembly.as_bytes(), source_identity)
    {
        return Err(PrepareScalarAddWorkerV2ErrorV1::MissingEmbeddedSourceIdentity);
    }

    let target = exact_target();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(PrepareScalarAddWorkerV2ErrorV1::CompilerEnvelope)?;
    let manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            SCALAR_ADD_KERNEL_SYMBOL_V1,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            SCALAR_ADD_KERNEL_DESCRIPTOR_SYMBOL_V1,
        ),
    ])
    .map_err(PrepareScalarAddWorkerV2ErrorV1::SymbolManifest)?;
    let manifest_identity = manifest.identity();
    let compiler_handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        manifest,
        assembly.as_bytes(),
    )
    .map_err(PrepareScalarAddWorkerV2ErrorV1::CompilerHandoff)?;
    let compiler_handoff_identity = compiler_handoff.identity();

    Ok(PreparedScalarAddWorkerV2 {
        source_identity,
        assembly_sha256: assembly.sha256(),
        assembly_len: assembly.len() as u64,
        compiler_handoff_identity,
        manifest_identity,
        compiler_handoff,
    })
}

/// One exact request field checked by the scalar bridge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ScalarAddWorkerRequestFieldV1 {
    /// Attempt-scoped handoff bytes and SHA-256 identity.
    ConsumedHandoff,
    /// Exact gfx942:xnack- link-plan target.
    PlanTarget,
    /// The plan's sole serializer-derived input identity.
    PlanInputs,
    /// The exact COV6/O2/strip-debug/verify-each option set.
    PlanOptions,
    /// The complete input-to-output provenance graph.
    PlanProvenance,
    /// The plan-bound sole LLVM-text input kind.
    InputKinds,
    /// The exact planned output length.
    Output,
    /// The retained transaction attempt.
    Attempt,
    /// The retained transaction handoff identity.
    TransactionHandoffIdentity,
    /// The derived manifest identity.
    ManifestIdentity,
    /// The derived empty compiler-FFI envelope identity.
    CompilerEnvelopeIdentity,
    /// The exact LLVM text compiler-module bytes and content identity.
    CompilerModule,
    /// The empty external-provider closure.
    ExternalProviders,
    /// The derived entry/descriptor symbol closure.
    Symbols,
    /// The exact sealed request target.
    SealedTarget,
    /// The exact sealed request code-object version.
    SealedCodeObjectVersion,
    /// The exact sealed request optimization policy.
    SealedOptions,
    /// The actual pinned worker executable and build measurement.
    WorkerMeasurement,
}

/// Failure to construct and recheck one sealed scalar-add Worker V2 request.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ConstructScalarAddWorkerRequestV2ErrorV1 {
    /// A caller-carried or sealed field differed from the prepared binding.
    Binding(ScalarAddWorkerRequestFieldV1),
    /// The existing sealed constructor rejected the coherent inputs.
    RequestConstruction(WorkerRequestConstructionError),
}

impl fmt::Display for ConstructScalarAddWorkerRequestV2ErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(field) => write!(formatter, "scalar-add request substituted {field:?}"),
            Self::RequestConstruction(error) => {
                write!(
                    formatter,
                    "sealed Worker V2 request construction failed: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ConstructScalarAddWorkerRequestV2ErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestConstruction(error) => Some(error),
            Self::Binding(_) => None,
        }
    }
}

/// Opaque inert evidence retaining the source-to-request identity chain.
#[derive(Debug)]
pub struct InertScalarAddWorkerRequestV2 {
    prepared: PreparedScalarAddWorkerV2,
    transaction_handoff_identity: CompilerModuleHandoffIdentityV1,
    plan_identity: LinkPlanIdentityV1,
    worker_measurement: WorkerMeasurementV1,
    request: CompilerHandoffWorkerRequestV2,
}

impl InertScalarAddWorkerRequestV2 {
    /// Returns the typed source handoff identity.
    pub const fn source_identity(&self) -> HandoffIdentityV2 {
        self.prepared.source_identity()
    }

    /// Returns the exact LLVM assembly SHA-256.
    pub const fn assembly_sha256(&self) -> LlvmAssemblySha256V2 {
        self.prepared.assembly_sha256()
    }

    /// Returns the exact LLVM assembly byte length.
    pub const fn assembly_len(&self) -> u64 {
        self.prepared.assembly_len()
    }

    /// Returns the complete compiler handoff identity.
    pub const fn compiler_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.prepared.compiler_handoff_identity()
    }

    /// Returns the attempt-scoped transaction identity of the handoff bytes.
    pub const fn transaction_handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.transaction_handoff_identity
    }

    /// Returns the exact manifest identity.
    pub const fn manifest_identity(&self) -> CompilerModuleSymbolManifestIdentityV1 {
        self.prepared.manifest_identity()
    }

    /// Returns the exact link-plan identity bound into request derivation.
    pub const fn plan_identity(&self) -> LinkPlanIdentityV1 {
        self.plan_identity
    }

    /// Returns the actual pinned worker measurement bound into the sealed request.
    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker_measurement
    }

    /// Returns the attempt that carried the exact compiler handoff bytes.
    pub const fn attempt(&self) -> BuildAttempt {
        self.request.attempt()
    }

    /// Returns the sealed request's stable request ID.
    pub const fn request_id(&self) -> &[u8; 32] {
        self.request.sealed_request().request_id()
    }

    /// Returns the canonical sealed request identity.
    pub const fn request_identity(&self) -> &[u8; 32] {
        self.request.sealed_request().identity()
    }

    /// Returns the compiler-handoff wrapper required by measured execution.
    pub const fn compiler_handoff_request(&self) -> &CompilerHandoffWorkerRequestV2 {
        &self.request
    }

    /// Returns the sealed request for inert inspection.
    pub const fn sealed_request(&self) -> &WorkerRequestV2 {
        self.request.sealed_request()
    }

    /// This record grants no worker execution authority.
    pub const fn grants_worker_authority(&self) -> bool {
        false
    }

    /// This record grants no link authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// This record grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// This record grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// This record grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Constructs and rechecks one exact sealed scalar-add Worker V2 request.
///
/// The consumed handoff is compared with the complete prepared canonical bytes
/// and its attempt-scoped SHA-256 before it is moved into the existing sealed
/// constructor. The only compiler module, symbols, target, code-object version,
/// providers, and worker build fields passed to that constructor are derived
/// from the prepared handoff and the actual [`PinnedWorkerV1`] measurement.
#[allow(clippy::too_many_arguments)]
pub fn construct_scalar_add_worker_request_v2(
    prepared: PreparedScalarAddWorkerV2,
    plan: &MultiInputLinkPlanV1,
    worker: &PinnedWorkerV1,
    consumed: ConsumedCompilerModuleHandoffV1,
    input_kinds: &LinkInputKindClosureV1,
    output: WorkerOutputConstraintsV1,
) -> Result<InertScalarAddWorkerRequestV2, ConstructScalarAddWorkerRequestV2ErrorV1> {
    let expected_transaction_identity = transaction_identity(&prepared.compiler_handoff);
    if consumed.bytes() != prepared.compiler_handoff.canonical_bytes()
        || consumed.identity() != expected_transaction_identity
    {
        return Err(ConstructScalarAddWorkerRequestV2ErrorV1::Binding(
            ScalarAddWorkerRequestFieldV1::ConsumedHandoff,
        ));
    }

    validate_exact_plan(&prepared, plan, input_kinds, &output)
        .map_err(ConstructScalarAddWorkerRequestV2ErrorV1::Binding)?;

    let attempt = consumed.attempt();
    let measurement = worker.measurement().clone();
    let request = construct_worker_request_v2_from_consumed_handoff(
        plan,
        &measurement,
        consumed,
        Vec::new(),
        input_kinds,
        output.clone(),
    )
    .map_err(ConstructScalarAddWorkerRequestV2ErrorV1::RequestConstruction)?;

    validate_sealed_request(
        &prepared,
        plan,
        attempt,
        expected_transaction_identity,
        &measurement,
        &output,
        &request,
    )
    .map_err(ConstructScalarAddWorkerRequestV2ErrorV1::Binding)?;

    Ok(InertScalarAddWorkerRequestV2 {
        prepared,
        transaction_handoff_identity: expected_transaction_identity,
        plan_identity: plan.identity(),
        worker_measurement: measurement,
        request,
    })
}

fn validate_scalar_add_profile(handoff: &Gfx942HandoffV2) -> Result<(), ScalarAddProfileFieldV1> {
    let base = handoff.base();
    if base.target() != &Gfx942TargetPolicyV1::canonical() {
        return Err(ScalarAddProfileFieldV1::Target);
    }
    if base.module().flags() != [ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2]
        || !base.module().named_metadata().is_empty()
    {
        return Err(ScalarAddProfileFieldV1::ModuleMetadata);
    }
    if !base.module().device_libraries().is_empty() {
        return Err(ScalarAddProfileFieldV1::ProviderClosure);
    }

    let [kernel] = base.kernels() else {
        return Err(ScalarAddProfileFieldV1::KernelInventory);
    };
    if kernel.symbol() != SCALAR_ADD_KERNEL_SYMBOL_V1 {
        return Err(ScalarAddProfileFieldV1::KernelInventory);
    }
    validate_kernel_abi(kernel)?;
    if kernel.function_attributes() != exact_function_attributes_v1() {
        return Err(ScalarAddProfileFieldV1::KernelAttributes);
    }
    let expected_evidence = validate_source_evidence(handoff)?;

    let module = handoff.module();
    if module.flags() != base.module().flags()
        || module.named_metadata() != base.module().named_metadata()
    {
        return Err(ScalarAddProfileFieldV1::ModuleMetadata);
    }
    if !module.globals().is_empty() || !module.intrinsics().is_empty() {
        return Err(ScalarAddProfileFieldV1::CompilerClosure);
    }
    let [function] = module.functions() else {
        return Err(ScalarAddProfileFieldV1::CompilerClosure);
    };
    if function.id() != FunctionIdV2::new(0)
        || function.symbol() != SCALAR_ADD_KERNEL_SYMBOL_V1
        || function.kind() != FunctionKindV2::Kernel
        || function.calling_convention() != CallingConventionV2::AmdGpuKernel
        || function.return_type() != ReturnTypeV2::Void
    {
        return Err(ScalarAddProfileFieldV1::KernelInventory);
    }
    validate_function_abi(function)?;
    if function.attributes() != exact_function_attributes_v2() {
        return Err(ScalarAddProfileFieldV1::KernelAttributes);
    }
    if function.evidence() != &expected_evidence {
        return Err(ScalarAddProfileFieldV1::GraphEvidence);
    }
    validate_function_body(function, &expected_evidence)
}

fn validate_kernel_abi(
    kernel: &fe2o3_llvm_handoff::KernelEntryV1,
) -> Result<(), ScalarAddProfileFieldV1> {
    let [input, output, addend] = kernel.parameters() else {
        return Err(ScalarAddProfileFieldV1::KernelAbi);
    };
    let pointer = KernelValueTypeV1::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    if input.name() != INPUT_PARAMETER_NAME
        || input.value_type() != pointer
        || !input.attributes().is_empty()
        || output.name() != OUTPUT_PARAMETER_NAME
        || output.value_type() != pointer
        || !output.attributes().is_empty()
        || addend.name() != ADDEND_PARAMETER_NAME
        || addend.value_type() != KernelValueTypeV1::Scalar(ScalarTypeV1::F32)
        || !addend.attributes().is_empty()
    {
        return Err(ScalarAddProfileFieldV1::KernelAbi);
    }
    Ok(())
}

fn validate_function_abi(
    function: &fe2o3_llvm_handoff::FunctionV2,
) -> Result<(), ScalarAddProfileFieldV1> {
    let [input, output, addend] = function.parameters() else {
        return Err(ScalarAddProfileFieldV1::KernelAbi);
    };
    let pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let scalar = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    if input.value() != TypedValueV2::new(ValueIdV2::new(0), pointer)
        || input.name() != INPUT_PARAMETER_NAME
        || !input.attributes().is_empty()
        || output.value() != TypedValueV2::new(ValueIdV2::new(1), pointer)
        || output.name() != OUTPUT_PARAMETER_NAME
        || !output.attributes().is_empty()
        || addend.value() != TypedValueV2::new(ValueIdV2::new(2), scalar)
        || addend.name() != ADDEND_PARAMETER_NAME
        || !addend.attributes().is_empty()
    {
        return Err(ScalarAddProfileFieldV1::KernelAbi);
    }
    Ok(())
}

fn validate_source_evidence(
    handoff: &Gfx942HandoffV2,
) -> Result<EvidenceV2, ScalarAddProfileFieldV1> {
    let base = handoff.base();
    let [origin] = base.origins() else {
        return Err(ScalarAddProfileFieldV1::SourceEvidence);
    };
    if origin.kind() != fe2o3_llvm_handoff::OriginKindV1::AmdgcnIr
        || origin.span().is_some()
        || base.kernels()[0].origin() != origin.identity()
        || base.obligations().len() != REQUIRED_OBLIGATIONS.len()
    {
        return Err(ScalarAddProfileFieldV1::SourceEvidence);
    }
    for required in REQUIRED_OBLIGATIONS {
        let mut matches = base
            .obligations()
            .iter()
            .copied()
            .filter(|obligation| obligation.kind() == required);
        let Some(obligation) = matches.next() else {
            return Err(ScalarAddProfileFieldV1::SourceEvidence);
        };
        if matches.next().is_some()
            || obligation.origin() != origin.identity()
            || obligation.subject() != expected_obligation_subject(base, required)
        {
            return Err(ScalarAddProfileFieldV1::SourceEvidence);
        }
    }
    EvidenceV2::new(
        origin.identity(),
        base.obligations()
            .iter()
            .map(|obligation| obligation.identity())
            .collect(),
    )
    .map_err(|_| ScalarAddProfileFieldV1::SourceEvidence)
}

const REQUIRED_OBLIGATIONS: [ObligationKindV1; 7] = [
    ObligationKindV1::PreserveKernelAbi,
    ObligationKindV1::PreserveAddressSpaces,
    ObligationKindV1::PreserveTargetFeatures,
    ObligationKindV1::PreserveCallingConvention,
    ObligationKindV1::PreserveFunctionAttributes,
    ObligationKindV1::PreserveModuleMetadata,
    ObligationKindV1::MaintainOriginCoverage,
];

fn expected_obligation_subject(
    base: &fe2o3_llvm_handoff::Gfx942HandoffV1,
    kind: ObligationKindV1,
) -> fe2o3_llvm_handoff::IdentityV1 {
    match kind {
        ObligationKindV1::PreserveKernelAbi | ObligationKindV1::MaintainOriginCoverage => {
            base.stage_identities().semantic()
        }
        ObligationKindV1::PreserveAddressSpaces
        | ObligationKindV1::PreserveTargetFeatures
        | ObligationKindV1::PreserveCallingConvention
        | ObligationKindV1::PreserveFunctionAttributes
        | ObligationKindV1::PreserveModuleMetadata => base.stage_identities().target_plan(),
        ObligationKindV1::AuthenticateDeviceLibraries => base.stage_identities().schedule(),
    }
}

fn validate_function_body(
    function: &fe2o3_llvm_handoff::FunctionV2,
    expected_evidence: &EvidenceV2,
) -> Result<(), ScalarAddProfileFieldV1> {
    let [block] = function.blocks() else {
        return Err(ScalarAddProfileFieldV1::KernelBody);
    };
    if function.entry() != BlockIdV2::new(0)
        || block.id() != BlockIdV2::new(0)
        || block.terminator() != &TerminatorV2::Return(None)
    {
        return Err(ScalarAddProfileFieldV1::KernelBody);
    }
    let [load, add, store] = block.instructions() else {
        return Err(ScalarAddProfileFieldV1::KernelBody);
    };
    let scalar = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let exact_body = load.result() == Some(TypedValueV2::new(ValueIdV2::new(3), scalar))
        && load.kind()
            == &InstructionKindV2::Load {
                pointer: ValueIdV2::new(0),
                value_type: ScalarTypeV1::F32,
                alignment: 4,
            }
        && add.result() == Some(TypedValueV2::new(ValueIdV2::new(4), scalar))
        && add.kind()
            == &InstructionKindV2::Binary {
                operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Add),
                left: ValueIdV2::new(3),
                right: ValueIdV2::new(2),
            }
        && store.result().is_none()
        && store.kind()
            == &InstructionKindV2::Store {
                pointer: ValueIdV2::new(1),
                value: ValueIdV2::new(4),
                value_type: ScalarTypeV1::F32,
                alignment: 4,
            };
    if !exact_body {
        return Err(ScalarAddProfileFieldV1::KernelBody);
    }
    if [load, add, store]
        .into_iter()
        .any(|instruction| instruction.evidence() != expected_evidence)
    {
        return Err(ScalarAddProfileFieldV1::GraphEvidence);
    }
    Ok(())
}

fn exact_function_attributes_v1() -> Vec<FunctionAttributeV1> {
    FunctionAttributeV1::gfx942_kernel_defaults(
        WorkgroupSizeRangeV1::new(1, 64).expect("the fixed workgroup range is valid"),
    )
}

fn exact_function_attributes_v2() -> Vec<FunctionAttributeV2> {
    exact_function_attributes_v1()
        .into_iter()
        .map(FunctionAttributeV2::from)
        .collect()
}

fn exact_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(SCALAR_ADD_DEVICE_TARGET_V1)
        .expect("the fixed scalar-add target is valid")
}

fn transaction_identity(handoff: &CompilerModuleHandoffV2) -> CompilerModuleHandoffIdentityV1 {
    CompilerModuleHandoffIdentityV1::from_bytes(Sha256::digest(handoff.canonical_bytes()).into())
}

fn has_exact_embedded_source_identity(bytes: &[u8], expected: HandoffIdentityV2) -> bool {
    const NAMED_METADATA: &[u8] = b"!fe2o3.handoff.identity";
    const NAMED_PREFIX: &[u8] = b"!fe2o3.handoff.identity = !{!";
    const NODE_PAYLOAD_PREFIX: &[u8] = b"!{!\"sha256:";
    const NODE_PAYLOAD_SUFFIX: &[u8] = b"\"}";

    let mut named_node = None;
    let mut sha256_payload_count = 0_usize;
    let lines = bytes.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    for line in &lines {
        if contains_bytes(line, b"sha256:") {
            sha256_payload_count += 1;
        }
        if contains_bytes(line, NAMED_METADATA) {
            if named_node.is_some() || !line.starts_with(NAMED_PREFIX) || !line.ends_with(b"}") {
                return false;
            }
            let decimal = &line[NAMED_PREFIX.len()..line.len() - 1];
            let Some(node) = parse_canonical_decimal(decimal) else {
                return false;
            };
            named_node = Some(node);
        }
    }
    let Some(named_node) = named_node else {
        return false;
    };
    if sha256_payload_count != 1 {
        return false;
    }

    let node_prefix = format!("!{named_node} = ");
    let mut definitions = lines
        .iter()
        .copied()
        .filter(|line| line.starts_with(node_prefix.as_bytes()));
    let Some(definition) = definitions.next() else {
        return false;
    };
    if definitions.next().is_some() {
        return false;
    }
    let payload = &definition[node_prefix.len()..];
    if !payload.starts_with(NODE_PAYLOAD_PREFIX)
        || !payload.ends_with(NODE_PAYLOAD_SUFFIX)
        || payload.len() != NODE_PAYLOAD_PREFIX.len() + 64 + NODE_PAYLOAD_SUFFIX.len()
    {
        return false;
    }
    let encoded = &payload[NODE_PAYLOAD_PREFIX.len()..NODE_PAYLOAD_PREFIX.len() + 64];
    decode_lower_hex_identity(encoded).is_some_and(|identity| identity == *expected.as_bytes())
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn parse_canonical_decimal(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty()
        || !bytes.iter().all(u8::is_ascii_digit)
        || (bytes.len() > 1 && bytes[0] == b'0')
    {
        return None;
    }
    bytes.iter().try_fold(0_usize, |value, byte| {
        value
            .checked_mul(10)?
            .checked_add(usize::from(*byte - b'0'))
    })
}

fn decode_lower_hex_identity(encoded: &[u8]) -> Option<[u8; 32]> {
    let encoded: &[u8; 64] = encoded.try_into().ok()?;
    let mut identity = [0_u8; 32];
    for (output, pair) in identity.iter_mut().zip(encoded.chunks_exact(2)) {
        *output = decode_lower_hex_nibble(pair[0])?
            .checked_mul(16)?
            .checked_add(decode_lower_hex_nibble(pair[1])?)?;
    }
    Some(identity)
}

const fn decode_lower_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn validate_exact_plan(
    prepared: &PreparedScalarAddWorkerV2,
    plan: &MultiInputLinkPlanV1,
    input_kinds: &LinkInputKindClosureV1,
    output: &WorkerOutputConstraintsV1,
) -> Result<(), ScalarAddWorkerRequestFieldV1> {
    if plan.target() != exact_target() || plan.output().target() != exact_target() {
        return Err(ScalarAddWorkerRequestFieldV1::PlanTarget);
    }
    let [input] = plan.inputs() else {
        return Err(ScalarAddWorkerRequestFieldV1::PlanInputs);
    };
    let input_identity = prepared.assembly_content_identity();
    if input.identity() != input_identity || input.target() != exact_target() {
        return Err(ScalarAddWorkerRequestFieldV1::PlanInputs);
    }
    let options = plan.options();
    if options.len() != 4
        || options[0].name() != "code-object-version"
        || options[0].value() != "6"
        || options[1].name() != "opt-level"
        || options[1].value() != "2"
        || options[2].name() != "strip-debug"
        || options[2].value() != "true"
        || options[3].name() != "verify-each"
        || options[3].value() != "true"
    {
        return Err(ScalarAddWorkerRequestFieldV1::PlanOptions);
    }
    let output_identity = plan.output().identity();
    if plan.provenance().len() != 2
        || !plan
            .provenance()
            .iter()
            .any(|node| node.identity() == input_identity && node.parents().is_empty())
        || !plan
            .provenance()
            .iter()
            .any(|node| node.identity() == output_identity && node.parents() == [input_identity])
    {
        return Err(ScalarAddWorkerRequestFieldV1::PlanProvenance);
    }
    if input_kinds.plan_identity() != plan.identity()
        || input_kinds.kinds() != [WorkerInputKindV1::LlvmTextIr]
    {
        return Err(ScalarAddWorkerRequestFieldV1::InputKinds);
    }
    if output.max_bytes() != output_identity.byte_len() {
        return Err(ScalarAddWorkerRequestFieldV1::Output);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_sealed_request(
    prepared: &PreparedScalarAddWorkerV2,
    plan: &MultiInputLinkPlanV1,
    attempt: BuildAttempt,
    transaction_handoff_identity: CompilerModuleHandoffIdentityV1,
    measurement: &WorkerMeasurementV1,
    output: &WorkerOutputConstraintsV1,
    constructed: &CompilerHandoffWorkerRequestV2,
) -> Result<(), ScalarAddWorkerRequestFieldV1> {
    if constructed.attempt() != attempt {
        return Err(ScalarAddWorkerRequestFieldV1::Attempt);
    }
    if constructed.handoff_identity() != transaction_handoff_identity {
        return Err(ScalarAddWorkerRequestFieldV1::TransactionHandoffIdentity);
    }
    if constructed.manifest_identity() != prepared.manifest_identity {
        return Err(ScalarAddWorkerRequestFieldV1::ManifestIdentity);
    }
    let request = constructed.sealed_request();
    if request.compiler_envelope_identity().as_bytes()
        != prepared.compiler_handoff.envelope().identity().as_bytes()
    {
        return Err(ScalarAddWorkerRequestFieldV1::CompilerEnvelopeIdentity);
    }
    let module = request.compiler_module();
    if module.kind() != WorkerInputKindV1::LlvmTextIr
        || module.bytes() != prepared.compiler_handoff.module_bytes()
        || module.identity() != prepared.assembly_content_identity()
        || !module.identity().matches(module.bytes())
    {
        return Err(ScalarAddWorkerRequestFieldV1::CompilerModule);
    }
    if !request.external_providers().is_empty() {
        return Err(ScalarAddWorkerRequestFieldV1::ExternalProviders);
    }
    if !request.import_symbols().is_empty()
        || !request.export_symbols().is_empty()
        || !request.final_symbols().iter().map(String::as_str).eq([
            SCALAR_ADD_KERNEL_SYMBOL_V1,
            SCALAR_ADD_KERNEL_DESCRIPTOR_SYMBOL_V1,
        ])
    {
        return Err(ScalarAddWorkerRequestFieldV1::Symbols);
    }
    if request.target() != exact_target() || request.target() != plan.target() {
        return Err(ScalarAddWorkerRequestFieldV1::SealedTarget);
    }
    if request.code_object_version() != CodeObjectVersion::V6 {
        return Err(ScalarAddWorkerRequestFieldV1::SealedCodeObjectVersion);
    }
    if request.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true) {
        return Err(ScalarAddWorkerRequestFieldV1::SealedOptions);
    }
    if request.output_constraints() != output {
        return Err(ScalarAddWorkerRequestFieldV1::Output);
    }
    if request.worker_executable() != measurement.executable()
        || request.worker_build_identity() != measurement.worker_build_identity()
        || request.llvm_build_identity() != measurement.llvm_build_identity()
    {
        return Err(ScalarAddWorkerRequestFieldV1::WorkerMeasurement);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use fe2o3_amdgcn_pliron_llvm::{ScalarKernelModuleV1, lower_scalar_kernel_v2};
    use fe2o3_llvm_handoff::{IdentityV1, StageIdentitiesV1};

    use super::*;

    fn serialized_fixture() -> (Vec<u8>, HandoffIdentityV2) {
        let request = ScalarKernelModuleV1::canonical(
            "scalar_module",
            SCALAR_ADD_KERNEL_SYMBOL_V1,
            IdentityV1::new([1; 32]).expect("the identity is nonzero"),
            StageIdentitiesV1::new([2; 32], [3; 32], [4; 32])
                .expect("the stage identities are nonzero"),
        );
        let handoff = lower_scalar_kernel_v2(&request).expect("the fixture lowers");
        let source_identity = handoff.identity();
        let assembly = serialize_gfx942_handoff_v2(&handoff).expect("the fixture serializes");
        (assembly.as_bytes().to_vec(), source_identity)
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("the serializer emits the expected marker fragment")
    }

    #[test]
    fn source_identity_parser_requires_the_exact_serializer_representation() {
        let (bytes, source_identity) = serialized_fixture();
        assert!(has_exact_embedded_source_identity(&bytes, source_identity));

        let mut uppercase_hex = bytes.clone();
        let digest = find_bytes(&uppercase_hex, b"sha256:") + b"sha256:".len();
        uppercase_hex[digest] = b'A';
        assert!(!has_exact_embedded_source_identity(
            &uppercase_hex,
            source_identity
        ));

        let mut other_lowercase_identity = bytes.clone();
        other_lowercase_identity[digest] = if other_lowercase_identity[digest] == b'0' {
            b'1'
        } else {
            b'0'
        };
        assert!(!has_exact_embedded_source_identity(
            &other_lowercase_identity,
            source_identity
        ));

        let mut malformed_named_node = bytes.clone();
        let named_wrapper = find_bytes(&malformed_named_node, b"!fe2o3.handoff.identity = !{!")
            + b"!fe2o3.handoff.identity = ".len();
        malformed_named_node[named_wrapper] = b'[';
        assert!(!has_exact_embedded_source_identity(
            &malformed_named_node,
            source_identity
        ));

        let mut malformed_md_string = bytes.clone();
        let md_string = find_bytes(&malformed_md_string, b"!{!\"sha256:") + b"!{!".len();
        malformed_md_string[md_string] = b'\'';
        assert!(!has_exact_embedded_source_identity(
            &malformed_md_string,
            source_identity
        ));

        let named_line = bytes
            .split(|byte| *byte == b'\n')
            .find(|line| line.starts_with(b"!fe2o3.handoff.identity = "))
            .expect("the named metadata line exists");
        let mut duplicate_named_metadata = bytes.clone();
        duplicate_named_metadata.extend_from_slice(named_line);
        duplicate_named_metadata.push(b'\n');
        assert!(!has_exact_embedded_source_identity(
            &duplicate_named_metadata,
            source_identity
        ));
    }
}
