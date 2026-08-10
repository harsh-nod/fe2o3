use std::{fmt, mem};

use fe2o3_artifact_transaction::TargetIdentityV1;
use fe2o3_artifacts::{
    ArtifactContainerV1, Capability, CodeObjectFormat, DigestAlgorithm, DigestBytes,
    DirectLinkContainerIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    DirectLinkLinkedOutputIdentityV1, DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1,
    Endianness, IdentityText, MeasuredToolIdentity, PayloadDigest, PointerWidth,
};
use fe2o3_hsaco_finalize::{
    WorkerRequestV1, WorkerResponseV1, WorkerStageV1, finalize_unfinalized, inspect_unfinalized,
};
use fe2o3_kernel_descriptor::{
    CapabilityV1, CodeObjectVersion, DeviceDescriptorTableV1, decode_device_descriptor_table_v1,
    encode_device_descriptor_table_v1,
};
use fe2o3_rustc_invocation::{InvocationDigestV2, decode_descriptor_v2, encode_descriptor_v2};
use sha2::{Digest as _, Sha256};

use crate::{
    CallerMeasuredBackendInvocationIdentityV2, CallerMeasuredKernelIrIdentityV2,
    CallerMeasuredSemanticWitnessIdentityV2, CallerMeasuredSourceDependencyV2,
    CallerMeasuredSourceRootIdentityV2, CompilerSourceClosureV2,
    CompilerTransactionEvidenceCapsuleV2, CompilerTransactionEvidencePartsV2,
    CompilerTransactionValidationErrorV2, MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2,
    MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V2, MAX_COMPILER_TRANSACTION_FEATURES_V2,
};

pub const SEALED_COMPILER_TRANSACTION_MAGIC_V1: [u8; 8] = *b"FE2CTR1\0";
pub const SEALED_COMPILER_TRANSACTION_VERSION_V1: u16 = 1;
pub const MAX_COMPILER_TRANSACTION_SOURCE_FILE_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_COMPILER_TRANSACTION_SOURCE_BYTES_V1: usize = 64 * 1024 * 1024;
pub const MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1: usize =
    MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V2 + 1024;

const HEADER_BYTES: usize = 16;
const IDENTITY_BYTES: usize = 32;
const MEASUREMENT_COUNT: usize = 17;
const FIXED_BODY_BYTES: usize =
    IDENTITY_BYTES + (MEASUREMENT_COUNT * IDENTITY_BYTES) + IDENTITY_BYTES + 4 + IDENTITY_BYTES;

const TRANSACTION_DOMAIN: &[u8] = b"FE2O3/COMPILER-TRANSACTION-RECORDER/V1\0";
const SOURCE_TREE_DOMAIN: &[u8] = b"FE2O3/EXACT-COMPILER-SOURCE-TREE/V1\0";
const TARGET_DOMAIN: &[u8] = b"FE2O3/GFX942-XNACK-MINUS-COV6-COMPILER-TARGET/V1\0";
const SEMANTIC_WITNESSES_DOMAIN: &[u8] = b"FE2O3/ALPHA-ZETA-SEMANTIC-LAYOUT-WITNESSES/V1\0";
const CHECKPOINT_DOMAIN: &[u8] = b"FE2O3/COMPILER-TRANSACTION-CHECKPOINT/V1\0";
const RECORD_IDENTITY_DOMAIN: &[u8] = b"FE2O3/SEALED-COMPILER-TRANSACTION/V1\0";

const GFX942_TRIPLE: &str = "amdgcn-amd-amdhsa";
const GFX942_AMD_TARGET: &str = "gfx942:xnack-";
const GFX942_WAVEFRONT_SIZE: u8 = 64;
const GFX942_CODE_OBJECT_VERSION: CodeObjectVersion = CodeObjectVersion::V6;
const GFX942_ARTIFACT_CAPABILITIES: &[Capability] = &[Capability::AmdWave];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerTransactionContentIdentityV1([u8; 32]);

impl CompilerTransactionContentIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    fn measure(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    fn from_nonzero(
        bytes: [u8; 32],
        field: &'static str,
    ) -> Result<Self, SealedCompilerTransactionDecodeErrorV1> {
        if bytes == [0; 32] {
            return Err(SealedCompilerTransactionDecodeErrorV1::ReservedZeroIdentity { field });
        }
        Ok(Self(bytes))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SealedCompilerTransactionIdentityV1([u8; 32]);

impl SealedCompilerTransactionIdentityV1 {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, SealedCompilerTransactionDecodeErrorV1> {
        if bytes == [0; 32] {
            return Err(
                SealedCompilerTransactionDecodeErrorV1::ReservedZeroIdentity {
                    field: "sealed transaction",
                },
            );
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCompilerSourceFileV1 {
    path: IdentityText,
    byte_len: u64,
    content: CompilerTransactionContentIdentityV1,
}

impl ExactCompilerSourceFileV1 {
    pub fn measure(
        path: IdentityText,
        bytes: &[u8],
    ) -> Result<Self, CompilerTransactionRecorderErrorV1> {
        if bytes.len() > MAX_COMPILER_TRANSACTION_SOURCE_FILE_BYTES_V1 {
            return Err(CompilerTransactionRecorderErrorV1::SourceFileTooLarge {
                max: MAX_COMPILER_TRANSACTION_SOURCE_FILE_BYTES_V1,
            });
        }
        Ok(Self {
            path,
            byte_len: bytes.len() as u64,
            content: CompilerTransactionContentIdentityV1::measure(bytes),
        })
    }

    pub const fn path(&self) -> &IdentityText {
        &self.path
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub const fn content_identity(&self) -> CompilerTransactionContentIdentityV1 {
        self.content
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCompilerSourceClosureV1 {
    root: ExactCompilerSourceFileV1,
    dependencies: Vec<ExactCompilerSourceFileV1>,
    features: Vec<IdentityText>,
    capsule_closure: CompilerSourceClosureV2,
    tree_identity: CompilerTransactionContentIdentityV1,
}

impl ExactCompilerSourceClosureV1 {
    pub fn new(
        root: ExactCompilerSourceFileV1,
        mut dependencies: Vec<ExactCompilerSourceFileV1>,
        mut features: Vec<IdentityText>,
    ) -> Result<Self, CompilerTransactionRecorderErrorV1> {
        if dependencies.len() > MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2 {
            return Err(CompilerTransactionRecorderErrorV1::TooManySourceFiles {
                max: MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2,
            });
        }
        if features.len() > MAX_COMPILER_TRANSACTION_FEATURES_V2 {
            return Err(CompilerTransactionRecorderErrorV1::TooManyFeatures {
                max: MAX_COMPILER_TRANSACTION_FEATURES_V2,
            });
        }
        dependencies.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        if dependencies
            .windows(2)
            .any(|pair| pair[0].path == pair[1].path)
        {
            return Err(CompilerTransactionRecorderErrorV1::DuplicateSourcePath);
        }
        if dependencies
            .iter()
            .any(|dependency| dependency.path == root.path)
        {
            return Err(CompilerTransactionRecorderErrorV1::RootRepeatedAsDependency);
        }
        features.sort_unstable();
        if features.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(CompilerTransactionRecorderErrorV1::DuplicateFeature);
        }
        let total_bytes = dependencies.iter().try_fold(root.byte_len, |total, file| {
            total.checked_add(file.byte_len)
        });
        if total_bytes.is_none_or(|value| value > MAX_COMPILER_TRANSACTION_SOURCE_BYTES_V1 as u64) {
            return Err(CompilerTransactionRecorderErrorV1::SourceClosureTooLarge {
                max: MAX_COMPILER_TRANSACTION_SOURCE_BYTES_V1,
            });
        }

        let source_root =
            CallerMeasuredSourceRootIdentityV2::try_from_sha256(root.content.into_bytes())
                .map_err(CompilerTransactionRecorderErrorV1::Capsule)?;
        let capsule_dependencies = dependencies
            .iter()
            .map(|dependency| {
                CallerMeasuredSourceDependencyV2::try_from_sha256(
                    dependency.path.clone(),
                    dependency.content.into_bytes(),
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(CompilerTransactionRecorderErrorV1::Capsule)?;
        let capsule_closure =
            CompilerSourceClosureV2::new(source_root, capsule_dependencies, features.clone())
                .map_err(CompilerTransactionRecorderErrorV1::Capsule)?;
        let tree_identity = calculate_source_tree_identity(&root, &dependencies, &features);
        Ok(Self {
            root,
            dependencies,
            features,
            capsule_closure,
            tree_identity,
        })
    }

    pub const fn root(&self) -> &ExactCompilerSourceFileV1 {
        &self.root
    }

    pub fn dependencies(&self) -> &[ExactCompilerSourceFileV1] {
        &self.dependencies
    }

    pub fn features(&self) -> &[IdentityText] {
        &self.features
    }

    pub const fn tree_identity(&self) -> CompilerTransactionContentIdentityV1 {
        self.tree_identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCompilerToolV1 {
    name: IdentityText,
    version: IdentityText,
    executable: CompilerTransactionContentIdentityV1,
    configuration: CompilerTransactionContentIdentityV1,
}

impl ExactCompilerToolV1 {
    pub fn measure(
        name: IdentityText,
        version: IdentityText,
        executable_bytes: &[u8],
        configuration_bytes: &[u8],
    ) -> Result<Self, CompilerTransactionRecorderErrorV1> {
        require_nonempty("compiler executable", executable_bytes)?;
        require_nonempty("compiler configuration", configuration_bytes)?;
        Ok(Self {
            name,
            version,
            executable: CompilerTransactionContentIdentityV1::measure(executable_bytes),
            configuration: CompilerTransactionContentIdentityV1::measure(configuration_bytes),
        })
    }

    pub const fn executable_identity(&self) -> CompilerTransactionContentIdentityV1 {
        self.executable
    }

    pub const fn configuration_identity(&self) -> CompilerTransactionContentIdentityV1 {
        self.configuration
    }

    fn measured_identity(&self) -> MeasuredToolIdentity {
        MeasuredToolIdentity::new(
            self.name.clone(),
            self.version.clone(),
            payload_digest(self.executable),
            payload_digest(self.configuration),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCompilerInvocationV1 {
    rustc_tool: ExactCompilerToolV1,
    backend_tool: ExactCompilerToolV1,
    rustc_descriptor: CompilerTransactionContentIdentityV1,
    rustc_invocation: InvocationDigestV2,
    backend_invocation: CompilerTransactionContentIdentityV1,
    amd_target: String,
}

impl ExactCompilerInvocationV1 {
    pub fn measure(
        rustc_descriptor_bytes: &[u8],
        rustc_tool: ExactCompilerToolV1,
        backend_tool: ExactCompilerToolV1,
        backend_invocation_bytes: &[u8],
    ) -> Result<Self, CompilerTransactionRecorderErrorV1> {
        require_nonempty("rustc invocation descriptor", rustc_descriptor_bytes)?;
        require_nonempty("backend invocation", backend_invocation_bytes)?;
        let descriptor = decode_descriptor_v2(rustc_descriptor_bytes)
            .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)?;
        let canonical = encode_descriptor_v2(&descriptor)
            .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)?;
        if canonical != rustc_descriptor_bytes {
            return Err(CompilerTransactionRecorderErrorV1::NonCanonicalRustcDescriptor);
        }
        if descriptor.rustc_executable_sha256() != rustc_tool.executable.as_bytes() {
            return Err(CompilerTransactionRecorderErrorV1::RustcExecutableMismatch);
        }
        if descriptor.codegen_backend_sha256() != backend_tool.executable.as_bytes() {
            return Err(CompilerTransactionRecorderErrorV1::BackendExecutableMismatch);
        }
        if descriptor.amd_target() != GFX942_AMD_TARGET {
            return Err(CompilerTransactionRecorderErrorV1::UnsupportedTarget);
        }
        let rustc_invocation = InvocationDigestV2::calculate(&descriptor)
            .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)?;
        Ok(Self {
            rustc_tool,
            backend_tool,
            rustc_descriptor: CompilerTransactionContentIdentityV1::measure(rustc_descriptor_bytes),
            rustc_invocation,
            backend_invocation: CompilerTransactionContentIdentityV1::measure(
                backend_invocation_bytes,
            ),
            amd_target: descriptor.amd_target().to_owned(),
        })
    }

    pub const fn rustc_descriptor_identity(&self) -> CompilerTransactionContentIdentityV1 {
        self.rustc_descriptor
    }

    pub const fn rustc_invocation_identity(&self) -> InvocationDigestV2 {
        self.rustc_invocation
    }

    pub const fn backend_invocation_identity(&self) -> CompilerTransactionContentIdentityV1 {
        self.backend_invocation
    }

    pub fn amd_target(&self) -> &str {
        &self.amd_target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942CompilerTargetV1 {
    amd_target: String,
    code_object_version: CodeObjectVersion,
    measurement: CompilerTransactionContentIdentityV1,
}

impl Gfx942CompilerTargetV1 {
    /// Derives the one supported target profile from an exact validated rustc invocation.
    ///
    /// The resulting measurement is recorder-local and is not a
    /// [`TargetIdentityV1`] or an authority-bearing target claim.
    pub fn for_invocation(
        invocation: &ExactCompilerInvocationV1,
    ) -> Result<Self, CompilerTransactionRecorderErrorV1> {
        if invocation.amd_target() != GFX942_AMD_TARGET {
            return Err(CompilerTransactionRecorderErrorV1::UnsupportedTarget);
        }
        Ok(Self {
            amd_target: invocation.amd_target.clone(),
            code_object_version: GFX942_CODE_OBJECT_VERSION,
            measurement: calculate_target_measurement(),
        })
    }

    pub fn amd_target(&self) -> &str {
        &self.amd_target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub fn capabilities(&self) -> &'static [Capability] {
        GFX942_ARTIFACT_CAPABILITIES
    }

    pub const fn measurement(&self) -> CompilerTransactionContentIdentityV1 {
        self.measurement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSemanticLayoutWitnessV1 {
    kernel: IdentityText,
    byte_len: u64,
    content: CompilerTransactionContentIdentityV1,
}

impl ExactSemanticLayoutWitnessV1 {
    pub fn measure(
        kernel: IdentityText,
        bytes: &[u8],
    ) -> Result<Self, CompilerTransactionRecorderErrorV1> {
        require_nonempty("semantic layout witness", bytes)?;
        Ok(Self {
            kernel,
            byte_len: bytes.len() as u64,
            content: CompilerTransactionContentIdentityV1::measure(bytes),
        })
    }

    pub const fn kernel(&self) -> &IdentityText {
        &self.kernel
    }

    pub const fn content_identity(&self) -> CompilerTransactionContentIdentityV1 {
        self.content
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaZetaSemanticLayoutWitnessesV1 {
    witnesses: Vec<ExactSemanticLayoutWitnessV1>,
    identity: CompilerTransactionContentIdentityV1,
}

impl AlphaZetaSemanticLayoutWitnessesV1 {
    pub fn new(
        mut witnesses: Vec<ExactSemanticLayoutWitnessV1>,
    ) -> Result<Self, CompilerTransactionRecorderErrorV1> {
        witnesses.sort_unstable_by(|left, right| left.kernel.cmp(&right.kernel));
        if witnesses
            .windows(2)
            .any(|pair| pair[0].kernel == pair[1].kernel)
        {
            return Err(CompilerTransactionRecorderErrorV1::DuplicateSemanticWitness);
        }
        if witnesses.len() != 2
            || witnesses[0].kernel.as_str() != "alpha"
            || witnesses[1].kernel.as_str() != "zeta"
        {
            return Err(CompilerTransactionRecorderErrorV1::MissingAlphaZetaWitnesses);
        }
        let identity = calculate_semantic_witnesses_identity(&witnesses);
        Ok(Self {
            witnesses,
            identity,
        })
    }

    pub fn witnesses(&self) -> &[ExactSemanticLayoutWitnessV1] {
        &self.witnesses
    }

    pub const fn identity(&self) -> CompilerTransactionContentIdentityV1 {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompilerTransactionStageV1 {
    Source = 1,
    Compiler = 2,
    Target = 3,
    SemanticLayouts = 4,
    KernelIr = 5,
    WorkerExchange = 6,
    RawHsaco = 7,
    FinalizedArtifact = 8,
    Sealed = 9,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerTransactionCheckpointV1 {
    transaction: [u8; 32],
    stage: CompilerTransactionStageV1,
    chain: [u8; 32],
}

impl CompilerTransactionCheckpointV1 {
    pub const fn stage(self) -> CompilerTransactionStageV1 {
        self.stage
    }
}

#[derive(Clone, Debug)]
pub struct CompilerTransactionRecorderV1 {
    freshness: [u8; 32],
    transaction: [u8; 32],
    stage: CompilerTransactionStageV1,
    chain: [u8; 32],
    source: ExactCompilerSourceClosureV1,
    invocation: Option<ExactCompilerInvocationV1>,
    target: Option<Gfx942CompilerTargetV1>,
    semantic_layouts: Option<AlphaZetaSemanticLayoutWitnessesV1>,
    kernel_ir: Option<CompilerTransactionContentIdentityV1>,
    worker_exchange: Option<ValidatedWorkerExchangeV1>,
    raw_hsaco: Option<ValidatedRawHsacoV1>,
    finalized: Option<FinalizedCompilerArtifactMeasurementsV1>,
}

#[derive(Clone, Debug)]
struct ValidatedWorkerExchangeV1 {
    response: WorkerResponseV1,
    request_measurement: CompilerTransactionContentIdentityV1,
    response_measurement: CompilerTransactionContentIdentityV1,
}

#[derive(Clone, Debug)]
struct ValidatedRawHsacoV1 {
    bytes: Vec<u8>,
    measurement: CompilerTransactionContentIdentityV1,
    descriptor_source: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct FinalizedCompilerArtifactMeasurementsV1 {
    finalized_hsaco: CompilerTransactionContentIdentityV1,
    descriptor_source: CompilerTransactionContentIdentityV1,
    finalized_descriptor: CompilerTransactionContentIdentityV1,
    artifact: CompilerTransactionContentIdentityV1,
    canonical_target: TargetIdentityV1,
}

impl CompilerTransactionRecorderV1 {
    pub fn begin(
        freshness: [u8; 32],
        source: ExactCompilerSourceClosureV1,
    ) -> Result<(Self, CompilerTransactionCheckpointV1), CompilerTransactionRecorderErrorV1> {
        if freshness == [0; 32] {
            return Err(CompilerTransactionRecorderErrorV1::ReservedZeroFreshness);
        }
        let transaction = calculate_transaction_identity(
            freshness,
            source.tree_identity,
            source.capsule_closure.identity(),
        );
        let chain = calculate_checkpoint(
            transaction,
            [0; 32],
            CompilerTransactionStageV1::Source,
            &[
                source.tree_identity.into_bytes(),
                *source.capsule_closure.identity().as_bytes(),
            ],
        );
        let recorder = Self {
            freshness,
            transaction,
            stage: CompilerTransactionStageV1::Source,
            chain,
            source,
            invocation: None,
            target: None,
            semantic_layouts: None,
            kernel_ir: None,
            worker_exchange: None,
            raw_hsaco: None,
            finalized: None,
        };
        let checkpoint = recorder.checkpoint();
        Ok((recorder, checkpoint))
    }

    pub fn record_compiler(
        &mut self,
        checkpoint: CompilerTransactionCheckpointV1,
        invocation: ExactCompilerInvocationV1,
    ) -> Result<CompilerTransactionCheckpointV1, CompilerTransactionRecorderErrorV1> {
        self.authorize(checkpoint, CompilerTransactionStageV1::Source)?;
        let payload = compiler_checkpoint_payload(&invocation);
        self.invocation = Some(invocation);
        Ok(self.advance(CompilerTransactionStageV1::Compiler, &payload))
    }

    pub fn record_target(
        &mut self,
        checkpoint: CompilerTransactionCheckpointV1,
        target: Gfx942CompilerTargetV1,
    ) -> Result<CompilerTransactionCheckpointV1, CompilerTransactionRecorderErrorV1> {
        self.authorize(checkpoint, CompilerTransactionStageV1::Compiler)?;
        let invocation = self
            .invocation
            .as_ref()
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;
        if target.amd_target != invocation.amd_target {
            return Err(CompilerTransactionRecorderErrorV1::MixedTarget);
        }
        let measurement = target.measurement.into_bytes();
        self.target = Some(target);
        Ok(self.advance(CompilerTransactionStageV1::Target, &[measurement]))
    }

    pub fn record_semantic_layouts(
        &mut self,
        checkpoint: CompilerTransactionCheckpointV1,
        semantic_layouts: AlphaZetaSemanticLayoutWitnessesV1,
    ) -> Result<CompilerTransactionCheckpointV1, CompilerTransactionRecorderErrorV1> {
        self.authorize(checkpoint, CompilerTransactionStageV1::Target)?;
        let identity = semantic_layouts.identity.into_bytes();
        self.semantic_layouts = Some(semantic_layouts);
        Ok(self.advance(CompilerTransactionStageV1::SemanticLayouts, &[identity]))
    }

    pub fn record_kernel_ir(
        &mut self,
        checkpoint: CompilerTransactionCheckpointV1,
        canonical_kernel_ir: &[u8],
    ) -> Result<CompilerTransactionCheckpointV1, CompilerTransactionRecorderErrorV1> {
        self.authorize(checkpoint, CompilerTransactionStageV1::SemanticLayouts)?;
        require_nonempty("Kernel IR", canonical_kernel_ir)?;
        let identity = CompilerTransactionContentIdentityV1::measure(canonical_kernel_ir);
        self.kernel_ir = Some(identity);
        Ok(self.advance(
            CompilerTransactionStageV1::KernelIr,
            &[identity.into_bytes()],
        ))
    }

    pub fn record_worker_exchange(
        &mut self,
        checkpoint: CompilerTransactionCheckpointV1,
        canonical_request: &[u8],
        canonical_response: &[u8],
    ) -> Result<CompilerTransactionCheckpointV1, CompilerTransactionRecorderErrorV1> {
        self.authorize(checkpoint, CompilerTransactionStageV1::KernelIr)?;
        let request = WorkerRequestV1::decode(canonical_request)
            .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidWorkerRequest)?;
        if request.target().to_string() != GFX942_AMD_TARGET {
            return Err(CompilerTransactionRecorderErrorV1::WorkerTargetMismatch);
        }
        if request.code_object_version() != GFX942_CODE_OBJECT_VERSION {
            return Err(CompilerTransactionRecorderErrorV1::WorkerCodeObjectVersionMismatch);
        }
        if request.expected_defined_symbols() != ["alpha", "zeta"] {
            return Err(CompilerTransactionRecorderErrorV1::WorkerKernelSetMismatch);
        }
        let response = WorkerResponseV1::decode(canonical_response)
            .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidWorkerResponse)?;
        if !response.binds_request(&request) {
            return Err(CompilerTransactionRecorderErrorV1::WorkerResponseMismatch);
        }
        if response.stage() != WorkerStageV1::Complete || response.output().is_none() {
            return Err(CompilerTransactionRecorderErrorV1::WorkerResponseIncomplete);
        }
        let request_measurement = CompilerTransactionContentIdentityV1(*request.identity());
        let response_measurement =
            CompilerTransactionContentIdentityV1::measure(response.canonical_bytes());
        self.worker_exchange = Some(ValidatedWorkerExchangeV1 {
            response,
            request_measurement,
            response_measurement,
        });
        Ok(self.advance(
            CompilerTransactionStageV1::WorkerExchange,
            &[
                request_measurement.into_bytes(),
                response_measurement.into_bytes(),
            ],
        ))
    }

    pub fn record_raw_hsaco(
        &mut self,
        checkpoint: CompilerTransactionCheckpointV1,
        raw_hsaco: &[u8],
    ) -> Result<CompilerTransactionCheckpointV1, CompilerTransactionRecorderErrorV1> {
        self.authorize(checkpoint, CompilerTransactionStageV1::WorkerExchange)?;
        let exchange = self
            .worker_exchange
            .as_ref()
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;
        let output = exchange
            .response
            .output()
            .ok_or(CompilerTransactionRecorderErrorV1::WorkerResponseIncomplete)?;
        if output.bytes() != raw_hsaco {
            return Err(CompilerTransactionRecorderErrorV1::WorkerOutputMismatch);
        }
        let inspection = inspect_unfinalized(raw_hsaco)
            .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidRawHsaco)?;
        validate_descriptor_profile(inspection.descriptor_table())?;
        let descriptor_source = encode_device_descriptor_table_v1(inspection.descriptor_table())
            .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidDescriptorSource)?;
        let identity = CompilerTransactionContentIdentityV1::measure(raw_hsaco);
        self.raw_hsaco = Some(ValidatedRawHsacoV1 {
            bytes: raw_hsaco.to_vec(),
            measurement: identity,
            descriptor_source,
        });
        Ok(self.advance(
            CompilerTransactionStageV1::RawHsaco,
            &[identity.into_bytes()],
        ))
    }

    /// Validates the finalization and artifact closure before accepting its canonical target.
    ///
    /// `canonical_target` must come from the caller's already validated artifact-transaction
    /// scope. This inert recorder binds that shared identity but neither derives nor authenticates
    /// the publication scope that assigned it.
    pub fn record_finalized_artifact(
        &mut self,
        checkpoint: CompilerTransactionCheckpointV1,
        finalized_hsaco: &[u8],
        descriptor_source: &[u8],
        finalized_descriptor: &[u8],
        artifact_container: &[u8],
        canonical_target: TargetIdentityV1,
    ) -> Result<CompilerTransactionCheckpointV1, CompilerTransactionRecorderErrorV1> {
        self.authorize(checkpoint, CompilerTransactionStageV1::RawHsaco)?;
        if canonical_target.as_bytes() == &[0; 32] {
            return Err(CompilerTransactionRecorderErrorV1::ReservedZeroCanonicalTarget);
        }
        let raw = self
            .raw_hsaco
            .as_ref()
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;
        let decoded_source = decode_canonical_descriptor(
            descriptor_source,
            CompilerTransactionRecorderErrorV1::InvalidDescriptorSource,
        )?;
        validate_descriptor_profile(&decoded_source)?;
        if decoded_source.canonical_code_object_digest().as_bytes() != &[0; 32]
            || descriptor_source != raw.descriptor_source
        {
            return Err(CompilerTransactionRecorderErrorV1::DescriptorSourceMismatch);
        }
        let expected = finalize_unfinalized(&raw.bytes)
            .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidRawHsaco)?;
        if expected.as_bytes() != finalized_hsaco {
            return Err(CompilerTransactionRecorderErrorV1::FinalizedHsacoMismatch);
        }
        let decoded_final = decode_canonical_descriptor(
            finalized_descriptor,
            CompilerTransactionRecorderErrorV1::InvalidFinalizedDescriptor,
        )?;
        validate_descriptor_profile(&decoded_final)?;
        let expected_descriptor =
            encode_device_descriptor_table_v1(expected.inspection().descriptor_table())
                .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidFinalizedDescriptor)?;
        if finalized_descriptor != expected_descriptor
            || decoded_final.canonical_code_object_digest().as_bytes() == &[0; 32]
        {
            return Err(CompilerTransactionRecorderErrorV1::FinalizedDescriptorMismatch);
        }
        let container = ArtifactContainerV1::from_bytes(artifact_container)
            .map_err(|_| CompilerTransactionRecorderErrorV1::InvalidArtifactContainer)?;
        if container.to_bytes() != artifact_container {
            return Err(CompilerTransactionRecorderErrorV1::InvalidArtifactContainer);
        }
        validate_artifact_container(
            &container,
            finalized_hsaco,
            expected.inspection().descriptor_table(),
        )?;
        let finalized = FinalizedCompilerArtifactMeasurementsV1 {
            finalized_hsaco: CompilerTransactionContentIdentityV1::measure(finalized_hsaco),
            descriptor_source: CompilerTransactionContentIdentityV1::measure(descriptor_source),
            finalized_descriptor: CompilerTransactionContentIdentityV1::measure(
                finalized_descriptor,
            ),
            artifact: CompilerTransactionContentIdentityV1::measure(artifact_container),
            canonical_target,
        };
        let payload = [
            finalized.finalized_hsaco.into_bytes(),
            finalized.descriptor_source.into_bytes(),
            finalized.finalized_descriptor.into_bytes(),
            finalized.artifact.into_bytes(),
        ];
        self.finalized = Some(finalized);
        Ok(self.advance(CompilerTransactionStageV1::FinalizedArtifact, &payload))
    }

    pub fn seal(
        &mut self,
        checkpoint: CompilerTransactionCheckpointV1,
    ) -> Result<SealedCompilerTransactionV1, CompilerTransactionRecorderErrorV1> {
        self.authorize(checkpoint, CompilerTransactionStageV1::FinalizedArtifact)?;
        let invocation = self
            .invocation
            .as_ref()
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;
        let target = self
            .target
            .as_ref()
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;
        let semantic = self
            .semantic_layouts
            .as_ref()
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;
        let kernel_ir = self
            .kernel_ir
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;
        let worker_exchange = self
            .worker_exchange
            .as_ref()
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;
        let raw_hsaco = self
            .raw_hsaco
            .as_ref()
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;
        let finalized = self
            .finalized
            .ok_or(CompilerTransactionRecorderErrorV1::MissingStage)?;

        let capsule =
            CompilerTransactionEvidenceCapsuleV2::new(CompilerTransactionEvidencePartsV2 {
                source_closure: self.source.capsule_closure.clone(),
                rustc_tool: invocation.rustc_tool.measured_identity(),
                rustc_invocation: invocation.rustc_invocation,
                backend_tool: invocation.backend_tool.measured_identity(),
                backend_invocation: CallerMeasuredBackendInvocationIdentityV2::try_from_sha256(
                    invocation.backend_invocation.into_bytes(),
                )
                .map_err(CompilerTransactionRecorderErrorV1::Capsule)?,
                semantic_witness: CallerMeasuredSemanticWitnessIdentityV2::try_from_sha256(
                    semantic.identity.into_bytes(),
                )
                .map_err(CompilerTransactionRecorderErrorV1::Capsule)?,
                kernel_ir: CallerMeasuredKernelIrIdentityV2::try_from_sha256(
                    kernel_ir.into_bytes(),
                )
                .map_err(CompilerTransactionRecorderErrorV1::Capsule)?,
                worker_request: DirectLinkRequestIdentityV1::new(payload_digest(
                    worker_exchange.request_measurement,
                )),
                worker_response: DirectLinkResponseIdentityV1::new(payload_digest(
                    worker_exchange.response_measurement,
                )),
                target: finalized.canonical_target,
                raw_hsaco: DirectLinkLinkedOutputIdentityV1::new(payload_digest(
                    raw_hsaco.measurement,
                )),
                finalized_hsaco: DirectLinkFinalizedPayloadIdentityV1::new(payload_digest(
                    finalized.finalized_hsaco,
                )),
                artifact: DirectLinkContainerIdentityV1::new(payload_digest(finalized.artifact)),
            })
            .map_err(CompilerTransactionRecorderErrorV1::Capsule)?;
        let sealed_chain = calculate_checkpoint(
            self.transaction,
            self.chain,
            CompilerTransactionStageV1::Sealed,
            &[*capsule.identity().as_bytes()],
        );
        let measurements = CompilerTransactionMeasurementsV1 {
            source_tree: self.source.tree_identity,
            rustc_descriptor: invocation.rustc_descriptor,
            rustc_executable: invocation.rustc_tool.executable,
            rustc_configuration: invocation.rustc_tool.configuration,
            backend_executable: invocation.backend_tool.executable,
            backend_configuration: invocation.backend_tool.configuration,
            backend_invocation: invocation.backend_invocation,
            target_profile: target.measurement,
            semantic_layouts: semantic.identity,
            kernel_ir,
            worker_request: worker_exchange.request_measurement,
            worker_response: worker_exchange.response_measurement,
            raw_hsaco: raw_hsaco.measurement,
            finalized_hsaco: finalized.finalized_hsaco,
            descriptor_source: finalized.descriptor_source,
            finalized_descriptor: finalized.finalized_descriptor,
            artifact: finalized.artifact,
        };
        let sealed = SealedCompilerTransactionV1::from_recorded(
            self.freshness,
            sealed_chain,
            measurements,
            capsule,
        )?;
        self.chain = sealed_chain;
        self.stage = CompilerTransactionStageV1::Sealed;
        Ok(sealed)
    }

    fn checkpoint(&self) -> CompilerTransactionCheckpointV1 {
        CompilerTransactionCheckpointV1 {
            transaction: self.transaction,
            stage: self.stage,
            chain: self.chain,
        }
    }

    fn authorize(
        &self,
        checkpoint: CompilerTransactionCheckpointV1,
        expected: CompilerTransactionStageV1,
    ) -> Result<(), CompilerTransactionRecorderErrorV1> {
        if self.stage != expected {
            return Err(CompilerTransactionRecorderErrorV1::UnexpectedStage {
                expected,
                actual: self.stage,
            });
        }
        if checkpoint.transaction != self.transaction {
            return Err(CompilerTransactionRecorderErrorV1::MixedTransaction);
        }
        if checkpoint.stage != self.stage || checkpoint.chain != self.chain {
            return Err(CompilerTransactionRecorderErrorV1::StaleCheckpoint);
        }
        Ok(())
    }

    fn advance(
        &mut self,
        stage: CompilerTransactionStageV1,
        payload: &[[u8; 32]],
    ) -> CompilerTransactionCheckpointV1 {
        self.chain = calculate_checkpoint(self.transaction, self.chain, stage, payload);
        self.stage = stage;
        self.checkpoint()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerTransactionMeasurementsV1 {
    source_tree: CompilerTransactionContentIdentityV1,
    rustc_descriptor: CompilerTransactionContentIdentityV1,
    rustc_executable: CompilerTransactionContentIdentityV1,
    rustc_configuration: CompilerTransactionContentIdentityV1,
    backend_executable: CompilerTransactionContentIdentityV1,
    backend_configuration: CompilerTransactionContentIdentityV1,
    backend_invocation: CompilerTransactionContentIdentityV1,
    target_profile: CompilerTransactionContentIdentityV1,
    semantic_layouts: CompilerTransactionContentIdentityV1,
    kernel_ir: CompilerTransactionContentIdentityV1,
    worker_request: CompilerTransactionContentIdentityV1,
    worker_response: CompilerTransactionContentIdentityV1,
    raw_hsaco: CompilerTransactionContentIdentityV1,
    finalized_hsaco: CompilerTransactionContentIdentityV1,
    descriptor_source: CompilerTransactionContentIdentityV1,
    finalized_descriptor: CompilerTransactionContentIdentityV1,
    artifact: CompilerTransactionContentIdentityV1,
}

impl CompilerTransactionMeasurementsV1 {
    pub const fn source_tree(self) -> CompilerTransactionContentIdentityV1 {
        self.source_tree
    }

    pub const fn rustc_descriptor(self) -> CompilerTransactionContentIdentityV1 {
        self.rustc_descriptor
    }

    pub const fn rustc_executable(self) -> CompilerTransactionContentIdentityV1 {
        self.rustc_executable
    }

    pub const fn rustc_configuration(self) -> CompilerTransactionContentIdentityV1 {
        self.rustc_configuration
    }

    pub const fn backend_executable(self) -> CompilerTransactionContentIdentityV1 {
        self.backend_executable
    }

    pub const fn backend_configuration(self) -> CompilerTransactionContentIdentityV1 {
        self.backend_configuration
    }

    pub const fn backend_invocation(self) -> CompilerTransactionContentIdentityV1 {
        self.backend_invocation
    }

    pub const fn target_profile(self) -> CompilerTransactionContentIdentityV1 {
        self.target_profile
    }

    pub const fn semantic_layouts(self) -> CompilerTransactionContentIdentityV1 {
        self.semantic_layouts
    }

    pub const fn kernel_ir(self) -> CompilerTransactionContentIdentityV1 {
        self.kernel_ir
    }

    pub const fn worker_request(self) -> CompilerTransactionContentIdentityV1 {
        self.worker_request
    }

    pub const fn worker_response(self) -> CompilerTransactionContentIdentityV1 {
        self.worker_response
    }

    pub const fn raw_hsaco(self) -> CompilerTransactionContentIdentityV1 {
        self.raw_hsaco
    }

    pub const fn finalized_hsaco(self) -> CompilerTransactionContentIdentityV1 {
        self.finalized_hsaco
    }

    pub const fn descriptor_source(self) -> CompilerTransactionContentIdentityV1 {
        self.descriptor_source
    }

    pub const fn finalized_descriptor(self) -> CompilerTransactionContentIdentityV1 {
        self.finalized_descriptor
    }

    pub const fn artifact(self) -> CompilerTransactionContentIdentityV1 {
        self.artifact
    }

    fn as_array(self) -> [CompilerTransactionContentIdentityV1; MEASUREMENT_COUNT] {
        [
            self.source_tree,
            self.rustc_descriptor,
            self.rustc_executable,
            self.rustc_configuration,
            self.backend_executable,
            self.backend_configuration,
            self.backend_invocation,
            self.target_profile,
            self.semantic_layouts,
            self.kernel_ir,
            self.worker_request,
            self.worker_response,
            self.raw_hsaco,
            self.finalized_hsaco,
            self.descriptor_source,
            self.finalized_descriptor,
            self.artifact,
        ]
    }

    fn from_array(values: [CompilerTransactionContentIdentityV1; MEASUREMENT_COUNT]) -> Self {
        let [
            source_tree,
            rustc_descriptor,
            rustc_executable,
            rustc_configuration,
            backend_executable,
            backend_configuration,
            backend_invocation,
            target_profile,
            semantic_layouts,
            kernel_ir,
            worker_request,
            worker_response,
            raw_hsaco,
            finalized_hsaco,
            descriptor_source,
            finalized_descriptor,
            artifact,
        ] = values;
        Self {
            source_tree,
            rustc_descriptor,
            rustc_executable,
            rustc_configuration,
            backend_executable,
            backend_configuration,
            backend_invocation,
            target_profile,
            semantic_layouts,
            kernel_ir,
            worker_request,
            worker_response,
            raw_hsaco,
            finalized_hsaco,
            descriptor_source,
            finalized_descriptor,
            artifact,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedCompilerTransactionV1 {
    freshness: [u8; 32],
    final_chain: [u8; 32],
    measurements: CompilerTransactionMeasurementsV1,
    capsule: CompilerTransactionEvidenceCapsuleV2,
    identity: SealedCompilerTransactionIdentityV1,
    encoded_len: usize,
}

impl SealedCompilerTransactionV1 {
    fn from_recorded(
        freshness: [u8; 32],
        final_chain: [u8; 32],
        measurements: CompilerTransactionMeasurementsV1,
        capsule: CompilerTransactionEvidenceCapsuleV2,
    ) -> Result<Self, CompilerTransactionRecorderErrorV1> {
        let encoded_len = HEADER_BYTES
            .checked_add(FIXED_BODY_BYTES)
            .and_then(|length| length.checked_add(capsule.to_bytes().len()))
            .ok_or(CompilerTransactionRecorderErrorV1::RecordTooLarge {
                max: MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1,
            })?;
        if encoded_len > MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1 {
            return Err(CompilerTransactionRecorderErrorV1::RecordTooLarge {
                max: MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1,
            });
        }
        let mut record = Self {
            freshness,
            final_chain,
            measurements,
            capsule,
            identity: SealedCompilerTransactionIdentityV1([1; 32]),
            encoded_len,
        };
        let prefix = record.encode_prefix();
        record.identity = calculate_record_identity(&prefix)
            .map_err(|_| CompilerTransactionRecorderErrorV1::ReservedZeroRecordIdentity)?;
        Ok(record)
    }

    pub const fn identity(&self) -> SealedCompilerTransactionIdentityV1 {
        self.identity
    }

    pub const fn freshness_binding(&self) -> &[u8; 32] {
        &self.freshness
    }

    pub const fn final_checkpoint_identity(&self) -> &[u8; 32] {
        &self.final_chain
    }

    pub const fn measurements(&self) -> CompilerTransactionMeasurementsV1 {
        self.measurements
    }

    pub const fn evidence_capsule(&self) -> &CompilerTransactionEvidenceCapsuleV2 {
        &self.capsule
    }

    pub const fn authenticates_producer(&self) -> bool {
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

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.encode_prefix();
        bytes.extend_from_slice(self.identity.as_bytes());
        debug_assert_eq!(bytes.len(), self.encoded_len);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SealedCompilerTransactionDecodeErrorV1> {
        decode_sealed_record(bytes, None)
    }

    pub fn from_bytes_for_identity(
        bytes: &[u8],
        expected: SealedCompilerTransactionIdentityV1,
    ) -> Result<Self, SealedCompilerTransactionDecodeErrorV1> {
        decode_sealed_record(bytes, Some(expected))
    }

    fn encode_prefix(&self) -> Vec<u8> {
        let capsule = self.capsule.to_bytes();
        let mut bytes = Vec::with_capacity(self.encoded_len);
        bytes.extend_from_slice(&SEALED_COMPILER_TRANSACTION_MAGIC_V1);
        bytes.extend_from_slice(&SEALED_COMPILER_TRANSACTION_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(self.encoded_len as u32).to_le_bytes());
        bytes.extend_from_slice(&self.freshness);
        for measurement in self.measurements.as_array() {
            bytes.extend_from_slice(measurement.as_bytes());
        }
        bytes.extend_from_slice(&self.final_chain);
        bytes.extend_from_slice(&(capsule.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&capsule);
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerTransactionRecorderErrorV1 {
    EmptyInput {
        field: &'static str,
    },
    SourceFileTooLarge {
        max: usize,
    },
    SourceClosureTooLarge {
        max: usize,
    },
    TooManySourceFiles {
        max: usize,
    },
    TooManyFeatures {
        max: usize,
    },
    DuplicateSourcePath,
    RootRepeatedAsDependency,
    DuplicateFeature,
    DuplicateSemanticWitness,
    MissingAlphaZetaWitnesses,
    InvalidRustcDescriptor,
    NonCanonicalRustcDescriptor,
    RustcExecutableMismatch,
    BackendExecutableMismatch,
    UnsupportedTarget,
    MixedTarget,
    InvalidWorkerRequest,
    InvalidWorkerResponse,
    WorkerTargetMismatch,
    WorkerCodeObjectVersionMismatch,
    WorkerKernelSetMismatch,
    WorkerResponseMismatch,
    WorkerResponseIncomplete,
    WorkerOutputMismatch,
    InvalidRawHsaco,
    DescriptorCodeObjectVersionMismatch,
    DescriptorTargetMismatch,
    DescriptorKernelSetMismatch,
    DescriptorCapabilityMismatch,
    InvalidDescriptorSource,
    DescriptorSourceMismatch,
    InvalidFinalizedDescriptor,
    FinalizedHsacoMismatch,
    FinalizedDescriptorMismatch,
    InvalidArtifactContainer,
    ArtifactTargetMismatch,
    ArtifactCapabilityMismatch,
    ArtifactPayloadMismatch,
    ArtifactKernelSetMismatch,
    ReservedZeroCanonicalTarget,
    ReservedZeroFreshness,
    UnexpectedStage {
        expected: CompilerTransactionStageV1,
        actual: CompilerTransactionStageV1,
    },
    MixedTransaction,
    StaleCheckpoint,
    MissingStage,
    RecordTooLarge {
        max: usize,
    },
    ReservedZeroRecordIdentity,
    Capsule(CompilerTransactionValidationErrorV2),
}

impl fmt::Display for CompilerTransactionRecorderErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput { field } => write!(formatter, "{field} must not be empty"),
            Self::SourceFileTooLarge { max } => {
                write!(formatter, "compiler source file exceeds {max} bytes")
            }
            Self::SourceClosureTooLarge { max } => {
                write!(formatter, "compiler source closure exceeds {max} bytes")
            }
            Self::TooManySourceFiles { max } => {
                write!(
                    formatter,
                    "compiler source closure exceeds {max} dependencies"
                )
            }
            Self::TooManyFeatures { max } => {
                write!(formatter, "compiler source closure exceeds {max} features")
            }
            Self::DuplicateSourcePath => {
                formatter.write_str("compiler source closure contains a duplicate path")
            }
            Self::RootRepeatedAsDependency => {
                formatter.write_str("compiler source root is repeated as a dependency")
            }
            Self::DuplicateFeature => {
                formatter.write_str("compiler source closure contains a duplicate feature")
            }
            Self::DuplicateSemanticWitness => {
                formatter.write_str("semantic layout witnesses contain a duplicate kernel")
            }
            Self::MissingAlphaZetaWitnesses => {
                formatter.write_str("semantic layout witnesses must contain exactly alpha and zeta")
            }
            Self::InvalidRustcDescriptor => {
                formatter.write_str("rustc invocation descriptor is invalid")
            }
            Self::NonCanonicalRustcDescriptor => {
                formatter.write_str("rustc invocation descriptor is not canonical")
            }
            Self::RustcExecutableMismatch => {
                formatter.write_str("rustc descriptor does not match measured rustc bytes")
            }
            Self::BackendExecutableMismatch => {
                formatter.write_str("rustc descriptor does not match measured backend bytes")
            }
            Self::UnsupportedTarget => {
                formatter.write_str("compiler transaction is not for canonical gfx942:xnack-")
            }
            Self::MixedTarget => {
                formatter.write_str("target profile does not match the compiler invocation")
            }
            Self::InvalidWorkerRequest => formatter.write_str("Worker request is invalid"),
            Self::InvalidWorkerResponse => formatter.write_str("Worker response is invalid"),
            Self::WorkerTargetMismatch => {
                formatter.write_str("Worker request target is not gfx942:xnack-")
            }
            Self::WorkerCodeObjectVersionMismatch => {
                formatter.write_str("Worker request does not require code object V6")
            }
            Self::WorkerKernelSetMismatch => {
                formatter.write_str("Worker request does not define exactly alpha and zeta")
            }
            Self::WorkerResponseMismatch => {
                formatter.write_str("Worker response does not bind the recorded request")
            }
            Self::WorkerResponseIncomplete => {
                formatter.write_str("Worker response is not a complete successful response")
            }
            Self::WorkerOutputMismatch => {
                formatter.write_str("raw HSACO does not equal the Worker response output")
            }
            Self::InvalidRawHsaco => formatter.write_str("raw HSACO is invalid"),
            Self::DescriptorCodeObjectVersionMismatch => {
                formatter.write_str("descriptor table does not require code object V6")
            }
            Self::DescriptorTargetMismatch => {
                formatter.write_str("descriptor table target is not gfx942:xnack-")
            }
            Self::DescriptorKernelSetMismatch => {
                formatter.write_str("descriptor table does not contain exactly alpha and zeta")
            }
            Self::DescriptorCapabilityMismatch => {
                formatter.write_str("descriptor kernel capabilities do not match the fixed profile")
            }
            Self::InvalidDescriptorSource => {
                formatter.write_str("descriptor source is invalid or noncanonical")
            }
            Self::DescriptorSourceMismatch => {
                formatter.write_str("descriptor source does not equal the raw HSACO table")
            }
            Self::InvalidFinalizedDescriptor => {
                formatter.write_str("finalized descriptor is invalid or noncanonical")
            }
            Self::FinalizedHsacoMismatch => {
                formatter.write_str("finalized HSACO is not the deterministic raw-HSACO result")
            }
            Self::FinalizedDescriptorMismatch => {
                formatter.write_str("finalized descriptor does not equal the finalized HSACO table")
            }
            Self::InvalidArtifactContainer => {
                formatter.write_str("artifact container is invalid or noncanonical")
            }
            Self::ArtifactTargetMismatch => {
                formatter.write_str("artifact target is not the fixed gfx942:xnack- profile")
            }
            Self::ArtifactCapabilityMismatch => {
                formatter.write_str("artifact capabilities do not match the fixed profile")
            }
            Self::ArtifactPayloadMismatch => {
                formatter.write_str("artifact payload does not equal the finalized HSACO")
            }
            Self::ArtifactKernelSetMismatch => {
                formatter.write_str("artifact kernels do not match the alpha/zeta descriptors")
            }
            Self::ReservedZeroCanonicalTarget => {
                formatter.write_str("canonical target identity must not be all zero")
            }
            Self::ReservedZeroFreshness => {
                formatter.write_str("compiler transaction freshness must not be all zero")
            }
            Self::UnexpectedStage { expected, actual } => {
                write!(
                    formatter,
                    "expected recorder stage {expected:?}, found {actual:?}"
                )
            }
            Self::MixedTransaction => {
                formatter.write_str("checkpoint belongs to a different compiler transaction")
            }
            Self::StaleCheckpoint => formatter.write_str("checkpoint is stale"),
            Self::MissingStage => formatter.write_str("compiler transaction has a missing stage"),
            Self::RecordTooLarge { max } => {
                write!(formatter, "sealed compiler transaction exceeds {max} bytes")
            }
            Self::ReservedZeroRecordIdentity => {
                formatter.write_str("sealed transaction has a reserved all-zero identity")
            }
            Self::Capsule(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompilerTransactionRecorderErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capsule(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum SealedCompilerTransactionDecodeErrorV1 {
    TooLarge { max: usize },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidTotalLength,
    TrailingBytes,
    InvalidCapsuleLength,
    ReservedZeroIdentity { field: &'static str },
    Capsule(crate::CompilerTransactionDecodeErrorV2),
    MeasurementMismatch { field: &'static str },
    CheckpointMismatch,
    RecordIdentityMismatch,
    UnexpectedRecordIdentity,
    NonCanonical,
}

impl fmt::Display for SealedCompilerTransactionDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => {
                write!(formatter, "sealed compiler transaction exceeds {max} bytes")
            }
            Self::Truncated => formatter.write_str("sealed compiler transaction is truncated"),
            Self::InvalidMagic => {
                formatter.write_str("sealed compiler transaction magic is invalid")
            }
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unsupported sealed compiler transaction version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported sealed compiler transaction flags {flags:#x}"
                )
            }
            Self::InvalidTotalLength => {
                formatter.write_str("sealed compiler transaction total length is invalid")
            }
            Self::TrailingBytes => {
                formatter.write_str("sealed compiler transaction has trailing bytes")
            }
            Self::InvalidCapsuleLength => {
                formatter.write_str("sealed compiler transaction capsule length is invalid")
            }
            Self::ReservedZeroIdentity { field } => {
                write!(formatter, "{field} uses the reserved all-zero identity")
            }
            Self::Capsule(error) => write!(formatter, "invalid evidence capsule: {error}"),
            Self::MeasurementMismatch { field } => {
                write!(
                    formatter,
                    "sealed transaction {field} measurement does not match"
                )
            }
            Self::CheckpointMismatch => {
                formatter.write_str("sealed transaction checkpoint chain does not match")
            }
            Self::RecordIdentityMismatch => {
                formatter.write_str("sealed transaction identity does not match its bytes")
            }
            Self::UnexpectedRecordIdentity => {
                formatter.write_str("sealed transaction is stale or substituted")
            }
            Self::NonCanonical => {
                formatter.write_str("sealed compiler transaction is not canonical")
            }
        }
    }
}

impl std::error::Error for SealedCompilerTransactionDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capsule(error) => Some(error),
            _ => None,
        }
    }
}

fn decode_sealed_record(
    bytes: &[u8],
    expected: Option<SealedCompilerTransactionIdentityV1>,
) -> Result<SealedCompilerTransactionV1, SealedCompilerTransactionDecodeErrorV1> {
    if bytes.len() > MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1 {
        return Err(SealedCompilerTransactionDecodeErrorV1::TooLarge {
            max: MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1,
        });
    }
    if bytes.len() < HEADER_BYTES + FIXED_BODY_BYTES {
        return Err(SealedCompilerTransactionDecodeErrorV1::Truncated);
    }
    let mut reader = SealedRecordReader::new(bytes);
    if reader.array::<8>()? != SEALED_COMPILER_TRANSACTION_MAGIC_V1 {
        return Err(SealedCompilerTransactionDecodeErrorV1::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != SEALED_COMPILER_TRANSACTION_VERSION_V1 {
        return Err(SealedCompilerTransactionDecodeErrorV1::UnknownVersion(
            version,
        ));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(SealedCompilerTransactionDecodeErrorV1::UnsupportedFlags(
            flags,
        ));
    }
    let total_len = reader.u32()? as usize;
    if total_len > bytes.len() {
        return Err(SealedCompilerTransactionDecodeErrorV1::Truncated);
    }
    if total_len < bytes.len() {
        return Err(SealedCompilerTransactionDecodeErrorV1::TrailingBytes);
    }
    if total_len != bytes.len() {
        return Err(SealedCompilerTransactionDecodeErrorV1::InvalidTotalLength);
    }
    let freshness = reader.nonzero_identity("freshness")?;
    let mut values = [CompilerTransactionContentIdentityV1([1; 32]); MEASUREMENT_COUNT];
    for (index, value) in values.iter_mut().enumerate() {
        *value = CompilerTransactionContentIdentityV1::from_nonzero(
            reader.array()?,
            measurement_name(index),
        )?;
    }
    let measurements = CompilerTransactionMeasurementsV1::from_array(values);
    let final_chain = reader.nonzero_identity("final checkpoint")?;
    let capsule_len = reader.u32()? as usize;
    if capsule_len == 0
        || capsule_len > MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V2
        || capsule_len > reader.remaining().saturating_sub(IDENTITY_BYTES)
    {
        return Err(SealedCompilerTransactionDecodeErrorV1::InvalidCapsuleLength);
    }
    let capsule_bytes = reader.take(capsule_len)?;
    let identity_prefix_len = reader.position();
    let encoded_identity = SealedCompilerTransactionIdentityV1::from_bytes(reader.array()?)?;
    if !reader.is_empty() {
        return Err(SealedCompilerTransactionDecodeErrorV1::TrailingBytes);
    }
    let calculated_identity = calculate_record_identity(&bytes[..identity_prefix_len])?;
    if calculated_identity != encoded_identity {
        return Err(SealedCompilerTransactionDecodeErrorV1::RecordIdentityMismatch);
    }
    if expected.is_some_and(|value| value != calculated_identity) {
        return Err(SealedCompilerTransactionDecodeErrorV1::UnexpectedRecordIdentity);
    }
    let capsule = CompilerTransactionEvidenceCapsuleV2::from_bytes(capsule_bytes)
        .map_err(SealedCompilerTransactionDecodeErrorV1::Capsule)?;
    validate_measurements(measurements, &capsule)?;
    let expected_chain = reconstruct_final_chain(freshness, measurements, &capsule);
    if final_chain != expected_chain {
        return Err(SealedCompilerTransactionDecodeErrorV1::CheckpointMismatch);
    }
    let record = SealedCompilerTransactionV1 {
        freshness,
        final_chain,
        measurements,
        capsule,
        identity: encoded_identity,
        encoded_len: bytes.len(),
    };
    if record.to_bytes() != bytes {
        return Err(SealedCompilerTransactionDecodeErrorV1::NonCanonical);
    }
    Ok(record)
}

fn validate_measurements(
    values: CompilerTransactionMeasurementsV1,
    capsule: &CompilerTransactionEvidenceCapsuleV2,
) -> Result<(), SealedCompilerTransactionDecodeErrorV1> {
    let checks = [
        (
            "rustc executable",
            values.rustc_executable,
            content_from_payload(capsule.rustc_tool().executable_digest())?,
        ),
        (
            "rustc configuration",
            values.rustc_configuration,
            content_from_payload(capsule.rustc_tool().configuration_digest())?,
        ),
        (
            "backend executable",
            values.backend_executable,
            content_from_payload(capsule.backend_tool().executable_digest())?,
        ),
        (
            "backend configuration",
            values.backend_configuration,
            content_from_payload(capsule.backend_tool().configuration_digest())?,
        ),
        (
            "backend invocation",
            values.backend_invocation,
            content_from_payload(capsule.backend_invocation().digest())?,
        ),
        (
            "semantic layouts",
            values.semantic_layouts,
            content_from_payload(capsule.semantic_witness().digest())?,
        ),
        (
            "Kernel IR",
            values.kernel_ir,
            content_from_payload(capsule.kernel_ir().digest())?,
        ),
        (
            "Worker V2 request",
            values.worker_request,
            content_from_payload(capsule.worker_request().digest())?,
        ),
        (
            "Worker V2 response",
            values.worker_response,
            content_from_payload(capsule.worker_response().digest())?,
        ),
        (
            "raw HSACO",
            values.raw_hsaco,
            content_from_payload(capsule.raw_hsaco().digest())?,
        ),
        (
            "finalized HSACO",
            values.finalized_hsaco,
            content_from_payload(capsule.finalized_hsaco().digest())?,
        ),
        (
            "artifact",
            values.artifact,
            content_from_payload(capsule.artifact().digest())?,
        ),
    ];
    for (field, measured, expected) in checks {
        if measured != expected {
            return Err(SealedCompilerTransactionDecodeErrorV1::MeasurementMismatch { field });
        }
    }
    Ok(())
}

fn reconstruct_final_chain(
    freshness: [u8; 32],
    values: CompilerTransactionMeasurementsV1,
    capsule: &CompilerTransactionEvidenceCapsuleV2,
) -> [u8; 32] {
    let transaction = calculate_transaction_identity(
        freshness,
        values.source_tree,
        capsule.source_closure().identity(),
    );
    let mut chain = calculate_checkpoint(
        transaction,
        [0; 32],
        CompilerTransactionStageV1::Source,
        &[
            values.source_tree.into_bytes(),
            *capsule.source_closure().identity().as_bytes(),
        ],
    );
    let compiler = [
        values.rustc_descriptor.into_bytes(),
        values.rustc_executable.into_bytes(),
        values.rustc_configuration.into_bytes(),
        *capsule.rustc_invocation().as_bytes(),
        values.backend_executable.into_bytes(),
        values.backend_configuration.into_bytes(),
        values.backend_invocation.into_bytes(),
    ];
    chain = calculate_checkpoint(
        transaction,
        chain,
        CompilerTransactionStageV1::Compiler,
        &compiler,
    );
    for (stage, payload) in [
        (
            CompilerTransactionStageV1::Target,
            vec![values.target_profile.into_bytes()],
        ),
        (
            CompilerTransactionStageV1::SemanticLayouts,
            vec![values.semantic_layouts.into_bytes()],
        ),
        (
            CompilerTransactionStageV1::KernelIr,
            vec![values.kernel_ir.into_bytes()],
        ),
        (
            CompilerTransactionStageV1::WorkerExchange,
            vec![
                values.worker_request.into_bytes(),
                values.worker_response.into_bytes(),
            ],
        ),
        (
            CompilerTransactionStageV1::RawHsaco,
            vec![values.raw_hsaco.into_bytes()],
        ),
        (
            CompilerTransactionStageV1::FinalizedArtifact,
            vec![
                values.finalized_hsaco.into_bytes(),
                values.descriptor_source.into_bytes(),
                values.finalized_descriptor.into_bytes(),
                values.artifact.into_bytes(),
            ],
        ),
        (
            CompilerTransactionStageV1::Sealed,
            vec![*capsule.identity().as_bytes()],
        ),
    ] {
        chain = calculate_checkpoint(transaction, chain, stage, &payload);
    }
    chain
}

fn compiler_checkpoint_payload(invocation: &ExactCompilerInvocationV1) -> [[u8; 32]; 7] {
    [
        invocation.rustc_descriptor.into_bytes(),
        invocation.rustc_tool.executable.into_bytes(),
        invocation.rustc_tool.configuration.into_bytes(),
        *invocation.rustc_invocation.as_bytes(),
        invocation.backend_tool.executable.into_bytes(),
        invocation.backend_tool.configuration.into_bytes(),
        invocation.backend_invocation.into_bytes(),
    ]
}

fn calculate_source_tree_identity(
    root: &ExactCompilerSourceFileV1,
    dependencies: &[ExactCompilerSourceFileV1],
    features: &[IdentityText],
) -> CompilerTransactionContentIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(SOURCE_TREE_DOMAIN);
    hash_source_file(&mut digest, root);
    digest.update((dependencies.len() as u32).to_le_bytes());
    for dependency in dependencies {
        hash_source_file(&mut digest, dependency);
    }
    digest.update((features.len() as u32).to_le_bytes());
    for feature in features {
        hash_text(&mut digest, feature.as_str());
    }
    CompilerTransactionContentIdentityV1(digest.finalize().into())
}

fn calculate_target_measurement() -> CompilerTransactionContentIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(TARGET_DOMAIN);
    hash_text(&mut digest, GFX942_TRIPLE);
    hash_text(&mut digest, GFX942_AMD_TARGET);
    digest.update([GFX942_WAVEFRONT_SIZE]);
    digest.update([code_object_version_tag(GFX942_CODE_OBJECT_VERSION)]);
    digest.update((GFX942_ARTIFACT_CAPABILITIES.len() as u32).to_le_bytes());
    hash_text(&mut digest, "amd-wave");
    CompilerTransactionContentIdentityV1(digest.finalize().into())
}

fn calculate_semantic_witnesses_identity(
    witnesses: &[ExactSemanticLayoutWitnessV1],
) -> CompilerTransactionContentIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(SEMANTIC_WITNESSES_DOMAIN);
    digest.update((witnesses.len() as u32).to_le_bytes());
    for witness in witnesses {
        hash_text(&mut digest, witness.kernel.as_str());
        digest.update(witness.byte_len.to_le_bytes());
        digest.update(witness.content.as_bytes());
    }
    CompilerTransactionContentIdentityV1(digest.finalize().into())
}

fn calculate_transaction_identity(
    freshness: [u8; 32],
    source_tree: CompilerTransactionContentIdentityV1,
    closure: crate::SourceClosureIdentityV2,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(TRANSACTION_DOMAIN);
    digest.update(freshness);
    digest.update(source_tree.as_bytes());
    digest.update(closure.as_bytes());
    digest.finalize().into()
}

fn calculate_checkpoint(
    transaction: [u8; 32],
    previous: [u8; 32],
    stage: CompilerTransactionStageV1,
    payload: &[[u8; 32]],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CHECKPOINT_DOMAIN);
    digest.update(transaction);
    digest.update(previous);
    digest.update([stage as u8]);
    digest.update((payload.len() as u32).to_le_bytes());
    for identity in payload {
        digest.update(identity);
    }
    digest.finalize().into()
}

fn calculate_record_identity(
    prefix: &[u8],
) -> Result<SealedCompilerTransactionIdentityV1, SealedCompilerTransactionDecodeErrorV1> {
    let mut digest = Sha256::new();
    digest.update(RECORD_IDENTITY_DOMAIN);
    digest.update(prefix);
    SealedCompilerTransactionIdentityV1::from_bytes(digest.finalize().into())
}

fn hash_source_file(digest: &mut Sha256, file: &ExactCompilerSourceFileV1) {
    hash_text(digest, file.path.as_str());
    digest.update(file.byte_len.to_le_bytes());
    digest.update(file.content.as_bytes());
}

fn hash_text(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u32).to_le_bytes());
    digest.update(value.as_bytes());
}

fn payload_digest(identity: CompilerTransactionContentIdentityV1) -> PayloadDigest {
    PayloadDigest::new(
        DigestAlgorithm::Sha256,
        DigestBytes::from_bytes(identity.into_bytes()),
    )
}

fn content_from_payload(
    digest: PayloadDigest,
) -> Result<CompilerTransactionContentIdentityV1, SealedCompilerTransactionDecodeErrorV1> {
    if digest.algorithm() != DigestAlgorithm::Sha256 {
        return Err(
            SealedCompilerTransactionDecodeErrorV1::MeasurementMismatch {
                field: "digest algorithm",
            },
        );
    }
    CompilerTransactionContentIdentityV1::from_nonzero(
        *digest.bytes().as_bytes(),
        "capsule measurement",
    )
}

fn decode_canonical_descriptor(
    bytes: &[u8],
    error: CompilerTransactionRecorderErrorV1,
) -> Result<DeviceDescriptorTableV1, CompilerTransactionRecorderErrorV1> {
    let table = decode_device_descriptor_table_v1(bytes).map_err(|_| error.clone())?;
    let canonical = encode_device_descriptor_table_v1(&table).map_err(|_| error.clone())?;
    if canonical != bytes {
        return Err(error);
    }
    Ok(table)
}

fn validate_descriptor_profile(
    table: &DeviceDescriptorTableV1,
) -> Result<(), CompilerTransactionRecorderErrorV1> {
    if table.code_object_version() != GFX942_CODE_OBJECT_VERSION {
        return Err(CompilerTransactionRecorderErrorV1::DescriptorCodeObjectVersionMismatch);
    }
    if table.device_target().to_string() != GFX942_AMD_TARGET {
        return Err(CompilerTransactionRecorderErrorV1::DescriptorTargetMismatch);
    }
    let mut kernels = table.kernels().iter().collect::<Vec<_>>();
    kernels.sort_unstable_by_key(|kernel| kernel.entry_name().as_str());
    if kernels.len() != 2
        || kernels[0].entry_name().as_str() != "alpha"
        || kernels[1].entry_name().as_str() != "zeta"
    {
        return Err(CompilerTransactionRecorderErrorV1::DescriptorKernelSetMismatch);
    }
    if kernels
        .iter()
        .any(|kernel| kernel.capabilities() != [CapabilityV1::AmdWave])
    {
        return Err(CompilerTransactionRecorderErrorV1::DescriptorCapabilityMismatch);
    }
    Ok(())
}

fn validate_artifact_container(
    container: &ArtifactContainerV1,
    finalized_hsaco: &[u8],
    descriptor: &DeviceDescriptorTableV1,
) -> Result<(), CompilerTransactionRecorderErrorV1> {
    let target = container.manifest().target();
    if target.triple().as_str() != GFX942_TRIPLE
        || target.architecture().as_str() != GFX942_AMD_TARGET
        || target.pointer_width() != PointerWidth::Bits64
        || target.endianness() != Endianness::Little
    {
        return Err(CompilerTransactionRecorderErrorV1::ArtifactTargetMismatch);
    }
    if target.capabilities() != GFX942_ARTIFACT_CAPABILITIES {
        return Err(CompilerTransactionRecorderErrorV1::ArtifactCapabilityMismatch);
    }

    let payloads = container.payloads();
    let code_objects = container.manifest().code_objects();
    if payloads.len() != 1
        || code_objects.len() != 1
        || payloads[0].bytes() != finalized_hsaco
        || code_objects[0].format() != CodeObjectFormat::NativeExecutable
        || code_objects[0].byte_len() != finalized_hsaco.len() as u64
        || code_objects[0].digest() != payloads[0].digest().bytes()
    {
        return Err(CompilerTransactionRecorderErrorV1::ArtifactPayloadMismatch);
    }

    let kernels = container.manifest().kernels();
    if kernels.len() != 2 {
        return Err(CompilerTransactionRecorderErrorV1::ArtifactKernelSetMismatch);
    }
    for descriptor_kernel in descriptor.kernels() {
        let Some(kernel) = kernels
            .iter()
            .find(|kernel| kernel.name().as_str() == descriptor_kernel.entry_name().as_str())
        else {
            return Err(CompilerTransactionRecorderErrorV1::ArtifactKernelSetMismatch);
        };
        if kernel.symbol().as_str() != descriptor_kernel.entry_name().as_str()
            || kernel.kernel_id().as_bytes() != descriptor_kernel.kernel_id().as_bytes()
            || kernel.code_object_digest() != code_objects[0].digest()
            || kernel.required_capabilities() != GFX942_ARTIFACT_CAPABILITIES
        {
            return Err(CompilerTransactionRecorderErrorV1::ArtifactKernelSetMismatch);
        }
    }
    Ok(())
}

const fn code_object_version_tag(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

fn require_nonempty(
    field: &'static str,
    bytes: &[u8],
) -> Result<(), CompilerTransactionRecorderErrorV1> {
    if bytes.is_empty() {
        Err(CompilerTransactionRecorderErrorV1::EmptyInput { field })
    } else {
        Ok(())
    }
}

fn measurement_name(index: usize) -> &'static str {
    const NAMES: [&str; MEASUREMENT_COUNT] = [
        "source tree",
        "rustc descriptor",
        "rustc executable",
        "rustc configuration",
        "backend executable",
        "backend configuration",
        "backend invocation",
        "target profile",
        "semantic layouts",
        "Kernel IR",
        "Worker V2 request",
        "Worker V2 response",
        "raw HSACO",
        "finalized HSACO",
        "descriptor source",
        "finalized descriptor",
        "artifact",
    ];
    NAMES[index]
}

struct SealedRecordReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> SealedRecordReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    const fn position(&self) -> usize {
        self.position
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    const fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], SealedCompilerTransactionDecodeErrorV1> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(SealedCompilerTransactionDecodeErrorV1::Truncated)?;
        if end > self.bytes.len() {
            return Err(SealedCompilerTransactionDecodeErrorV1::Truncated);
        }
        let result = &self.bytes[self.position..end];
        self.position = end;
        Ok(result)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SealedCompilerTransactionDecodeErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| SealedCompilerTransactionDecodeErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, SealedCompilerTransactionDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, SealedCompilerTransactionDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn nonzero_identity(
        &mut self,
        field: &'static str,
    ) -> Result<[u8; 32], SealedCompilerTransactionDecodeErrorV1> {
        let value = self.array()?;
        if value == [0; 32] {
            return Err(SealedCompilerTransactionDecodeErrorV1::ReservedZeroIdentity { field });
        }
        Ok(value)
    }
}

const _: () = {
    assert!(HEADER_BYTES + FIXED_BODY_BYTES < MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1);
    assert!(mem::size_of::<CompilerTransactionMeasurementsV1>() == MEASUREMENT_COUNT * 32);
};
