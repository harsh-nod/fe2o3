//! Target and host-contract feasibility for workload-neutral ranked PLIRON.
//!
//! Limits and allocation bindings are compiler inputs. They are not read from
//! user-authored IR and this analysis does not authenticate runtime addresses.

use std::{collections::BTreeMap, fmt};

use dialect_gpu::ExecutionLayoutOp;
use dialect_kernel::{DYNAMIC_EXTENT, MemorySpaceAttr, RankedViewOp};
use pliron::{builtin::ops::FuncOp, common_traits::Named, context::Context, operation::Operation};

use crate::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::{KernelCheckStatusV1, derive_pliron_ir_structural_identity_v1};

pub const MAX_PLIRON_TARGET_SUBGROUP_SIZES_V1: usize = 16;
pub const MAX_PLIRON_HOST_ALLOCATIONS_V1: usize = 64;
pub const MAX_PLIRON_RUNTIME_PRECONDITIONS_V1: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironLaunchContractInputErrorV1 {
    InvalidTargetLimit(&'static str),
    InvalidHostAllocation(&'static str),
    DuplicateHostAllocation {
        origin: u64,
    },
    EmptyTargetDecisionSetIdentity,
    ResourceLimitExceeded {
        resource: &'static str,
        limit: usize,
    },
}

impl fmt::Display for PlironLaunchContractInputErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTargetLimit(detail) => write!(formatter, "invalid target limit: {detail}"),
            Self::InvalidHostAllocation(detail) => {
                write!(formatter, "invalid host allocation contract: {detail}")
            }
            Self::DuplicateHostAllocation { origin } => {
                write!(formatter, "host allocation origin {origin} is duplicated")
            }
            Self::EmptyTargetDecisionSetIdentity => {
                formatter.write_str("target decision-set identity must be nonzero")
            }
            Self::ResourceLimitExceeded { resource, limit } => {
                write!(formatter, "{resource} exceeds limit {limit}")
            }
        }
    }
}

impl std::error::Error for PlironLaunchContractInputErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironLaunchTargetLimitsV1 {
    max_grid_extents: [u64; 3],
    max_workgroup_extents: [u64; 3],
    max_workgroup_invocations: u64,
    supported_subgroup_sizes: Vec<u64>,
    max_workgroup_memory_bytes: u64,
    required_global_alignment: u64,
    max_global_allocations: usize,
}

impl PlironLaunchTargetLimitsV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_grid_extents: [u64; 3],
        max_workgroup_extents: [u64; 3],
        max_workgroup_invocations: u64,
        mut supported_subgroup_sizes: Vec<u64>,
        max_workgroup_memory_bytes: u64,
        required_global_alignment: u64,
        max_global_allocations: usize,
    ) -> Result<Self, PlironLaunchContractInputErrorV1> {
        if max_grid_extents.contains(&0)
            || max_grid_extents.contains(&u64::MAX)
            || max_workgroup_extents.contains(&0)
            || max_workgroup_extents.contains(&u64::MAX)
            || max_workgroup_invocations == 0
            || max_workgroup_invocations == u64::MAX
            || max_workgroup_memory_bytes == u64::MAX
            || required_global_alignment == 0
            || !required_global_alignment.is_power_of_two()
            || max_global_allocations > MAX_PLIRON_HOST_ALLOCATIONS_V1
        {
            return Err(PlironLaunchContractInputErrorV1::InvalidTargetLimit(
                "extents and capacities must be finite, dimensions and invocation limits must be nonzero, and alignment must be a nonzero power of two",
            ));
        }
        if supported_subgroup_sizes.is_empty()
            || supported_subgroup_sizes.len() > MAX_PLIRON_TARGET_SUBGROUP_SIZES_V1
            || supported_subgroup_sizes.contains(&0)
        {
            return Err(PlironLaunchContractInputErrorV1::ResourceLimitExceeded {
                resource: "supported subgroup sizes",
                limit: MAX_PLIRON_TARGET_SUBGROUP_SIZES_V1,
            });
        }
        supported_subgroup_sizes.sort_unstable();
        supported_subgroup_sizes.dedup();
        Ok(Self {
            max_grid_extents,
            max_workgroup_extents,
            max_workgroup_invocations,
            supported_subgroup_sizes,
            max_workgroup_memory_bytes,
            required_global_alignment,
            max_global_allocations,
        })
    }

    pub const fn max_grid_extents(&self) -> [u64; 3] {
        self.max_grid_extents
    }
    pub const fn max_workgroup_extents(&self) -> [u64; 3] {
        self.max_workgroup_extents
    }
    pub const fn max_workgroup_invocations(&self) -> u64 {
        self.max_workgroup_invocations
    }
    pub fn supported_subgroup_sizes(&self) -> &[u64] {
        &self.supported_subgroup_sizes
    }
    pub const fn max_workgroup_memory_bytes(&self) -> u64 {
        self.max_workgroup_memory_bytes
    }
    pub const fn required_global_alignment(&self) -> u64 {
        self.required_global_alignment
    }
    pub const fn max_global_allocations(&self) -> usize {
        self.max_global_allocations
    }
}

/// Exact launch geometry retained outside a function body by canonical KIR.
///
/// Production W4 derives this value from the immutable final-graph kernel root;
/// callers cannot use it as launch authority. A zero global extent denotes the
/// canonical dynamic extent and remains a runtime precondition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlironCanonicalExecutionLayoutV1 {
    global_extents: [u64; 3],
    workgroup_extents: [u64; 3],
    subgroup_size: u64,
}

impl PlironCanonicalExecutionLayoutV1 {
    pub fn try_new(
        global_extents: [u64; 3],
        workgroup_extents: [u64; 3],
        subgroup_size: u64,
    ) -> Result<Self, PlironLaunchContractInputErrorV1> {
        if workgroup_extents.contains(&0)
            || subgroup_size == 0
            || global_extents.contains(&u64::MAX)
            || workgroup_extents.contains(&u64::MAX)
            || subgroup_size == u64::MAX
        {
            return Err(PlironLaunchContractInputErrorV1::InvalidTargetLimit(
                "canonical execution layout must have finite extents and nonzero workgroup/subgroup dimensions",
            ));
        }
        Ok(Self {
            global_extents,
            workgroup_extents,
            subgroup_size,
        })
    }

    pub const fn global_extents(self) -> [u64; 3] {
        self.global_extents
    }

    pub const fn workgroup_extents(self) -> [u64; 3] {
        self.workgroup_extents
    }

    pub const fn subgroup_size(self) -> u64 {
        self.subgroup_size
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlironHostAllocationV1 {
    origin: u64,
    byte_length: Option<u64>,
    guaranteed_alignment: Option<u64>,
    noalias_class: Option<u64>,
}

impl PlironHostAllocationV1 {
    pub fn new(
        origin: u64,
        byte_length: u64,
        guaranteed_alignment: u64,
    ) -> Result<Self, PlironLaunchContractInputErrorV1> {
        if origin == 0
            || byte_length == 0
            || byte_length == u64::MAX
            || guaranteed_alignment == 0
            || !guaranteed_alignment.is_power_of_two()
        {
            return Err(PlironLaunchContractInputErrorV1::InvalidHostAllocation(
                "origin and byte length must be nonzero and alignment must be a nonzero power of two",
            ));
        }
        Ok(Self {
            origin,
            byte_length: Some(byte_length),
            guaranteed_alignment: Some(guaranteed_alignment),
            noalias_class: None,
        })
    }

    /// Declares that byte length, alignment, and alias separation must be
    /// checked by the generated host contract. It is not static evidence.
    pub fn runtime_required(origin: u64) -> Result<Self, PlironLaunchContractInputErrorV1> {
        if origin == 0 {
            return Err(PlironLaunchContractInputErrorV1::InvalidHostAllocation(
                "runtime-checked allocation origin must be nonzero",
            ));
        }
        Ok(Self {
            origin,
            byte_length: None,
            guaranteed_alignment: None,
            noalias_class: None,
        })
    }

    /// Adds a compiler-known alias partition. Unknown partitions remain a
    /// runtime precondition rather than being inferred from the origin.
    pub fn with_static_noalias_class(mut self, noalias_class: u64) -> Self {
        self.noalias_class = Some(noalias_class);
        self
    }
    pub const fn origin(self) -> u64 {
        self.origin
    }
    pub const fn byte_length(self) -> Option<u64> {
        self.byte_length
    }
    pub const fn guaranteed_alignment(self) -> Option<u64> {
        self.guaranteed_alignment
    }
    pub const fn noalias_class(self) -> Option<u64> {
        self.noalias_class
    }

    pub const fn requires_runtime_validation(self) -> bool {
        self.byte_length.is_none()
            || self.guaranteed_alignment.is_none()
            || self.noalias_class.is_none()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironLaunchContractV1 {
    limits: PlironLaunchTargetLimitsV1,
    host_allocations: BTreeMap<u64, PlironHostAllocationV1>,
    target_decision_set_identity: Option<[u8; 32]>,
}

impl PlironLaunchContractV1 {
    pub fn new(
        limits: PlironLaunchTargetLimitsV1,
        host_allocations: Vec<PlironHostAllocationV1>,
    ) -> Result<Self, PlironLaunchContractInputErrorV1> {
        if host_allocations.len() > MAX_PLIRON_HOST_ALLOCATIONS_V1 {
            return Err(PlironLaunchContractInputErrorV1::ResourceLimitExceeded {
                resource: "host allocation bindings",
                limit: MAX_PLIRON_HOST_ALLOCATIONS_V1,
            });
        }
        let mut by_origin = BTreeMap::new();
        for allocation in host_allocations {
            if by_origin.insert(allocation.origin(), allocation).is_some() {
                return Err(PlironLaunchContractInputErrorV1::DuplicateHostAllocation {
                    origin: allocation.origin(),
                });
            }
        }
        Ok(Self {
            limits,
            host_allocations: by_origin,
            target_decision_set_identity: None,
        })
    }

    /// Binds all target limits in this contract to the exact validated target
    /// decision set that W4 will retain. The identity is compared again at the
    /// W4 boundary; this method itself grants no authority.
    pub fn new_bound_to_target_decisions(
        limits: PlironLaunchTargetLimitsV1,
        host_allocations: Vec<PlironHostAllocationV1>,
        target_decision_set_identity: [u8; 32],
    ) -> Result<Self, PlironLaunchContractInputErrorV1> {
        if target_decision_set_identity == [0; 32] {
            return Err(PlironLaunchContractInputErrorV1::EmptyTargetDecisionSetIdentity);
        }
        let mut contract = Self::new(limits, host_allocations)?;
        contract.target_decision_set_identity = Some(target_decision_set_identity);
        Ok(contract)
    }
    pub const fn limits(&self) -> &PlironLaunchTargetLimitsV1 {
        &self.limits
    }
    pub fn host_allocation(&self, origin: u64) -> Option<PlironHostAllocationV1> {
        self.host_allocations.get(&origin).copied()
    }
    pub fn host_allocations(&self) -> impl ExactSizeIterator<Item = PlironHostAllocationV1> + '_ {
        self.host_allocations.values().copied()
    }
    pub const fn target_decision_set_identity(&self) -> Option<&[u8; 32]> {
        self.target_decision_set_identity.as_ref()
    }
}

/// A dynamic launch/allocation obligation emitted for the checked host API.
/// Presence of these records never counts as static proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironRuntimePreconditionV1 {
    GridExtentAtMost {
        axis: usize,
        limit: u64,
    },
    WorkgroupMemoryBytesAtMost {
        limit: u64,
    },
    GlobalViewByteLengthFits {
        view: String,
        origin: u64,
        static_required: Option<u64>,
    },
    GlobalAllocationAlignmentAtLeast {
        view: String,
        origin: u64,
        required: u64,
    },
    GlobalAllocationNoAliasClass {
        view: String,
        origin: u64,
        required_class: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironLaunchContractFindingV1 {
    StructuralPrerequisiteRejected,
    MissingExecutionLayout,
    DuplicateExecutionLayout {
        count: usize,
    },
    DynamicGridExtent {
        axis: usize,
    },
    GridExtentExceedsTarget {
        axis: usize,
        actual: u64,
        limit: u64,
    },
    WorkgroupExtentExceedsTarget {
        axis: usize,
        actual: u64,
        limit: u64,
    },
    WorkgroupInvocationsExceedTarget {
        actual: u64,
        limit: u64,
    },
    UnsupportedSubgroupSize {
        actual: u64,
        supported: Vec<u64>,
    },
    WorkgroupMemorySizeUnknown {
        view: String,
        dimension: usize,
    },
    WorkgroupMemoryProvenanceUnknown {
        view: String,
    },
    WorkgroupMemoryArithmeticOverflow {
        view: String,
    },
    WorkgroupMemoryExceedsTarget {
        actual: u64,
        limit: u64,
    },
    GlobalAllocationOriginUnknown {
        view: String,
    },
    MissingHostAllocation {
        view: String,
        origin: u64,
    },
    GlobalViewSizeUnknown {
        view: String,
        origin: u64,
        dimension: usize,
    },
    GlobalViewSizeArithmeticOverflow {
        view: String,
        origin: u64,
    },
    HostAllocationTooSmall {
        view: String,
        origin: u64,
        required: u64,
        available: u64,
    },
    HostAllocationAlignmentInsufficient {
        view: String,
        origin: u64,
        required: u64,
        guaranteed: u64,
    },
    TooManyGlobalAllocations {
        actual: usize,
        limit: usize,
    },
    GlobalAliasClassMismatch {
        view: String,
        origin: u64,
        required: u64,
        observed: u64,
    },
    ConflictingGlobalAliasClasses {
        origin: u64,
        first: u64,
        second: u64,
    },
    RuntimePreconditionLimitExceeded {
        limit: usize,
    },
}

impl PlironLaunchContractFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::MissingExecutionLayout
            | Self::DynamicGridExtent { .. }
            | Self::WorkgroupMemorySizeUnknown { .. }
            | Self::WorkgroupMemoryProvenanceUnknown { .. }
            | Self::GlobalAllocationOriginUnknown { .. }
            | Self::MissingHostAllocation { .. }
            | Self::GlobalViewSizeUnknown { .. }
            | Self::RuntimePreconditionLimitExceeded { .. } => KernelCheckStatusV1::Incomplete,
            Self::StructuralPrerequisiteRejected
            | Self::DuplicateExecutionLayout { .. }
            | Self::GridExtentExceedsTarget { .. }
            | Self::WorkgroupExtentExceedsTarget { .. }
            | Self::WorkgroupInvocationsExceedTarget { .. }
            | Self::UnsupportedSubgroupSize { .. }
            | Self::WorkgroupMemoryArithmeticOverflow { .. }
            | Self::WorkgroupMemoryExceedsTarget { .. }
            | Self::GlobalViewSizeArithmeticOverflow { .. }
            | Self::HostAllocationTooSmall { .. }
            | Self::HostAllocationAlignmentInsufficient { .. }
            | Self::TooManyGlobalAllocations { .. }
            | Self::GlobalAliasClassMismatch { .. }
            | Self::ConflictingGlobalAliasClasses { .. } => KernelCheckStatusV1::Rejected,
        }
    }
}

impl fmt::Display for PlironLaunchContractFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructuralPrerequisiteRejected => formatter.write_str("error[FE2O3-TARGET-000]: target feasibility requires a structurally verified PLIRON function; help: repair the malformed operation, type, attribute, or CFG before target admission"),
            Self::MissingExecutionLayout => formatter.write_str("error[FE2O3-TARGET-001]: target feasibility is incomplete because gpu.execution_layout is absent; help: retain compiler-derived launch geometry before target admission"),
            Self::DuplicateExecutionLayout { count } => write!(formatter, "error[FE2O3-TARGET-002]: kernel has {count} execution layouts; help: retain exactly one compiler-derived launch layout in the entry block"),
            Self::DynamicGridExtent { axis } => write!(formatter, "error[FE2O3-TARGET-003]: grid axis {axis} is dynamic, so the target limit cannot be proved statically; help: add a host launch guard for this target or specialize the extent"),
            Self::GridExtentExceedsTarget { axis, actual, limit } => write!(formatter, "error[FE2O3-TARGET-004]: grid axis {axis} extent {actual} exceeds target limit {limit}; help: tile the launch or select a target whose grid limit is sufficient"),
            Self::WorkgroupExtentExceedsTarget { axis, actual, limit } => write!(formatter, "error[FE2O3-TARGET-005]: workgroup axis {axis} extent {actual} exceeds target limit {limit}; help: reduce the workgroup shape"),
            Self::WorkgroupInvocationsExceedTarget { actual, limit } => write!(formatter, "error[FE2O3-TARGET-006]: workgroup has {actual} invocations, exceeding target limit {limit}; help: reduce the workgroup shape"),
            Self::UnsupportedSubgroupSize { actual, supported } => write!(formatter, "error[FE2O3-TARGET-007]: subgroup size {actual} is unsupported; target supports {supported:?}; help: select one supported wave size or another target"),
            Self::WorkgroupMemorySizeUnknown { view, dimension } => write!(formatter, "error[FE2O3-RESOURCE-001]: workgroup view {view} dimension {dimension} is dynamic, so LDS usage is incomplete; help: specialize the allocation extent or retain a bounded dynamic-LDS launch contract"),
            Self::WorkgroupMemoryProvenanceUnknown { view } => write!(formatter, "error[FE2O3-RESOURCE-002]: workgroup view {view} has unknown allocation provenance; help: retain a nonzero compiler-issued allocation origin"),
            Self::WorkgroupMemoryArithmeticOverflow { view } => write!(formatter, "error[FE2O3-RESOURCE-003]: byte-size arithmetic for workgroup view {view} overflowed; help: reduce its extents"),
            Self::WorkgroupMemoryExceedsTarget { actual, limit } => write!(formatter, "error[FE2O3-RESOURCE-004]: kernel requires {actual} workgroup-memory bytes, exceeding target limit {limit}; help: reduce or reuse staged storage"),
            Self::GlobalAllocationOriginUnknown { view } => write!(formatter, "error[FE2O3-ABI-001]: global view {view} has no compiler-issued allocation origin; help: preserve source allocation provenance through MIR-to-PLIRON lowering"),
            Self::MissingHostAllocation { view, origin } => write!(formatter, "error[FE2O3-ABI-002]: global view {view} origin {origin} has no host allocation contract; help: bind the kernel argument to an authenticated allocation descriptor"),
            Self::GlobalViewSizeUnknown { view, origin, dimension } => write!(formatter, "error[FE2O3-ABI-003]: global view {view} origin {origin} has dynamic dimension {dimension}, so static byte sufficiency is incomplete; help: add a runtime argument-size guard or specialize the shape"),
            Self::GlobalViewSizeArithmeticOverflow { view, origin } => write!(formatter, "error[FE2O3-ABI-007]: byte-size arithmetic for global view {view} origin {origin} overflowed; help: reduce its static extents or use a bounded dynamic view"),
            Self::HostAllocationTooSmall { view, origin, required, available } => write!(formatter, "error[FE2O3-ABI-004]: global view {view} origin {origin} requires {required} bytes but the host contract provides {available}; help: bind a sufficiently large allocation or reduce the view"),
            Self::HostAllocationAlignmentInsufficient { view, origin, required, guaranteed } => write!(formatter, "error[FE2O3-ABI-005]: global view {view} origin {origin} requires alignment {required} but the host contract guarantees {guaranteed}; help: use an aligned allocation or a target-supported access width"),
            Self::TooManyGlobalAllocations { actual, limit } => write!(formatter, "error[FE2O3-ABI-006]: kernel uses {actual} global allocations, exceeding target ABI limit {limit}; help: pack arguments into a bounded descriptor or reduce live allocations"),
            Self::GlobalAliasClassMismatch { view, origin, required, observed } => write!(formatter, "error[FE2O3-ABI-008]: global view {view} origin {origin} requires no-alias class {required} but the static host contract names class {observed}; help: preserve the compiler-derived alias partition or defer an exact alias check to host preparation"),
            Self::ConflictingGlobalAliasClasses { origin, first, second } => write!(formatter, "error[FE2O3-ABI-010]: global allocation origin {origin} carries conflicting no-alias classes {first} and {second}; help: preserve one canonical alias class for every view of the same allocation"),
            Self::RuntimePreconditionLimitExceeded { limit } => write!(formatter, "error[FE2O3-ABI-009]: dynamic launch preconditions exceed bounded limit {limit}; help: simplify or split the kernel interface"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironLaunchContractReportV1 {
    findings: Vec<PlironLaunchContractFindingV1>,
    runtime_preconditions: Vec<PlironRuntimePreconditionV1>,
    workgroup_memory_bytes: Option<u64>,
    checked_global_allocations: usize,
}

impl PlironLaunchContractReportV1 {
    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }
    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }
    pub fn findings(&self) -> &[PlironLaunchContractFindingV1] {
        &self.findings
    }
    pub fn runtime_preconditions(&self) -> &[PlironRuntimePreconditionV1] {
        &self.runtime_preconditions
    }
    pub const fn workgroup_memory_bytes(&self) -> Option<u64> {
        self.workgroup_memory_bytes
    }
    pub const fn checked_global_allocation_count(&self) -> usize {
        self.checked_global_allocations
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironLaunchContractCheckErrorV1 {
    report: PlironLaunchContractReportV1,
}

impl PlironLaunchContractCheckErrorV1 {
    pub const fn report(&self) -> &PlironLaunchContractReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironLaunchContractCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, finding) in self.report.findings.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for PlironLaunchContractCheckErrorV1 {}

pub fn run_pliron_launch_contract_check_v1(
    context: &Context,
    function: &FuncOp,
    contract: &PlironLaunchContractV1,
) -> PlironLaunchContractReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_launch_contract_check_with_analyses_v1(context, function, contract, &mut analyses)
}

pub(crate) fn run_pliron_launch_contract_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    contract: &PlironLaunchContractV1,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironLaunchContractReportV1 {
    run_pliron_launch_contract_check_with_canonical_layout_and_analyses_v1(
        context, function, contract, None, analyses,
    )
}

pub(crate) fn run_pliron_launch_contract_check_with_canonical_layout_and_analyses_v1(
    context: &Context,
    function: &FuncOp,
    contract: &PlironLaunchContractV1,
    canonical_layout: Option<PlironCanonicalExecutionLayoutV1>,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironLaunchContractReportV1 {
    // This shared preflight bounds the closed ranked subset before invoking
    // Pliron recursive verification, and contains traversal/verifier panics. A
    // successful identity therefore bounds the streaming scan below without a
    // second retained raw-operation inventory.
    if derive_pliron_ir_structural_identity_v1(context, function).is_err() {
        return structural_prerequisite_failure();
    }
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(_) => return structural_prerequisite_failure(),
    };

    let mut layout_count = usize::from(canonical_layout.is_some());
    let mut first_layout = canonical_layout.map(|layout| {
        (
            Some(layout.global_extents()),
            Some(layout.workgroup_extents()),
            Some(layout.subgroup_size()),
        )
    });
    let mut view_findings = Vec::new();
    let mut workgroup_by_origin = BTreeMap::<u64, Option<u64>>::new();
    let mut global_origins = BTreeMap::<u64, (String, Option<u64>, u64)>::new();
    let mut runtime_preconditions = Vec::new();
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        if let Some(layout) = operation.downcast_ref::<ExecutionLayoutOp>() {
            layout_count += 1;
            first_layout.get_or_insert_with(|| {
                (
                    layout.global_extents(context),
                    layout.workgroup_extents(context),
                    layout.subgroup_size(context),
                )
            });
        } else if let Some(view) = operation.downcast_ref::<RankedViewOp>() {
            fold_ranked_view(
                context,
                view,
                &mut workgroup_by_origin,
                &mut global_origins,
                &mut view_findings,
            );
        }
    }
    let mut findings = Vec::new();
    if layout_count == 0 {
        findings.push(PlironLaunchContractFindingV1::MissingExecutionLayout);
    }
    if layout_count > 1 {
        findings.push(PlironLaunchContractFindingV1::DuplicateExecutionLayout {
            count: layout_count,
        });
    }
    if let Some((Some(global), Some(workgroup), Some(subgroup))) = first_layout {
        for (axis, (actual, limit)) in global
            .into_iter()
            .zip(contract.limits.max_grid_extents)
            .enumerate()
        {
            if actual == 0 {
                push_runtime_precondition(
                    &mut runtime_preconditions,
                    &mut findings,
                    PlironRuntimePreconditionV1::GridExtentAtMost { axis, limit },
                );
            } else if actual > limit {
                findings.push(PlironLaunchContractFindingV1::GridExtentExceedsTarget {
                    axis,
                    actual,
                    limit,
                });
            }
        }
        for (axis, (actual, limit)) in workgroup
            .into_iter()
            .zip(contract.limits.max_workgroup_extents)
            .enumerate()
        {
            if actual > limit {
                findings.push(
                    PlironLaunchContractFindingV1::WorkgroupExtentExceedsTarget {
                        axis,
                        actual,
                        limit,
                    },
                );
            }
        }
        if let Some(actual) = workgroup.into_iter().try_fold(1_u64, u64::checked_mul)
            && actual > contract.limits.max_workgroup_invocations
        {
            findings.push(
                PlironLaunchContractFindingV1::WorkgroupInvocationsExceedTarget {
                    actual,
                    limit: contract.limits.max_workgroup_invocations,
                },
            );
        }
        if !contract.limits.supported_subgroup_sizes.contains(&subgroup) {
            findings.push(PlironLaunchContractFindingV1::UnsupportedSubgroupSize {
                actual: subgroup,
                supported: contract.limits.supported_subgroup_sizes.clone(),
            });
        }
    }

    findings.append(&mut view_findings);
    let has_dynamic_workgroup_memory = workgroup_by_origin.values().any(Option::is_none);
    let static_workgroup_memory_bytes = workgroup_by_origin
        .values()
        .filter_map(|bytes| *bytes)
        .try_fold(0_u64, u64::checked_add);
    if has_dynamic_workgroup_memory {
        push_runtime_precondition(
            &mut runtime_preconditions,
            &mut findings,
            PlironRuntimePreconditionV1::WorkgroupMemoryBytesAtMost {
                limit: contract.limits.max_workgroup_memory_bytes,
            },
        );
    }
    let workgroup_memory_bytes = if has_dynamic_workgroup_memory {
        None
    } else {
        static_workgroup_memory_bytes
    };
    match static_workgroup_memory_bytes {
        Some(actual) if actual > contract.limits.max_workgroup_memory_bytes => findings.push(
            PlironLaunchContractFindingV1::WorkgroupMemoryExceedsTarget {
                actual,
                limit: contract.limits.max_workgroup_memory_bytes,
            },
        ),
        None => findings.push(
            PlironLaunchContractFindingV1::WorkgroupMemoryArithmeticOverflow {
                view: "aggregate workgroup allocations".to_owned(),
            },
        ),
        _ => {}
    }
    if global_origins.len() > contract.limits.max_global_allocations {
        findings.push(PlironLaunchContractFindingV1::TooManyGlobalAllocations {
            actual: global_origins.len(),
            limit: contract.limits.max_global_allocations,
        });
    }
    let mut checked_global_allocations = 0;
    for (origin, (view, required, noalias_class)) in global_origins {
        let Some(allocation) = contract.host_allocation(origin) else {
            findings.push(PlironLaunchContractFindingV1::MissingHostAllocation { view, origin });
            continue;
        };
        let alignment_is_sufficient = match allocation.guaranteed_alignment() {
            Some(guaranteed) if guaranteed >= contract.limits.required_global_alignment => true,
            Some(guaranteed) => {
                findings.push(
                    PlironLaunchContractFindingV1::HostAllocationAlignmentInsufficient {
                        view: view.clone(),
                        origin,
                        required: contract.limits.required_global_alignment,
                        guaranteed,
                    },
                );
                false
            }
            None => {
                push_runtime_precondition(
                    &mut runtime_preconditions,
                    &mut findings,
                    PlironRuntimePreconditionV1::GlobalAllocationAlignmentAtLeast {
                        view: view.clone(),
                        origin,
                        required: contract.limits.required_global_alignment,
                    },
                );
                false
            }
        };
        let bytes_are_sufficient = match (required, allocation.byte_length()) {
            (Some(required), Some(available)) if available < required => {
                findings.push(PlironLaunchContractFindingV1::HostAllocationTooSmall {
                    view: view.clone(),
                    origin,
                    required,
                    available,
                });
                false
            }
            (Some(_), Some(_)) => true,
            (required, None) => {
                push_runtime_precondition(
                    &mut runtime_preconditions,
                    &mut findings,
                    PlironRuntimePreconditionV1::GlobalViewByteLengthFits {
                        view: view.clone(),
                        origin,
                        static_required: required,
                    },
                );
                false
            }
            (None, Some(_)) => {
                push_runtime_precondition(
                    &mut runtime_preconditions,
                    &mut findings,
                    PlironRuntimePreconditionV1::GlobalViewByteLengthFits {
                        view: view.clone(),
                        origin,
                        static_required: None,
                    },
                );
                false
            }
        };
        let alias_is_sufficient = if noalias_class == 0 {
            true
        } else {
            match allocation.noalias_class() {
                Some(observed) if observed == noalias_class => true,
                Some(observed) => {
                    findings.push(PlironLaunchContractFindingV1::GlobalAliasClassMismatch {
                        view: view.clone(),
                        origin,
                        required: noalias_class,
                        observed,
                    });
                    false
                }
                None => {
                    push_runtime_precondition(
                        &mut runtime_preconditions,
                        &mut findings,
                        PlironRuntimePreconditionV1::GlobalAllocationNoAliasClass {
                            view: view.clone(),
                            origin,
                            required_class: noalias_class,
                        },
                    );
                    false
                }
            }
        };
        if alignment_is_sufficient && bytes_are_sufficient && alias_is_sufficient {
            checked_global_allocations += 1;
        }
    }
    PlironLaunchContractReportV1 {
        findings,
        runtime_preconditions,
        workgroup_memory_bytes,
        checked_global_allocations,
    }
}

fn structural_prerequisite_failure() -> PlironLaunchContractReportV1 {
    PlironLaunchContractReportV1 {
        findings: vec![PlironLaunchContractFindingV1::StructuralPrerequisiteRejected],
        runtime_preconditions: Vec::new(),
        workgroup_memory_bytes: None,
        checked_global_allocations: 0,
    }
}

fn fold_ranked_view(
    context: &Context,
    view: &RankedViewOp,
    workgroup_by_origin: &mut BTreeMap<u64, Option<u64>>,
    global_origins: &mut BTreeMap<u64, (String, Option<u64>, u64)>,
    findings: &mut Vec<PlironLaunchContractFindingV1>,
) {
    let name = view.result(context).unique_name(context).to_string();
    let (Some(memory_space), Some(origin), Some(noalias_class), Some(view_type)) = (
        view.memory_space(context),
        view.allocation_origin(context),
        view.noalias_class(context),
        view.view_type(context),
    ) else {
        return;
    };
    let view_type = view_type.deref(context);
    let size = static_view_bytes(view_type.shape(), u64::from(view_type.element_width()));
    match memory_space {
        MemorySpaceAttr::Private => {}
        MemorySpaceAttr::Workgroup => {
            if origin == 0 {
                findings.push(
                    PlironLaunchContractFindingV1::WorkgroupMemoryProvenanceUnknown { view: name },
                );
                return;
            }
            match size {
                Ok(bytes) => {
                    workgroup_by_origin
                        .entry(origin)
                        .and_modify(|current| {
                            *current = current.map(|current| current.max(bytes));
                        })
                        .or_insert(Some(bytes));
                }
                Err(ViewSizeFailureV1::Dynamic(dimension)) => {
                    workgroup_by_origin.insert(origin, None);
                    let _ = dimension;
                }
                Err(ViewSizeFailureV1::Overflow) => findings.push(
                    PlironLaunchContractFindingV1::WorkgroupMemoryArithmeticOverflow { view: name },
                ),
            }
        }
        MemorySpaceAttr::Global => {
            if origin == 0 {
                findings.push(
                    PlironLaunchContractFindingV1::GlobalAllocationOriginUnknown { view: name },
                );
                return;
            }
            let required = match size {
                Ok(bytes) => Some(bytes),
                Err(ViewSizeFailureV1::Dynamic(dimension)) => {
                    let _ = dimension;
                    None
                }
                Err(ViewSizeFailureV1::Overflow) => {
                    findings.push(
                        PlironLaunchContractFindingV1::GlobalViewSizeArithmeticOverflow {
                            view: name.clone(),
                            origin,
                        },
                    );
                    None
                }
            };
            global_origins
                .entry(origin)
                .and_modify(|(_, current, class)| {
                    *current = current.zip(required).map(|(a, b)| a.max(b));
                    if *class == 0 {
                        *class = noalias_class;
                    } else if noalias_class != 0 && *class != noalias_class {
                        findings.push(
                            PlironLaunchContractFindingV1::ConflictingGlobalAliasClasses {
                                origin,
                                first: *class,
                                second: noalias_class,
                            },
                        );
                    }
                })
                .or_insert((name, required, noalias_class));
        }
    }
}

fn push_runtime_precondition(
    preconditions: &mut Vec<PlironRuntimePreconditionV1>,
    findings: &mut Vec<PlironLaunchContractFindingV1>,
    precondition: PlironRuntimePreconditionV1,
) {
    if preconditions.contains(&precondition) {
        return;
    }
    if preconditions.len() == MAX_PLIRON_RUNTIME_PRECONDITIONS_V1 {
        if !findings.iter().any(|finding| {
            matches!(
                finding,
                PlironLaunchContractFindingV1::RuntimePreconditionLimitExceeded { .. }
            )
        }) {
            findings.push(
                PlironLaunchContractFindingV1::RuntimePreconditionLimitExceeded {
                    limit: MAX_PLIRON_RUNTIME_PRECONDITIONS_V1,
                },
            );
        }
    } else {
        preconditions.push(precondition);
    }
}

pub fn require_pliron_launch_contract_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
    contract: &PlironLaunchContractV1,
) -> Result<PlironLaunchContractReportV1, PlironLaunchContractCheckErrorV1> {
    let report = run_pliron_launch_contract_check_v1(context, function, contract);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironLaunchContractCheckErrorV1 { report })
    }
}

pub(crate) fn require_pliron_launch_contract_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    contract: &PlironLaunchContractV1,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<PlironLaunchContractReportV1, PlironLaunchContractCheckErrorV1> {
    let report =
        run_pliron_launch_contract_check_with_analyses_v1(context, function, contract, analyses);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironLaunchContractCheckErrorV1 { report })
    }
}

pub(crate) fn require_pliron_launch_contract_with_canonical_layout_and_analyses_v1(
    context: &Context,
    function: &FuncOp,
    contract: &PlironLaunchContractV1,
    canonical_layout: PlironCanonicalExecutionLayoutV1,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<PlironLaunchContractReportV1, PlironLaunchContractCheckErrorV1> {
    let report = run_pliron_launch_contract_check_with_canonical_layout_and_analyses_v1(
        context,
        function,
        contract,
        Some(canonical_layout),
        analyses,
    );
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironLaunchContractCheckErrorV1 { report })
    }
}

enum ViewSizeFailureV1 {
    Dynamic(usize),
    Overflow,
}

fn static_view_bytes(shape: &[u64], element_width: u64) -> Result<u64, ViewSizeFailureV1> {
    let mut elements = 1_u64;
    for (dimension, extent) in shape.iter().copied().enumerate() {
        if extent == DYNAMIC_EXTENT {
            return Err(ViewSizeFailureV1::Dynamic(dimension));
        }
        elements = elements
            .checked_mul(extent)
            .ok_or(ViewSizeFailureV1::Overflow)?;
    }
    let bits = elements
        .checked_mul(element_width)
        .ok_or(ViewSizeFailureV1::Overflow)?;
    bits.checked_add(7)
        .map(|bits| bits / 8)
        .ok_or(ViewSizeFailureV1::Overflow)
}
