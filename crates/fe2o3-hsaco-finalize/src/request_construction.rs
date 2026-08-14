//! Closed construction of direct LLVM worker requests from validated plans.

use std::fmt;

use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffIdentityV1, ConsumedCompilerModuleHandoffV1,
};
use fe2o3_compiler_ffi::{
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestIdentityV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1,
};

use crate::{
    ContentIdentityV1, LinkOptionV1, LinkPlanIdentityV1, MultiInputLinkPlanV1,
    StagedCompilerFfiEnvelopeV1, WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1,
    WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerOutputConstraintsV1, WorkerProtocolError,
    WorkerRequestV1, WorkerRequestV2,
    worker_protocol::validate_symbols,
    worker_protocol_v2::{SealedWorkerRequestV2Parts, WorkerCompilerFfiEnvelopeIdentityV2},
};

const INPUT_KIND_CLOSURE_DOMAIN_V1: &[u8] = b"FE2O3/DEVICE-LINK-INPUT-KIND-CLOSURE/V1\0";
const SYMBOL_CLOSURE_DOMAIN_V1: &[u8] = b"FE2O3/DEVICE-LINK-SYMBOL-CLOSURE/V1\0";
const PLAN_REQUEST_DOMAIN_V1: &[u8] = b"FE2O3/PLAN-BOUND-WORKER-REQUEST/V1\0";
#[allow(dead_code)] // V2 stays unconnected until compiler-owned provenance exists.
const PLAN_REQUEST_DOMAIN_V2: &[u8] = b"FE2O3/PLAN-BOUND-WORKER-REQUEST/V2\0";
const FIRST_BUILD_REQUEST_DOMAIN_V2: &[u8] =
    b"FE2O3/FIRST-BUILD-COMPILER-HANDOFF-WORKER-REQUEST/V2\0";

/// Exact compiler-module bytes retained without accepting a caller-supplied digest.
///
/// This neutral witness exists until G3 provides its own sealed artifact type. It
/// authenticates only byte/kind consistency, not compiler origin, and can only be
/// consumed by the sealed Worker V2 constructor.
#[derive(Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) struct ExactCompilerModuleArtifactV1 {
    input: WorkerInputV1,
}

#[allow(dead_code)]
impl ExactCompilerModuleArtifactV1 {
    pub const fn identity(&self) -> ContentIdentityV1 {
        self.input.identity()
    }

    fn into_input(self) -> WorkerInputV1 {
        self.input
    }
}

/// Seals exact compiler-module bytes into a non-forgeable-by-digest witness.
#[allow(dead_code)]
pub(crate) fn stage_exact_compiler_module_artifact_v1(
    kind: WorkerInputKindV1,
    bytes: Vec<u8>,
) -> Result<ExactCompilerModuleArtifactV1, WorkerProtocolError> {
    Ok(ExactCompilerModuleArtifactV1 {
        input: WorkerInputV1::new(kind, bytes)?,
    })
}

/// Stable identity of a canonical required/import/export symbol closure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LinkSymbolClosureIdentityV1([u8; 32]);

impl LinkSymbolClosureIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable identity of the plan-bound input-role sequence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LinkInputKindClosureIdentityV1([u8; 32]);

impl LinkInputKindClosureIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Independent source of truth for each canonical link input's file kind.
///
/// `MultiInputLinkPlanV1` predates typed inputs, so changing its V1 canonical
/// bytes would be a wire-format break. This companion closure binds one kind to
/// each plan input in canonical identity order. It is inert data and grants no
/// link, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkInputKindClosureV1 {
    plan_identity: LinkPlanIdentityV1,
    kinds: Vec<WorkerInputKindV1>,
    identity: LinkInputKindClosureIdentityV1,
}

impl LinkInputKindClosureV1 {
    pub fn new(
        plan: &MultiInputLinkPlanV1,
        kinds: Vec<WorkerInputKindV1>,
    ) -> Result<Self, WorkerRequestConstructionError> {
        if kinds.len() != plan.inputs().len() {
            return Err(WorkerRequestConstructionError::InputKindCountMismatch {
                planned: plan.inputs().len(),
                declared: kinds.len(),
            });
        }
        let plan_identity = plan.identity();
        let identity = calculate_input_kind_closure_identity(plan, &kinds);
        Ok(Self {
            plan_identity,
            kinds,
            identity,
        })
    }

    pub const fn plan_identity(&self) -> LinkPlanIdentityV1 {
        self.plan_identity
    }

    pub fn kinds(&self) -> &[WorkerInputKindV1] {
        &self.kinds
    }

    pub const fn identity(&self) -> LinkInputKindClosureIdentityV1 {
        self.identity
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Exact externally visible symbol closure expected from a native device link.
///
/// `required_symbols` is the complete final defined-symbol set. Imports and
/// exports are disjoint directional annotations and must each be subsets of
/// that set. The closure is inert data and grants no link, load, or launch
/// authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkSymbolClosureV1 {
    required_symbols: Vec<String>,
    import_symbols: Vec<String>,
    export_symbols: Vec<String>,
    identity: LinkSymbolClosureIdentityV1,
}

impl LinkSymbolClosureV1 {
    pub fn new(
        required_symbols: Vec<String>,
        import_symbols: Vec<String>,
        export_symbols: Vec<String>,
    ) -> Result<Self, WorkerRequestConstructionError> {
        if required_symbols.is_empty() {
            return Err(WorkerRequestConstructionError::EmptySymbolClosure);
        }
        validate_symbols(&required_symbols)
            .map_err(WorkerRequestConstructionError::InvalidRequiredSymbols)?;
        validate_symbols(&import_symbols)
            .map_err(WorkerRequestConstructionError::InvalidImportSymbols)?;
        validate_symbols(&export_symbols)
            .map_err(WorkerRequestConstructionError::InvalidExportSymbols)?;

        for symbol in &import_symbols {
            if required_symbols.binary_search(symbol).is_err() {
                return Err(WorkerRequestConstructionError::UnreferencedImport(
                    symbol.clone(),
                ));
            }
            if export_symbols.binary_search(symbol).is_ok() {
                return Err(WorkerRequestConstructionError::ConflictingSymbolRole(
                    symbol.clone(),
                ));
            }
        }
        for symbol in &export_symbols {
            if required_symbols.binary_search(symbol).is_err() {
                return Err(WorkerRequestConstructionError::UnreferencedExport(
                    symbol.clone(),
                ));
            }
        }

        let identity =
            calculate_closure_identity(&required_symbols, &import_symbols, &export_symbols);
        Ok(Self {
            required_symbols,
            import_symbols,
            export_symbols,
            identity,
        })
    }

    pub fn required_symbols(&self) -> &[String] {
        &self.required_symbols
    }

    pub fn import_symbols(&self) -> &[String] {
        &self.import_symbols
    }

    pub fn export_symbols(&self) -> &[String] {
        &self.export_symbols
    }

    pub const fn identity(&self) -> LinkSymbolClosureIdentityV1 {
        self.identity
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

pub(crate) fn derive_manifest_symbol_closure(
    manifest: &CompilerModuleSymbolManifestV1,
    import_symbols: Vec<String>,
    export_symbols: Vec<String>,
) -> Result<LinkSymbolClosureV1, WorkerRequestConstructionError> {
    use CompilerModuleSymbolRoleV1 as Role;

    let manifest_imports = manifest
        .symbols(Role::UnresolvedExternalImport)
        .collect::<Vec<_>>();
    if let Some(symbol) = first_directional_mismatch(&manifest_imports, &import_symbols) {
        return Err(WorkerRequestConstructionError::CompilerEnvelopeImportRoleMismatch(symbol));
    }

    let manifest_exports = manifest.symbols(Role::DeviceFfiExport).collect::<Vec<_>>();
    if let Some(symbol) = first_directional_mismatch(&manifest_exports, &export_symbols) {
        return Err(WorkerRequestConstructionError::CompilerEnvelopeExportRoleMismatch(symbol));
    }

    let mut required_symbols = Vec::new();
    for role in [
        Role::KernelEntry,
        Role::KernelDescriptor,
        Role::DeviceFfiExport,
        Role::UnresolvedExternalImport,
    ] {
        required_symbols.extend(manifest.symbols(role).map(str::to_owned));
    }
    required_symbols.sort();

    LinkSymbolClosureV1::new(required_symbols, import_symbols, export_symbols)
}

fn first_directional_mismatch(manifest: &[&str], envelope: &[String]) -> Option<String> {
    for (manifest_symbol, envelope_symbol) in manifest.iter().zip(envelope) {
        if *manifest_symbol != envelope_symbol {
            return Some((*manifest_symbol).to_owned());
        }
    }
    manifest
        .get(envelope.len())
        .map(|symbol| (*symbol).to_owned())
        .or_else(|| envelope.get(manifest.len()).cloned())
}

/// Builds one deterministic worker request from a fully validated link plan.
///
/// The caller must supply inputs in the plan's canonical identity order. The
/// request target, code-object version, structured worker options, exact input
/// bytes, symbol closure, and output bound are checked before a request can be
/// returned for execution.
#[allow(clippy::too_many_arguments)]
pub fn construct_worker_request_v1(
    plan: &MultiInputLinkPlanV1,
    llvm_build_identity: impl Into<String>,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    options: WorkerOptionsV1,
    inputs: Vec<WorkerInputV1>,
    input_kinds: &LinkInputKindClosureV1,
    symbols: &LinkSymbolClosureV1,
    output: WorkerOutputConstraintsV1,
) -> Result<WorkerRequestV1, WorkerRequestConstructionError> {
    if target != plan.target() {
        return Err(WorkerRequestConstructionError::TargetMismatch);
    }
    let (planned_code_object_version, planned_options) = decode_plan_options(plan)?;
    if code_object_version != planned_code_object_version {
        return Err(WorkerRequestConstructionError::CodeObjectVersionMismatch {
            planned: planned_code_object_version,
            requested: code_object_version,
        });
    }
    if options != planned_options {
        return Err(WorkerRequestConstructionError::OptionsMismatch {
            planned: planned_options,
            requested: options,
        });
    }
    validate_inputs(plan, input_kinds, &inputs)?;

    let expected_output_bytes = plan.output().identity().byte_len();
    if output.max_bytes() != expected_output_bytes {
        return Err(WorkerRequestConstructionError::OutputBoundMismatch {
            planned: expected_output_bytes,
            requested: output.max_bytes(),
        });
    }

    let llvm_build_identity = llvm_build_identity.into();
    let request_id = calculate_request_id(
        plan,
        &llvm_build_identity,
        target,
        code_object_version,
        options,
        &inputs,
        input_kinds,
        symbols,
        &output,
    );
    if request_id == [0; 32] {
        return Err(WorkerRequestConstructionError::ReservedRequestId);
    }

    WorkerRequestV1::new(
        request_id,
        llvm_build_identity,
        target,
        code_object_version,
        options,
        inputs,
        symbols.required_symbols.clone(),
        symbols.required_symbols.clone(),
        output,
    )
    .map_err(WorkerRequestConstructionError::WorkerProtocol)
}

/// Builds one compiler-FFI-aware V2 request through the sealed path.
///
/// The complete staged compiler envelope and exact compiler-module witness are
/// consumed. The module plus every external provider must exactly cover the
/// plan inputs. Import and export symbols are derived from the retained
/// envelope, while the complete final symbol closure is derived from the
/// compiler manifest. No caller-provided symbol list or V1 handoff participates
/// in this construction.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn construct_worker_request_v2(
    plan: &MultiInputLinkPlanV1,
    measurement: &WorkerMeasurementV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    options: WorkerOptionsV1,
    staged_envelope: StagedCompilerFfiEnvelopeV1,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    compiler_module: ExactCompilerModuleArtifactV1,
    external_providers: Vec<WorkerInputV1>,
    input_kinds: &LinkInputKindClosureV1,
    output: WorkerOutputConstraintsV1,
) -> Result<WorkerRequestV2, WorkerRequestConstructionError> {
    if target != plan.target() {
        return Err(WorkerRequestConstructionError::TargetMismatch);
    }
    let envelope = staged_envelope.inspection();
    if envelope.target() != target {
        return Err(WorkerRequestConstructionError::CompilerEnvelopeTargetMismatch);
    }
    if envelope.code_object_version() != code_object_version {
        return Err(WorkerRequestConstructionError::CompilerEnvelopeCodeObjectVersionMismatch);
    }
    let (import_symbols, export_symbols) = staged_envelope.directional_symbols();
    let symbols = derive_manifest_symbol_closure(&symbol_manifest, import_symbols, export_symbols)?;
    let manifest_identity = symbol_manifest.identity();
    let (planned_code_object_version, planned_options) = decode_plan_options(plan)?;
    if code_object_version != planned_code_object_version {
        return Err(WorkerRequestConstructionError::CodeObjectVersionMismatch {
            planned: planned_code_object_version,
            requested: code_object_version,
        });
    }
    if options != planned_options {
        return Err(WorkerRequestConstructionError::OptionsMismatch {
            planned: planned_options,
            requested: options,
        });
    }
    let expected_output_bytes = plan.output().identity().byte_len();
    if output.max_bytes() != expected_output_bytes {
        return Err(WorkerRequestConstructionError::OutputBoundMismatch {
            planned: expected_output_bytes,
            requested: output.max_bytes(),
        });
    }

    let compiler_module = compiler_module.into_input();
    let mut all_inputs = external_providers.clone();
    all_inputs.push(compiler_module.clone());
    all_inputs.sort_by_key(|input| (input.identity(), input.kind()));
    validate_inputs(plan, input_kinds, &all_inputs)?;

    let request_id = calculate_request_id_v2(
        plan,
        measurement,
        staged_envelope.identity().as_bytes(),
        envelope.envelope_identity().as_bytes(),
        manifest_identity,
        target,
        code_object_version,
        options,
        &compiler_module,
        &external_providers,
        input_kinds,
        symbols.import_symbols(),
        symbols.export_symbols(),
        symbols.required_symbols(),
        &output,
    );
    if request_id == [0; 32] {
        return Err(WorkerRequestConstructionError::ReservedRequestId);
    }

    WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
        request_id,
        llvm_build_identity: measurement.llvm_build_identity().to_owned(),
        worker_build_identity: measurement.worker_build_identity().to_owned(),
        worker_executable: measurement.executable(),
        target,
        code_object_version,
        options,
        compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(
            envelope.envelope_identity(),
        ),
        compiler_module,
        external_providers,
        import_symbols: symbols.import_symbols().to_vec(),
        export_symbols: symbols.export_symbols().to_vec(),
        final_symbols: symbols.required_symbols().to_vec(),
        output,
    })
    .map_err(WorkerRequestConstructionError::WorkerProtocol)
}

/// A Worker V2 request whose compiler module crossed one exact build-attempt handoff.
///
/// This value has no public constructor. It retains the attempt and complete handoff identity so
/// later evidence cannot accidentally bind the sealed request to a different build generation.
/// It remains inert and grants no publication, loading, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct CompilerHandoffWorkerRequestV2 {
    attempt: BuildAttempt,
    handoff_identity: CompilerModuleHandoffIdentityV1,
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    request: WorkerRequestV2,
}

impl CompilerHandoffWorkerRequestV2 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.handoff_identity
    }

    pub const fn manifest_identity(&self) -> CompilerModuleSymbolManifestIdentityV1 {
        self.manifest_identity
    }

    pub const fn sealed_request(&self) -> &WorkerRequestV2 {
        &self.request
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

/// Constructs a sealed Worker V2 request before the exact output identity is known.
///
/// This path is crate-private because its output ceiling is not backed by a link plan. It exists
/// only to obtain inert first-build bytes from the same compiler-aware worker path that will
/// immediately replay those bytes under an exact-output plan.
pub(crate) fn construct_first_build_worker_request_v2_from_consumed_handoff(
    measurement: &WorkerMeasurementV1,
    consumed: ConsumedCompilerModuleHandoffV1,
    mut external_providers: Vec<WorkerInputV1>,
    options: WorkerOptionsV1,
    output: WorkerOutputConstraintsV1,
) -> Result<CompilerHandoffWorkerRequestV2, WorkerRequestConstructionError> {
    let attempt = consumed.attempt();
    let handoff_identity = consumed.identity();
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes())
        .map_err(WorkerRequestConstructionError::CompilerModuleHandoff)?;
    let parts = handoff.into_parts();
    let target = parts.target();
    let code_object_version = parts.code_object_version();
    let (envelope, symbol_manifest, module) = parts.into_envelope_manifest_and_module();
    let manifest_identity = symbol_manifest.identity();
    let directional_symbols = envelope.directional_symbols();
    let symbols = derive_manifest_symbol_closure(
        &symbol_manifest,
        directional_symbols.imports().map(str::to_owned).collect(),
        directional_symbols.exports().map(str::to_owned).collect(),
    )?;
    let kind = match module.kind() {
        CompilerModuleKindV1::LlvmTextIr => WorkerInputKindV1::LlvmTextIr,
        CompilerModuleKindV1::LlvmBitcode => WorkerInputKindV1::LlvmBitcode,
    };
    let compiler_module = WorkerInputV1::new(kind, module.into_bytes())
        .map_err(WorkerRequestConstructionError::WorkerProtocol)?;
    external_providers.sort_by_key(|input| (input.identity(), input.kind()));
    let request_id = calculate_first_build_request_id_v2(
        attempt,
        handoff_identity,
        measurement,
        envelope.identity().as_bytes(),
        manifest_identity,
        target,
        code_object_version,
        options,
        &compiler_module,
        &external_providers,
        symbols.import_symbols(),
        symbols.export_symbols(),
        symbols.required_symbols(),
        &output,
    );
    if request_id == [0; 32] {
        return Err(WorkerRequestConstructionError::ReservedRequestId);
    }
    let request = WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
        request_id,
        llvm_build_identity: measurement.llvm_build_identity().to_owned(),
        worker_build_identity: measurement.worker_build_identity().to_owned(),
        worker_executable: measurement.executable(),
        target,
        code_object_version,
        options,
        compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(
            envelope.identity(),
        ),
        compiler_module,
        external_providers,
        import_symbols: symbols.import_symbols().to_vec(),
        export_symbols: symbols.export_symbols().to_vec(),
        final_symbols: symbols.required_symbols().to_vec(),
        output,
    })
    .map_err(WorkerRequestConstructionError::WorkerProtocol)?;
    Ok(CompilerHandoffWorkerRequestV2 {
        attempt,
        handoff_identity,
        manifest_identity,
        request,
    })
}

/// Constructs the only public Worker V2 request from a consumed, attempt-scoped handoff.
///
/// The complete canonical handoff is decoded again after one-shot consumption. Target,
/// code-object version, envelope, symbol manifest, module kind, exact module bytes, plan inputs,
/// worker measurement, and output bound must all agree. The final symbol closure is derived only
/// from the manifest. A decode or construction failure consumes no lesser authority: the on-disk
/// tombstone prevents replay.
pub fn construct_worker_request_v2_from_consumed_handoff(
    plan: &MultiInputLinkPlanV1,
    measurement: &WorkerMeasurementV1,
    consumed: ConsumedCompilerModuleHandoffV1,
    external_providers: Vec<WorkerInputV1>,
    input_kinds: &LinkInputKindClosureV1,
    output: WorkerOutputConstraintsV1,
) -> Result<CompilerHandoffWorkerRequestV2, WorkerRequestConstructionError> {
    let attempt = consumed.attempt();
    let handoff_identity = consumed.identity();
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes())
        .map_err(WorkerRequestConstructionError::CompilerModuleHandoff)?;
    let parts = handoff.into_parts();
    let target = parts.target();
    let code_object_version = parts.code_object_version();
    let (envelope, symbol_manifest, module) = parts.into_envelope_manifest_and_module();
    let manifest_identity = symbol_manifest.identity();
    let kind = match module.kind() {
        CompilerModuleKindV1::LlvmTextIr => WorkerInputKindV1::LlvmTextIr,
        CompilerModuleKindV1::LlvmBitcode => WorkerInputKindV1::LlvmBitcode,
    };
    let compiler_module = stage_exact_compiler_module_artifact_v1(kind, module.into_bytes())
        .map_err(WorkerRequestConstructionError::WorkerProtocol)?;
    let staged_envelope = crate::stage_compiler_ffi_envelope_v1(envelope);
    let (_, options) = decode_plan_options(plan)?;
    let request = construct_worker_request_v2(
        plan,
        measurement,
        target,
        code_object_version,
        options,
        staged_envelope,
        symbol_manifest,
        compiler_module,
        external_providers,
        input_kinds,
        output,
    )?;
    Ok(CompilerHandoffWorkerRequestV2 {
        attempt,
        handoff_identity,
        manifest_identity,
        request,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerRequestConstructionError {
    CompilerModuleHandoff(CompilerModuleHandoffErrorV2),
    EmptySymbolClosure,
    InvalidRequiredSymbols(WorkerProtocolError),
    InvalidImportSymbols(WorkerProtocolError),
    InvalidExportSymbols(WorkerProtocolError),
    InvalidFinalSymbols(WorkerProtocolError),
    UnreferencedImport(String),
    UnreferencedExport(String),
    ConflictingSymbolRole(String),
    CompilerEnvelopeTargetMismatch,
    CompilerEnvelopeCodeObjectVersionMismatch,
    CompilerEnvelopeSymbolAbsentFromFinal(String),
    CompilerEnvelopeImportRoleMismatch(String),
    CompilerEnvelopeExportRoleMismatch(String),
    TargetMismatch,
    MissingCodeObjectVersion,
    InvalidCodeObjectVersion(String),
    UnsupportedLinkOption(String),
    InvalidLinkOptionValue {
        name: String,
        value: String,
    },
    CodeObjectVersionMismatch {
        planned: CodeObjectVersion,
        requested: CodeObjectVersion,
    },
    OptionsMismatch {
        planned: WorkerOptionsV1,
        requested: WorkerOptionsV1,
    },
    InputCountMismatch {
        planned: usize,
        provided: usize,
    },
    InputKindCountMismatch {
        planned: usize,
        declared: usize,
    },
    InputKindPlanMismatch {
        planned: LinkPlanIdentityV1,
        declared: LinkPlanIdentityV1,
    },
    InputKindMismatch {
        index: usize,
        planned: WorkerInputKindV1,
        provided: WorkerInputKindV1,
    },
    InputIdentityMismatch {
        index: usize,
        planned: ContentIdentityV1,
        provided: ContentIdentityV1,
    },
    InputBytesMismatch {
        index: usize,
        planned: ContentIdentityV1,
    },
    OutputBoundMismatch {
        planned: u64,
        requested: u64,
    },
    ReservedRequestId,
    WorkerProtocol(WorkerProtocolError),
}

impl fmt::Display for WorkerRequestConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompilerModuleHandoff(error) => {
                write!(
                    formatter,
                    "invalid consumed compiler-module handoff: {error}"
                )
            }
            Self::EmptySymbolClosure => formatter.write_str("device link symbol closure is empty"),
            Self::InvalidRequiredSymbols(error) => {
                write!(formatter, "invalid required-symbol set: {error}")
            }
            Self::InvalidImportSymbols(error) => {
                write!(formatter, "invalid import-symbol set: {error}")
            }
            Self::InvalidExportSymbols(error) => {
                write!(formatter, "invalid export-symbol set: {error}")
            }
            Self::InvalidFinalSymbols(error) => {
                write!(formatter, "invalid final-symbol set: {error}")
            }
            Self::UnreferencedImport(symbol) => {
                write!(
                    formatter,
                    "import {symbol} is absent from the required-symbol set"
                )
            }
            Self::UnreferencedExport(symbol) => {
                write!(
                    formatter,
                    "export {symbol} is absent from the required-symbol set"
                )
            }
            Self::ConflictingSymbolRole(symbol) => {
                write!(formatter, "symbol {symbol} is both imported and exported")
            }
            Self::CompilerEnvelopeTargetMismatch => {
                formatter.write_str("compiler FFI envelope target does not match worker request")
            }
            Self::CompilerEnvelopeCodeObjectVersionMismatch => formatter.write_str(
                "compiler FFI envelope code-object version does not match worker request",
            ),
            Self::CompilerEnvelopeSymbolAbsentFromFinal(symbol) => write!(
                formatter,
                "compiler envelope symbol {symbol} is absent from the final-symbol set"
            ),
            Self::CompilerEnvelopeImportRoleMismatch(symbol) => write!(
                formatter,
                "compiler manifest and FFI envelope disagree about import {symbol}"
            ),
            Self::CompilerEnvelopeExportRoleMismatch(symbol) => write!(
                formatter,
                "compiler manifest and FFI envelope disagree about export {symbol}"
            ),
            Self::TargetMismatch => {
                formatter.write_str("worker request target does not match link plan")
            }
            Self::MissingCodeObjectVersion => {
                formatter.write_str("link plan has no code-object-version option")
            }
            Self::InvalidCodeObjectVersion(value) => {
                write!(formatter, "unsupported code-object-version value {value}")
            }
            Self::UnsupportedLinkOption(name) => {
                write!(formatter, "unsupported direct-link option {name}")
            }
            Self::InvalidLinkOptionValue { name, value } => {
                write!(
                    formatter,
                    "invalid value {value} for direct-link option {name}"
                )
            }
            Self::CodeObjectVersionMismatch { planned, requested } => write!(
                formatter,
                "requested code-object version {requested:?} does not match plan {planned:?}"
            ),
            Self::OptionsMismatch { planned, requested } => write!(
                formatter,
                "requested worker options {requested:?} do not match plan {planned:?}"
            ),
            Self::InputCountMismatch { planned, provided } => write!(
                formatter,
                "provided input count {provided} does not match plan count {planned}"
            ),
            Self::InputKindCountMismatch { planned, declared } => write!(
                formatter,
                "declared input-kind count {declared} does not match plan count {planned}"
            ),
            Self::InputKindPlanMismatch { planned, declared } => write!(
                formatter,
                "input-kind closure plan identity {declared:?} does not match plan {planned:?}"
            ),
            Self::InputKindMismatch {
                index,
                planned,
                provided,
            } => write!(
                formatter,
                "provided input {index} kind {provided:?} does not match declared kind {planned:?}"
            ),
            Self::InputIdentityMismatch {
                index,
                planned,
                provided,
            } => write!(
                formatter,
                "provided input {index} identity {provided} does not match plan {planned}"
            ),
            Self::InputBytesMismatch { index, planned } => write!(
                formatter,
                "provided input {index} bytes do not match plan identity {planned}"
            ),
            Self::OutputBoundMismatch { planned, requested } => write!(
                formatter,
                "requested output bound {requested} does not match planned length {planned}"
            ),
            Self::ReservedRequestId => {
                formatter.write_str("derived worker request ID is the reserved zero value")
            }
            Self::WorkerProtocol(error) => {
                write!(formatter, "worker request validation failed: {error}")
            }
        }
    }
}

impl std::error::Error for WorkerRequestConstructionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CompilerModuleHandoff(error) => Some(error),
            Self::WorkerProtocol(error)
            | Self::InvalidRequiredSymbols(error)
            | Self::InvalidImportSymbols(error)
            | Self::InvalidExportSymbols(error)
            | Self::InvalidFinalSymbols(error) => Some(error),
            _ => None,
        }
    }
}

fn validate_inputs(
    plan: &MultiInputLinkPlanV1,
    input_kinds: &LinkInputKindClosureV1,
    inputs: &[WorkerInputV1],
) -> Result<(), WorkerRequestConstructionError> {
    if input_kinds.plan_identity != plan.identity() {
        return Err(WorkerRequestConstructionError::InputKindPlanMismatch {
            planned: plan.identity(),
            declared: input_kinds.plan_identity,
        });
    }
    if inputs.len() != plan.inputs().len() {
        return Err(WorkerRequestConstructionError::InputCountMismatch {
            planned: plan.inputs().len(),
            provided: inputs.len(),
        });
    }
    for (index, ((planned, planned_kind), provided)) in plan
        .inputs()
        .iter()
        .zip(&input_kinds.kinds)
        .zip(inputs)
        .enumerate()
    {
        if provided.identity() != planned.identity() {
            return Err(WorkerRequestConstructionError::InputIdentityMismatch {
                index,
                planned: planned.identity(),
                provided: provided.identity(),
            });
        }
        if !planned.identity().matches(provided.bytes()) {
            return Err(WorkerRequestConstructionError::InputBytesMismatch {
                index,
                planned: planned.identity(),
            });
        }
        if *planned_kind != provided.kind() {
            return Err(WorkerRequestConstructionError::InputKindMismatch {
                index,
                planned: *planned_kind,
                provided: provided.kind(),
            });
        }
    }
    Ok(())
}

fn decode_plan_options(
    plan: &MultiInputLinkPlanV1,
) -> Result<(CodeObjectVersion, WorkerOptionsV1), WorkerRequestConstructionError> {
    decode_link_options(plan.options())
}

pub(crate) fn decode_link_options(
    options: &[LinkOptionV1],
) -> Result<(CodeObjectVersion, WorkerOptionsV1), WorkerRequestConstructionError> {
    let mut code_object_version = None;
    let mut optimization = WorkerOptimizationLevelV1::O0;
    let mut strip_debug = false;
    let mut verify_each = false;

    for option in options {
        match option.name() {
            "code-object-version" => {
                code_object_version = Some(match option.value() {
                    "4" => CodeObjectVersion::V4,
                    "5" => CodeObjectVersion::V5,
                    "6" => CodeObjectVersion::V6,
                    value => {
                        return Err(WorkerRequestConstructionError::InvalidCodeObjectVersion(
                            value.to_owned(),
                        ));
                    }
                });
            }
            "opt-level" => {
                optimization = match option.value() {
                    "0" => WorkerOptimizationLevelV1::O0,
                    "1" => WorkerOptimizationLevelV1::O1,
                    "2" => WorkerOptimizationLevelV1::O2,
                    "3" => WorkerOptimizationLevelV1::O3,
                    value => {
                        return Err(WorkerRequestConstructionError::InvalidLinkOptionValue {
                            name: option.name().to_owned(),
                            value: value.to_owned(),
                        });
                    }
                };
            }
            "strip-debug" => strip_debug = decode_bool_option(option.name(), option.value())?,
            "verify-each" => verify_each = decode_bool_option(option.name(), option.value())?,
            name => {
                return Err(WorkerRequestConstructionError::UnsupportedLinkOption(
                    name.to_owned(),
                ));
            }
        }
    }
    let code_object_version =
        code_object_version.ok_or(WorkerRequestConstructionError::MissingCodeObjectVersion)?;
    Ok((
        code_object_version,
        WorkerOptionsV1::new(optimization, strip_debug, verify_each),
    ))
}

fn decode_bool_option(name: &str, value: &str) -> Result<bool, WorkerRequestConstructionError> {
    match value {
        "false" => Ok(false),
        "true" => Ok(true),
        _ => Err(WorkerRequestConstructionError::InvalidLinkOptionValue {
            name: name.to_owned(),
            value: value.to_owned(),
        }),
    }
}

fn calculate_closure_identity(
    required_symbols: &[String],
    import_symbols: &[String],
    export_symbols: &[String],
) -> LinkSymbolClosureIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(SYMBOL_CLOSURE_DOMAIN_V1);
    hash_strings(&mut hasher, required_symbols);
    hash_strings(&mut hasher, import_symbols);
    hash_strings(&mut hasher, export_symbols);
    LinkSymbolClosureIdentityV1(hasher.finalize().into())
}

fn calculate_input_kind_closure_identity(
    plan: &MultiInputLinkPlanV1,
    kinds: &[WorkerInputKindV1],
) -> LinkInputKindClosureIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(INPUT_KIND_CLOSURE_DOMAIN_V1);
    hasher.update(plan.identity().as_bytes());
    hasher.update((kinds.len() as u64).to_le_bytes());
    for (input, kind) in plan.inputs().iter().zip(kinds) {
        hasher.update(input.identity().sha256());
        hasher.update(input.identity().byte_len().to_le_bytes());
        hasher.update([*kind as u8]);
    }
    LinkInputKindClosureIdentityV1(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
fn calculate_request_id(
    plan: &MultiInputLinkPlanV1,
    llvm_build_identity: &str,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    options: WorkerOptionsV1,
    inputs: &[WorkerInputV1],
    input_kinds: &LinkInputKindClosureV1,
    symbols: &LinkSymbolClosureV1,
    output: &WorkerOutputConstraintsV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PLAN_REQUEST_DOMAIN_V1);
    hasher.update(plan.identity().as_bytes());
    hasher.update(input_kinds.identity.as_bytes());
    hasher.update(symbols.identity.as_bytes());
    hash_text(&mut hasher, llvm_build_identity);
    hash_text(&mut hasher, &target.to_string());
    hasher.update([code_object_version_byte(code_object_version)]);
    hasher.update([
        options.optimization() as u8,
        u8::from(options.strip_debug()),
        u8::from(options.verify_each()),
    ]);
    hasher.update((inputs.len() as u64).to_le_bytes());
    for input in inputs {
        hasher.update([input.kind() as u8]);
        hasher.update(input.identity().sha256());
        hasher.update(input.identity().byte_len().to_le_bytes());
    }
    hasher.update(output.max_bytes().to_le_bytes());
    hasher.finalize().into()
}

#[allow(dead_code, clippy::too_many_arguments)]
fn calculate_request_id_v2(
    plan: &MultiInputLinkPlanV1,
    measurement: &WorkerMeasurementV1,
    staged_envelope_identity: [u8; 32],
    compiler_envelope_identity: [u8; 32],
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    options: WorkerOptionsV1,
    compiler_module: &WorkerInputV1,
    external_providers: &[WorkerInputV1],
    input_kinds: &LinkInputKindClosureV1,
    import_symbols: &[String],
    export_symbols: &[String],
    final_symbols: &[String],
    output: &WorkerOutputConstraintsV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PLAN_REQUEST_DOMAIN_V2);
    hasher.update(plan.identity().as_bytes());
    hasher.update(input_kinds.identity.as_bytes());
    hasher.update(staged_envelope_identity);
    hasher.update(compiler_envelope_identity);
    hasher.update(manifest_identity.sha256());
    hasher.update(manifest_identity.byte_len().to_le_bytes());
    hasher.update(measurement.executable().sha256());
    hasher.update(measurement.executable().byte_len().to_le_bytes());
    hash_text(&mut hasher, measurement.worker_build_identity());
    hash_text(&mut hasher, measurement.llvm_build_identity());
    hash_text(&mut hasher, &target.to_string());
    hasher.update([code_object_version_byte(code_object_version)]);
    hasher.update([
        options.optimization() as u8,
        u8::from(options.strip_debug()),
        u8::from(options.verify_each()),
    ]);
    hash_input(&mut hasher, compiler_module);
    hasher.update((external_providers.len() as u64).to_le_bytes());
    for input in external_providers {
        hash_input(&mut hasher, input);
    }
    hash_strings(&mut hasher, import_symbols);
    hash_strings(&mut hasher, export_symbols);
    hash_strings(&mut hasher, final_symbols);
    hasher.update(output.max_bytes().to_le_bytes());
    hasher.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn calculate_first_build_request_id_v2(
    attempt: BuildAttempt,
    handoff_identity: CompilerModuleHandoffIdentityV1,
    measurement: &WorkerMeasurementV1,
    compiler_envelope_identity: [u8; 32],
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    options: WorkerOptionsV1,
    compiler_module: &WorkerInputV1,
    external_providers: &[WorkerInputV1],
    import_symbols: &[String],
    export_symbols: &[String],
    final_symbols: &[String],
    output: &WorkerOutputConstraintsV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(FIRST_BUILD_REQUEST_DOMAIN_V2);
    hasher.update(attempt.generation().to_le_bytes());
    hasher.update(attempt.session().as_bytes());
    hasher.update(attempt.invocation().as_bytes());
    hasher.update(handoff_identity.as_bytes());
    hasher.update(compiler_envelope_identity);
    hasher.update(manifest_identity.sha256());
    hasher.update(manifest_identity.byte_len().to_le_bytes());
    hasher.update(measurement.executable().sha256());
    hasher.update(measurement.executable().byte_len().to_le_bytes());
    hash_text(&mut hasher, measurement.worker_build_identity());
    hash_text(&mut hasher, measurement.llvm_build_identity());
    hash_text(&mut hasher, &target.to_string());
    hasher.update([code_object_version_byte(code_object_version)]);
    hasher.update([
        options.optimization() as u8,
        u8::from(options.strip_debug()),
        u8::from(options.verify_each()),
    ]);
    hash_input(&mut hasher, compiler_module);
    hasher.update((external_providers.len() as u64).to_le_bytes());
    for input in external_providers {
        hash_input(&mut hasher, input);
    }
    hash_strings(&mut hasher, import_symbols);
    hash_strings(&mut hasher, export_symbols);
    hash_strings(&mut hasher, final_symbols);
    hasher.update(output.max_bytes().to_le_bytes());
    hasher.finalize().into()
}

#[allow(dead_code)]
fn hash_input(hasher: &mut Sha256, input: &WorkerInputV1) {
    hasher.update([input.kind() as u8]);
    hasher.update(input.identity().sha256());
    hasher.update(input.identity().byte_len().to_le_bytes());
}

fn hash_strings(hasher: &mut Sha256, strings: &[String]) {
    hasher.update((strings.len() as u64).to_le_bytes());
    for string in strings {
        hash_text(hasher, string);
    }
}

fn hash_text(hasher: &mut Sha256, text: &str) {
    hasher.update((text.len() as u64).to_le_bytes());
    hasher.update(text.as_bytes());
}

const fn code_object_version_byte(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

#[cfg(test)]
mod v2_tests {
    use super::*;
    use crate::{LinkInputV1, LinkOptionV1, LinkOutputV1, ProvenanceNodeV1};
    use fe2o3_artifact_transaction::{
        BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
        consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
    };
    use fe2o3_compiler_ffi::{
        CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
        CompilerFfiEnvelopeBuilderV1, CompilerFfiEnvelopeV1, CompilerFfiLinkRoleV1,
        CompilerFfiSourceOwnerV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
        CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
        DeviceTargetV1 as CompilerDeviceTargetV1,
    };
    use reserved_fe2o3_symbols::{
        DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
        DeviceFfiDirectionV1, derive_device_ffi_contract_id_v1,
    };
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    const MODULE: &[u8] = b"exact compiler module bitcode";
    const PROVIDER: &[u8] = b"exact external provider object";

    struct Fixture {
        plan: MultiInputLinkPlanV1,
        kinds: LinkInputKindClosureV1,
        provider: WorkerInputV1,
        output: WorkerOutputConstraintsV1,
    }

    fn target() -> DeviceTargetV1 {
        DeviceTargetV1::parse("gfx942:xnack-").unwrap()
    }

    fn options() -> WorkerOptionsV1 {
        WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true)
    }

    fn fixture() -> Fixture {
        let module = WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, MODULE.to_vec()).unwrap();
        let provider =
            WorkerInputV1::new(WorkerInputKindV1::AmdGpuRelocatable, PROVIDER.to_vec()).unwrap();
        let mut inputs = [module, provider.clone()];
        inputs.sort_by_key(|input| (input.identity(), input.kind()));
        let link_inputs = inputs
            .iter()
            .map(|input| LinkInputV1::new(input.identity(), target()))
            .collect::<Vec<_>>();
        let output_identity = ContentIdentityV1::calculate(b"expected exact hsaco");
        let mut provenance = link_inputs
            .iter()
            .map(|input| ProvenanceNodeV1::new(input.identity(), vec![]).unwrap())
            .collect::<Vec<_>>();
        provenance.push(
            ProvenanceNodeV1::new(
                output_identity,
                link_inputs.iter().map(|input| input.identity()).collect(),
            )
            .unwrap(),
        );
        let plan = MultiInputLinkPlanV1::canonicalized(
            target(),
            link_inputs,
            vec![
                LinkOptionV1::new("code-object-version", "6").unwrap(),
                LinkOptionV1::new("opt-level", "2").unwrap(),
                LinkOptionV1::new("strip-debug", "true").unwrap(),
                LinkOptionV1::new("verify-each", "true").unwrap(),
            ],
            LinkOutputV1::new(output_identity, target()),
            provenance,
        )
        .unwrap();
        let kinds =
            LinkInputKindClosureV1::new(&plan, inputs.iter().map(|input| input.kind()).collect())
                .unwrap();
        Fixture {
            plan,
            kinds,
            provider,
            output: WorkerOutputConstraintsV1::new(output_identity.byte_len()).unwrap(),
        }
    }

    fn compiler_envelope(import_symbol: &str) -> CompilerFfiEnvelopeV1 {
        let compiler_target = CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap();
        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(compiler_target, CompilerCodeObjectVersion::V6, 2)
                .unwrap();
        builder
            .push(contract(
                import_symbol,
                DeviceFfiDirectionV1::Import,
                CompilerFfiLinkRoleV1::RequiresExternalDefinition,
                0x31,
            ))
            .unwrap();
        builder
            .push(contract(
                "rust_helper",
                DeviceFfiDirectionV1::Export,
                CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
                0x42,
            ))
            .unwrap();
        builder.finish().unwrap()
    }

    fn envelope(import_symbol: &str) -> StagedCompilerFfiEnvelopeV1 {
        crate::stage_compiler_ffi_envelope_v1(compiler_envelope(import_symbol))
    }

    fn contract(
        symbol: &str,
        direction: DeviceFfiDirectionV1,
        role: CompilerFfiLinkRoleV1,
        semantic_byte: u8,
    ) -> CompilerFfiContractV1 {
        let semantic_identity = [semantic_byte; 32];
        let semantic_text = lower_hex(&semantic_identity);
        let direction_tag = match direction {
            DeviceFfiDirectionV1::Import => DEVICE_FFI_DIRECTION_IMPORT_V1,
            DeviceFfiDirectionV1::Export => DEVICE_FFI_DIRECTION_EXPORT_V1,
        };
        let fields = DeviceFfiContractFieldsV1 {
            direction: direction_tag,
            symbol,
            calling_convention: "C",
            code_object_version: 6,
            target: "gfx942:xnack-",
            physical_abi: ABI,
            effects: "none",
            semantic_identity: &semantic_text,
        };
        CompilerFfiContractV1::new(
            derive_device_ffi_contract_id_v1(fields),
            direction,
            role,
            CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            CompilerCodeObjectVersion::V6,
            CompilerFfiSourceOwnerV1::new(
                "ffi_crate",
                &format!("ffi_crate::{symbol}"),
                [semantic_byte; 16],
                &format!("_RINvNtCs1234_9ffi_crate{symbol}"),
            )
            .unwrap(),
            symbol,
            ABI,
            "none",
            semantic_identity,
        )
        .unwrap()
    }

    fn measurement() -> WorkerMeasurementV1 {
        WorkerMeasurementV1::new(
            ContentIdentityV1::calculate(b"pinned worker executable"),
            "worker-v1-build",
            "llvm-v2-build",
        )
        .unwrap()
    }

    fn symbol_manifest(imports: &[&str], internal_helper: &str) -> CompilerModuleSymbolManifestV1 {
        use CompilerModuleSymbolRoleV1 as Role;

        let mut entries = vec![
            (Role::KernelEntry, "kernel_main".to_owned()),
            (Role::KernelDescriptor, "kernel_main.kd".to_owned()),
            (Role::DeviceFfiExport, "rust_helper".to_owned()),
            (Role::InternalHelper, internal_helper.to_owned()),
        ];
        entries.extend(
            imports
                .iter()
                .map(|symbol| (Role::UnresolvedExternalImport, (*symbol).to_owned())),
        );
        entries.sort();
        CompilerModuleSymbolManifestV1::new(entries).unwrap()
    }

    fn final_symbols() -> Vec<String> {
        [
            "external_add",
            "kernel_main",
            "kernel_main.kd",
            "rust_helper",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-finalizer-consumed-handoff-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn public_v2_construction_requires_and_retains_consumed_attempt_handoff() {
        let fixture = fixture();
        let directory = TestDirectory::new();
        let producer =
            ProducerIdentity::from_codegen("ffi_crate", Some(std::path::Path::new("src/lib.rs")))
                .unwrap();
        let attempt = begin_build_attempt(
            &directory.0,
            &producer,
            BuildInvocation::from_bytes([0x71; 32]),
            BuildSession::from_bytes([0x72; 16]),
        )
        .unwrap();
        let manifest = symbol_manifest(&["external_add"], "internal_only");
        let manifest_identity = manifest.identity();
        let handoff = CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmBitcode,
            CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            CompilerCodeObjectVersion::V6,
            compiler_envelope("external_add"),
            manifest,
            MODULE,
        )
        .unwrap();
        let receipt = publish_compiler_module_handoff_v1(
            &directory.0,
            &producer,
            attempt,
            handoff.canonical_bytes(),
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        let request = construct_worker_request_v2_from_consumed_handoff(
            &fixture.plan,
            &measurement(),
            consumed,
            vec![fixture.provider.clone()],
            &fixture.kinds,
            fixture.output,
        )
        .unwrap();

        assert_eq!(request.attempt(), attempt);
        assert_eq!(request.handoff_identity(), receipt.identity());
        assert_eq!(request.manifest_identity(), manifest_identity);
        assert_eq!(request.sealed_request().compiler_module().bytes(), MODULE);
        assert_eq!(
            request.sealed_request().external_providers(),
            &[fixture.provider]
        );
        assert!(!request.grants_publication_authority());
        assert!(!request.grants_load_authority());
        assert!(!request.grants_launch_authority());
    }

    #[test]
    fn operator_cannot_omit_or_inject_manifest_symbols_and_internal_helpers_do_not_escape() {
        let fixture = fixture();
        let staged = envelope("external_add");
        let compiler_identity = staged.inspection().envelope_identity();
        let artifact = stage_exact_compiler_module_artifact_v1(
            WorkerInputKindV1::LlvmBitcode,
            MODULE.to_vec(),
        )
        .unwrap();
        let artifact_identity = artifact.identity();
        let request = construct_worker_request_v2(
            &fixture.plan,
            &measurement(),
            target(),
            CodeObjectVersion::V6,
            options(),
            staged,
            symbol_manifest(&["external_add"], "internal_only"),
            artifact,
            vec![fixture.provider.clone()],
            &fixture.kinds,
            fixture.output.clone(),
        )
        .unwrap();

        assert_eq!(request.compiler_module().identity(), artifact_identity);
        assert_eq!(
            request.compiler_envelope_identity().as_bytes(),
            compiler_identity.as_bytes()
        );
        assert_eq!(request.import_symbols(), ["external_add"]);
        assert_eq!(request.export_symbols(), ["rust_helper"]);
        assert_eq!(request.final_symbols(), final_symbols());
        assert!(
            !request
                .final_symbols()
                .iter()
                .any(|symbol| symbol == "internal_only")
        );
        assert_eq!(request.external_providers(), &[fixture.provider]);
    }

    #[test]
    fn same_cardinality_envelope_substitution_and_wrong_module_fail_closed() {
        let fixture = fixture();
        let artifact = stage_exact_compiler_module_artifact_v1(
            WorkerInputKindV1::LlvmBitcode,
            MODULE.to_vec(),
        )
        .unwrap();
        assert_eq!(
            construct_worker_request_v2(
                &fixture.plan,
                &measurement(),
                target(),
                CodeObjectVersion::V6,
                options(),
                envelope("substituted_add"),
                symbol_manifest(&["external_add"], "internal_only"),
                artifact,
                vec![fixture.provider.clone()],
                &fixture.kinds,
                fixture.output.clone(),
            ),
            Err(
                WorkerRequestConstructionError::CompilerEnvelopeImportRoleMismatch(
                    "external_add".to_owned()
                )
            )
        );

        let wrong_artifact = stage_exact_compiler_module_artifact_v1(
            WorkerInputKindV1::LlvmBitcode,
            b"different module".to_vec(),
        )
        .unwrap();
        assert!(
            construct_worker_request_v2(
                &fixture.plan,
                &measurement(),
                target(),
                CodeObjectVersion::V6,
                options(),
                envelope("external_add"),
                symbol_manifest(&["external_add"], "internal_only"),
                wrong_artifact,
                vec![fixture.provider],
                &fixture.kinds,
                fixture.output,
            )
            .is_err()
        );
    }

    #[test]
    fn uncontracted_manifest_import_fails_closed() {
        let fixture = fixture();
        let artifact = stage_exact_compiler_module_artifact_v1(
            WorkerInputKindV1::LlvmBitcode,
            MODULE.to_vec(),
        )
        .unwrap();

        assert_eq!(
            construct_worker_request_v2(
                &fixture.plan,
                &measurement(),
                target(),
                CodeObjectVersion::V6,
                options(),
                envelope("external_add"),
                symbol_manifest(&["external_add", "uncontracted_external"], "internal_only",),
                artifact,
                vec![fixture.provider],
                &fixture.kinds,
                fixture.output,
            ),
            Err(
                WorkerRequestConstructionError::CompilerEnvelopeImportRoleMismatch(
                    "uncontracted_external".to_owned()
                )
            )
        );
    }

    #[test]
    fn manifest_identity_binds_roles_that_do_not_escape_the_final_closure() {
        let fixture = fixture();
        let construct = |internal_helper: &str| {
            construct_worker_request_v2(
                &fixture.plan,
                &measurement(),
                target(),
                CodeObjectVersion::V6,
                options(),
                envelope("external_add"),
                symbol_manifest(&["external_add"], internal_helper),
                stage_exact_compiler_module_artifact_v1(
                    WorkerInputKindV1::LlvmBitcode,
                    MODULE.to_vec(),
                )
                .unwrap(),
                vec![fixture.provider.clone()],
                &fixture.kinds,
                fixture.output.clone(),
            )
            .unwrap()
        };

        let first = construct("internal_alpha");
        let second = construct("internal_beta");
        assert_eq!(first.final_symbols(), second.final_symbols());
        assert_ne!(first.request_id(), second.request_id());
        assert_ne!(first.identity(), second.identity());
    }

    fn lower_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
