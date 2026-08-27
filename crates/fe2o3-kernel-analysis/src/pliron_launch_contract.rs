//! Target and host-contract feasibility for workload-neutral ranked PLIRON.
//!
//! Limits and allocation bindings are compiler inputs. They are not read from
//! user-authored IR and this analysis does not authenticate runtime addresses.

use std::{collections::BTreeMap, fmt};

use dialect_gpu::ExecutionLayoutOp;
use dialect_kernel::{DYNAMIC_EXTENT, MemorySpaceAttr, RankedViewOp};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::Context,
    linked_list::ContainsLinkedList,
    operation::Operation,
};

use crate::{KernelCheckStatusV1, derive_pliron_ir_structural_identity_v1};

pub const MAX_PLIRON_TARGET_SUBGROUP_SIZES_V1: usize = 16;
pub const MAX_PLIRON_HOST_ALLOCATIONS_V1: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironLaunchContractInputErrorV1 {
    InvalidTargetLimit(&'static str),
    InvalidHostAllocation(&'static str),
    DuplicateHostAllocation {
        origin: u64,
    },
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
            || max_workgroup_extents.contains(&0)
            || max_workgroup_invocations == 0
            || max_workgroup_memory_bytes == 0
            || required_global_alignment == 0
            || !required_global_alignment.is_power_of_two()
            || max_global_allocations == 0
            || max_global_allocations > MAX_PLIRON_HOST_ALLOCATIONS_V1
        {
            return Err(PlironLaunchContractInputErrorV1::InvalidTargetLimit(
                "extents, capacities, alignment, and allocation count must be bounded and nonzero",
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlironHostAllocationV1 {
    origin: u64,
    byte_length: u64,
    guaranteed_alignment: u64,
}

impl PlironHostAllocationV1 {
    pub fn new(
        origin: u64,
        byte_length: u64,
        guaranteed_alignment: u64,
    ) -> Result<Self, PlironLaunchContractInputErrorV1> {
        if origin == 0
            || byte_length == 0
            || guaranteed_alignment == 0
            || !guaranteed_alignment.is_power_of_two()
        {
            return Err(PlironLaunchContractInputErrorV1::InvalidHostAllocation(
                "origin and byte length must be nonzero and alignment must be a nonzero power of two",
            ));
        }
        Ok(Self {
            origin,
            byte_length,
            guaranteed_alignment,
        })
    }
    pub const fn origin(self) -> u64 {
        self.origin
    }
    pub const fn byte_length(self) -> u64 {
        self.byte_length
    }
    pub const fn guaranteed_alignment(self) -> u64 {
        self.guaranteed_alignment
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironLaunchContractV1 {
    limits: PlironLaunchTargetLimitsV1,
    host_allocations: BTreeMap<u64, PlironHostAllocationV1>,
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
        })
    }
    pub const fn limits(&self) -> &PlironLaunchTargetLimitsV1 {
        &self.limits
    }
    pub fn host_allocation(&self, origin: u64) -> Option<PlironHostAllocationV1> {
        self.host_allocations.get(&origin).copied()
    }
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
            | Self::GlobalViewSizeUnknown { .. } => KernelCheckStatusV1::Incomplete,
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
            | Self::TooManyGlobalAllocations { .. } => KernelCheckStatusV1::Rejected,
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
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironLaunchContractReportV1 {
    findings: Vec<PlironLaunchContractFindingV1>,
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
    // This shared preflight bounds the closed ranked subset before invoking
    // Pliron recursive verification, and contains traversal/verifier panics. A
    // successful identity therefore bounds the streaming scan below without a
    // second retained raw-operation inventory.
    if derive_pliron_ir_structural_identity_v1(context, function).is_err() {
        return structural_prerequisite_failure();
    }

    let mut layout_count = 0_usize;
    let mut first_layout = None;
    let mut view_findings = Vec::new();
    let mut workgroup_by_origin = BTreeMap::<u64, u64>::new();
    let mut global_origins = BTreeMap::<u64, (String, Option<u64>)>::new();
    for block in function.get_region(context).deref(context).iter(context) {
        for operation in block.deref(context).iter(context) {
            let operation = Operation::get_op_dyn(operation, context);
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
                findings.push(PlironLaunchContractFindingV1::DynamicGridExtent { axis });
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
    let workgroup_memory_bytes = workgroup_by_origin
        .values()
        .try_fold(0_u64, |total, bytes| total.checked_add(*bytes));
    match workgroup_memory_bytes {
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
    for (origin, (view, required)) in global_origins {
        let Some(allocation) = contract.host_allocation(origin) else {
            findings.push(PlironLaunchContractFindingV1::MissingHostAllocation { view, origin });
            continue;
        };
        let alignment_is_sufficient =
            allocation.guaranteed_alignment() >= contract.limits.required_global_alignment;
        if !alignment_is_sufficient {
            findings.push(
                PlironLaunchContractFindingV1::HostAllocationAlignmentInsufficient {
                    view: view.clone(),
                    origin,
                    required: contract.limits.required_global_alignment,
                    guaranteed: allocation.guaranteed_alignment(),
                },
            );
        }
        if let Some(required) = required {
            if allocation.byte_length() < required {
                findings.push(PlironLaunchContractFindingV1::HostAllocationTooSmall {
                    view,
                    origin,
                    required,
                    available: allocation.byte_length(),
                });
            } else if alignment_is_sufficient {
                checked_global_allocations += 1;
            }
        }
    }
    PlironLaunchContractReportV1 {
        findings,
        workgroup_memory_bytes,
        checked_global_allocations,
    }
}

fn structural_prerequisite_failure() -> PlironLaunchContractReportV1 {
    PlironLaunchContractReportV1 {
        findings: vec![PlironLaunchContractFindingV1::StructuralPrerequisiteRejected],
        workgroup_memory_bytes: None,
        checked_global_allocations: 0,
    }
}

fn fold_ranked_view(
    context: &Context,
    view: &RankedViewOp,
    workgroup_by_origin: &mut BTreeMap<u64, u64>,
    global_origins: &mut BTreeMap<u64, (String, Option<u64>)>,
    findings: &mut Vec<PlironLaunchContractFindingV1>,
) {
    let name = view.result(context).unique_name(context).to_string();
    let (Some(memory_space), Some(origin), Some(view_type)) = (
        view.memory_space(context),
        view.allocation_origin(context),
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
                        .and_modify(|current| *current = (*current).max(bytes))
                        .or_insert(bytes);
                }
                Err(ViewSizeFailureV1::Dynamic(dimension)) => {
                    findings.push(PlironLaunchContractFindingV1::WorkgroupMemorySizeUnknown {
                        view: name,
                        dimension,
                    })
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
                    findings.push(PlironLaunchContractFindingV1::GlobalViewSizeUnknown {
                        view: name.clone(),
                        origin,
                        dimension,
                    });
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
                .and_modify(|(_, current)| *current = current.zip(required).map(|(a, b)| a.max(b)))
                .or_insert((name, required));
        }
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
