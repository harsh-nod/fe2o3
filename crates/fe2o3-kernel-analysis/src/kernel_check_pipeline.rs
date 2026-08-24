//! Closed, deterministic target-neutral kernel check pipeline.
//!
//! These passes report rejected and unresolved obligations. They never grant
//! proof, compiler-refinement, artifact, publication, load, or launch
//! authority. Frontends remain responsible for authenticating their source-to-
//! Kernel-IR correspondence before a future authority-bearing consumer may use
//! a clean report.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fe2o3_kernel_ir::{
    AddressSpace, Diagnostic as IrDiagnostic, ExplicitLaunchExtent1d, FormalBoundsRequirement,
    FormalIndexWidth, FormalMemoryIncompleteReason, FormalMemoryObligationAnalysis, Function,
    FunctionId, FunctionOperationLocation, InterInvocationConflictRequirement, KernelId,
    MatrixLdsProfile, MatrixOperationKind, Module, OperationKind, RuntimeAliasRequirement,
    TensorLayoutFindingV1, ValueId, VerificationErrors, VerifiedKernelIrModuleV1,
    derive_kernel_memory_obligations_from_verified, verify_module_ref,
    verify_tensor_layout_contract_v1,
};

use crate::{
    ControlFlowDiagnosticV2, ControlFlowErrors, Diagnostic as UniformityDiagnostic,
    UnsupportedReason, Variation, analyze_control_flow, analyze_kernel_entry,
};

/// Exact order of the mandatory target-neutral kernel checks.
///
/// The order is part of the API: later passes consume facts established or
/// cached by earlier passes, and no lowering pass may run between them.
pub const GENERAL_KERNEL_CHECK_PASS_ORDER_V1: [KernelCheckPassKindV1; 7] = [
    KernelCheckPassKindV1::Structural,
    KernelCheckPassKindV1::ControlFlow,
    KernelCheckPassKindV1::TensorLayout,
    KernelCheckPassKindV1::MemoryBounds,
    KernelCheckPassKindV1::RaceFreedom,
    KernelCheckPassKindV1::BarrierConvergence,
    KernelCheckPassKindV1::WorkgroupMemory,
];

/// One mandatory analysis pass in the general kernel-check pipeline.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelCheckPassKindV1 {
    Structural,
    ControlFlow,
    MemoryBounds,
    TensorLayout,
    AtomicLegality,
    RaceFreedom,
    HierarchicalOwnership,
    BarrierConvergence,
    WorkgroupMemory,
    SemanticRefinement,
}

impl KernelCheckPassKindV1 {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Structural => "kernel-structural-v1",
            Self::ControlFlow => "kernel-control-flow-v1",
            Self::MemoryBounds => "kernel-memory-bounds-v1",
            Self::TensorLayout => "kernel-tensor-layout-v1",
            Self::AtomicLegality => "kernel-atomic-legality-v1",
            Self::RaceFreedom => "kernel-race-freedom-v1",
            Self::HierarchicalOwnership => "kernel-hierarchical-ownership-v1",
            Self::BarrierConvergence => "kernel-barrier-convergence-v1",
            Self::WorkgroupMemory => "kernel-workgroup-memory-v1",
            Self::SemanticRefinement => "kernel-semantic-refinement-v1",
        }
    }
}

/// Conservative outcome of one analysis pass.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelCheckStatusV1 {
    Clean,
    Incomplete,
    Rejected,
}

impl KernelCheckStatusV1 {
    /// Combines pass evidence conservatively, with rejection taking precedence.
    pub const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Rejected, _) | (_, Self::Rejected) => Self::Rejected,
            (Self::Incomplete, _) | (_, Self::Incomplete) => Self::Incomplete,
            (Self::Clean, Self::Clean) => Self::Clean,
        }
    }
}

/// A stable symbolic name used only to explain a source-level bound.
///
/// It is diagnostic metadata, not proof evidence. Construction is checked so
/// diagnostics remain bounded and single-line.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KernelCheckSymbolV1(String);

impl KernelCheckSymbolV1 {
    pub fn new(value: impl Into<String>) -> Result<Self, KernelCheckDescriptionErrorV1> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_graphic() && byte != b'`')
        {
            return Err(KernelCheckDescriptionErrorV1::InvalidSymbol);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KernelCheckSymbolV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelMemoryAccessKindV1 {
    Read,
    Write,
    Atomic,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelBoundStatusV1 {
    Proven,
    Unproved,
}

/// One dimension of one source-level memory access.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KernelBoundDimensionV1 {
    memory: KernelCheckSymbolV1,
    dimension: u8,
    index: KernelCheckSymbolV1,
    extent: KernelCheckSymbolV1,
    status: KernelBoundStatusV1,
}

impl KernelBoundDimensionV1 {
    pub fn new(
        memory: impl Into<String>,
        dimension: u8,
        index: impl Into<String>,
        extent: impl Into<String>,
        status: KernelBoundStatusV1,
    ) -> Result<Self, KernelCheckDescriptionErrorV1> {
        Ok(Self {
            memory: KernelCheckSymbolV1::new(memory)?,
            dimension,
            index: KernelCheckSymbolV1::new(index)?,
            extent: KernelCheckSymbolV1::new(extent)?,
            status,
        })
    }

    pub const fn dimension(&self) -> u8 {
        self.dimension
    }

    pub const fn status(&self) -> KernelBoundStatusV1 {
        self.status
    }

    pub fn memory(&self) -> &KernelCheckSymbolV1 {
        &self.memory
    }

    pub fn index(&self) -> &KernelCheckSymbolV1 {
        &self.index
    }

    pub fn extent(&self) -> &KernelCheckSymbolV1 {
        &self.extent
    }

    fn write_relation(&self, formatter: &mut fmt::Formatter<'_>, verb: &str) -> fmt::Result {
        write!(
            formatter,
            "{} dimension {} {verb} `{} < {}`",
            self.memory, self.dimension, self.index, self.extent,
        )
    }
}

impl fmt::Display for KernelBoundDimensionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_relation(formatter, "requires")
    }
}

/// All dimension checks for one source-level memory access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelBoundAssessmentV1 {
    access: KernelMemoryAccessKindV1,
    dimensions: Vec<KernelBoundDimensionV1>,
}

impl KernelBoundAssessmentV1 {
    pub fn new(
        access: KernelMemoryAccessKindV1,
        dimensions: impl IntoIterator<Item = KernelBoundDimensionV1>,
    ) -> Result<Self, KernelCheckDescriptionErrorV1> {
        let dimensions = dimensions.into_iter().collect::<Vec<_>>();
        if dimensions.is_empty() || dimensions.len() > 16 {
            return Err(KernelCheckDescriptionErrorV1::InvalidDimensionCount);
        }
        let memory = dimensions[0].memory();
        let mut previous = None;
        for dimension in &dimensions {
            if dimension.memory() != memory
                || previous.is_some_and(|previous| dimension.dimension() <= previous)
            {
                return Err(KernelCheckDescriptionErrorV1::InconsistentDimensions);
            }
            previous = Some(dimension.dimension());
        }
        Ok(Self { access, dimensions })
    }

    pub const fn access(&self) -> KernelMemoryAccessKindV1 {
        self.access
    }

    pub fn dimensions(&self) -> &[KernelBoundDimensionV1] {
        &self.dimensions
    }

    pub fn has_exact_statuses(&self, statuses: &[KernelBoundStatusV1]) -> bool {
        self.dimensions.len() == statuses.len()
            && self
                .dimensions
                .iter()
                .zip(statuses)
                .all(|(dimension, expected)| dimension.status() == *expected)
    }

    pub fn is_proven(&self) -> bool {
        self.dimensions
            .iter()
            .all(|dimension| dimension.status() == KernelBoundStatusV1::Proven)
    }
}

impl fmt::Display for KernelBoundAssessmentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut failed = self
            .dimensions
            .iter()
            .filter(|dimension| dimension.status() == KernelBoundStatusV1::Unproved);
        let Some(first_failed) = failed.next() else {
            return formatter.write_str("all memory access bounds are proven");
        };
        write!(
            formatter,
            "failed bound: {first_failed}, but that relation is not established on every path to the access",
        )?;
        for dimension in failed {
            write!(formatter, ", and {dimension}")?;
        }
        for dimension in self
            .dimensions
            .iter()
            .filter(|dimension| dimension.status() == KernelBoundStatusV1::Proven)
        {
            formatter.write_str("; proven bound: ")?;
            dimension.write_relation(formatter, "satisfies")?;
        }
        formatter.write_str(
            "; help: guard every path to the access with the failed relation or use a checked operation that supplies a defined tail value",
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelCheckDescriptionErrorV1 {
    InvalidSymbol,
    InvalidDimensionCount,
    InconsistentDimensions,
}

impl fmt::Display for KernelCheckDescriptionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSymbol => {
                "kernel-check symbols must be 1..=128 single-line graphic ASCII bytes without backticks"
            }
            Self::InvalidDimensionCount => {
                "a memory access must have 1..=16 checked dimensions"
            }
            Self::InconsistentDimensions => {
                "memory dimensions must name one object and have strictly increasing dimension numbers"
            }
        })
    }
}

impl std::error::Error for KernelCheckDescriptionErrorV1 {}

/// A typed, non-authoritative finding from one general analysis pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelCheckFindingV1 {
    Structural(IrDiagnostic),
    ControlFlow(ControlFlowDiagnosticV2),
    MemoryAnalysisIncomplete(FormalMemoryIncompleteReason),
    RuntimeBoundsAuthenticationRequired(FormalBoundsRequirement),
    RuntimeAliasAuthenticationRequired(RuntimeAliasRequirement),
    InterInvocationConflict(InterInvocationConflictRequirement),
    TensorLayout {
        function: FunctionId,
        location: FunctionOperationLocation,
        finding: TensorLayoutFindingV1,
    },
    DivergentTensorInstruction {
        function: FunctionId,
        location: FunctionOperationLocation,
        control: Variation,
    },
    DivergentBarrier {
        function: FunctionId,
        location: FunctionOperationLocation,
        control: Variation,
    },
    BarrierAnalysisIncomplete {
        function: FunctionId,
        block: Option<fe2o3_kernel_ir::BlockId>,
        operation_index: Option<usize>,
        reason: UnsupportedReason,
    },
    WorkgroupReadBeforePublish {
        function: FunctionId,
        location: FunctionOperationLocation,
        base: ValueId,
        profile: MatrixLdsProfile,
    },
    WorkgroupMemoryIncomplete {
        function: FunctionId,
        location: FunctionOperationLocation,
        reason: WorkgroupMemoryIncompleteReasonV1,
    },
    WorkgroupMemoryUnavailable {
        function: FunctionId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkgroupMemoryIncompleteReasonV1 {
    UnsupportedWorkgroupMemoryEffect,
    BarrierWithoutWorkgroupMemorySemantics,
}

impl WorkgroupMemoryIncompleteReasonV1 {
    const fn description(self) -> &'static str {
        match self {
            Self::UnsupportedWorkgroupMemoryEffect => {
                "the workgroup-memory operation is outside the modeled matrix-LDS profile"
            }
            Self::BarrierWithoutWorkgroupMemorySemantics => {
                "the barrier does not publish workgroup memory"
            }
        }
    }
}

impl KernelCheckFindingV1 {
    /// Returns whether this finding is a concrete rejection or an unresolved obligation.
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::Structural(_)
            | Self::ControlFlow(_)
            | Self::InterInvocationConflict(_)
            | Self::DivergentTensorInstruction { .. }
            | Self::DivergentBarrier { .. }
            | Self::WorkgroupReadBeforePublish { .. } => KernelCheckStatusV1::Rejected,
            Self::TensorLayout { finding, .. } if !finding.is_incomplete() => {
                KernelCheckStatusV1::Rejected
            }
            Self::TensorLayout { .. } => KernelCheckStatusV1::Incomplete,
            Self::MemoryAnalysisIncomplete(_)
            | Self::RuntimeBoundsAuthenticationRequired(_)
            | Self::RuntimeAliasAuthenticationRequired(_)
            | Self::BarrierAnalysisIncomplete { .. }
            | Self::WorkgroupMemoryIncomplete { .. }
            | Self::WorkgroupMemoryUnavailable { .. } => KernelCheckStatusV1::Incomplete,
        }
    }

    /// Primary Kernel IR operation location, when the finding names one.
    pub const fn operation_location(&self) -> Option<FunctionOperationLocation> {
        match self {
            Self::RuntimeBoundsAuthenticationRequired(requirement) => Some(requirement.location()),
            Self::TensorLayout { location, .. } => Some(*location),
            Self::InterInvocationConflict(requirement) => Some(requirement.left()),
            Self::DivergentTensorInstruction { location, .. }
            | Self::DivergentBarrier { location, .. }
            | Self::WorkgroupReadBeforePublish { location, .. }
            | Self::WorkgroupMemoryIncomplete { location, .. } => Some(*location),
            Self::BarrierAnalysisIncomplete {
                block: Some(block),
                operation_index: Some(operation_index),
                ..
            } => Some(FunctionOperationLocation::new(*block, *operation_index)),
            Self::Structural(_)
            | Self::ControlFlow(_)
            | Self::MemoryAnalysisIncomplete(_)
            | Self::RuntimeAliasAuthenticationRequired(_)
            | Self::BarrierAnalysisIncomplete { .. }
            | Self::WorkgroupMemoryUnavailable { .. } => None,
        }
    }
}

impl fmt::Display for KernelCheckFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structural(diagnostic) => diagnostic.fmt(formatter),
            Self::ControlFlow(diagnostic) => diagnostic.fmt(formatter),
            Self::MemoryAnalysisIncomplete(reason) => {
                formatter.write_str("memory analysis is incomplete: ")?;
                write_memory_incomplete_reason(formatter, reason)
            }
            Self::RuntimeBoundsAuthenticationRequired(requirement) => write!(
                formatter,
                "memory access at {} requires runtime argument {} to contain at least {} bytes",
                display_operation_location(requirement.location()),
                requirement.allocation().parameter_index(),
                requirement.minimum_byte_len(),
            ),
            Self::RuntimeAliasAuthenticationRequired(requirement) => write!(
                formatter,
                "runtime arguments {} and {} require an authenticated non-aliasing check for byte ranges {}..{} and {}..{}",
                requirement.left().parameter_index(),
                requirement.right().parameter_index(),
                requirement.left_accessed_bytes().start(),
                requirement.left_accessed_bytes().end_exclusive(),
                requirement.right_accessed_bytes().start(),
                requirement.right_accessed_bytes().end_exclusive(),
            ),
            Self::InterInvocationConflict(requirement) => write!(
                formatter,
                "possible inter-invocation memory conflict on argument {} between {} and {}",
                requirement.allocation().parameter_index(),
                display_operation_location(requirement.left()),
                display_operation_location(requirement.right()),
            ),
            Self::TensorLayout {
                function,
                location,
                finding,
            } => write!(
                formatter,
                "tensor layout in {function} at {}: {finding}",
                display_operation_location(*location),
            ),
            Self::DivergentTensorInstruction {
                function,
                location,
                control,
            } => write!(
                formatter,
                "tensor instruction in {function} at {} is controlled by {control:?} data",
                display_operation_location(*location),
            ),
            Self::DivergentBarrier {
                function,
                location,
                control,
            } => write!(
                formatter,
                "barrier in {function} at {} is controlled by {control:?} data",
                display_operation_location(*location),
            ),
            Self::BarrierAnalysisIncomplete {
                function,
                block,
                operation_index,
                reason,
            } => {
                write!(formatter, "barrier analysis for {function}")?;
                if let Some(block) = block {
                    write!(formatter, " at {block}")?;
                }
                if let Some(operation_index) = operation_index {
                    write!(formatter, " op {operation_index}")?;
                }
                write!(formatter, " is incomplete: {reason:?}")
            }
            Self::WorkgroupReadBeforePublish {
                function,
                location,
                base,
                profile,
            } => write!(
                formatter,
                "workgroup-memory read in {function} at {} may observe unpublished data from {base} with profile {profile:?}",
                display_operation_location(*location),
            ),
            Self::WorkgroupMemoryIncomplete {
                function,
                location,
                reason,
            } => write!(
                formatter,
                "workgroup-memory analysis for {function} at {} is incomplete: {}",
                display_operation_location(*location),
                reason.description(),
            ),
            Self::WorkgroupMemoryUnavailable { function } => write!(
                formatter,
                "workgroup-memory analysis for {function} requires valid reducible control flow",
            ),
        }
    }
}

#[derive(Clone, Copy)]
struct DisplayOperationLocation(FunctionOperationLocation);

impl fmt::Display for DisplayOperationLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} op {}", self.0.block, self.0.operation_index,)
    }
}

const fn display_operation_location(
    location: FunctionOperationLocation,
) -> DisplayOperationLocation {
    DisplayOperationLocation(location)
}

fn write_memory_incomplete_reason(
    formatter: &mut fmt::Formatter<'_>,
    reason: &FormalMemoryIncompleteReason,
) -> fmt::Result {
    match reason {
        FormalMemoryIncompleteReason::UnsupportedIndexWidth { width } => {
            write!(formatter, "index width {width:?} is unsupported")
        }
        FormalMemoryIncompleteReason::LaunchExtentUnknown => {
            formatter.write_str("the launch extent is unknown")
        }
        FormalMemoryIncompleteReason::LaunchExtentZero => {
            formatter.write_str("the launch extent is zero")
        }
        FormalMemoryIncompleteReason::LaunchRankUnsupported { rank } => {
            write!(formatter, "launch rank {rank} is unsupported")
        }
        FormalMemoryIncompleteReason::LaunchRankMismatch {
            domain_rank,
            extent_rank,
        } => write!(
            formatter,
            "launch-domain rank {domain_rank} does not match analyzed rank {extent_rank}",
        ),
        FormalMemoryIncompleteReason::LaunchExtentShapeMismatch { rank, extents } => write!(
            formatter,
            "rank-{rank} analyzed launch has invalid extents {extents:?}",
        ),
        FormalMemoryIncompleteReason::LaunchExtentOverflow { rank, extents } => write!(
            formatter,
            "rank-{rank} analyzed launch extents {extents:?} overflow the invocation range",
        ),
        FormalMemoryIncompleteReason::StaticLaunchExtentMismatch { expected, actual } => write!(
            formatter,
            "static launch extent {expected} does not match analyzed extent {actual}",
        ),
        FormalMemoryIncompleteReason::StaticLaunchAxisExtentMismatch {
            axis,
            expected,
            actual,
        } => write!(
            formatter,
            "static {axis:?} launch extent {expected} does not match analyzed extent {actual}",
        ),
        FormalMemoryIncompleteReason::CallEffectsUnavailable { location, callee } => write!(
            formatter,
            "effects of call to {callee} at {} are unavailable",
            display_operation_location(*location),
        ),
        FormalMemoryIncompleteReason::UnsupportedMemoryEffect { location } => write!(
            formatter,
            "memory effect at {} is outside the modeled profile",
            display_operation_location(*location),
        ),
        FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { location } => write!(
            formatter,
            "guarded memory access at {} requires an exact ranked bounds/race proof",
            display_operation_location(*location),
        ),
        FormalMemoryIncompleteReason::UnsupportedPointerDerivation { location, pointer } => write!(
            formatter,
            "pointer {pointer} at {} has an unsupported derivation",
            display_operation_location(*location),
        ),
        FormalMemoryIncompleteReason::UnsupportedIndexExpression {
            location,
            index,
            allocation,
        } => write!(
            formatter,
            "index {index} at {} has an unsupported expression for allocation parameter {}",
            display_operation_location(*location),
            allocation.parameter_index(),
        ),
        FormalMemoryIncompleteReason::ElementWidthUnavailable { location, pointer } => write!(
            formatter,
            "element width for pointer {pointer} at {} is unavailable",
            display_operation_location(*location),
        ),
        FormalMemoryIncompleteReason::AddressArithmeticOverflow { location } => write!(
            formatter,
            "address arithmetic at {} overflows the modeled range",
            display_operation_location(*location),
        ),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelCheckPassReportV1 {
    pass: KernelCheckPassKindV1,
    findings: Vec<KernelCheckFindingV1>,
}

impl KernelCheckPassReportV1 {
    fn new(pass: KernelCheckPassKindV1, findings: Vec<KernelCheckFindingV1>) -> Self {
        Self { pass, findings }
    }

    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        self.pass
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[KernelCheckFindingV1] {
        &self.findings
    }
}

/// Complete output of the fixed general kernel-check pass sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelCheckReportV1 {
    kernel: KernelId,
    passes: Vec<KernelCheckPassReportV1>,
}

impl KernelCheckReportV1 {
    pub fn kernel(&self) -> &KernelId {
        &self.kernel
    }

    pub fn passes(&self) -> &[KernelCheckPassReportV1] {
        &self.passes
    }

    pub fn pass(&self, kind: KernelCheckPassKindV1) -> Option<&KernelCheckPassReportV1> {
        self.passes.iter().find(|report| report.pass == kind)
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.passes
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, pass| {
                status.join(pass.status())
            })
    }

    pub fn rejected_findings(&self) -> impl Iterator<Item = &KernelCheckFindingV1> {
        self.passes
            .iter()
            .flat_map(KernelCheckPassReportV1::findings)
            .filter(|finding| finding.status() == KernelCheckStatusV1::Rejected)
    }

    pub const fn proves_source_correspondence(&self) -> bool {
        false
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelCheckRequestV1<'module> {
    pub module: &'module Module,
    pub kernel: &'module KernelId,
    pub launch_extent: ExplicitLaunchExtent1d,
    pub index_width: FormalIndexWidth,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelCheckPipelineErrorV1 {
    MissingKernel { kernel: KernelId },
}

impl fmt::Display for KernelCheckPipelineErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKernel { kernel } => {
                write!(formatter, "kernel {kernel} is not present in the module")
            }
        }
    }
}

impl std::error::Error for KernelCheckPipelineErrorV1 {}

/// Runs the fixed target-neutral checks without invoking lowering or codegen.
///
/// Structural failure is terminal because later analyses require verified SSA
/// and operation typing. All other passes run in their fixed order so one
/// invocation reports every independent safety obligation it can derive.
pub fn run_general_kernel_checks_v1(
    request: KernelCheckRequestV1<'_>,
) -> Result<KernelCheckReportV1, KernelCheckPipelineErrorV1> {
    let kernel = request
        .module
        .kernels
        .iter()
        .find(|kernel| &kernel.id == request.kernel)
        .ok_or_else(|| KernelCheckPipelineErrorV1::MissingKernel {
            kernel: request.kernel.clone(),
        })?;

    let verified = match verify_module_ref(request.module) {
        Ok(verified) => verified,
        Err(errors) => {
            let passes = vec![structural_failure(errors)];
            return Ok(KernelCheckReportV1 {
                kernel: kernel.id.clone(),
                passes,
            });
        }
    };
    run_general_kernel_checks_from_verified_v1(
        verified,
        request.kernel,
        request.launch_extent,
        request.index_width,
    )
}

/// Runs the fixed checks while reusing a prior structural verification.
///
/// This is the production composition entry point when one module contains
/// several kernels. The private field of [`VerifiedKernelIrModuleV1`] prevents
/// callers from skipping structural verification.
pub fn run_general_kernel_checks_from_verified_v1(
    verified: VerifiedKernelIrModuleV1<'_>,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent1d,
    index_width: FormalIndexWidth,
) -> Result<KernelCheckReportV1, KernelCheckPipelineErrorV1> {
    let module = verified.module();
    let kernel = module
        .kernels
        .iter()
        .find(|kernel| &kernel.id == kernel_id)
        .ok_or_else(|| KernelCheckPipelineErrorV1::MissingKernel {
            kernel: kernel_id.clone(),
        })?;

    let mut passes = Vec::with_capacity(GENERAL_KERNEL_CHECK_PASS_ORDER_V1.len());
    passes.push(KernelCheckPassReportV1::new(
        KernelCheckPassKindV1::Structural,
        Vec::new(),
    ));

    let entry = module
        .function(&kernel.entry)
        .expect("verified kernel entry exists");
    let control_flow = analyze_control_flow(entry);
    let uniformity = analyze_kernel_entry(module, entry);
    passes.push(control_flow_report(&control_flow));
    passes.push(tensor_layout_report(entry, &uniformity));

    // One extraction is shared by the bounds and race passes.
    let memory = derive_kernel_memory_obligations_from_verified(
        verified,
        kernel_id,
        launch_extent,
        index_width,
    )
    .expect("verified module and selected kernel remain valid");
    passes.push(memory_bounds_report(&memory));
    passes.push(race_report(&memory));
    passes.push(barrier_report(entry, &uniformity));
    passes.push(match control_flow {
        Ok(control_flow) => workgroup_memory_report(entry, &control_flow),
        Err(_) => KernelCheckPassReportV1::new(
            KernelCheckPassKindV1::WorkgroupMemory,
            vec![KernelCheckFindingV1::WorkgroupMemoryUnavailable {
                function: entry.id.clone(),
            }],
        ),
    });

    Ok(KernelCheckReportV1 {
        kernel: kernel.id.clone(),
        passes,
    })
}

fn structural_failure(errors: VerificationErrors) -> KernelCheckPassReportV1 {
    KernelCheckPassReportV1::new(
        KernelCheckPassKindV1::Structural,
        errors
            .into_diagnostics()
            .into_iter()
            .map(KernelCheckFindingV1::Structural)
            .collect(),
    )
}

fn control_flow_report(
    analysis: &Result<crate::ControlFlowAnalysis, ControlFlowErrors>,
) -> KernelCheckPassReportV1 {
    KernelCheckPassReportV1::new(
        KernelCheckPassKindV1::ControlFlow,
        analysis
            .as_ref()
            .err()
            .into_iter()
            .flat_map(|errors| errors.diagnostics_v2().iter().cloned())
            .map(KernelCheckFindingV1::ControlFlow)
            .collect(),
    )
}

fn tensor_layout_report(
    function: &Function,
    uniformity: &crate::AnalysisReport,
) -> KernelCheckPassReportV1 {
    let findings =
        function
            .body
            .as_ref()
            .into_iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| {
                block.operations.iter().enumerate().filter_map(
                    move |(operation_index, operation)| {
                        let OperationKind::Matrix(matrix) = &operation.kind else {
                            return None;
                        };
                        let MatrixOperationKind::MultiplyAccumulate { .. } = &matrix.kind else {
                            return None;
                        };
                        Some((
                            FunctionOperationLocation::new(block.id, operation_index),
                            matrix.tensor_layout.as_ref(),
                        ))
                    },
                )
            })
            .flat_map(|(location, contract)| {
                let mut findings = contract
                    .into_iter()
                    .flat_map(verify_tensor_layout_contract_v1)
                    .map(|finding| KernelCheckFindingV1::TensorLayout {
                        function: function.id.clone(),
                        location,
                        finding,
                    })
                    .collect::<Vec<_>>();
                let control = uniformity.block_control(location.block);
                if !control.is_uniform_for(fe2o3_kernel_ir::SynchronizationScope::Subgroup) {
                    findings.push(KernelCheckFindingV1::DivergentTensorInstruction {
                        function: function.id.clone(),
                        location,
                        control,
                    });
                }
                findings
            })
            .collect();
    KernelCheckPassReportV1::new(KernelCheckPassKindV1::TensorLayout, findings)
}

fn memory_bounds_report(analysis: &FormalMemoryObligationAnalysis) -> KernelCheckPassReportV1 {
    let obligations = analysis.obligations();
    let findings = analysis
        .incomplete_reasons()
        .iter()
        .cloned()
        .map(KernelCheckFindingV1::MemoryAnalysisIncomplete)
        .chain(
            obligations
                .bounds_requirements()
                .iter()
                .copied()
                .map(KernelCheckFindingV1::RuntimeBoundsAuthenticationRequired),
        )
        .collect();
    KernelCheckPassReportV1::new(KernelCheckPassKindV1::MemoryBounds, findings)
}

fn race_report(analysis: &FormalMemoryObligationAnalysis) -> KernelCheckPassReportV1 {
    let obligations = analysis.obligations();
    let findings = analysis
        .incomplete_reasons()
        .iter()
        .cloned()
        .map(KernelCheckFindingV1::MemoryAnalysisIncomplete)
        .chain(
            obligations
                .runtime_alias_requirements()
                .iter()
                .copied()
                .map(KernelCheckFindingV1::RuntimeAliasAuthenticationRequired),
        )
        .chain(
            obligations
                .inter_invocation_conflicts()
                .iter()
                .copied()
                .map(KernelCheckFindingV1::InterInvocationConflict),
        )
        .collect();
    KernelCheckPassReportV1::new(KernelCheckPassKindV1::RaceFreedom, findings)
}

fn barrier_report(
    function: &Function,
    analysis: &crate::AnalysisReport,
) -> KernelCheckPassReportV1 {
    let findings = analysis
        .diagnostics()
        .iter()
        .map(|diagnostic| match diagnostic {
            UniformityDiagnostic::DivergentBarrier {
                block,
                operation_index,
                control,
                ..
            } => KernelCheckFindingV1::DivergentBarrier {
                function: function.id.clone(),
                location: FunctionOperationLocation::new(*block, *operation_index),
                control: *control,
            },
            UniformityDiagnostic::Unsupported {
                block,
                operation_index,
                reason,
            } => KernelCheckFindingV1::BarrierAnalysisIncomplete {
                function: function.id.clone(),
                block: *block,
                operation_index: *operation_index,
                reason: reason.clone(),
            },
        })
        .collect();
    KernelCheckPassReportV1::new(KernelCheckPassKindV1::BarrierConvergence, findings)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct WorkgroupMemoryStateV1 {
    written: BTreeSet<LdsRegionV1>,
    published: BTreeSet<LdsRegionV1>,
}

impl WorkgroupMemoryStateV1 {
    fn intersection<'a>(states: impl IntoIterator<Item = &'a Self>) -> Self {
        let mut states = states.into_iter();
        let Some(first) = states.next() else {
            return Self::default();
        };
        let mut result = first.clone();
        for state in states {
            result
                .written
                .retain(|region| state.written.contains(region));
            result
                .published
                .retain(|region| state.published.contains(region));
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct LdsRegionV1 {
    base: ValueId,
    profile: MatrixLdsProfile,
}

fn workgroup_memory_report(
    function: &Function,
    control_flow: &crate::ControlFlowAnalysis,
) -> KernelCheckPassReportV1 {
    let body = function
        .body
        .as_ref()
        .expect("control-flow analysis accepted a definition");
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let universe = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(matrix_lds_region)
        .collect::<BTreeSet<_>>();
    let top = WorkgroupMemoryStateV1 {
        written: universe.clone(),
        published: universe,
    };
    let mut inputs = control_flow
        .reachable_blocks()
        .iter()
        .copied()
        .map(|block| {
            let state = if block == control_flow.entry() {
                WorkgroupMemoryStateV1::default()
            } else {
                top.clone()
            };
            (block, state)
        })
        .collect::<BTreeMap<_, _>>();
    let mut outputs = inputs.clone();
    let mut successors = control_flow
        .reachable_blocks()
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in control_flow.reachable_blocks() {
        for predecessor in control_flow
            .predecessors(*block)
            .into_iter()
            .flatten()
            .filter(|predecessor| control_flow.is_reachable(**predecessor))
        {
            successors
                .get_mut(predecessor)
                .expect("reachable predecessor has successor storage")
                .insert(*block);
        }
    }

    // Finite descending worklist: each (block, region) fact is removed at most
    // once, and only successors of a changed block are revisited.
    let mut pending = control_flow.reachable_blocks().clone();
    while let Some(block) = pending.pop_first() {
        let next_input = if block == control_flow.entry() {
            WorkgroupMemoryStateV1::default()
        } else {
            WorkgroupMemoryStateV1::intersection(
                control_flow
                    .predecessors(block)
                    .into_iter()
                    .flatten()
                    .filter(|predecessor| control_flow.is_reachable(**predecessor))
                    .map(|predecessor| &outputs[predecessor]),
            )
        };
        if inputs.get(&block) != Some(&next_input) {
            inputs.insert(block, next_input.clone());
        }
        let next_output = transfer_workgroup_memory(next_input, blocks[&block], None, &function.id);
        if outputs.get(&block) != Some(&next_output) {
            outputs.insert(block, next_output);
            pending.extend(&successors[&block]);
        }
    }

    let mut findings = Vec::new();
    for block in control_flow.reachable_blocks() {
        transfer_workgroup_memory(
            inputs[block].clone(),
            blocks[block],
            Some(&mut findings),
            &function.id,
        );
    }
    KernelCheckPassReportV1::new(KernelCheckPassKindV1::WorkgroupMemory, findings)
}

fn matrix_lds_region(operation: &fe2o3_kernel_ir::Operation) -> Option<LdsRegionV1> {
    let OperationKind::Matrix(matrix) = &operation.kind else {
        return None;
    };
    match &matrix.kind {
        MatrixOperationKind::LdsLoad { base, profile }
        | MatrixOperationKind::LdsStore { base, profile, .. } => Some(LdsRegionV1 {
            base: *base,
            profile: *profile,
        }),
        MatrixOperationKind::MultiplyAccumulate { .. } => None,
    }
}

fn transfer_workgroup_memory(
    mut state: WorkgroupMemoryStateV1,
    block: &fe2o3_kernel_ir::BasicBlock,
    mut findings: Option<&mut Vec<KernelCheckFindingV1>>,
    function: &FunctionId,
) -> WorkgroupMemoryStateV1 {
    for (operation_index, operation) in block.operations.iter().enumerate() {
        let location = FunctionOperationLocation::new(block.id, operation_index);
        match &operation.kind {
            OperationKind::Matrix(matrix) => match &matrix.kind {
                MatrixOperationKind::LdsStore { base, profile, .. } => {
                    let region = LdsRegionV1 {
                        base: *base,
                        profile: *profile,
                    };
                    state.written.insert(region);
                    // A store starts a new LDS epoch. A barrier that published
                    // the prior value does not publish this replacement.
                    state.published.remove(&region);
                }
                MatrixOperationKind::LdsLoad { base, profile } => {
                    let region = LdsRegionV1 {
                        base: *base,
                        profile: *profile,
                    };
                    if !state.published.contains(&region)
                        && let Some(findings) = findings.as_mut()
                    {
                        findings.push(KernelCheckFindingV1::WorkgroupReadBeforePublish {
                            function: function.clone(),
                            location,
                            base: *base,
                            profile: *profile,
                        });
                    }
                }
                MatrixOperationKind::MultiplyAccumulate { .. } => {}
            },
            OperationKind::WorkgroupBarrier(barrier) => {
                if barrier
                    .semantics
                    .address_spaces
                    .contains(&AddressSpace::Workgroup)
                {
                    state.published.clone_from(&state.written);
                } else if let Some(findings) = findings.as_mut() {
                    findings.push(KernelCheckFindingV1::WorkgroupMemoryIncomplete {
                        function: function.clone(),
                        location,
                        reason: WorkgroupMemoryIncompleteReasonV1::BarrierWithoutWorkgroupMemorySemantics,
                    });
                }
            }
            OperationKind::Load { access, .. }
            | OperationKind::Store { access, .. }
            | OperationKind::Atomic(fe2o3_kernel_ir::Atomic { access, .. })
                if access.address_space == AddressSpace::Workgroup =>
            {
                if let Some(findings) = findings.as_mut() {
                    findings.push(KernelCheckFindingV1::WorkgroupMemoryIncomplete {
                        function: function.clone(),
                        location,
                        reason: WorkgroupMemoryIncompleteReasonV1::UnsupportedWorkgroupMemoryEffect,
                    });
                }
            }
            _ => {}
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        Axis, Barrier, BarrierSemantics, BasicBlock, BinaryOp, BlockId, CheckedBinaryOperator,
        Constant, IndexKind, IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent,
        MemoryOrdering, Operation, Signature, SynchronizationScope, TILED_GEMM_LDS_V1_KERNEL_ID,
        TILED_GEMM_LDS_V1_LANES, Terminator, Type, ValueDef, WorkgroupSize,
        tiled_gemm_lds_v1_module,
    };

    fn request(module: &Module) -> KernelCheckRequestV1<'_> {
        KernelCheckRequestV1 {
            module,
            kernel: &module.kernels[0].id,
            launch_extent: ExplicitLaunchExtent1d::Exact(u64::from(TILED_GEMM_LDS_V1_LANES)),
            index_width: FormalIndexWidth::Bits64,
        }
    }

    #[test]
    fn pipeline_order_is_fixed_and_reports_no_authority() {
        let module = tiled_gemm_lds_v1_module();
        let report = run_general_kernel_checks_v1(request(&module)).unwrap();
        assert_eq!(report.kernel().as_str(), TILED_GEMM_LDS_V1_KERNEL_ID);
        assert_eq!(
            report
                .passes()
                .iter()
                .map(KernelCheckPassReportV1::pass)
                .collect::<Vec<_>>(),
            GENERAL_KERNEL_CHECK_PASS_ORDER_V1,
        );
        assert_eq!(
            report
                .pass(KernelCheckPassKindV1::MemoryBounds)
                .unwrap()
                .status(),
            KernelCheckStatusV1::Incomplete,
        );
        assert_eq!(
            report
                .pass(KernelCheckPassKindV1::RaceFreedom)
                .unwrap()
                .status(),
            KernelCheckStatusV1::Incomplete,
        );
        assert!(!report.proves_source_correspondence());
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
    }

    #[test]
    fn production_pipeline_uses_the_authenticated_workgroup_quotient_contract() {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(0), Type::INDEX),
                OperationKind::Intrinsic(IntrinsicOperation::new(
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Global,
                        axis: Axis::X,
                    },
                    Type::INDEX,
                )),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(1), Type::INDEX),
                OperationKind::Constant(Constant::Index(64)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::INDEX),
                OperationKind::Binary {
                    op: BinaryOp::Divide,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(3), Type::INDEX),
                OperationKind::Binary {
                    op: BinaryOp::Divide,
                    lhs: ValueId(2),
                    rhs: ValueId(100),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(4), Type::INDEX),
                OperationKind::Binary {
                    op: BinaryOp::Multiply,
                    lhs: ValueId(3),
                    rhs: ValueId(100),
                },
            ),
            Operation::new(
                vec![
                    ValueDef::new(ValueId(5), Type::INDEX),
                    ValueDef::new(ValueId(6), Type::BOOL),
                ],
                OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs: ValueId(4),
                    rhs: ValueId(100),
                },
            ),
        ];
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(6),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut barrier = BasicBlock::new(BlockId(1));
        barrier.operations.push(Operation::new(
            vec![],
            OperationKind::Barrier(Barrier {
                execution_scope: SynchronizationScope::Workgroup,
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
            }),
        ));
        barrier.terminator = Some(Terminator::Return { values: vec![] });
        let mut exit = BasicBlock::new(BlockId(2));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::kernel_entry(
            "pipeline_uniform_quotient",
            Signature::new(vec![Type::INDEX], vec![]),
            vec![ValueId(100)],
            vec![entry, barrier, exit],
        );
        let mut kernel = Kernel::new(
            "pipeline_uniform_quotient_kernel",
            function.id.clone(),
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new("pipeline_uniform_quotient_module");
        module.functions.push(function);
        module.kernels.push(kernel);

        let report = run_general_kernel_checks_v1(KernelCheckRequestV1 {
            module: &module,
            kernel: &module.kernels[0].id,
            launch_extent: ExplicitLaunchExtent1d::Exact(64),
            index_width: FormalIndexWidth::Bits64,
        })
        .unwrap();
        assert_eq!(report.passes().len(), 7, "{report:#?}");
        assert_eq!(
            report
                .pass(KernelCheckPassKindV1::BarrierConvergence)
                .unwrap()
                .status(),
            KernelCheckStatusV1::Clean,
        );
    }

    #[test]
    fn source_bound_diagnostic_is_kernel_agnostic_and_dimension_specific() {
        let assessment = KernelBoundAssessmentV1::new(
            KernelMemoryAccessKindV1::Read,
            [
                KernelBoundDimensionV1::new(
                    "input",
                    0,
                    "row",
                    "height",
                    KernelBoundStatusV1::Unproved,
                )
                .unwrap(),
                KernelBoundDimensionV1::new(
                    "input",
                    1,
                    "column",
                    "width",
                    KernelBoundStatusV1::Proven,
                )
                .unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(
            assessment.to_string(),
            "failed bound: input dimension 0 requires `row < height`, but that relation is not established on every path to the access; proven bound: input dimension 1 satisfies `column < width`; help: guard every path to the access with the failed relation or use a checked operation that supplies a defined tail value",
        );
        assert!(
            assessment
                .has_exact_statuses(&[KernelBoundStatusV1::Unproved, KernelBoundStatusV1::Proven,])
        );
    }

    #[test]
    fn structural_failure_stops_before_semantic_passes() {
        let mut module = tiled_gemm_lds_v1_module();
        module.functions[0].body.as_mut().unwrap().blocks[0].terminator = None;
        let report = run_general_kernel_checks_v1(request(&module)).unwrap();
        assert_eq!(report.passes().len(), 1);
        assert_eq!(report.passes()[0].pass(), KernelCheckPassKindV1::Structural);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    }

    #[test]
    fn workgroup_memory_is_a_must_analysis_and_tracks_epochs() {
        let module = tiled_gemm_lds_v1_module();
        let clean = run_general_kernel_checks_v1(request(&module)).unwrap();
        assert_eq!(
            clean
                .pass(KernelCheckPassKindV1::WorkgroupMemory)
                .unwrap()
                .status(),
            KernelCheckStatusV1::Clean,
        );

        let mut missing_publish = tiled_gemm_lds_v1_module();
        missing_publish.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .retain(|operation| !matches!(operation.kind, OperationKind::WorkgroupBarrier(_)));
        let rejected = run_general_kernel_checks_v1(request(&missing_publish)).unwrap();
        let initialization = rejected
            .pass(KernelCheckPassKindV1::WorkgroupMemory)
            .unwrap();
        assert_eq!(initialization.status(), KernelCheckStatusV1::Rejected);
        assert!(initialization.findings().iter().any(|finding| matches!(
            finding,
            KernelCheckFindingV1::WorkgroupReadBeforePublish { .. }
        )));

        let mut missing_reuse_publish = tiled_gemm_lds_v1_module();
        let block = &mut missing_reuse_publish.functions[0]
            .body
            .as_mut()
            .unwrap()
            .blocks[0];
        let replacement_store = block
            .operations
            .iter()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Matrix(ref matrix)
                        if matches!(matrix.kind, MatrixOperationKind::LdsStore { .. })
                )
            })
            .unwrap()
            .clone();
        let barrier = block
            .operations
            .iter()
            .position(|operation| matches!(operation.kind, OperationKind::WorkgroupBarrier(_)))
            .unwrap();
        block.operations.insert(barrier + 1, replacement_store);
        let rejected = run_general_kernel_checks_v1(request(&missing_reuse_publish)).unwrap();
        assert_eq!(
            rejected
                .pass(KernelCheckPassKindV1::WorkgroupMemory)
                .unwrap()
                .status(),
            KernelCheckStatusV1::Rejected,
        );
    }

    #[test]
    fn bound_descriptions_reject_ambiguous_or_unbounded_metadata() {
        assert!(KernelCheckSymbolV1::new("").is_err());
        assert!(KernelCheckSymbolV1::new("bad`name").is_err());
        let duplicate =
            KernelBoundDimensionV1::new("buffer", 0, "i", "n", KernelBoundStatusV1::Proven)
                .unwrap();
        assert!(
            KernelBoundAssessmentV1::new(
                KernelMemoryAccessKindV1::Read,
                [duplicate.clone(), duplicate],
            )
            .is_err()
        );
        let dimension_zero =
            KernelBoundDimensionV1::new("buffer", 0, "i", "n", KernelBoundStatusV1::Proven)
                .unwrap();
        let dimension_one =
            KernelBoundDimensionV1::new("buffer", 1, "j", "m", KernelBoundStatusV1::Proven)
                .unwrap();
        assert!(
            KernelBoundAssessmentV1::new(
                KernelMemoryAccessKindV1::Read,
                [dimension_one, dimension_zero],
            )
            .is_err()
        );
    }
}
