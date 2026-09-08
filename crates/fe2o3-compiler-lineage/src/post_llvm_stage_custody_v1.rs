//! Bounded, authority-free custody for exact post-lowering compiler artifacts.
//!
//! This schema records content and an ordered pass declaration. It deliberately does not claim
//! that the declaration is the complete worker pipeline, that the passes ran or preserve
//! semantics, that LLVM selected any particular instruction, or that linking preserved object
//! semantics.

use core::fmt;
use sha2::{Digest, Sha256};

const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/POST-LLVM-STAGE-CUSTODY/V1\0";
const MAX_STAGE_BYTES_V1: usize = 256 * 1024 * 1024;
const MAX_BUILD_ID_BYTES_V1: usize = 256;
const MAX_PASS_NAME_BYTES_V1: usize = 128;
const MAX_PASS_OPTIONS_BYTES_V1: usize = 4 * 1024;
const PIPELINE_OCCURRENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/POST-LLVM-PIPELINE-OCCURRENCE-TRANSCRIPT/V1\0";

/// Maximum number of ordered LLVM pass invocations retained by one record.
pub const MAX_LLVM_PASS_INVOCATIONS_V1: usize = 4_096;

/// Maximum nesting depth retained for one expanded new-pass-manager callback.
pub const MAX_LLVM_PASS_NESTING_DEPTH_V1: u16 = 64;

/// Complete fixed phase order implemented by the production scalar-f32 worker path.
pub const FIXED_PRODUCTION_LLVM_PHASES_V1: [FixedProductionLlvmPhaseV1; 8] = [
    FixedProductionLlvmPhaseV1::ParseSetContractAndLink,
    FixedProductionLlvmPhaseV1::StripDebugInfo,
    FixedProductionLlvmPhaseV1::PreserveExpectedExports,
    FixedProductionLlvmPhaseV1::DefaultPerModulePipelineO2,
    FixedProductionLlvmPhaseV1::CanonicalizeCov6ImplicitArguments,
    FixedProductionLlvmPhaseV1::VerifyOptimizedModule,
    FixedProductionLlvmPhaseV1::EmitAmdgpuObject,
    FixedProductionLlvmPhaseV1::LinkHsacoWithFixedLldPolicy,
];

/// Closed production worker phases around LLVM optimization and code generation.
///
/// `DefaultPerModulePipelineO2` names the production `PassBuilder` call. It is not an expanded
/// per-pass transcript; expansion remains dependent on the exact LLVM build.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FixedProductionLlvmPhaseV1 {
    /// Parse LLVM text, set the target contract, and link compiler inputs.
    ParseSetContractAndLink,
    /// Apply `StripDebugInfo` when required by the fixed request policy.
    StripDebugInfo,
    /// Append expected externally visible definitions to `llvm.used`.
    PreserveExpectedExports,
    /// Run `PassBuilder::buildPerModuleDefaultPipeline(O2)`.
    DefaultPerModulePipelineO2,
    /// Restore the COV6 256-byte implicit-argument ABI attributes after optimization.
    CanonicalizeCov6ImplicitArguments,
    /// Verify the optimized LLVM module.
    VerifyOptimizedModule,
    /// Run the AMDGPU target-machine object-emission pipeline.
    EmitAmdgpuObject,
    /// Link the sole generated object with the fixed in-process LLD ELF policy.
    LinkHsacoWithFixedLldPolicy,
}

/// Exact input/output content for one fixed production worker phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactProductionLlvmPhaseContentsV1 {
    phase: FixedProductionLlvmPhaseV1,
    input: Box<[u8]>,
    output: Box<[u8]>,
}

impl ExactProductionLlvmPhaseContentsV1 {
    /// Retains bounded exact phase input and output bytes.
    pub fn new(
        phase: FixedProductionLlvmPhaseV1,
        input: impl Into<Box<[u8]>>,
        output: impl Into<Box<[u8]>>,
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        let input = input.into();
        let output = output.into();
        check_stage_bytes(&input)?;
        check_stage_bytes(&output)?;
        Ok(Self {
            phase,
            input,
            output,
        })
    }

    /// Returns the fixed phase identifier.
    pub const fn phase(&self) -> FixedProductionLlvmPhaseV1 {
        self.phase
    }

    /// Returns the exact phase input.
    pub fn input(&self) -> &[u8] {
        &self.input
    }

    /// Returns the exact phase output.
    pub fn output(&self) -> &[u8] {
        &self.output
    }
}

/// Identity-only phase edge stored in the canonical custody record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionLlvmPhaseCustodyV1 {
    phase: FixedProductionLlvmPhaseV1,
    input: ExactCompilerStageContentIdentityV1,
    output: ExactCompilerStageContentIdentityV1,
}

impl ProductionLlvmPhaseCustodyV1 {
    fn from_contents(
        contents: &ExactProductionLlvmPhaseContentsV1,
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        Ok(Self {
            phase: contents.phase,
            input: ExactCompilerStageContentIdentityV1::calculate(&contents.input)?,
            output: ExactCompilerStageContentIdentityV1::calculate(&contents.output)?,
        })
    }

    /// Returns the fixed phase identifier.
    pub const fn phase(self) -> FixedProductionLlvmPhaseV1 {
        self.phase
    }

    /// Returns the exact phase-input identity.
    pub const fn input(self) -> ExactCompilerStageContentIdentityV1 {
        self.input
    }

    /// Returns the exact phase-output identity.
    pub const fn output(self) -> ExactCompilerStageContentIdentityV1 {
        self.output
    }
}

/// Exact content identity for one compiler stage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactCompilerStageContentIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ExactCompilerStageContentIdentityV1 {
    /// Reconstructs a non-empty content identity decoded by another canonical lineage schema.
    pub(crate) fn from_checked_parts(sha256: [u8; 32], byte_len: u64) -> Option<Self> {
        if sha256 == [0; 32] || byte_len == 0 {
            None
        } else {
            Some(Self { sha256, byte_len })
        }
    }

    /// Derives an identity from retained bytes.
    pub fn calculate(bytes: &[u8]) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        if bytes.is_empty() {
            return Err(PostLlvmStageCustodyErrorV1::EmptyStage);
        }
        if bytes.len() > MAX_STAGE_BYTES_V1 {
            return Err(PostLlvmStageCustodyErrorV1::StageTooLarge);
        }
        Ok(Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        })
    }

    /// Returns the exact SHA-256 digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Checks this identity against retained bytes.
    pub fn matches(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && self.sha256 == <[u8; 32]>::from(Sha256::digest(bytes))
    }
}

/// One invocation in the exact ordered LLVM pass configuration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LlvmPassInvocationV1 {
    name: String,
    options: Box<[u8]>,
}

impl LlvmPassInvocationV1 {
    /// Creates one bounded pass invocation.
    pub fn new(
        name: impl Into<String>,
        canonical_options: impl Into<Box<[u8]>>,
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        let name = name.into();
        let options = canonical_options.into();
        if name.is_empty()
            || name.len() > MAX_PASS_NAME_BYTES_V1
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(PostLlvmStageCustodyErrorV1::InvalidPassName);
        }
        if options.len() > MAX_PASS_OPTIONS_BYTES_V1 {
            return Err(PostLlvmStageCustodyErrorV1::PassOptionsTooLarge);
        }
        Ok(Self { name, options })
    }

    /// Returns the stable pass identifier.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the canonical pass-option bytes.
    pub fn canonical_options(&self) -> &[u8] {
        &self.options
    }
}

/// Public parts for transport and hostile replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostLlvmStageCustodyPartsV1 {
    /// Exact lowered LLVM module identity before assembly to bitcode.
    pub lowered_llvm_module: ExactCompilerStageContentIdentityV1,
    /// Exact pre-optimization LLVM bitcode identity.
    pub pre_optimization_bitcode: ExactCompilerStageContentIdentityV1,
    /// Exact post-optimization LLVM bitcode identity.
    pub post_optimization_bitcode: ExactCompilerStageContentIdentityV1,
    /// Exact generated relocatable-object identity.
    pub generated_object: ExactCompilerStageContentIdentityV1,
    /// Exact final linked code-object identity.
    pub final_code_object: ExactCompilerStageContentIdentityV1,
    /// Recorded LLVM build identifier.
    pub llvm_build_identity: String,
    /// Measured production worker build identifier, when fixed-pipeline custody is attached.
    pub worker_build_identity: Option<String>,
    /// Exact production worker executable identity, when fixed-pipeline custody is attached.
    pub worker_executable: Option<ExactCompilerStageContentIdentityV1>,
    /// Exact ordered pass declaration, including repeated passes.
    pub passes: Box<[LlvmPassInvocationV1]>,
    /// Exact fixed production phase edges in execution order.
    pub production_phases: Box<[ProductionLlvmPhaseCustodyV1]>,
}

/// Identity of one canonical stage-custody record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PostLlvmStageCustodyIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl PostLlvmStageCustodyIdentityV1 {
    /// Returns the canonical record digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the canonical record byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Reconstructs a nonzero custody identity decoded by another canonical lineage schema.
///
/// This does not authenticate the named record; consumers must match it to an owned checked
/// custody record.
pub fn decode_post_llvm_stage_custody_identity_v1(
    sha256: [u8; 32],
    byte_len: u64,
) -> Option<PostLlvmStageCustodyIdentityV1> {
    (sha256 != [0; 32] && byte_len != 0)
        .then_some(PostLlvmStageCustodyIdentityV1 { sha256, byte_len })
}

/// Inert exact stage-custody record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostLlvmStageCustodyV1 {
    parts: PostLlvmStageCustodyPartsV1,
    identity: PostLlvmStageCustodyIdentityV1,
}

impl PostLlvmStageCustodyV1 {
    /// Records identities derived from the four exact stage bodies.
    pub fn from_exact_stage_bytes(
        lowered_llvm_module: &[u8],
        pre_optimization_bitcode: &[u8],
        post_optimization_bitcode: &[u8],
        generated_object: &[u8],
        final_code_object: &[u8],
        llvm_build_identity: impl Into<String>,
        passes: impl Into<Box<[LlvmPassInvocationV1]>>,
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        Self::from_parts(PostLlvmStageCustodyPartsV1 {
            lowered_llvm_module: ExactCompilerStageContentIdentityV1::calculate(
                lowered_llvm_module,
            )?,
            pre_optimization_bitcode: ExactCompilerStageContentIdentityV1::calculate(
                pre_optimization_bitcode,
            )?,
            post_optimization_bitcode: ExactCompilerStageContentIdentityV1::calculate(
                post_optimization_bitcode,
            )?,
            generated_object: ExactCompilerStageContentIdentityV1::calculate(generated_object)?,
            final_code_object: ExactCompilerStageContentIdentityV1::calculate(final_code_object)?,
            llvm_build_identity: llvm_build_identity.into(),
            worker_build_identity: None,
            worker_executable: None,
            passes: passes.into(),
            production_phases: Box::new([]),
        })
    }

    /// Attaches exact fixed production phase and measured worker identity custody.
    ///
    /// This checks the complete outer worker phase order and exact byte custody at every edge. The
    /// resulting record is still inert: the public inputs do not authenticate that this executable
    /// ran, and the production worker does not currently emit expanded `PassBuilder` pass events.
    pub fn with_fixed_production_pipeline(
        self,
        worker_executable: &[u8],
        worker_build_identity: impl Into<String>,
        phases: &[ExactProductionLlvmPhaseContentsV1],
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        check_stage_bytes(worker_executable)?;
        validate_fixed_production_phase_contents(&self, phases)?;
        let mut parts = self.into_parts();
        parts.worker_build_identity = Some(worker_build_identity.into());
        parts.worker_executable = Some(ExactCompilerStageContentIdentityV1::calculate(
            worker_executable,
        )?);
        parts.production_phases = phases
            .iter()
            .map(ProductionLlvmPhaseCustodyV1::from_contents)
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        Self::from_parts(parts)
    }

    /// Reconstructs one inert record from explicit parts.
    pub fn from_parts(
        parts: PostLlvmStageCustodyPartsV1,
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        validate_parts(&parts)?;
        let canonical = encode_parts(&parts)?;
        Ok(Self {
            parts,
            identity: PostLlvmStageCustodyIdentityV1 {
                sha256: Sha256::digest(&canonical).into(),
                byte_len: canonical.len() as u64,
            },
        })
    }

    /// Returns the exact lowered LLVM module identity.
    pub const fn lowered_llvm_module(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.lowered_llvm_module
    }

    /// Returns the pre-optimization bitcode identity.
    pub const fn pre_optimization_bitcode(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.pre_optimization_bitcode
    }

    /// Returns the post-optimization bitcode identity.
    pub const fn post_optimization_bitcode(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.post_optimization_bitcode
    }

    /// Returns the generated relocatable-object identity.
    pub const fn generated_object(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.generated_object
    }

    /// Returns the final linked code-object identity.
    pub const fn final_code_object(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.final_code_object
    }

    /// Returns the recorded LLVM build identifier.
    pub fn llvm_build_identity(&self) -> &str {
        &self.parts.llvm_build_identity
    }

    /// Returns the measured worker build declaration, when attached.
    pub fn worker_build_identity(&self) -> Option<&str> {
        self.parts.worker_build_identity.as_deref()
    }

    /// Returns the exact worker executable identity, when attached.
    pub const fn worker_executable(&self) -> Option<ExactCompilerStageContentIdentityV1> {
        self.parts.worker_executable
    }

    /// Returns the exact ordered pass declaration.
    pub fn passes(&self) -> &[LlvmPassInvocationV1] {
        &self.parts.passes
    }

    /// Returns the exact fixed production phase edges.
    pub fn production_phases(&self) -> &[ProductionLlvmPhaseCustodyV1] {
        &self.parts.production_phases
    }

    /// Reports whether the complete outer production worker phase order is retained.
    pub fn retains_complete_fixed_production_phase_order(&self) -> bool {
        self.parts
            .production_phases
            .iter()
            .map(|phase| phase.phase)
            .eq(FIXED_PRODUCTION_LLVM_PHASES_V1)
    }

    /// Returns false because the worker does not emit expanded per-pass `PassBuilder` events.
    pub const fn authenticates_expanded_llvm_pass_pipeline_execution(&self) -> bool {
        false
    }

    /// Returns false because executable/build identity custody is not an execution attestation.
    pub const fn authenticates_worker_execution(&self) -> bool {
        false
    }

    /// Returns the canonical content identity.
    pub const fn identity(&self) -> PostLlvmStageCustodyIdentityV1 {
        self.identity
    }

    /// Returns false because content custody is not a derivation proof.
    pub const fn proves_optimization_semantics(&self) -> bool {
        false
    }

    /// Returns false because content custody is not instruction-selection evidence.
    pub const fn proves_instruction_selection(&self) -> bool {
        false
    }

    /// Returns false because identities do not prove relocation or link preservation.
    pub const fn proves_object_to_code_object_preservation(&self) -> bool {
        false
    }

    /// Returns false because this inert record grants no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Decomposes this record for transport and hostile testing.
    pub fn into_parts(self) -> PostLlvmStageCustodyPartsV1 {
        self.parts
    }
}

/// LLVM IR unit observed by one expanded new-pass-manager callback.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LlvmPassIrUnitV1 {
    /// Whole LLVM module.
    Module,
    /// Lazy call-graph SCC.
    Cgscc,
    /// LLVM function.
    Function,
    /// LLVM loop.
    Loop,
}

/// Exact full-module snapshots around one expanded `PassBuilder` callback.
///
/// The full module is retained even when the callback's native IR unit is a function, loop, or
/// CGSCC. This gives adjacent callbacks one common custody domain without claiming that serializing
/// the module proves pass semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactLlvmPassOccurrenceContentsV1 {
    ordinal: u32,
    nesting_depth: u16,
    ir_unit: LlvmPassIrUnitV1,
    name: String,
    options: Box<[u8]>,
    input_module: Box<[u8]>,
    output_module: Box<[u8]>,
}

impl ExactLlvmPassOccurrenceContentsV1 {
    /// Retains one bounded expanded pass callback and exact module snapshots.
    pub fn new(
        ordinal: u32,
        nesting_depth: u16,
        ir_unit: LlvmPassIrUnitV1,
        name: impl Into<String>,
        canonical_options: impl Into<Box<[u8]>>,
        input_module: impl Into<Box<[u8]>>,
        output_module: impl Into<Box<[u8]>>,
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        let invocation = LlvmPassInvocationV1::new(name, canonical_options)?;
        let input_module = input_module.into();
        let output_module = output_module.into();
        check_stage_bytes(&input_module)?;
        check_stage_bytes(&output_module)?;
        if nesting_depth > MAX_LLVM_PASS_NESTING_DEPTH_V1 {
            return Err(PostLlvmStageCustodyErrorV1::InvalidPassNestingDepth);
        }
        Ok(Self {
            ordinal,
            nesting_depth,
            ir_unit,
            name: invocation.name,
            options: invocation.options,
            input_module,
            output_module,
        })
    }

    /// Returns the zero-based callback ordinal.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Returns the pass-manager nesting depth.
    pub const fn nesting_depth(&self) -> u16 {
        self.nesting_depth
    }

    /// Returns the callback IR unit.
    pub const fn ir_unit(&self) -> LlvmPassIrUnitV1 {
        self.ir_unit
    }

    /// Returns the stable expanded pass name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns canonical pass options.
    pub fn canonical_options(&self) -> &[u8] {
        &self.options
    }

    /// Returns the exact full module before the callback.
    pub fn input_module(&self) -> &[u8] {
        &self.input_module
    }

    /// Returns the exact full module after the callback.
    pub fn output_module(&self) -> &[u8] {
        &self.output_module
    }
}

/// Identity-only custody for one expanded pass callback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlvmPassOccurrenceCustodyV1 {
    ordinal: u32,
    nesting_depth: u16,
    ir_unit: LlvmPassIrUnitV1,
    invocation: LlvmPassInvocationV1,
    input_module: ExactCompilerStageContentIdentityV1,
    output_module: ExactCompilerStageContentIdentityV1,
}

impl LlvmPassOccurrenceCustodyV1 {
    fn from_contents(
        contents: &ExactLlvmPassOccurrenceContentsV1,
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        Ok(Self {
            ordinal: contents.ordinal,
            nesting_depth: contents.nesting_depth,
            ir_unit: contents.ir_unit,
            invocation: LlvmPassInvocationV1::new(contents.name.clone(), contents.options.clone())?,
            input_module: ExactCompilerStageContentIdentityV1::calculate(&contents.input_module)?,
            output_module: ExactCompilerStageContentIdentityV1::calculate(&contents.output_module)?,
        })
    }

    /// Returns the callback ordinal.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Returns the pass-manager nesting depth.
    pub const fn nesting_depth(&self) -> u16 {
        self.nesting_depth
    }

    /// Returns the native callback IR unit.
    pub const fn ir_unit(&self) -> LlvmPassIrUnitV1 {
        self.ir_unit
    }

    /// Returns the exact configured invocation.
    pub const fn invocation(&self) -> &LlvmPassInvocationV1 {
        &self.invocation
    }

    /// Returns the exact pre-callback module identity.
    pub const fn input_module(&self) -> ExactCompilerStageContentIdentityV1 {
        self.input_module
    }

    /// Returns the exact post-callback module identity.
    pub const fn output_module(&self) -> ExactCompilerStageContentIdentityV1 {
        self.output_module
    }
}

/// Why an otherwise complete occurrence transcript is not producer-authenticated.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PostLlvmOccurrenceAuthenticationV1 {
    /// Worker response V4 carries aggregate identities but no phase/pass event stream.
    ProductionResponseDoesNotCarryTranscript,
}

/// Public parts for canonical hostile replay of one occurrence transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostLlvmPipelineOccurrenceTranscriptPartsV1 {
    /// Compiler occurrence to which the producer must bind this transcript.
    pub compiler_occurrence_identity: [u8; 32],
    /// Exact worker request identity.
    pub worker_request_identity: [u8; 32],
    /// Exact worker response identity.
    pub worker_response_identity: [u8; 32],
    /// Exact worker executable identity.
    pub worker_executable: ExactCompilerStageContentIdentityV1,
    /// Exact text assembler executable identity.
    pub assembler_executable: ExactCompilerStageContentIdentityV1,
    /// Production worker build identifier.
    pub worker_build_identity: String,
    /// LLVM build identifier shared by assembly, optimization, and code generation.
    pub llvm_build_identity: String,
    /// Exact source LLVM text supplied to assembly.
    pub assembly_input: ExactCompilerStageContentIdentityV1,
    /// Exact bitcode emitted by the production text-assembly occurrence.
    pub assembly_output: ExactCompilerStageContentIdentityV1,
    /// Complete fixed outer phase roster.
    pub phases: Box<[ProductionLlvmPhaseCustodyV1]>,
    /// Complete expanded pass callback roster.
    pub passes: Box<[LlvmPassOccurrenceCustodyV1]>,
    /// Current production authentication availability.
    pub authentication: PostLlvmOccurrenceAuthenticationV1,
}

/// Identity of one canonical post-LLVM occurrence transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PostLlvmPipelineOccurrenceTranscriptIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl PostLlvmPipelineOccurrenceTranscriptIdentityV1 {
    /// Returns the transcript digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the canonical transcript length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Bounded occurrence transcript awaiting producer authentication.
///
/// This record is deliberately inert. It captures every configured event and exact checkpoint,
/// but the only current authentication state records that production response V4 cannot bind the
/// event stream to the measured worker occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostLlvmPipelineOccurrenceTranscriptV1 {
    parts: PostLlvmPipelineOccurrenceTranscriptPartsV1,
    identity: PostLlvmPipelineOccurrenceTranscriptIdentityV1,
}

impl PostLlvmPipelineOccurrenceTranscriptV1 {
    /// Captures bounded phase/pass custody and text-assembly output for one declared occurrence.
    #[allow(clippy::too_many_arguments)]
    pub fn capture_unavailable_production_v4(
        compiler_occurrence_identity: [u8; 32],
        worker_request_identity: [u8; 32],
        worker_response_identity: [u8; 32],
        worker_executable: &[u8],
        assembler_executable: &[u8],
        worker_build_identity: impl Into<String>,
        llvm_build_identity: impl Into<String>,
        assembly_input: &[u8],
        assembly_output: &[u8],
        phases: &[ExactProductionLlvmPhaseContentsV1],
        passes: &[ExactLlvmPassOccurrenceContentsV1],
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        Self::from_parts(PostLlvmPipelineOccurrenceTranscriptPartsV1 {
            compiler_occurrence_identity,
            worker_request_identity,
            worker_response_identity,
            worker_executable: ExactCompilerStageContentIdentityV1::calculate(worker_executable)?,
            assembler_executable: ExactCompilerStageContentIdentityV1::calculate(
                assembler_executable,
            )?,
            worker_build_identity: worker_build_identity.into(),
            llvm_build_identity: llvm_build_identity.into(),
            assembly_input: ExactCompilerStageContentIdentityV1::calculate(assembly_input)?,
            assembly_output: ExactCompilerStageContentIdentityV1::calculate(assembly_output)?,
            phases: phases
                .iter()
                .map(ProductionLlvmPhaseCustodyV1::from_contents)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            passes: passes
                .iter()
                .map(LlvmPassOccurrenceCustodyV1::from_contents)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            authentication:
                PostLlvmOccurrenceAuthenticationV1::ProductionResponseDoesNotCarryTranscript,
        })
    }

    /// Reconstructs and validates one canonical transcript from explicit parts.
    pub fn from_parts(
        parts: PostLlvmPipelineOccurrenceTranscriptPartsV1,
    ) -> Result<Self, PostLlvmStageCustodyErrorV1> {
        validate_occurrence_transcript_parts(&parts)?;
        let canonical = encode_occurrence_transcript_parts(&parts)?;
        Ok(Self {
            parts,
            identity: PostLlvmPipelineOccurrenceTranscriptIdentityV1 {
                sha256: Sha256::digest(&canonical).into(),
                byte_len: canonical.len() as u64,
            },
        })
    }

    /// Returns the compiler occurrence identity requiring producer authentication.
    pub const fn compiler_occurrence_identity(&self) -> [u8; 32] {
        self.parts.compiler_occurrence_identity
    }

    /// Returns the exact worker request identity.
    pub const fn worker_request_identity(&self) -> [u8; 32] {
        self.parts.worker_request_identity
    }

    /// Returns the exact worker response identity.
    pub const fn worker_response_identity(&self) -> [u8; 32] {
        self.parts.worker_response_identity
    }

    /// Returns the complete fixed phase roster.
    pub fn phases(&self) -> &[ProductionLlvmPhaseCustodyV1] {
        &self.parts.phases
    }

    /// Returns the complete expanded pass roster.
    pub fn passes(&self) -> &[LlvmPassOccurrenceCustodyV1] {
        &self.parts.passes
    }

    /// Returns the exact text-assembly output identity.
    pub const fn assembly_output(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.assembly_output
    }

    /// Returns why this transcript is not yet producer-authenticated.
    pub const fn authentication(&self) -> PostLlvmOccurrenceAuthenticationV1 {
        self.parts.authentication
    }

    /// Returns the canonical transcript identity.
    pub const fn identity(&self) -> PostLlvmPipelineOccurrenceTranscriptIdentityV1 {
        self.identity
    }

    /// Returns false until the production worker response binds this exact event stream.
    pub const fn authenticates_production_occurrence(&self) -> bool {
        false
    }

    /// Transcript custody is not pass or compiler semantic preservation.
    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }

    /// Decomposes this transcript for hostile replay.
    pub fn into_parts(self) -> PostLlvmPipelineOccurrenceTranscriptPartsV1 {
        self.parts
    }
}

/// Move-only custody after all exact stage bodies have been independently matched.
#[must_use = "dropping exact stage custody abandons post-LLVM evidence"]
pub struct CheckedPostLlvmStageContentsV1 {
    record: PostLlvmStageCustodyV1,
    lowered_llvm_module: Box<[u8]>,
    pre_optimization_bitcode: Box<[u8]>,
    post_optimization_bitcode: Box<[u8]>,
    generated_object: Box<[u8]>,
    final_code_object: Box<[u8]>,
    worker_executable: Option<Box<[u8]>>,
    production_phases: Option<Box<[ExactProductionLlvmPhaseContentsV1]>>,
    occurrence_transcript: Option<PostLlvmPipelineOccurrenceTranscriptV1>,
    assembler_executable: Option<Box<[u8]>>,
    assembly_input: Option<Box<[u8]>>,
    assembly_output: Option<Box<[u8]>>,
    pass_occurrences: Option<Box<[ExactLlvmPassOccurrenceContentsV1]>>,
}

impl fmt::Debug for CheckedPostLlvmStageContentsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedPostLlvmStageContentsV1")
            .field("record", &self.record.identity())
            .finish_non_exhaustive()
    }
}

impl CheckedPostLlvmStageContentsV1 {
    /// Returns the exact stage-custody record.
    pub const fn record(&self) -> &PostLlvmStageCustodyV1 {
        &self.record
    }

    /// Returns the exact lowered LLVM module bytes.
    pub fn lowered_llvm_module(&self) -> &[u8] {
        &self.lowered_llvm_module
    }

    /// Returns the exact pre-optimization bitcode bytes.
    pub fn pre_optimization_bitcode(&self) -> &[u8] {
        &self.pre_optimization_bitcode
    }

    /// Returns the exact post-optimization bitcode bytes.
    pub fn post_optimization_bitcode(&self) -> &[u8] {
        &self.post_optimization_bitcode
    }

    /// Returns the exact generated relocatable-object bytes.
    pub fn generated_object(&self) -> &[u8] {
        &self.generated_object
    }

    /// Returns the exact final code-object bytes.
    pub fn final_code_object(&self) -> &[u8] {
        &self.final_code_object
    }

    /// Reports only exact content matching and ordered pass-declaration retention.
    pub const fn retains_exact_stage_contents_and_recorded_passes(&self) -> bool {
        true
    }

    /// Reports whether every fixed outer production phase and the worker executable were matched.
    pub const fn retains_exact_fixed_production_pipeline_contents(&self) -> bool {
        self.worker_executable.is_some() && self.production_phases.is_some()
    }

    /// Returns the exact checked worker executable, when fixed-pipeline custody was supplied.
    pub fn worker_executable(&self) -> Option<&[u8]> {
        self.worker_executable.as_deref()
    }

    /// Returns every checked fixed production phase body, when supplied.
    pub fn production_phases(&self) -> Option<&[ExactProductionLlvmPhaseContentsV1]> {
        self.production_phases.as_deref()
    }

    /// Returns the checked occurrence transcript, when exact replay custody was supplied.
    pub const fn occurrence_transcript(&self) -> Option<&PostLlvmPipelineOccurrenceTranscriptV1> {
        self.occurrence_transcript.as_ref()
    }

    /// Returns the exact checked text-assembler executable.
    pub fn assembler_executable(&self) -> Option<&[u8]> {
        self.assembler_executable.as_deref()
    }

    /// Returns the exact LLVM text supplied to both assembly occurrences.
    pub fn assembly_input(&self) -> Option<&[u8]> {
        self.assembly_input.as_deref()
    }

    /// Returns the exact bitcode reproduced by both assembly occurrences.
    pub fn assembly_output(&self) -> Option<&[u8]> {
        self.assembly_output.as_deref()
    }

    /// Returns every independently matched expanded pass checkpoint.
    pub fn pass_occurrences(&self) -> Option<&[ExactLlvmPassOccurrenceContentsV1]> {
        self.pass_occurrences.as_deref()
    }

    /// Reports complete bounded custody for phase, pass, and exact assembly-replay bodies.
    pub const fn retains_complete_occurrence_and_assembly_replay_custody(&self) -> bool {
        self.occurrence_transcript.is_some()
            && self.assembler_executable.is_some()
            && self.assembly_input.is_some()
            && self.assembly_output.is_some()
            && self.pass_occurrences.is_some()
    }

    /// Exact bodies and identities do not authenticate process execution.
    pub const fn authenticates_production_pipeline_execution(&self) -> bool {
        false
    }

    /// Requires a producer-authenticated occurrence rather than an unattested transcript.
    pub fn require_authenticated_production_pipeline_execution(
        &self,
    ) -> Result<(), PostLlvmStageCustodyErrorV1> {
        Err(PostLlvmStageCustodyErrorV1::ProducerTranscriptUnavailable)
    }

    /// Returns false because matching bytes to identities is not semantic refinement.
    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }

    /// Returns false because this owner grants no compiler or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Decomposes this move-only owner.
    pub fn into_parts(
        self,
    ) -> (
        PostLlvmStageCustodyV1,
        Box<[u8]>,
        Box<[u8]>,
        Box<[u8]>,
        Box<[u8]>,
        Box<[u8]>,
    ) {
        (
            self.record,
            self.lowered_llvm_module,
            self.pre_optimization_bitcode,
            self.post_optimization_bitcode,
            self.generated_object,
            self.final_code_object,
        )
    }
}

/// Independently matches every exact stage body to an inert custody record.
pub fn check_exact_post_llvm_stage_contents_v1(
    record: PostLlvmStageCustodyV1,
    lowered_llvm_module: impl Into<Box<[u8]>>,
    pre_optimization_bitcode: impl Into<Box<[u8]>>,
    post_optimization_bitcode: impl Into<Box<[u8]>>,
    generated_object: impl Into<Box<[u8]>>,
    final_code_object: impl Into<Box<[u8]>>,
) -> Result<CheckedPostLlvmStageContentsV1, PostLlvmStageCustodyErrorV1> {
    let lowered_llvm_module = lowered_llvm_module.into();
    let pre_optimization_bitcode = pre_optimization_bitcode.into();
    let post_optimization_bitcode = post_optimization_bitcode.into();
    let generated_object = generated_object.into();
    let final_code_object = final_code_object.into();
    for bytes in [
        lowered_llvm_module.as_ref(),
        pre_optimization_bitcode.as_ref(),
        post_optimization_bitcode.as_ref(),
        generated_object.as_ref(),
        final_code_object.as_ref(),
    ] {
        if bytes.is_empty() {
            return Err(PostLlvmStageCustodyErrorV1::EmptyStage);
        }
        if bytes.len() > MAX_STAGE_BYTES_V1 {
            return Err(PostLlvmStageCustodyErrorV1::StageTooLarge);
        }
    }
    if !record.lowered_llvm_module().matches(&lowered_llvm_module) {
        return Err(PostLlvmStageCustodyErrorV1::LoweredLlvmMismatch);
    }
    if !record
        .pre_optimization_bitcode()
        .matches(&pre_optimization_bitcode)
    {
        return Err(PostLlvmStageCustodyErrorV1::PreOptimizationMismatch);
    }
    if !record
        .post_optimization_bitcode()
        .matches(&post_optimization_bitcode)
    {
        return Err(PostLlvmStageCustodyErrorV1::PostOptimizationMismatch);
    }
    if !record.generated_object().matches(&generated_object) {
        return Err(PostLlvmStageCustodyErrorV1::GeneratedObjectMismatch);
    }
    if !record.final_code_object().matches(&final_code_object) {
        return Err(PostLlvmStageCustodyErrorV1::FinalCodeObjectMismatch);
    }
    Ok(CheckedPostLlvmStageContentsV1 {
        record,
        lowered_llvm_module,
        pre_optimization_bitcode,
        post_optimization_bitcode,
        generated_object,
        final_code_object,
        worker_executable: None,
        production_phases: None,
        occurrence_transcript: None,
        assembler_executable: None,
        assembly_input: None,
        assembly_output: None,
        pass_occurrences: None,
    })
}

/// Adds independently matched fixed production phase and worker-executable contents in place.
///
/// This consumes and returns the existing checked custody owner rather than creating a parallel
/// artifact record. It authenticates content against the attached declaration only; it cannot
/// authenticate an execution occurrence because the production response does not carry these
/// phase bodies or an expanded pass-event transcript.
pub fn check_exact_fixed_production_pipeline_contents_v1(
    mut contents: CheckedPostLlvmStageContentsV1,
    worker_executable: impl Into<Box<[u8]>>,
    expected_worker_build_identity: &str,
    expected_llvm_build_identity: &str,
    phases: impl Into<Box<[ExactProductionLlvmPhaseContentsV1]>>,
) -> Result<CheckedPostLlvmStageContentsV1, PostLlvmStageCustodyErrorV1> {
    let worker_executable = worker_executable.into();
    let phases = phases.into();
    check_stage_bytes(&worker_executable)?;
    validate_fixed_production_phase_contents(contents.record(), &phases)?;
    let expected_worker = contents
        .record()
        .worker_executable()
        .ok_or(PostLlvmStageCustodyErrorV1::MissingProductionPipeline)?;
    if !expected_worker.matches(&worker_executable) {
        return Err(PostLlvmStageCustodyErrorV1::WorkerExecutableMismatch);
    }
    if contents.record().worker_build_identity() != Some(expected_worker_build_identity) {
        return Err(PostLlvmStageCustodyErrorV1::WorkerBuildIdentityMismatch);
    }
    if contents.record().llvm_build_identity() != expected_llvm_build_identity {
        return Err(PostLlvmStageCustodyErrorV1::LlvmBuildIdentityMismatch);
    }
    if contents.record().worker_build_identity().is_none()
        || contents.record().production_phases().len() != phases.len()
    {
        return Err(PostLlvmStageCustodyErrorV1::MissingProductionPipeline);
    }
    for (declared, retained) in contents.record().production_phases().iter().zip(&phases) {
        if declared.phase() != retained.phase()
            || !declared.input().matches(retained.input())
            || !declared.output().matches(retained.output())
        {
            return Err(PostLlvmStageCustodyErrorV1::ProductionPhaseContentMismatch);
        }
    }
    contents.worker_executable = Some(worker_executable);
    contents.production_phases = Some(phases);
    Ok(contents)
}

/// Matches exact phase/pass occurrence bodies and independently replayed text assembly.
///
/// The replay establishes an exact deterministic-custody premise for the supplied assembler,
/// input, and LLVM build. The phase/pass stream remains unauthenticated because production
/// response V4 does not carry it; this function therefore cannot establish execution or semantic
/// refinement.
#[allow(clippy::too_many_arguments)]
pub fn check_exact_post_llvm_pipeline_occurrence_v1(
    mut contents: CheckedPostLlvmStageContentsV1,
    transcript: PostLlvmPipelineOccurrenceTranscriptV1,
    worker_executable: impl Into<Box<[u8]>>,
    assembler_executable: impl Into<Box<[u8]>>,
    assembly_input: impl Into<Box<[u8]>>,
    assembly_output: impl Into<Box<[u8]>>,
    replay_assembly_input: &[u8],
    replay_assembly_output: &[u8],
    phases: impl Into<Box<[ExactProductionLlvmPhaseContentsV1]>>,
    passes: impl Into<Box<[ExactLlvmPassOccurrenceContentsV1]>>,
) -> Result<CheckedPostLlvmStageContentsV1, PostLlvmStageCustodyErrorV1> {
    let worker_executable = worker_executable.into();
    let assembler_executable = assembler_executable.into();
    let assembly_input = assembly_input.into();
    let assembly_output = assembly_output.into();
    let phases = phases.into();
    let passes = passes.into();
    for bytes in [
        worker_executable.as_ref(),
        assembler_executable.as_ref(),
        assembly_input.as_ref(),
        assembly_output.as_ref(),
        replay_assembly_input,
        replay_assembly_output,
    ] {
        check_stage_bytes(bytes)?;
    }
    if contents.worker_executable.is_none() || contents.production_phases.is_none() {
        return Err(PostLlvmStageCustodyErrorV1::MissingProductionPipeline);
    }
    if contents.worker_executable.as_deref() != Some(worker_executable.as_ref()) {
        return Err(PostLlvmStageCustodyErrorV1::WorkerExecutableMismatch);
    }
    if !transcript
        .parts
        .worker_executable
        .matches(&worker_executable)
    {
        return Err(PostLlvmStageCustodyErrorV1::WorkerExecutableMismatch);
    }
    if !transcript
        .parts
        .assembler_executable
        .matches(&assembler_executable)
    {
        return Err(PostLlvmStageCustodyErrorV1::AssemblerExecutableMismatch);
    }
    if transcript.parts.worker_build_identity
        != contents
            .record()
            .worker_build_identity()
            .unwrap_or_default()
    {
        return Err(PostLlvmStageCustodyErrorV1::WorkerBuildIdentityMismatch);
    }
    if transcript.parts.llvm_build_identity != contents.record().llvm_build_identity() {
        return Err(PostLlvmStageCustodyErrorV1::LlvmBuildIdentityMismatch);
    }
    if contents.lowered_llvm_module() != assembly_input.as_ref()
        || !transcript.parts.assembly_input.matches(&assembly_input)
        || replay_assembly_input != assembly_input.as_ref()
    {
        return Err(PostLlvmStageCustodyErrorV1::AssemblyInputMismatch);
    }
    if !transcript.parts.assembly_output.matches(&assembly_output) {
        return Err(PostLlvmStageCustodyErrorV1::AssemblyOutputMismatch);
    }
    if replay_assembly_output != assembly_output.as_ref() {
        return Err(PostLlvmStageCustodyErrorV1::AssemblyReplayMismatch);
    }
    validate_fixed_production_phase_contents(contents.record(), &phases)?;
    if contents.production_phases.as_deref() != Some(phases.as_ref()) {
        return Err(PostLlvmStageCustodyErrorV1::ProductionPhaseContentMismatch);
    }
    validate_exact_occurrence_contents(&transcript, contents.record(), &phases, &passes)?;

    contents.occurrence_transcript = Some(transcript);
    contents.assembler_executable = Some(assembler_executable);
    contents.assembly_input = Some(assembly_input);
    contents.assembly_output = Some(assembly_output);
    contents.pass_occurrences = Some(passes);
    Ok(contents)
}

/// Stable failure categories for exact post-LLVM stage custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostLlvmStageCustodyErrorV1 {
    /// A required stage body was empty.
    EmptyStage,
    /// A stage body exceeded the fixed allocation bound.
    StageTooLarge,
    /// The LLVM build identifier was empty, unsafe, or too large.
    InvalidLlvmBuildIdentity,
    /// The attached production worker build identifier was empty, unsafe, or too large.
    InvalidWorkerBuildIdentity,
    /// Worker identity and fixed phase custody were absent or only partially attached.
    MissingProductionPipeline,
    /// Fixed outer production phases were omitted, reordered, or substituted.
    ProductionPhaseOrderMismatch,
    /// Fixed production phase edges were disconnected from each other or top-level stages.
    ProductionPhaseBoundaryMismatch,
    /// A retained fixed phase body did not match its declared exact identity.
    ProductionPhaseContentMismatch,
    /// Exact worker executable bytes did not match their attached identity.
    WorkerExecutableMismatch,
    /// The expected production worker build identity did not match custody.
    WorkerBuildIdentityMismatch,
    /// The expected production LLVM build identity did not match custody.
    LlvmBuildIdentityMismatch,
    /// The pass roster exceeded its fixed bound.
    InvalidPassRoster,
    /// A pass name was empty, unsafe, or too large.
    InvalidPassName,
    /// Canonical pass options exceeded their fixed bound.
    PassOptionsTooLarge,
    /// Expanded pass nesting exceeded the fixed transcript bound.
    InvalidPassNestingDepth,
    /// A required occurrence, request, or response identity was all zeroes.
    InvalidOccurrenceIdentity,
    /// The occurrence transcript omitted or reordered a fixed outer phase.
    OccurrencePhaseMismatch,
    /// The expanded pass transcript omitted or substituted a configured invocation.
    OccurrencePassRosterMismatch,
    /// Expanded pass ordinals were omitted, duplicated, or reordered.
    OccurrencePassOrderMismatch,
    /// Expanded pass checkpoints were disconnected from each other or the O2 phase.
    OccurrencePassBoundaryMismatch,
    /// An expanded pass body did not match its transcript identity.
    OccurrencePassContentMismatch,
    /// Exact assembler bytes did not match the occurrence transcript.
    AssemblerExecutableMismatch,
    /// Assembly input did not match the lowered LLVM text or replay input.
    AssemblyInputMismatch,
    /// Assembly output did not match the occurrence transcript.
    AssemblyOutputMismatch,
    /// Independent text-assembly replay did not reproduce exact bitcode.
    AssemblyReplayMismatch,
    /// Production response V4 cannot authenticate a phase/pass occurrence stream.
    ProducerTranscriptUnavailable,
    /// Lowered LLVM module bytes did not match the record.
    LoweredLlvmMismatch,
    /// Pre-optimization bitcode did not match the record.
    PreOptimizationMismatch,
    /// Post-optimization bitcode did not match the record.
    PostOptimizationMismatch,
    /// Generated object bytes did not match the record.
    GeneratedObjectMismatch,
    /// Final code-object bytes did not match the record.
    FinalCodeObjectMismatch,
    /// A canonical length could not be represented.
    LengthOverflow,
}

impl fmt::Display for PostLlvmStageCustodyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "post-LLVM stage custody rejected: {self:?}")
    }
}

impl std::error::Error for PostLlvmStageCustodyErrorV1 {}

fn validate_parts(parts: &PostLlvmStageCustodyPartsV1) -> Result<(), PostLlvmStageCustodyErrorV1> {
    if validate_build_identity(&parts.llvm_build_identity).is_err() {
        return Err(PostLlvmStageCustodyErrorV1::InvalidLlvmBuildIdentity);
    }
    if let Some(worker_build_identity) = &parts.worker_build_identity {
        validate_build_identity(worker_build_identity)
            .map_err(|_| PostLlvmStageCustodyErrorV1::InvalidWorkerBuildIdentity)?;
    }
    if parts.worker_build_identity.is_some() != parts.worker_executable.is_some()
        || parts.worker_build_identity.is_some() != !parts.production_phases.is_empty()
    {
        return Err(PostLlvmStageCustodyErrorV1::MissingProductionPipeline);
    }
    if parts.passes.len() > MAX_LLVM_PASS_INVOCATIONS_V1 {
        return Err(PostLlvmStageCustodyErrorV1::InvalidPassRoster);
    }
    for pass in &parts.passes {
        LlvmPassInvocationV1::new(pass.name.clone(), pass.options.clone())?;
    }
    for identity in [
        parts.lowered_llvm_module,
        parts.pre_optimization_bitcode,
        parts.post_optimization_bitcode,
        parts.generated_object,
        parts.final_code_object,
    ] {
        if identity.sha256 == [0; 32] || identity.byte_len == 0 {
            return Err(PostLlvmStageCustodyErrorV1::EmptyStage);
        }
        if identity.byte_len > MAX_STAGE_BYTES_V1 as u64 {
            return Err(PostLlvmStageCustodyErrorV1::StageTooLarge);
        }
    }
    if let Some(worker_executable) = parts.worker_executable {
        validate_stage_identity(worker_executable)?;
    }
    if !parts.production_phases.is_empty() {
        validate_fixed_production_phase_custody(parts)?;
    }
    Ok(())
}

fn validate_occurrence_transcript_parts(
    parts: &PostLlvmPipelineOccurrenceTranscriptPartsV1,
) -> Result<(), PostLlvmStageCustodyErrorV1> {
    if [
        parts.compiler_occurrence_identity,
        parts.worker_request_identity,
        parts.worker_response_identity,
    ]
    .contains(&[0; 32])
    {
        return Err(PostLlvmStageCustodyErrorV1::InvalidOccurrenceIdentity);
    }
    validate_stage_identity(parts.worker_executable)?;
    validate_stage_identity(parts.assembler_executable)?;
    validate_stage_identity(parts.assembly_input)?;
    validate_stage_identity(parts.assembly_output)?;
    validate_build_identity(&parts.worker_build_identity)
        .map_err(|_| PostLlvmStageCustodyErrorV1::InvalidWorkerBuildIdentity)?;
    validate_build_identity(&parts.llvm_build_identity)
        .map_err(|_| PostLlvmStageCustodyErrorV1::InvalidLlvmBuildIdentity)?;
    if parts
        .phases
        .iter()
        .map(|phase| phase.phase)
        .ne(FIXED_PRODUCTION_LLVM_PHASES_V1)
    {
        return Err(PostLlvmStageCustodyErrorV1::OccurrencePhaseMismatch);
    }
    for phase in &parts.phases {
        validate_stage_identity(phase.input)?;
        validate_stage_identity(phase.output)?;
    }
    if parts.phases[0].input != parts.assembly_input
        || parts
            .phases
            .windows(2)
            .any(|pair| pair[0].output != pair[1].input)
    {
        return Err(PostLlvmStageCustodyErrorV1::ProductionPhaseBoundaryMismatch);
    }
    if parts.passes.len() > MAX_LLVM_PASS_INVOCATIONS_V1 {
        return Err(PostLlvmStageCustodyErrorV1::InvalidPassRoster);
    }
    for (expected_ordinal, pass) in parts.passes.iter().enumerate() {
        if pass.ordinal != expected_ordinal as u32 {
            return Err(PostLlvmStageCustodyErrorV1::OccurrencePassOrderMismatch);
        }
        if pass.nesting_depth > MAX_LLVM_PASS_NESTING_DEPTH_V1 {
            return Err(PostLlvmStageCustodyErrorV1::InvalidPassNestingDepth);
        }
        LlvmPassInvocationV1::new(
            pass.invocation.name.clone(),
            pass.invocation.options.clone(),
        )?;
        validate_stage_identity(pass.input_module)?;
        validate_stage_identity(pass.output_module)?;
    }
    let optimization_phase = &parts.phases[3];
    match (parts.passes.first(), parts.passes.last()) {
        (Some(first), Some(last)) => {
            if first.input_module != optimization_phase.input
                || last.output_module != optimization_phase.output
                || parts
                    .passes
                    .windows(2)
                    .any(|pair| pair[0].output_module != pair[1].input_module)
            {
                return Err(PostLlvmStageCustodyErrorV1::OccurrencePassBoundaryMismatch);
            }
        }
        (None, None) if optimization_phase.input == optimization_phase.output => {}
        (None, None) => {
            return Err(PostLlvmStageCustodyErrorV1::OccurrencePassBoundaryMismatch);
        }
        _ => unreachable!("first and last are simultaneously present"),
    }
    Ok(())
}

fn validate_exact_occurrence_contents(
    transcript: &PostLlvmPipelineOccurrenceTranscriptV1,
    record: &PostLlvmStageCustodyV1,
    phases: &[ExactProductionLlvmPhaseContentsV1],
    passes: &[ExactLlvmPassOccurrenceContentsV1],
) -> Result<(), PostLlvmStageCustodyErrorV1> {
    if transcript.parts.phases.len() != phases.len() {
        return Err(PostLlvmStageCustodyErrorV1::OccurrencePhaseMismatch);
    }
    for (declared, exact) in transcript.parts.phases.iter().zip(phases) {
        if declared != &ProductionLlvmPhaseCustodyV1::from_contents(exact)? {
            return Err(PostLlvmStageCustodyErrorV1::ProductionPhaseContentMismatch);
        }
    }
    if transcript.parts.passes.len() != passes.len() || record.passes().len() != passes.len() {
        return Err(PostLlvmStageCustodyErrorV1::OccurrencePassRosterMismatch);
    }
    for ((declared, exact), configured) in transcript
        .parts
        .passes
        .iter()
        .zip(passes)
        .zip(record.passes())
    {
        if declared != &LlvmPassOccurrenceCustodyV1::from_contents(exact)? {
            return Err(PostLlvmStageCustodyErrorV1::OccurrencePassContentMismatch);
        }
        if &declared.invocation != configured {
            return Err(PostLlvmStageCustodyErrorV1::OccurrencePassRosterMismatch);
        }
    }
    Ok(())
}

fn encode_occurrence_transcript_parts(
    parts: &PostLlvmPipelineOccurrenceTranscriptPartsV1,
) -> Result<Vec<u8>, PostLlvmStageCustodyErrorV1> {
    let mut output = Vec::new();
    output.extend_from_slice(PIPELINE_OCCURRENCE_IDENTITY_DOMAIN_V1);
    output.extend_from_slice(&parts.compiler_occurrence_identity);
    output.extend_from_slice(&parts.worker_request_identity);
    output.extend_from_slice(&parts.worker_response_identity);
    push_stage_identity(&mut output, parts.worker_executable);
    push_stage_identity(&mut output, parts.assembler_executable);
    push_bytes(&mut output, parts.worker_build_identity.as_bytes())?;
    push_bytes(&mut output, parts.llvm_build_identity.as_bytes())?;
    push_stage_identity(&mut output, parts.assembly_input);
    push_stage_identity(&mut output, parts.assembly_output);
    output.extend_from_slice(
        &u32::try_from(parts.phases.len())
            .map_err(|_| PostLlvmStageCustodyErrorV1::LengthOverflow)?
            .to_le_bytes(),
    );
    for phase in &parts.phases {
        output.push(fixed_phase_tag(phase.phase));
        push_stage_identity(&mut output, phase.input);
        push_stage_identity(&mut output, phase.output);
    }
    output.extend_from_slice(
        &u32::try_from(parts.passes.len())
            .map_err(|_| PostLlvmStageCustodyErrorV1::LengthOverflow)?
            .to_le_bytes(),
    );
    for pass in &parts.passes {
        output.extend_from_slice(&pass.ordinal.to_le_bytes());
        output.extend_from_slice(&pass.nesting_depth.to_le_bytes());
        output.push(pass_ir_unit_tag(pass.ir_unit));
        push_bytes(&mut output, pass.invocation.name.as_bytes())?;
        push_bytes(&mut output, &pass.invocation.options)?;
        push_stage_identity(&mut output, pass.input_module);
        push_stage_identity(&mut output, pass.output_module);
    }
    output.push(match parts.authentication {
        PostLlvmOccurrenceAuthenticationV1::ProductionResponseDoesNotCarryTranscript => 0,
    });
    Ok(output)
}

fn encode_parts(
    parts: &PostLlvmStageCustodyPartsV1,
) -> Result<Vec<u8>, PostLlvmStageCustodyErrorV1> {
    let mut output = Vec::new();
    output.extend_from_slice(IDENTITY_DOMAIN_V1);
    for identity in [
        parts.lowered_llvm_module,
        parts.pre_optimization_bitcode,
        parts.post_optimization_bitcode,
        parts.generated_object,
        parts.final_code_object,
    ] {
        output.extend_from_slice(&identity.sha256);
        output.extend_from_slice(&identity.byte_len.to_le_bytes());
    }
    push_bytes(&mut output, parts.llvm_build_identity.as_bytes())?;
    match (&parts.worker_build_identity, parts.worker_executable) {
        (Some(worker_build_identity), Some(worker_executable)) => {
            output.push(1);
            push_bytes(&mut output, worker_build_identity.as_bytes())?;
            push_stage_identity(&mut output, worker_executable);
        }
        (None, None) => output.push(0),
        _ => return Err(PostLlvmStageCustodyErrorV1::MissingProductionPipeline),
    }
    output.extend_from_slice(
        &u32::try_from(parts.passes.len())
            .map_err(|_| PostLlvmStageCustodyErrorV1::LengthOverflow)?
            .to_le_bytes(),
    );
    for pass in &parts.passes {
        push_bytes(&mut output, pass.name.as_bytes())?;
        push_bytes(&mut output, &pass.options)?;
    }
    output.extend_from_slice(
        &u32::try_from(parts.production_phases.len())
            .map_err(|_| PostLlvmStageCustodyErrorV1::LengthOverflow)?
            .to_le_bytes(),
    );
    for phase in &parts.production_phases {
        output.push(fixed_phase_tag(phase.phase));
        push_stage_identity(&mut output, phase.input);
        push_stage_identity(&mut output, phase.output);
    }
    Ok(output)
}

fn push_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), PostLlvmStageCustodyErrorV1> {
    output.extend_from_slice(
        &u32::try_from(bytes.len())
            .map_err(|_| PostLlvmStageCustodyErrorV1::LengthOverflow)?
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
    Ok(())
}

fn check_stage_bytes(bytes: &[u8]) -> Result<(), PostLlvmStageCustodyErrorV1> {
    if bytes.is_empty() {
        return Err(PostLlvmStageCustodyErrorV1::EmptyStage);
    }
    if bytes.len() > MAX_STAGE_BYTES_V1 {
        return Err(PostLlvmStageCustodyErrorV1::StageTooLarge);
    }
    Ok(())
}

fn validate_stage_identity(
    identity: ExactCompilerStageContentIdentityV1,
) -> Result<(), PostLlvmStageCustodyErrorV1> {
    if identity.sha256 == [0; 32] || identity.byte_len == 0 {
        return Err(PostLlvmStageCustodyErrorV1::EmptyStage);
    }
    if identity.byte_len > MAX_STAGE_BYTES_V1 as u64 {
        return Err(PostLlvmStageCustodyErrorV1::StageTooLarge);
    }
    Ok(())
}

fn validate_build_identity(identity: &str) -> Result<(), ()> {
    if identity.is_empty()
        || identity.len() > MAX_BUILD_ID_BYTES_V1
        || !identity.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(());
    }
    Ok(())
}

fn validate_fixed_production_phase_contents(
    record: &PostLlvmStageCustodyV1,
    phases: &[ExactProductionLlvmPhaseContentsV1],
) -> Result<(), PostLlvmStageCustodyErrorV1> {
    if phases
        .iter()
        .map(ExactProductionLlvmPhaseContentsV1::phase)
        .ne(FIXED_PRODUCTION_LLVM_PHASES_V1)
    {
        return Err(PostLlvmStageCustodyErrorV1::ProductionPhaseOrderMismatch);
    }
    for phase in phases {
        check_stage_bytes(phase.input())?;
        check_stage_bytes(phase.output())?;
    }
    if !record.lowered_llvm_module().matches(phases[0].input())
        || !record
            .pre_optimization_bitcode()
            .matches(phases[0].output())
        || phases
            .windows(2)
            .any(|pair| pair[0].output() != pair[1].input())
        || !record
            .post_optimization_bitcode()
            .matches(phases[4].output())
        || !record
            .post_optimization_bitcode()
            .matches(phases[5].input())
        || !record
            .post_optimization_bitcode()
            .matches(phases[5].output())
        || !record
            .post_optimization_bitcode()
            .matches(phases[6].input())
        || !record.generated_object().matches(phases[6].output())
        || !record.generated_object().matches(phases[7].input())
        || !record.final_code_object().matches(phases[7].output())
    {
        return Err(PostLlvmStageCustodyErrorV1::ProductionPhaseBoundaryMismatch);
    }
    Ok(())
}

fn validate_fixed_production_phase_custody(
    parts: &PostLlvmStageCustodyPartsV1,
) -> Result<(), PostLlvmStageCustodyErrorV1> {
    if parts
        .production_phases
        .iter()
        .map(|phase| phase.phase)
        .ne(FIXED_PRODUCTION_LLVM_PHASES_V1)
    {
        return Err(PostLlvmStageCustodyErrorV1::ProductionPhaseOrderMismatch);
    }
    for phase in &parts.production_phases {
        validate_stage_identity(phase.input)?;
        validate_stage_identity(phase.output)?;
    }
    if parts.production_phases[0].input != parts.lowered_llvm_module
        || parts.production_phases[0].output != parts.pre_optimization_bitcode
        || parts
            .production_phases
            .windows(2)
            .any(|pair| pair[0].output != pair[1].input)
        || parts.production_phases[4].output != parts.post_optimization_bitcode
        || parts.production_phases[5].input != parts.post_optimization_bitcode
        || parts.production_phases[5].output != parts.post_optimization_bitcode
        || parts.production_phases[6].input != parts.post_optimization_bitcode
        || parts.production_phases[6].output != parts.generated_object
        || parts.production_phases[7].input != parts.generated_object
        || parts.production_phases[7].output != parts.final_code_object
    {
        return Err(PostLlvmStageCustodyErrorV1::ProductionPhaseBoundaryMismatch);
    }
    Ok(())
}

const fn fixed_phase_tag(phase: FixedProductionLlvmPhaseV1) -> u8 {
    match phase {
        FixedProductionLlvmPhaseV1::ParseSetContractAndLink => 1,
        FixedProductionLlvmPhaseV1::StripDebugInfo => 2,
        FixedProductionLlvmPhaseV1::PreserveExpectedExports => 3,
        FixedProductionLlvmPhaseV1::DefaultPerModulePipelineO2 => 4,
        FixedProductionLlvmPhaseV1::CanonicalizeCov6ImplicitArguments => 5,
        FixedProductionLlvmPhaseV1::VerifyOptimizedModule => 6,
        FixedProductionLlvmPhaseV1::EmitAmdgpuObject => 7,
        FixedProductionLlvmPhaseV1::LinkHsacoWithFixedLldPolicy => 8,
    }
}

const fn pass_ir_unit_tag(unit: LlvmPassIrUnitV1) -> u8 {
    match unit {
        LlvmPassIrUnitV1::Module => 1,
        LlvmPassIrUnitV1::Cgscc => 2,
        LlvmPassIrUnitV1::Function => 3,
        LlvmPassIrUnitV1::Loop => 4,
    }
}

fn push_stage_identity(output: &mut Vec<u8>, identity: ExactCompilerStageContentIdentityV1) {
    output.extend_from_slice(&identity.sha256);
    output.extend_from_slice(&identity.byte_len.to_le_bytes());
}
