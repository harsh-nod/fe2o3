use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;
use std::mem::size_of;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, BlockId, CastKind, ComparePredicate, Constant, Function,
    FunctionId, FunctionRole, Kernel, LaunchExtent, Module, Operation, OperationKind, ScalarType,
    Terminator, Type, UnaryOp, ValueId,
};

use crate::resident::{
    ResidentLedger, conservative_hash_map_bytes_for_entries, geometric_vec_bytes,
    partitioned_geometric_vec_bytes, reserved_bool_vec_bytes, reserved_vec_bytes,
    type_retained_heap_bytes,
};
use crate::{
    AdmittedSimulationModuleV1, IndexWidthV1, SimulationArgumentV1, SimulationLimitsErrorV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

/// Maximum unsupported-site occurrences retained in one preflight diagnostic.
///
/// Preflight continues scanning after this prefix is full so `total_findings`
/// remains exact, but it never clones an unbounded number of KIR identifiers.
pub const MAX_REPORTED_UNSUPPORTED_FINDINGS_V1: usize = 4_096;

/// Maximum source-identifier bytes retained by one unsupported report.
///
/// Findings that would cross this bound are counted but not materialized. This
/// prevents repeated long function identifiers from amplifying diagnostics.
pub const MAX_REPORTED_UNSUPPORTED_IDENTIFIER_BYTES_V1: usize = 1 << 20;

/// A feature deliberately outside the first deterministic execution profile.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UnsupportedFeatureV1 {
    FloatType(ScalarType),
    UnsupportedType,
    MemoryIntrinsic,
    FloatConstant,
    FloatOperation,
    InvalidIntegerCast {
        from: ScalarType,
        to: ScalarType,
        kind: CastKind,
    },
    ExternalCall(FunctionId),
    NonInternalCall {
        callee: FunctionId,
        role: FunctionRole,
    },
    WorkgroupAllocation,
    NonScalarMemory,
    UnsupportedAddressSpace(AddressSpace),
    Barrier,
    Atomic,
    Fence,
    WorkgroupBarrier,
    WorkgroupMemory,
    DynamicWorkgroupMemory,
    Matrix,
    Wave,
    Gfx950LdsTranspose,
    InlineAssembly,
    UnsupportedScalarOperation,
    TargetConstantOutOfRange,
}

/// One typed unsupported finding in the selected kernel's reachable call graph.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnsupportedSimulationSiteV1 {
    /// Function containing the unsupported construct.
    pub function: FunctionId,
    /// Block containing it, when the finding is not a signature finding.
    pub block: Option<BlockId>,
    /// Operation ordinal, when the finding identifies an operation.
    pub operation: Option<u32>,
    /// Closed unsupported feature classification.
    pub feature: UnsupportedFeatureV1,
}

/// Bounded deterministic scan-order prefix of unsupported reachable sites.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedSimulationReportV1 {
    findings: Vec<UnsupportedSimulationSiteV1>,
    total_findings: u64,
}

impl UnsupportedSimulationReportV1 {
    /// Returns the retained scan-order occurrence prefix.
    pub fn findings(&self) -> &[UnsupportedSimulationSiteV1] {
        &self.findings
    }

    /// Returns the exact number of unsupported occurrences found by preflight.
    pub const fn total_findings(&self) -> u64 {
        self.total_findings
    }

    /// Whether occurrences beyond the retained prefix were omitted.
    pub fn is_truncated(&self) -> bool {
        self.total_findings > self.findings.len() as u64
    }
}

struct UnsupportedCollectorV1 {
    findings: Vec<UnsupportedSimulationSiteV1>,
    total_findings: u64,
    retained_identifier_bytes: usize,
    retention_stopped: bool,
    allocation_failed: bool,
}

impl UnsupportedCollectorV1 {
    fn new() -> Result<Self, SimulationPreflightErrorV1> {
        Ok(Self {
            findings: Vec::new(),
            total_findings: 0,
            retained_identifier_bytes: 0,
            retention_stopped: false,
            allocation_failed: false,
        })
    }

    fn push(
        &mut self,
        identifier_bytes: usize,
        finding: impl FnOnce() -> UnsupportedSimulationSiteV1,
    ) {
        // Canonical KIR is byte-bounded, so its number of syntactic findings is
        // strictly below u64::MAX. Keep the exact count after the prefix fills.
        self.total_findings += 1;
        let retained_identifier_bytes = self
            .retained_identifier_bytes
            .checked_add(identifier_bytes)
            .filter(|bytes| *bytes <= MAX_REPORTED_UNSUPPORTED_IDENTIFIER_BYTES_V1);
        if self.retention_stopped || self.allocation_failed {
            return;
        }
        let Some(retained_identifier_bytes) = retained_identifier_bytes else {
            self.retention_stopped = true;
            return;
        };
        if self.findings.len() == MAX_REPORTED_UNSUPPORTED_FINDINGS_V1 {
            self.retention_stopped = true;
            return;
        }
        if self.findings.len() == self.findings.capacity() && self.findings.try_reserve(1).is_err()
        {
            self.allocation_failed = true;
            return;
        }
        self.findings.push(finding());
        self.retained_identifier_bytes = retained_identifier_bytes;
    }

    fn finish(self) -> Result<UnsupportedSimulationReportV1, SimulationPreflightErrorV1> {
        if self.allocation_failed {
            return Err(SimulationPreflightErrorV1::AllocationFailure);
        }
        Ok(UnsupportedSimulationReportV1 {
            findings: self.findings,
            total_findings: self.total_findings,
        })
    }
}

/// Fully preflighted immutable execution shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationPlanV1 {
    pub(crate) kernel: KernelIdAndEntry,
    pub(crate) grid: [u64; 3],
    pub(crate) workgroup: [u32; 3],
    pub(crate) workgroup_count: [u64; 3],
    pub(crate) invocations: u64,
    pub(crate) workgroups: u64,
    pub(crate) scheduled_slots: u64,
    pub(crate) reachable_functions: usize,
    pub(crate) reachable_function_indices: Vec<usize>,
    pub(crate) reachable_operations: usize,
    pub(crate) execution_index_resident_bytes: usize,
    pub(crate) workgroup_allocation_sites: usize,
    pub(crate) resident_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KernelIdAndEntry {
    pub id: fe2o3_kernel_ir::KernelId,
    pub entry: FunctionId,
}

impl SimulationPlanV1 {
    /// Returns the selected kernel identity.
    pub fn kernel(&self) -> &fe2o3_kernel_ir::KernelId {
        &self.kernel.id
    }

    /// Returns exact global launch dimensions.
    pub const fn grid(&self) -> [u64; 3] {
        self.grid
    }

    /// Returns exact workgroup dimensions.
    pub const fn workgroup(&self) -> [u32; 3] {
        self.workgroup
    }

    /// Returns ceil-divided workgroup counts.
    pub const fn workgroup_count(&self) -> [u64; 3] {
        self.workgroup_count
    }

    /// Returns the number of logical invocations that will execute.
    pub const fn invocations(&self) -> u64 {
        self.invocations
    }

    /// Returns the number of workgroups visited by the scheduler.
    pub const fn workgroups(&self) -> u64 {
        self.workgroups
    }

    /// Returns workitems visited, including padded tail slots.
    pub const fn scheduled_slots(&self) -> u64 {
        self.scheduled_slots
    }

    /// Returns the number of reachable functions checked by preflight.
    pub const fn reachable_functions(&self) -> usize {
        self.reachable_functions
    }

    /// Returns the number of reachable operations checked by preflight.
    pub const fn reachable_operations(&self) -> usize {
        self.reachable_operations
    }

    /// Returns the larger conservative resident peak across preflight and execution.
    pub const fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }
}

/// Fail-closed launch and reachable-program preflight failure.
#[derive(Debug, Eq, PartialEq)]
pub enum SimulationPreflightErrorV1 {
    InvalidLimits(SimulationLimitsErrorV1),
    UnknownKernel(fe2o3_kernel_ir::KernelId),
    MissingEntry(FunctionId),
    InvalidLaunch(&'static str),
    StaticLaunchMismatch {
        axis: usize,
        expected: u64,
        actual: u64,
    },
    WorkgroupMismatch {
        expected: [u32; 3],
        actual: [u32; 3],
    },
    ResourceLimit {
        resource: &'static str,
        actual: u64,
        limit: u64,
    },
    Unsupported(UnsupportedSimulationReportV1),
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    ArgumentType {
        argument: usize,
        expected: Type,
    },
    BufferAccess {
        argument: usize,
        required: AccessMode,
        supplied: AccessMode,
    },
    TargetLayout {
        argument: usize,
    },
    TargetValueOutOfRange {
        argument: usize,
    },
    SharedTargetLayout(u32),
    DuplicateBacking(u32),
    MissingBacking {
        argument: usize,
        backing: u32,
    },
    BufferViewBounds {
        argument: usize,
    },
    AllocationFailure,
}

impl fmt::Display for SimulationPreflightErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits(error) => error.fmt(formatter),
            Self::UnknownKernel(kernel) => write!(formatter, "unknown simulation kernel {kernel}"),
            Self::MissingEntry(entry) => write!(formatter, "kernel entry {entry} is missing"),
            Self::InvalidLaunch(detail) => write!(formatter, "invalid simulation launch: {detail}"),
            Self::StaticLaunchMismatch {
                axis,
                expected,
                actual,
            } => write!(
                formatter,
                "launch axis {axis} has extent {actual}, expected static extent {expected}",
            ),
            Self::WorkgroupMismatch { expected, actual } => write!(
                formatter,
                "workgroup shape {actual:?} does not match KIR shape {expected:?}",
            ),
            Self::ResourceLimit {
                resource,
                actual,
                limit,
            } => {
                write!(
                    formatter,
                    "simulation {resource} {actual} exceeds limit {limit}"
                )
            }
            Self::Unsupported(report) => write!(
                formatter,
                "selected kernel has {} unsupported reachable site occurrence(s); {} retained",
                report.total_findings(),
                report.findings().len(),
            ),
            Self::ArgumentCount { expected, actual } => {
                write!(formatter, "expected {expected} arguments, found {actual}")
            }
            Self::ArgumentType { argument, expected } => {
                write!(formatter, "argument {argument} does not match {expected:?}")
            }
            Self::BufferAccess {
                argument,
                required,
                supplied,
            } => write!(
                formatter,
                "argument {argument} requires {required:?} access, supplied {supplied:?}",
            ),
            Self::TargetLayout { argument } => {
                write!(
                    formatter,
                    "argument {argument} was built for a different index layout"
                )
            }
            Self::TargetValueOutOfRange { argument } => write!(
                formatter,
                "argument {argument} has a value outside the target index range",
            ),
            Self::SharedTargetLayout(backing) => write!(
                formatter,
                "shared buffer backing {backing} was built for a different index layout",
            ),
            Self::DuplicateBacking(backing) => {
                write!(formatter, "shared buffer backing {backing} is duplicated")
            }
            Self::MissingBacking { argument, backing } => write!(
                formatter,
                "argument {argument} refers to missing shared backing {backing}",
            ),
            Self::BufferViewBounds { argument } => {
                write!(
                    formatter,
                    "argument {argument} has an invalid shared buffer view"
                )
            }
            Self::AllocationFailure => write!(formatter, "simulation preflight allocation failed"),
        }
    }
}

impl Error for SimulationPreflightErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidLimits(error) => Some(error),
            _ => None,
        }
    }
}

impl AdmittedSimulationModuleV1 {
    /// Validates the complete selected launch and every reachable function before execution.
    pub fn preflight(
        &self,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
    ) -> Result<SimulationPlanV1, SimulationPreflightErrorV1> {
        preflight(
            &self.module,
            self.admitted_resident_bytes,
            request,
            target,
            limits,
        )
    }
}

pub(crate) fn preflight(
    module: &Module,
    admitted_resident_bytes: usize,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
) -> Result<SimulationPlanV1, SimulationPreflightErrorV1> {
    let limits = limits
        .validate()
        .map_err(SimulationPreflightErrorV1::InvalidLimits)?;
    let input_peak = conservative_preflight_input_bytes(admitted_resident_bytes, module, request)
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
        resource: "resident bytes",
        actual: u64::MAX,
        limit: limits.max_resident_bytes as u64,
    })?;
    check_limit(
        "resident bytes",
        input_peak as u64,
        limits.max_resident_bytes as u64,
    )?;
    let kernel = module
        .kernels
        .iter()
        .find(|kernel| kernel.id == request.kernel)
        .ok_or_else(|| SimulationPreflightErrorV1::UnknownKernel(request.kernel.clone()))?;
    let entry = module
        .function(&kernel.entry)
        .ok_or_else(|| SimulationPreflightErrorV1::MissingEntry(kernel.entry.clone()))?;

    let preflight_scratch_bytes = conservative_preflight_scratch_bytes(module, request, limits)
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual: u64::MAX,
            limit: limits.max_resident_bytes as u64,
        })?;
    let preflight_peak = input_peak.checked_add(preflight_scratch_bytes).ok_or(
        SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual: u64::MAX,
            limit: limits.max_resident_bytes as u64,
        },
    )?;
    check_limit(
        "resident bytes",
        preflight_peak as u64,
        limits.max_resident_bytes as u64,
    )?;

    let (grid, workgroup, workgroup_count, invocations, workgroups, scheduled_slots) =
        validate_launch(kernel, request, target, limits)?;
    let (unsupported, reachable_function_indices, reachable_operations, reachable_ssa_values) =
        scan_reachable(module, entry, target, limits)?;
    if unsupported.total_findings() != 0 {
        return Err(SimulationPreflightErrorV1::Unsupported(unsupported));
    }
    validate_acyclic_call_depth(module, entry, limits)?;
    validate_arguments(entry, request, target, limits)?;
    let workgroup_resources = validate_workgroup_resources(
        module,
        &reachable_function_indices,
        request,
        target,
        workgroup,
        workgroups,
        limits,
    )?;
    let execution_index_resident_bytes =
        crate::execute::preflight_execution_indices_resident_bytes(
            module,
            &reachable_function_indices,
            target,
        )?
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual: u64::MAX,
            limit: limits.max_resident_bytes as u64,
        })?;
    let kernel_identity_bytes = kernel
        .id
        .retained_capacity_bytes()
        .checked_add(kernel.entry.retained_capacity_bytes())
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual: u64::MAX,
            limit: limits.max_resident_bytes as u64,
        })?;
    let maximum_reachable_identifier_bytes = reachable_function_indices
        .iter()
        .filter_map(|index| module.functions.get(*index))
        .map(|function| function.id.retained_capacity_bytes())
        .max()
        .unwrap_or(0);
    let execution_peak = crate::execute::conservative_execution_resident_bytes(
        admitted_resident_bytes,
        request,
        limits,
        reachable_ssa_values,
        kernel_identity_bytes,
        reachable_function_indices.capacity(),
        execution_index_resident_bytes,
        maximum_reachable_identifier_bytes,
        workgroup_resources.participants,
        workgroup_resources.allocation_sites,
        workgroup_resources.static_bytes,
    )
    .ok_or(SimulationPreflightErrorV1::ResourceLimit {
        resource: "resident bytes",
        actual: u64::MAX,
        limit: limits.max_resident_bytes as u64,
    })?;
    let resident_bytes = preflight_peak.max(execution_peak);
    check_limit(
        "resident bytes",
        resident_bytes as u64,
        limits.max_resident_bytes as u64,
    )?;

    Ok(SimulationPlanV1 {
        kernel: KernelIdAndEntry {
            id: kernel.id.clone(),
            entry: kernel.entry.clone(),
        },
        grid,
        workgroup,
        workgroup_count,
        invocations,
        workgroups,
        scheduled_slots,
        reachable_functions: reachable_function_indices.len(),
        reachable_function_indices,
        reachable_operations,
        execution_index_resident_bytes,
        workgroup_allocation_sites: workgroup_resources.allocation_sites,
        resident_bytes,
    })
}

#[derive(Clone, Copy)]
struct WorkgroupResourcePlan {
    participants: usize,
    allocation_sites: usize,
    static_bytes: usize,
}

fn validate_workgroup_resources(
    module: &Module,
    reachable: &[usize],
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    workgroup: [u32; 3],
    workgroups: u64,
    limits: SimulationLimitsV1,
) -> Result<WorkgroupResourcePlan, SimulationPreflightErrorV1> {
    let participants = workgroup
        .into_iter()
        .try_fold(1_u64, |product, dimension| {
            product.checked_mul(u64::from(dimension))
        })
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "workgroup participants",
            actual: u64::MAX,
            limit: limits.max_scheduled_slots,
        })?;
    let mut allocation_sites = 0usize;
    let mut static_bytes = 0usize;
    for function_index in reachable {
        let Some(body) = module
            .functions
            .get(*function_index)
            .and_then(|function| function.body.as_ref())
        else {
            continue;
        };
        for memory in body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::WorkgroupMemory(memory) => Some(memory),
                _ => None,
            })
        {
            let (Type::Scalar(element), fe2o3_kernel_ir::WorkgroupMemoryExtent::Static(elements)) =
                (&memory.element, memory.extent)
            else {
                continue;
            };
            let bytes = usize::try_from(elements)
                .ok()
                .and_then(|elements| {
                    target
                        .scalar_bytes(*element)
                        .and_then(|width| elements.checked_mul(width))
                })
                .ok_or(SimulationPreflightErrorV1::ResourceLimit {
                    resource: "static workgroup allocation bytes",
                    actual: u64::MAX,
                    limit: limits.max_allocation_bytes as u64,
                })?;
            check_limit(
                "static workgroup allocation bytes",
                bytes as u64,
                limits.max_allocation_bytes as u64,
            )?;
            allocation_sites = allocation_sites.checked_add(1).ok_or(
                SimulationPreflightErrorV1::ResourceLimit {
                    resource: "workgroup allocation sites",
                    actual: u64::MAX,
                    limit: limits.max_allocations as u64,
                },
            )?;
            static_bytes = static_bytes.checked_add(bytes).ok_or(
                SimulationPreflightErrorV1::ResourceLimit {
                    resource: "static workgroup bytes",
                    actual: u64::MAX,
                    limit: limits.max_total_bytes as u64,
                },
            )?;
        }
    }
    let argument_bytes = request
        .arguments
        .iter()
        .filter_map(|argument| match argument {
            SimulationArgumentV1::Buffer(buffer) => Some(buffer.bytes().len()),
            _ => None,
        })
        .chain(
            request
                .shared_buffers
                .iter()
                .map(|shared| shared.buffer.bytes().len()),
        )
        .try_fold(0usize, |total, bytes| total.checked_add(bytes))
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "live bytes with static workgroup memory",
            actual: u64::MAX,
            limit: limits.max_total_bytes as u64,
        })?;
    let live_bytes = argument_bytes.checked_add(static_bytes).ok_or(
        SimulationPreflightErrorV1::ResourceLimit {
            resource: "live bytes with static workgroup memory",
            actual: u64::MAX,
            limit: limits.max_total_bytes as u64,
        },
    )?;
    check_limit(
        "live bytes with static workgroup memory",
        live_bytes as u64,
        limits.max_total_bytes as u64,
    )?;
    let workgroup_allocations = u64::try_from(allocation_sites)
        .ok()
        .and_then(|sites| sites.checked_mul(workgroups))
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "workgroup allocations",
            actual: u64::MAX,
            limit: limits.max_allocations as u64,
        })?;
    let argument_allocations = request
        .arguments
        .iter()
        .filter(|argument| matches!(argument, SimulationArgumentV1::Buffer(_)))
        .count()
        .checked_add(request.shared_buffers.len())
        .and_then(|count| u64::try_from(count).ok())
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "allocations including workgroup memory",
            actual: u64::MAX,
            limit: limits.max_allocations as u64,
        })?;
    let total_allocations = argument_allocations
        .checked_add(workgroup_allocations)
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "allocations including workgroup memory",
            actual: u64::MAX,
            limit: limits.max_allocations as u64,
        })?;
    check_limit(
        "allocations including workgroup memory",
        total_allocations,
        limits.max_allocations as u64,
    )?;
    Ok(WorkgroupResourcePlan {
        participants,
        allocation_sites,
        static_bytes,
    })
}

fn conservative_preflight_input_bytes(
    admitted_resident_bytes: usize,
    module: &Module,
    request: &SimulationRequestV1,
) -> Option<usize> {
    let mut resident = ResidentLedger::new(admitted_resident_bytes);
    resident.add_bytes(size_of::<SimulationRequestV1>())?;
    resident.add_bytes(request.kernel.retained_capacity_bytes())?;
    resident.add_vec::<SimulationArgumentV1>(request.arguments.capacity())?;
    resident.add_vec::<crate::SharedBufferV1>(request.shared_buffers.capacity())?;
    for argument in &request.arguments {
        if let SimulationArgumentV1::Buffer(buffer) = argument {
            resident.add_bytes(buffer.retained_payload_capacity_bytes()?)?;
        }
    }
    for shared in &request.shared_buffers {
        resident.add_bytes(shared.buffer.retained_payload_capacity_bytes()?)?;
    }
    // Unknown-kernel and missing-entry diagnostics own one identifier. Charge
    // the largest possible visible clone before performing either lookup.
    let diagnostic_identifier_bytes = module
        .functions
        .iter()
        .map(|function| function.id.as_str().len())
        .chain(
            module
                .kernels
                .iter()
                .flat_map(|kernel| [kernel.id.as_str().len(), kernel.entry.as_str().len()]),
        )
        .chain(std::iter::once(request.kernel.as_str().len()))
        .max()
        .unwrap_or(0);
    resident.add_bytes(diagnostic_identifier_bytes)?;
    Some(resident.bytes())
}

fn conservative_preflight_scratch_bytes(
    module: &Module,
    request: &SimulationRequestV1,
    limits: SimulationLimitsV1,
) -> Option<usize> {
    let functions = module.functions.len();
    let reachable = functions.min(limits.max_reachable_functions);
    let mut calls = 0usize;
    let mut maximum_ssa_values = 0usize;
    let mut maximum_diagnostic_type_bytes = 0usize;
    for function in &module.functions {
        for ty in &function.signature.parameters {
            maximum_diagnostic_type_bytes =
                maximum_diagnostic_type_bytes.max(type_retained_heap_bytes(ty)?);
        }
        let Some(body) = &function.body else {
            continue;
        };
        let mut definitions = body.parameters.len();
        for block in &body.blocks {
            definitions = definitions.checked_add(block.parameters.len())?;
            for operation in &block.operations {
                definitions = definitions.checked_add(operation.results.len())?;
                if matches!(operation.kind, OperationKind::Call { .. }) {
                    calls = calls.checked_add(1)?;
                }
            }
        }
        maximum_ssa_values = maximum_ssa_values.max(definitions);
    }

    let mut resident = ResidentLedger::new(0);
    // Reachability scan: full-module function index, worklists, discovery bitset,
    // bounded unsupported prefix, and the largest borrowed per-function SSA index.
    resident
        .add_bytes(conservative_hash_map_bytes_for_entries::<&FunctionId, usize>(functions)?)?;
    resident.add_product(reserved_vec_bytes::<usize>(reachable)?, 2)?;
    resident.add_bytes(reserved_bool_vec_bytes(functions)?)?;
    resident.add_bytes(geometric_vec_bytes::<UnsupportedSimulationSiteV1>(
        MAX_REPORTED_UNSUPPORTED_FINDINGS_V1,
    )?)?;
    resident.add_bytes(MAX_REPORTED_UNSUPPORTED_IDENTIFIER_BYTES_V1)?;
    resident.add_bytes(maximum_diagnostic_type_bytes)?;
    resident.add_bytes(conservative_hash_map_bytes_for_entries::<ValueId, &Type>(
        maximum_ssa_values,
    )?)?;

    // Acyclic-depth validation: both graph directions, iterative DFS/SCC work,
    // condensation, and full-module-to-compact ordinal maps. The call-edge sum
    // deliberately includes unreachable functions so this pre-allocation bound
    // requires no attacker-sized reachability scratch of its own.
    resident
        .add_bytes(conservative_hash_map_bytes_for_entries::<&FunctionId, usize>(functions)?)?;
    resident.add_product(reserved_vec_bytes::<usize>(reachable)?, 11)?;
    resident.add_bytes(reserved_bool_vec_bytes(functions)?)?;
    resident.add_bytes(reserved_vec_bytes::<usize>(functions)?)?;
    resident.add_product(reserved_vec_bytes::<Vec<usize>>(reachable)?, 3)?;
    resident.add_product(
        partitioned_geometric_vec_bytes::<usize>(calls, functions.max(1))?,
        3,
    )?;
    resident.add_bytes(reserved_bool_vec_bytes(reachable)?)?;
    resident.add_bytes(reserved_vec_bytes::<CallGraphDfsFrame>(reachable)?)?;

    // Shared-backing validation temporarily retains a B-tree of borrowed inputs.
    resident.add_btree_set::<(crate::BufferBackingIdV1, &crate::BufferArgumentV1)>(
        request.shared_buffers.len(),
    )?;
    Some(resident.bytes())
}

type LaunchFacts = ([u64; 3], [u32; 3], [u64; 3], u64, u64, u64);

fn validate_launch(
    kernel: &Kernel,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
) -> Result<LaunchFacts, SimulationPreflightErrorV1> {
    let grid = request.grid.0;
    let workgroup = request.workgroup.0;
    if grid.contains(&0) {
        return Err(SimulationPreflightErrorV1::InvalidLaunch(
            "global dimensions must be nonzero",
        ));
    }
    if workgroup.contains(&0) {
        return Err(SimulationPreflightErrorV1::InvalidLaunch(
            "workgroup dimensions must be nonzero",
        ));
    }
    let rank = usize::from(kernel.domain.rank());
    for axis in rank..3 {
        if grid[axis] != 1 || workgroup[axis] != 1 {
            return Err(SimulationPreflightErrorV1::InvalidLaunch(
                "inactive launch dimensions must be one",
            ));
        }
    }
    for (axis, extent) in kernel.domain.extents().enumerate() {
        if let LaunchExtent::Static(expected) = extent {
            let expected = u64::from(expected);
            if grid[axis] != expected {
                return Err(SimulationPreflightErrorV1::StaticLaunchMismatch {
                    axis,
                    expected,
                    actual: grid[axis],
                });
            }
        }
    }
    let max_index = match target.index_width() {
        IndexWidthV1::Bits32 => u64::from(u32::MAX),
        IndexWidthV1::Bits64 => u64::MAX,
    };
    if grid.into_iter().any(|extent| extent - 1 > max_index) {
        return Err(SimulationPreflightErrorV1::InvalidLaunch(
            "launch coordinate exceeds the target index width",
        ));
    }
    if let Some(expected) = kernel.workgroup_size {
        let expected = [expected.x, expected.y, expected.z];
        if workgroup != expected {
            return Err(SimulationPreflightErrorV1::WorkgroupMismatch {
                expected,
                actual: workgroup,
            });
        }
    }
    let invocations = checked_product(grid, "invocation count")?;
    check_limit("invocations", invocations, limits.max_invocations)?;
    let workgroup_count = [
        ceil_div(grid[0], u64::from(workgroup[0])),
        ceil_div(grid[1], u64::from(workgroup[1])),
        ceil_div(grid[2], u64::from(workgroup[2])),
    ];
    let workgroups = checked_product(workgroup_count, "workgroup count")?;
    check_limit("workgroups", workgroups, limits.max_workgroups)?;
    let local_volume = checked_product(workgroup.map(u64::from), "workgroup invocation count")?;
    check_limit(
        "workgroup invocations",
        local_volume,
        target.max_workgroup_invocations(),
    )?;
    let scheduled_slots =
        workgroups
            .checked_mul(local_volume)
            .ok_or(SimulationPreflightErrorV1::ResourceLimit {
                resource: "scheduled slots",
                actual: u64::MAX,
                limit: limits.max_scheduled_slots,
            })?;
    check_limit(
        "scheduled slots",
        scheduled_slots,
        limits.max_scheduled_slots,
    )?;
    Ok((
        grid,
        workgroup,
        workgroup_count,
        invocations,
        workgroups,
        scheduled_slots,
    ))
}

fn checked_product(
    dimensions: [u64; 3],
    resource: &'static str,
) -> Result<u64, SimulationPreflightErrorV1> {
    dimensions
        .into_iter()
        .try_fold(1_u64, u64::checked_mul)
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource,
            actual: u64::MAX,
            limit: u64::MAX - 1,
        })
}

fn ceil_div(numerator: u64, denominator: u64) -> u64 {
    numerator.div_ceil(denominator)
}

fn check_limit(
    resource: &'static str,
    actual: u64,
    limit: u64,
) -> Result<(), SimulationPreflightErrorV1> {
    if actual <= limit {
        Ok(())
    } else {
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource,
            actual,
            limit,
        })
    }
}

fn scan_reachable(
    module: &Module,
    entry: &Function,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
) -> Result<(UnsupportedSimulationReportV1, Vec<usize>, usize, usize), SimulationPreflightErrorV1> {
    let mut functions = HashMap::new();
    functions
        .try_reserve(module.functions.len())
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    for (index, function) in module.functions.iter().enumerate() {
        functions.insert(&function.id, index);
    }
    let entry_index = functions
        .get(&entry.id)
        .copied()
        .ok_or_else(|| SimulationPreflightErrorV1::MissingEntry(entry.id.clone()))?;
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(module.functions.len().min(limits.max_reachable_functions))
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    pending.push(entry_index);
    let mut discovered = try_preflight_filled(module.functions.len(), false)?;
    discovered[entry_index] = true;
    let mut discovered_count = 1usize;
    let mut findings = UnsupportedCollectorV1::new()?;
    let mut operations = 0usize;
    let mut maximum_ssa_values = 0usize;
    let mut reachable = Vec::new();
    reachable
        .try_reserve_exact(module.functions.len().min(limits.max_reachable_functions))
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;

    while let Some(function_index) = pending.pop() {
        reachable.push(function_index);
        let function = &module.functions[function_index];
        scan_signature(function, target, &mut findings);
        let Some(body) = &function.body else {
            let identifier_bytes = function.id.retained_capacity_bytes().saturating_mul(2);
            findings.push(identifier_bytes, || {
                signature_finding(
                    function,
                    UnsupportedFeatureV1::ExternalCall(function.id.clone()),
                )
            });
            continue;
        };
        let value_types = value_types(function, limits)?;
        maximum_ssa_values = maximum_ssa_values.max(value_types.len());
        for block in &body.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                operations =
                    operations
                        .checked_add(1)
                        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
                            resource: "reachable operations",
                            actual: u64::MAX,
                            limit: limits.max_reachable_operations as u64,
                        })?;
                if operations > limits.max_reachable_operations {
                    return Err(SimulationPreflightErrorV1::ResourceLimit {
                        resource: "reachable operations",
                        actual: operations as u64,
                        limit: limits.max_reachable_operations as u64,
                    });
                }
                scan_operation(
                    function,
                    block.id,
                    ordinal,
                    operation,
                    &value_types,
                    module,
                    &functions,
                    &mut pending,
                    &mut discovered,
                    &mut discovered_count,
                    limits.max_reachable_functions,
                    &mut findings,
                    target,
                )?;
            }
            scan_terminator(
                function,
                block.id,
                block.terminator.as_ref(),
                &value_types,
                &mut findings,
                target,
            );
        }
    }
    Ok((
        findings.finish()?,
        reachable,
        operations,
        maximum_ssa_values,
    ))
}

fn validate_acyclic_call_depth(
    module: &Module,
    entry: &Function,
    limits: SimulationLimitsV1,
) -> Result<(), SimulationPreflightErrorV1> {
    let mut functions = HashMap::new();
    functions
        .try_reserve(module.functions.len())
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    for (index, function) in module.functions.iter().enumerate() {
        functions.insert(&function.id, index);
    }
    let entry_index = functions
        .get(&entry.id)
        .copied()
        .ok_or_else(|| SimulationPreflightErrorV1::MissingEntry(entry.id.clone()))?;
    let capacity = functions.len().min(limits.max_reachable_functions);
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(capacity)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    let mut reachable = Vec::new();
    reachable
        .try_reserve_exact(capacity)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    let mut seen = try_preflight_filled(module.functions.len(), false)?;
    seen[entry_index] = true;
    pending.push(entry_index);
    while let Some(index) = pending.pop() {
        reachable.push(index);
        for callee in function_callees(&module.functions[index]) {
            let callee = functions
                .get(callee)
                .copied()
                .ok_or_else(|| SimulationPreflightErrorV1::MissingEntry(callee.clone()))?;
            if !seen[callee] {
                seen[callee] = true;
                pending.push(callee);
            }
        }
    }

    let mut compact = try_preflight_filled(module.functions.len(), usize::MAX)?;
    for (compact_index, module_index) in reachable.iter().copied().enumerate() {
        compact[module_index] = compact_index;
    }
    let mut graph = Vec::new();
    graph
        .try_reserve_exact(reachable.len())
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    for module_index in reachable.iter().copied() {
        let function = &module.functions[module_index];
        let calls = function_callees(function).count();
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(calls)
            .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
        for callee in function_callees(function) {
            let module_callee = functions
                .get(callee)
                .copied()
                .ok_or_else(|| SimulationPreflightErrorV1::MissingEntry(callee.clone()))?;
            let compact_callee = compact[module_callee];
            if compact_callee == usize::MAX {
                return Err(SimulationPreflightErrorV1::MissingEntry(callee.clone()));
            }
            edges.push(compact_callee);
        }
        graph.push(edges);
    }

    let count = graph.len();
    let mut reverse_counts = try_preflight_filled(count, 0usize)?;
    for edges in &graph {
        for target in edges {
            reverse_counts[*target] = reverse_counts[*target].saturating_add(1);
        }
    }
    let mut reverse = Vec::new();
    reverse
        .try_reserve_exact(count)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    for incoming in reverse_counts {
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(incoming)
            .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
        reverse.push(edges);
    }
    for (source, edges) in graph.iter().enumerate() {
        for target in edges {
            reverse[*target].push(source);
        }
    }

    let mut visited = try_preflight_filled(count, false)?;
    let mut finish = Vec::new();
    finish
        .try_reserve_exact(count)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    let mut dfs = Vec::new();
    dfs.try_reserve_exact(count)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    for root in 0..count {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        dfs.push(CallGraphDfsFrame {
            node: root,
            next: 0,
        });
        while let Some(frame) = dfs.last_mut() {
            if let Some(target) = graph[frame.node].get(frame.next).copied() {
                frame.next += 1;
                if !visited[target] {
                    visited[target] = true;
                    dfs.push(CallGraphDfsFrame {
                        node: target,
                        next: 0,
                    });
                }
            } else {
                let completed = dfs
                    .pop()
                    .ok_or(SimulationPreflightErrorV1::AllocationFailure)?;
                finish.push(completed.node);
            }
        }
    }

    let mut component = try_preflight_filled(count, usize::MAX)?;
    let mut component_sizes = Vec::new();
    component_sizes
        .try_reserve_exact(count)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    let mut work = Vec::new();
    work.try_reserve_exact(count)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    for root in finish.into_iter().rev() {
        if component[root] != usize::MAX {
            continue;
        }
        let component_id = component_sizes.len();
        let mut size = 0usize;
        component[root] = component_id;
        work.push(root);
        while let Some(node) = work.pop() {
            size = size.saturating_add(1);
            for predecessor in reverse[node].iter().copied() {
                if component[predecessor] == usize::MAX {
                    component[predecessor] = component_id;
                    work.push(predecessor);
                }
            }
        }
        component_sizes.push(size);
    }

    let components = component_sizes.len();
    let mut condensed_counts = try_preflight_filled(components, 0usize)?;
    for (source, edges) in graph.iter().enumerate() {
        let source_component = component[source];
        for target in edges {
            if source_component != component[*target] {
                condensed_counts[source_component] = condensed_counts[source_component]
                    .checked_add(1)
                    .ok_or(SimulationPreflightErrorV1::ResourceLimit {
                        resource: "call graph edges",
                        actual: u64::MAX,
                        limit: limits.max_reachable_operations as u64,
                    })?;
            }
        }
    }
    let mut condensed = Vec::new();
    condensed
        .try_reserve_exact(components)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    for edges in condensed_counts {
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(edges)
            .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
        condensed.push(targets);
    }
    let mut indegree = try_preflight_filled(components, 0usize)?;
    for (source, edges) in graph.iter().enumerate() {
        let source_component = component[source];
        for target in edges {
            let target_component = component[*target];
            if source_component != target_component {
                condensed[source_component].push(target_component);
                indegree[target_component] = indegree[target_component].saturating_add(1);
            }
        }
    }

    let entry_component = component[compact[entry_index]];
    let mut depth = try_preflight_filled(components, 0usize)?;
    depth[entry_component] = component_sizes[entry_component];
    let mut queue = Vec::new();
    queue
        .try_reserve_exact(components)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    for (component, incoming) in indegree.iter().copied().enumerate() {
        if incoming == 0 {
            queue.push(component);
        }
    }
    let mut cursor = 0usize;
    while let Some(source) = queue.get(cursor).copied() {
        cursor += 1;
        for target in condensed[source].iter().copied() {
            if depth[source] != 0 {
                depth[target] =
                    depth[target].max(depth[source].saturating_add(component_sizes[target]));
            }
            indegree[target] = indegree[target].saturating_sub(1);
            if indegree[target] == 0 {
                queue.push(target);
            }
        }
    }
    let maximum = depth.into_iter().max().unwrap_or(1);
    check_limit(
        "acyclic call depth",
        maximum as u64,
        limits.max_call_depth as u64,
    )
}

struct CallGraphDfsFrame {
    node: usize,
    next: usize,
}

fn function_callees(function: &Function) -> impl Iterator<Item = &FunctionId> {
    function
        .body
        .iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call { callee, .. } => Some(callee),
            _ => None,
        })
}

fn try_preflight_filled<T: Clone>(
    length: usize,
    value: T,
) -> Result<Vec<T>, SimulationPreflightErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    values.resize(length, value);
    Ok(values)
}

fn scan_signature(
    function: &Function,
    target: SimulationTargetV1,
    findings: &mut UnsupportedCollectorV1,
) {
    let identifier_bytes = function.id.retained_capacity_bytes();
    for ty in function
        .signature
        .parameters
        .iter()
        .chain(&function.signature.results)
    {
        if let Some(feature) = unsupported_type(ty, target) {
            findings.push(identifier_bytes, || signature_finding(function, feature));
        }
    }
}

fn signature_finding(
    function: &Function,
    feature: UnsupportedFeatureV1,
) -> UnsupportedSimulationSiteV1 {
    UnsupportedSimulationSiteV1 {
        function: function.id.clone(),
        block: None,
        operation: None,
        feature,
    }
}

fn operation_finding(
    function: &Function,
    block: BlockId,
    operation: usize,
    feature: UnsupportedFeatureV1,
) -> UnsupportedSimulationSiteV1 {
    UnsupportedSimulationSiteV1 {
        function: function.id.clone(),
        block: Some(block),
        operation: Some(u32::try_from(operation).unwrap_or(u32::MAX)),
        feature,
    }
}

fn value_types(
    function: &Function,
    limits: SimulationLimitsV1,
) -> Result<HashMap<ValueId, &Type>, SimulationPreflightErrorV1> {
    let Some(body) = &function.body else {
        return Ok(HashMap::new());
    };
    let definitions = body
        .blocks
        .iter()
        .try_fold(body.parameters.len(), |count, block| {
            block
                .operations
                .iter()
                .try_fold(block.parameters.len(), |count, operation| {
                    count.checked_add(operation.results.len())
                })
                .and_then(|block_count| count.checked_add(block_count))
        })
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "SSA values in one frame",
            actual: u64::MAX,
            limit: limits.max_ssa_values as u64,
        })?;
    check_limit(
        "SSA values in one frame",
        definitions as u64,
        limits.max_ssa_values as u64,
    )?;
    let mut values = HashMap::new();
    values
        .try_reserve(definitions)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    values.extend(
        body.parameters
            .iter()
            .copied()
            .zip(function.signature.parameters.iter()),
    );
    for block in &body.blocks {
        values.extend(block.parameters.iter().map(|value| (value.id, &value.ty)));
        for operation in &block.operations {
            values.extend(operation.results.iter().map(|value| (value.id, &value.ty)));
        }
    }
    Ok(values)
}

#[allow(clippy::too_many_arguments)]
fn scan_operation(
    function: &Function,
    block: BlockId,
    ordinal: usize,
    operation: &Operation,
    value_types: &HashMap<ValueId, &Type>,
    module: &Module,
    functions: &HashMap<&FunctionId, usize>,
    pending: &mut Vec<usize>,
    discovered: &mut [bool],
    discovered_count: &mut usize,
    max_reachable_functions: usize,
    findings: &mut UnsupportedCollectorV1,
    target: SimulationTargetV1,
) -> Result<(), SimulationPreflightErrorV1> {
    let identifier_bytes = function.id.retained_capacity_bytes();
    macro_rules! reject {
        ($feature:expr) => {
            findings.push(identifier_bytes, || {
                operation_finding(function, block, ordinal, $feature)
            })
        };
        ($extra_identifier_bytes:expr => $feature:expr) => {
            findings.push(
                identifier_bytes.saturating_add($extra_identifier_bytes),
                || operation_finding(function, block, ordinal, $feature),
            )
        };
    }
    for result in &operation.results {
        if let Some(feature) = unsupported_type(&result.ty, target) {
            reject!(feature);
        }
    }
    match &operation.kind {
        OperationKind::Constant(constant) => {
            if matches!(
                constant,
                Constant::F16Bits(_)
                    | Constant::Bf16Bits(_)
                    | Constant::F32Bits(_)
                    | Constant::F64Bits(_)
            ) {
                reject!(UnsupportedFeatureV1::FloatConstant);
            }
            if matches!(constant, Constant::Index(value) if target.index_width() == IndexWidthV1::Bits32 && *value > u64::from(u32::MAX))
            {
                reject!(UnsupportedFeatureV1::TargetConstantOutOfRange);
            }
        }
        OperationKind::Intrinsic(_) => {}
        OperationKind::MemoryIntrinsic(_) => reject!(UnsupportedFeatureV1::MemoryIntrinsic),
        OperationKind::Unary { op, operand } => {
            if !matches!(value_types.get(operand), Some(Type::Scalar(ty)) if supports_unary(*op, *ty))
            {
                reject!(UnsupportedFeatureV1::UnsupportedScalarOperation);
            }
        }
        OperationKind::Cast { value: operand, .. } => {
            if let Some(Type::Scalar(scalar)) = value_types.get(operand)
                && scalar.is_float()
            {
                reject!(UnsupportedFeatureV1::FloatOperation);
            }
            if let OperationKind::Cast { kind, to, .. } = &operation.kind
                && let (Some(Type::Scalar(from)), Type::Scalar(to)) = (value_types.get(operand), to)
                && !supported_cast(*kind, *from, *to, target)
            {
                reject!(UnsupportedFeatureV1::InvalidIntegerCast {
                    from: *from,
                    to: *to,
                    kind: *kind,
                });
            }
        }
        OperationKind::Binary { op, lhs, rhs } => {
            if !matches!(
                (value_types.get(lhs), value_types.get(rhs)),
                (Some(Type::Scalar(lhs)), Some(Type::Scalar(rhs)))
                    if supports_binary(*op, *lhs, *rhs)
            ) {
                reject!(UnsupportedFeatureV1::UnsupportedScalarOperation);
            }
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            if !matches!(
                (value_types.get(lhs), value_types.get(rhs)),
                (Some(Type::Scalar(lhs)), Some(Type::Scalar(rhs)))
                    if supports_compare(*predicate, *lhs, *rhs)
            ) {
                reject!(UnsupportedFeatureV1::UnsupportedScalarOperation);
            }
        }
        OperationKind::Select { .. } => {}
        OperationKind::Call { callee, .. } => match functions.get(callee).copied() {
            Some(callee_index)
                if module.functions[callee_index].role == FunctionRole::InternalHelper =>
            {
                if !discovered[callee_index] {
                    let actual = discovered_count.saturating_add(1);
                    if actual > max_reachable_functions {
                        return Err(SimulationPreflightErrorV1::ResourceLimit {
                            resource: "reachable functions",
                            actual: actual as u64,
                            limit: max_reachable_functions as u64,
                        });
                    }
                    discovered[callee_index] = true;
                    *discovered_count = actual;
                    pending.push(callee_index);
                }
            }
            Some(callee_index)
                if module.functions[callee_index].role == FunctionRole::ExternalImport =>
            {
                reject!(callee.retained_capacity_bytes() => UnsupportedFeatureV1::ExternalCall(callee.clone()));
            }
            Some(callee_index) => {
                reject!(callee.retained_capacity_bytes() => UnsupportedFeatureV1::NonInternalCall {
                    callee: callee.clone(),
                    role: module.functions[callee_index].role,
                })
            }
            None => {
                reject!(callee.retained_capacity_bytes() => UnsupportedFeatureV1::ExternalCall(callee.clone()))
            }
        },
        OperationKind::Alloca {
            element,
            address_space,
            ..
        } => {
            if *address_space == AddressSpace::Workgroup {
                reject!(UnsupportedFeatureV1::WorkgroupAllocation);
            } else if *address_space != AddressSpace::Private {
                reject!(UnsupportedFeatureV1::UnsupportedAddressSpace(
                    *address_space,
                ));
            }
            if !matches!(element, Type::Scalar(scalar) if target.scalar_bits(*scalar).is_some()) {
                reject!(UnsupportedFeatureV1::NonScalarMemory);
            }
        }
        OperationKind::SliceLength { slice } | OperationKind::SliceData { slice } => {
            if let Some(Type::Slice(slice)) = value_types.get(slice) {
                scan_memory_type(
                    &slice.element,
                    slice.address_space,
                    &mut |feature| reject!(feature),
                    target,
                );
            }
        }
        OperationKind::GetElementPointer { base, .. }
        | OperationKind::Load { pointer: base, .. }
        | OperationKind::GuardedLoad { pointer: base, .. } => {
            if let Some(Type::Pointer(pointer)) = value_types.get(base) {
                scan_memory_type(
                    &pointer.pointee,
                    pointer.address_space,
                    &mut |feature| reject!(feature),
                    target,
                );
            }
        }
        OperationKind::Store { pointer, .. } => {
            if let Some(Type::Pointer(pointer)) = value_types.get(pointer) {
                scan_memory_type(
                    &pointer.pointee,
                    pointer.address_space,
                    &mut |feature| reject!(feature),
                    target,
                );
            }
        }
        OperationKind::Barrier(_) => reject!(UnsupportedFeatureV1::Barrier),
        OperationKind::Atomic(_) => reject!(UnsupportedFeatureV1::Atomic),
        OperationKind::Fence(_) => reject!(UnsupportedFeatureV1::Fence),
        OperationKind::WorkgroupBarrier(_) => {}
        OperationKind::WorkgroupMemory(memory) => {
            if memory.extent.is_dynamic() {
                reject!(UnsupportedFeatureV1::DynamicWorkgroupMemory);
            }
            if !matches!(&memory.element, Type::Scalar(scalar) if target.scalar_bits(*scalar).is_some())
            {
                reject!(UnsupportedFeatureV1::NonScalarMemory);
            }
        }
        OperationKind::Matrix(_) => reject!(UnsupportedFeatureV1::Matrix),
        OperationKind::Wave(_) => reject!(UnsupportedFeatureV1::Wave),
        OperationKind::Gfx950LdsTranspose(_) => {
            reject!(UnsupportedFeatureV1::Gfx950LdsTranspose)
        }
        OperationKind::InlineAssembly(_) => reject!(UnsupportedFeatureV1::InlineAssembly),
    }
    Ok(())
}

fn scan_memory_type(
    pointee: &Type,
    address_space: AddressSpace,
    reject: &mut impl FnMut(UnsupportedFeatureV1),
    target: SimulationTargetV1,
) {
    if !matches!(
        address_space,
        AddressSpace::Global
            | AddressSpace::Private
            | AddressSpace::Workgroup
            | AddressSpace::Constant
    ) {
        reject(UnsupportedFeatureV1::UnsupportedAddressSpace(address_space));
    }
    if !matches!(pointee, Type::Scalar(scalar) if target.scalar_bits(*scalar).is_some()) {
        reject(UnsupportedFeatureV1::NonScalarMemory);
    }
}

fn scan_terminator(
    function: &Function,
    block: BlockId,
    terminator: Option<&Terminator>,
    value_types: &HashMap<ValueId, &Type>,
    findings: &mut UnsupportedCollectorV1,
    target: SimulationTargetV1,
) {
    let Some(terminator) = terminator else {
        return;
    };
    let selector = match terminator {
        Terminator::IntegerSwitch {
            selector, cases, ..
        } => {
            if target.index_width() == IndexWidthV1::Bits32
                && cases.iter().any(
                    |case| matches!(&case.value, Constant::Index(value) if *value > u64::from(u32::MAX)),
                )
            {
                findings.push(function.id.retained_capacity_bytes(), || UnsupportedSimulationSiteV1 {
                    function: function.id.clone(),
                    block: Some(block),
                    operation: None,
                    feature: UnsupportedFeatureV1::TargetConstantOutOfRange,
                });
            }
            selector
        }
        Terminator::Switch { selector, .. } => selector,
        _ => return,
    };
    if let Some(Type::Scalar(scalar)) = value_types.get(selector)
        && scalar.is_float()
    {
        findings.push(function.id.retained_capacity_bytes(), || {
            UnsupportedSimulationSiteV1 {
                function: function.id.clone(),
                block: Some(block),
                operation: None,
                feature: UnsupportedFeatureV1::FloatOperation,
            }
        });
    }
}

fn unsupported_type(ty: &Type, target: SimulationTargetV1) -> Option<UnsupportedFeatureV1> {
    match ty {
        Type::Unit => Some(UnsupportedFeatureV1::UnsupportedType),
        Type::Scalar(scalar) if scalar.is_float() => Some(UnsupportedFeatureV1::FloatType(*scalar)),
        Type::Scalar(scalar) if target.scalar_bits(*scalar).is_some() => None,
        Type::Pointer(pointer) => {
            if !matches!(
                pointer.address_space,
                AddressSpace::Global
                    | AddressSpace::Private
                    | AddressSpace::Workgroup
                    | AddressSpace::Constant
            ) {
                Some(UnsupportedFeatureV1::UnsupportedAddressSpace(
                    pointer.address_space,
                ))
            } else if !matches!(pointer.pointee.as_ref(), Type::Scalar(scalar) if target.scalar_bits(*scalar).is_some())
            {
                Some(UnsupportedFeatureV1::NonScalarMemory)
            } else {
                None
            }
        }
        Type::Slice(slice) => {
            if !matches!(
                slice.address_space,
                AddressSpace::Global | AddressSpace::Constant
            ) {
                Some(UnsupportedFeatureV1::UnsupportedAddressSpace(
                    slice.address_space,
                ))
            } else if !matches!(slice.element.as_ref(), Type::Scalar(scalar) if target.scalar_bits(*scalar).is_some())
            {
                Some(UnsupportedFeatureV1::NonScalarMemory)
            } else {
                None
            }
        }
        _ => Some(UnsupportedFeatureV1::UnsupportedType),
    }
}

pub(crate) fn supported_cast(
    kind: CastKind,
    from: ScalarType,
    to: ScalarType,
    target: SimulationTargetV1,
) -> bool {
    let (Some(from_bits), Some(to_bits)) = (target.scalar_bits(from), target.scalar_bits(to))
    else {
        return false;
    };
    match kind {
        CastKind::Truncate => from.is_integer() && to.is_integer() && to_bits < from_bits,
        CastKind::ZeroExtend => from.is_integer() && to.is_integer() && to_bits > from_bits,
        CastKind::SignExtend => from.is_signed_integer() && to.is_integer() && to_bits > from_bits,
        CastKind::Bitcast => {
            from != ScalarType::Bool && to != ScalarType::Bool && from_bits == to_bits
        }
        CastKind::FloatExtend
        | CastKind::FloatTruncate
        | CastKind::IntegerToFloat
        | CastKind::FloatToInteger => false,
    }
}

pub(crate) fn supports_unary(op: UnaryOp, ty: ScalarType) -> bool {
    match op {
        UnaryOp::Not => ty == ScalarType::Bool || ty.is_integer(),
        UnaryOp::Negate => ty.is_signed_integer(),
    }
}

pub(crate) fn supports_binary(op: BinaryOp, lhs: ScalarType, rhs: ScalarType) -> bool {
    match op {
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor
            if lhs == ScalarType::Bool && rhs == ScalarType::Bool =>
        {
            true
        }
        BinaryOp::ShiftLeft | BinaryOp::ShiftRight => lhs.is_integer() && rhs.is_integer(),
        BinaryOp::Add
        | BinaryOp::Subtract
        | BinaryOp::Multiply
        | BinaryOp::Divide
        | BinaryOp::Remainder
        | BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::Checked(_) => lhs == rhs && lhs.is_integer(),
    }
}

pub(crate) fn supports_compare(
    predicate: ComparePredicate,
    lhs: ScalarType,
    rhs: ScalarType,
) -> bool {
    if lhs != rhs {
        return false;
    }
    if lhs == ScalarType::Bool {
        matches!(
            predicate,
            ComparePredicate::Equal | ComparePredicate::NotEqual
        )
    } else {
        lhs.is_integer()
    }
}

fn validate_arguments(
    entry: &Function,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
) -> Result<(), SimulationPreflightErrorV1> {
    let arguments = &request.arguments;
    if arguments.len() != entry.signature.parameters.len() {
        return Err(SimulationPreflightErrorV1::ArgumentCount {
            expected: entry.signature.parameters.len(),
            actual: arguments.len(),
        });
    }
    check_limit(
        "entry arguments",
        arguments.len() as u64,
        limits.max_ssa_values as u64,
    )?;
    let distinct_buffer_count = arguments
        .iter()
        .filter(|argument| matches!(argument, SimulationArgumentV1::Buffer(_)))
        .count();
    let buffer_count = distinct_buffer_count
        .checked_add(request.shared_buffers.len())
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "argument allocations",
            actual: u64::MAX,
            limit: limits.max_allocations as u64,
        })?;
    check_limit(
        "argument allocations",
        buffer_count as u64,
        limits.max_allocations as u64,
    )?;
    let mut backings = BTreeMap::new();
    let mut total_buffer_bytes = 0usize;
    for shared in &request.shared_buffers {
        if backings.insert(shared.id, &shared.buffer).is_some() {
            return Err(SimulationPreflightErrorV1::DuplicateBacking(shared.id.0));
        }
        if !shared.buffer.matches_target(target) {
            return Err(SimulationPreflightErrorV1::SharedTargetLayout(shared.id.0));
        }
        total_buffer_bytes =
            checked_argument_bytes(total_buffer_bytes, shared.buffer.bytes().len(), limits)?;
    }
    for (index, (argument, expected)) in arguments
        .iter()
        .zip(&entry.signature.parameters)
        .enumerate()
    {
        if let SimulationArgumentV1::Buffer(buffer) = argument {
            if !buffer.matches_target(target) {
                return Err(SimulationPreflightErrorV1::TargetLayout { argument: index });
            }
            total_buffer_bytes =
                checked_argument_bytes(total_buffer_bytes, buffer.bytes().len(), limits)?;
        }
        match (argument, expected) {
            (SimulationArgumentV1::Scalar(value), Type::Scalar(expected_scalar))
                if value.ty() == *expected_scalar
                    && target.scalar_bits(*expected_scalar).is_some() =>
            {
                if !value.matches_target(target) {
                    return Err(SimulationPreflightErrorV1::TargetLayout { argument: index });
                }
            }
            (SimulationArgumentV1::Buffer(buffer), Type::Slice(slice))
                if slice.address_space == AddressSpace::Global
                    && slice.element.as_ref() == &Type::Scalar(buffer.element()) =>
            {
                validate_buffer_access(index, slice.access, buffer.access())?;
                validate_slice_length(
                    index,
                    buffer.element_count(target).map_err(|_| {
                        SimulationPreflightErrorV1::TargetLayout { argument: index }
                    })?,
                    target,
                )?;
            }
            (SimulationArgumentV1::Buffer(buffer), Type::Pointer(pointer))
                if pointer.address_space == AddressSpace::Global
                    && pointer.pointee.as_ref() == &Type::Scalar(buffer.element()) =>
            {
                validate_buffer_access(index, pointer.access, buffer.access())?;
            }
            (SimulationArgumentV1::BufferView(view), Type::Slice(slice))
                if slice.address_space == AddressSpace::Global
                    && slice.element.as_ref() == &Type::Scalar(view.element()) =>
            {
                validate_buffer_view(index, view, slice.access, &backings, target)?;
                validate_slice_length(index, view.elements(), target)?;
            }
            (SimulationArgumentV1::BufferView(view), Type::Pointer(pointer))
                if pointer.address_space == AddressSpace::Global
                    && pointer.pointee.as_ref() == &Type::Scalar(view.element()) =>
            {
                validate_buffer_view(index, view, pointer.access, &backings, target)?;
            }
            _ => {
                return Err(SimulationPreflightErrorV1::ArgumentType {
                    argument: index,
                    expected: expected.clone(),
                });
            }
        }
    }
    check_limit(
        "argument total bytes",
        total_buffer_bytes as u64,
        limits.max_total_bytes as u64,
    )?;
    Ok(())
}

fn validate_slice_length(
    argument: usize,
    elements: usize,
    target: SimulationTargetV1,
) -> Result<(), SimulationPreflightErrorV1> {
    if target.index_width() == IndexWidthV1::Bits32
        && u64::try_from(elements).unwrap_or(u64::MAX) > u64::from(u32::MAX)
    {
        Err(SimulationPreflightErrorV1::TargetValueOutOfRange { argument })
    } else {
        Ok(())
    }
}

fn checked_argument_bytes(
    total: usize,
    bytes: usize,
    limits: SimulationLimitsV1,
) -> Result<usize, SimulationPreflightErrorV1> {
    check_limit(
        "argument allocation bytes",
        bytes as u64,
        limits.max_allocation_bytes as u64,
    )?;
    total
        .checked_add(bytes)
        .ok_or(SimulationPreflightErrorV1::ResourceLimit {
            resource: "argument total bytes",
            actual: u64::MAX,
            limit: limits.max_total_bytes as u64,
        })
}

fn validate_buffer_view(
    argument: usize,
    view: &crate::BufferViewArgumentV1,
    required: AccessMode,
    backings: &BTreeMap<crate::BufferBackingIdV1, &crate::BufferArgumentV1>,
    target: SimulationTargetV1,
) -> Result<(), SimulationPreflightErrorV1> {
    if !view.matches_target(target) {
        return Err(SimulationPreflightErrorV1::TargetLayout { argument });
    }
    let backing = backings.get(&view.backing()).copied().ok_or(
        SimulationPreflightErrorV1::MissingBacking {
            argument,
            backing: view.backing().0,
        },
    )?;
    validate_buffer_access(argument, required, view.access())?;
    validate_buffer_access(argument, view.access(), backing.access())?;
    let end = view
        .byte_offset()
        .checked_add(
            view.byte_len(target)
                .map_err(|_| SimulationPreflightErrorV1::BufferViewBounds { argument })?,
        )
        .ok_or(SimulationPreflightErrorV1::BufferViewBounds { argument })?;
    if backing.element() != view.element()
        || backing.alignment() < view.alignment()
        || end > backing.bytes().len()
    {
        return Err(SimulationPreflightErrorV1::BufferViewBounds { argument });
    }
    Ok(())
}

fn validate_buffer_access(
    argument: usize,
    required: AccessMode,
    supplied: AccessMode,
) -> Result<(), SimulationPreflightErrorV1> {
    if required == AccessMode::ReadWrite && supplied != AccessMode::ReadWrite {
        Err(SimulationPreflightErrorV1::BufferAccess {
            argument,
            required,
            supplied,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarded_load_accepts_a_scalar_workgroup_pointer() {
        let function = Function::kernel_entry(
            "guarded",
            fe2o3_kernel_ir::Signature::new(vec![], vec![]),
            vec![],
            vec![],
        );
        let operation = Operation::effect_free(
            fe2o3_kernel_ir::ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
            OperationKind::GuardedLoad {
                pointer: ValueId(0),
                predicate: ValueId(1),
                fallback: ValueId(2),
                access: fe2o3_kernel_ir::MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        );
        let pointer = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        );
        let value_types = HashMap::from([(ValueId(0), &pointer)]);
        let mut findings = UnsupportedCollectorV1::new().unwrap();
        let mut pending = Vec::new();
        let mut discovered = vec![true];
        let mut discovered_count = 1;

        scan_operation(
            &function,
            BlockId(0),
            0,
            &operation,
            &value_types,
            &Module::new("guarded"),
            &HashMap::new(),
            &mut pending,
            &mut discovered,
            &mut discovered_count,
            1,
            &mut findings,
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap();

        let report = findings.finish().unwrap();
        assert_eq!(report.total_findings(), 0);
        assert!(report.findings().is_empty());
    }

    #[test]
    fn guarded_load_rejects_a_non_scalar_workgroup_pointer() {
        let function = Function::kernel_entry(
            "guarded",
            fe2o3_kernel_ir::Signature::new(vec![], vec![]),
            vec![],
            vec![],
        );
        let operation = Operation::effect_free(
            fe2o3_kernel_ir::ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
            OperationKind::GuardedLoad {
                pointer: ValueId(0),
                predicate: ValueId(1),
                fallback: ValueId(2),
                access: fe2o3_kernel_ir::MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        );
        let nested = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        );
        let pointer = Type::pointer(nested, AddressSpace::Workgroup, AccessMode::ReadWrite);
        let value_types = HashMap::from([(ValueId(0), &pointer)]);
        let mut findings = UnsupportedCollectorV1::new().unwrap();
        let mut pending = Vec::new();
        let mut discovered = vec![true];
        let mut discovered_count = 1;

        scan_operation(
            &function,
            BlockId(0),
            0,
            &operation,
            &value_types,
            &Module::new("guarded"),
            &HashMap::new(),
            &mut pending,
            &mut discovered,
            &mut discovered_count,
            1,
            &mut findings,
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap();

        let report = findings.finish().unwrap();
        assert_eq!(report.total_findings(), 1);
        assert_eq!(
            report.findings()[0].feature,
            UnsupportedFeatureV1::NonScalarMemory
        );
    }

    #[test]
    fn gfx950_lds_transpose_has_an_explicit_unsupported_classification() {
        let function = Function::kernel_entry(
            "gfx950_transpose",
            fe2o3_kernel_ir::Signature::new(vec![], vec![]),
            vec![],
            vec![],
        );
        let operation = Operation::new(
            vec![fe2o3_kernel_ir::ValueDef::new(
                ValueId(0),
                Type::pointer(
                    Type::Scalar(ScalarType::U8),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Gfx950LdsTranspose(
                fe2o3_kernel_ir::Gfx950LdsTransposeOperationV1::full(
                    fe2o3_kernel_ir::Gfx950LdsTransposeOperationKindV1::Current {
                        format: fe2o3_kernel_ir::Gfx950LdsTransposeFormatV1::Fp8E4M3,
                    },
                ),
            ),
        );
        let value_types = HashMap::new();
        let mut findings = UnsupportedCollectorV1::new().unwrap();
        let mut pending = Vec::new();
        let mut discovered = vec![true];
        let mut discovered_count = 1;

        scan_operation(
            &function,
            BlockId(0),
            0,
            &operation,
            &value_types,
            &Module::new("gfx950_transpose"),
            &HashMap::new(),
            &mut pending,
            &mut discovered,
            &mut discovered_count,
            1,
            &mut findings,
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap();

        let report = findings.finish().unwrap();
        assert_eq!(report.total_findings(), 1);
        assert_eq!(report.findings()[0].function.as_str(), "gfx950_transpose");
        assert_eq!(report.findings()[0].block, Some(BlockId(0)));
        assert_eq!(report.findings()[0].operation, Some(0));
        assert_eq!(
            report.findings()[0].feature,
            UnsupportedFeatureV1::Gfx950LdsTranspose
        );
    }

    #[test]
    fn unsupported_report_retains_a_bounded_prefix_and_exact_total() {
        let extra = 17_usize;
        let mut collector = UnsupportedCollectorV1::new().unwrap();
        for occurrence in 0..MAX_REPORTED_UNSUPPORTED_FINDINGS_V1 + extra {
            collector.push("hostile".len(), || UnsupportedSimulationSiteV1 {
                function: FunctionId::new("hostile"),
                block: Some(BlockId(u32::try_from(occurrence).unwrap())),
                operation: Some(u32::try_from(occurrence).unwrap()),
                feature: UnsupportedFeatureV1::Wave,
            });
        }
        let report = collector.finish().unwrap();
        assert_eq!(
            report.findings().len(),
            MAX_REPORTED_UNSUPPORTED_FINDINGS_V1
        );
        assert_eq!(
            report.total_findings(),
            u64::try_from(MAX_REPORTED_UNSUPPORTED_FINDINGS_V1 + extra).unwrap()
        );
        assert!(report.is_truncated());
        assert_eq!(report.findings()[0].operation, Some(0));
        assert_eq!(
            report.findings().last().unwrap().operation,
            Some(u32::try_from(MAX_REPORTED_UNSUPPORTED_FINDINGS_V1 - 1).unwrap())
        );
    }

    #[test]
    fn oversized_first_finding_stops_prefix_retention_without_constructing_later_findings() {
        let mut collector = UnsupportedCollectorV1::new().unwrap();
        collector.push(MAX_REPORTED_UNSUPPORTED_IDENTIFIER_BYTES_V1 + 1, || {
            panic!("oversized finding must not be constructed")
        });
        collector.push(0, || {
            panic!("later finding must not break prefix semantics")
        });

        let report = collector.finish().unwrap();
        assert!(report.findings().is_empty());
        assert_eq!(report.total_findings(), 2);
        assert!(report.is_truncated());
    }

    #[test]
    fn repeated_deep_types_are_classified_without_owned_payload_clones() {
        let mut deep = Type::Unit;
        for _ in 0..64 {
            deep = Type::pointer(deep, AddressSpace::Private, AccessMode::ReadWrite);
        }
        let occurrences = MAX_REPORTED_UNSUPPORTED_FINDINGS_V1 + 1_000_000;
        let mut collector = UnsupportedCollectorV1::new().unwrap();
        for operation in 0..occurrences {
            let feature = unsupported_type(&deep, SimulationTargetV1::amdgpu_64()).unwrap();
            collector.push(1, || UnsupportedSimulationSiteV1 {
                function: FunctionId::new("f"),
                block: Some(BlockId(0)),
                operation: Some(u32::try_from(operation).unwrap()),
                feature,
            });
        }
        let report = collector.finish().unwrap();
        assert_eq!(
            report.findings().len(),
            MAX_REPORTED_UNSUPPORTED_FINDINGS_V1
        );
        assert_eq!(report.total_findings(), occurrences as u64);
        assert!(
            report
                .findings()
                .iter()
                .all(|finding| finding.feature == UnsupportedFeatureV1::NonScalarMemory)
        );
    }

    #[test]
    fn value_type_index_borrows_nested_types_from_the_function() {
        let mut deep = Type::Unit;
        for _ in 0..64 {
            deep = Type::pointer(deep, AddressSpace::Private, AccessMode::ReadWrite);
        }
        let mut block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
        block
            .parameters
            .push(fe2o3_kernel_ir::ValueDef::new(ValueId(1), deep.clone()));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::kernel_entry(
            "borrowed_types",
            fe2o3_kernel_ir::Signature::new(vec![deep], vec![]),
            vec![ValueId(0)],
            vec![block],
        );

        let indexed = value_types(&function, SimulationLimitsV1::default()).unwrap();
        assert!(std::ptr::eq(
            *indexed.get(&ValueId(0)).unwrap(),
            &function.signature.parameters[0],
        ));
        assert!(std::ptr::eq(
            *indexed.get(&ValueId(1)).unwrap(),
            &function.body.as_ref().unwrap().blocks[0].parameters[0].ty,
        ));
    }
}
