//! Bounded legality verification for target-neutral PLIRON atomic accesses.

use std::{
    collections::{BTreeSet, HashMap},
    fmt,
};

use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, MemorySpaceAttr, RankedAccessOp,
    RankedViewOp, SUPPORTED_ELEMENT_WIDTHS,
};
use pliron::{builtin::ops::FuncOp, context::Context, operation::Operation, value::Value};

use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationObserverV1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1};

pub const MAX_PLIRON_ATOMIC_OPERATIONS_V1: usize = 65_536;
pub const MAX_PLIRON_ATOMIC_FINDINGS_V1: usize = 4_096;
pub const MAX_PLIRON_ATOMIC_TARGET_CAPABILITIES_V1: usize = 256;
pub const MAX_PLIRON_SYSTEM_COHERENT_ALLOCATIONS_V1: usize = 4_096;

fn atomic_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::AtomicLegality,
        resource: "atomic legality resource upper bound",
    }
}

/// Bounds the view/access inventories and target-capability searches before
/// legality checking allocates either its indexes or diagnostic report.
pub(crate) fn preflight_atomic_legality_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    target: Option<&PlironAtomicTargetContextV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let capabilities = target.map_or(0, |target| target.capabilities().len());
    let coherent_allocations =
        target.map_or(0, |target| target.system_coherent_allocations().len());
    let target_search = capabilities
        .checked_add(coherent_allocations)
        .and_then(|items| items.checked_add(1))
        .ok_or_else(atomic_resource_overflow_v1)?;
    let work = census
        .operations
        .checked_mul(target_search)
        .and_then(|work| work.checked_add(census.operations))
        .ok_or_else(atomic_resource_overflow_v1)?;
    let findings = census
        .operations
        .checked_mul(2)
        .ok_or_else(atomic_resource_overflow_v1)?
        .min(MAX_PLIRON_ATOMIC_FINDINGS_V1);
    let retained = findings
        .checked_mul(16)
        .ok_or_else(atomic_resource_overflow_v1)?;
    let temporary = census
        .operations
        .checked_mul(8)
        .and_then(|storage| storage.checked_add(capabilities))
        .and_then(|storage| storage.checked_add(coherent_allocations))
        .ok_or_else(atomic_resource_overflow_v1)?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::AtomicLegality,
        work,
        retained,
        temporary,
    )?;
    limits.require(ProductionAnalysisResourcePhaseV1::AtomicLegality, bound)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlironAtomicTargetCapabilityV1 {
    element_width: u32,
    memory_space: MemorySpaceAttr,
    max_scope: AtomicScopeAttr,
}

impl PlironAtomicTargetCapabilityV1 {
    pub fn new(
        element_width: u32,
        memory_space: MemorySpaceAttr,
        max_scope: AtomicScopeAttr,
    ) -> Result<Self, PlironAtomicTargetContextErrorV1> {
        if !SUPPORTED_ELEMENT_WIDTHS.contains(&element_width)
            || memory_space == MemorySpaceAttr::Private
            || memory_space == MemorySpaceAttr::Workgroup && max_scope != AtomicScopeAttr::Workgroup
        {
            return Err(PlironAtomicTargetContextErrorV1::InvalidCapability);
        }
        Ok(Self {
            element_width,
            memory_space,
            max_scope,
        })
    }

    pub const fn element_width(self) -> u32 {
        self.element_width
    }

    pub const fn memory_space(self) -> MemorySpaceAttr {
        self.memory_space
    }

    pub const fn max_scope(self) -> AtomicScopeAttr {
        self.max_scope
    }

    fn supports(
        self,
        element_width: u32,
        memory_space: MemorySpaceAttr,
        scope: AtomicScopeAttr,
    ) -> bool {
        self.element_width == element_width
            && self.memory_space == memory_space
            && self.max_scope.rank() >= scope.rank()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironAtomicTargetContextV1 {
    capabilities: BTreeSet<PlironAtomicTargetCapabilityV1>,
    system_coherent_allocations: BTreeSet<u64>,
}

impl PlironAtomicTargetContextV1 {
    pub fn new(
        capabilities: impl IntoIterator<Item = PlironAtomicTargetCapabilityV1>,
    ) -> Result<Self, PlironAtomicTargetContextErrorV1> {
        let mut retained = BTreeSet::new();
        for (index, capability) in capabilities.into_iter().enumerate() {
            if index == MAX_PLIRON_ATOMIC_TARGET_CAPABILITIES_V1 {
                return Err(PlironAtomicTargetContextErrorV1::CapabilityLimitExceeded);
            }
            retained.insert(capability);
        }
        Ok(Self {
            capabilities: retained,
            system_coherent_allocations: BTreeSet::new(),
        })
    }

    pub fn with_system_coherent_allocations(
        mut self,
        allocations: impl IntoIterator<Item = u64>,
    ) -> Result<Self, PlironAtomicTargetContextErrorV1> {
        for allocation in allocations {
            if allocation == 0 {
                return Err(PlironAtomicTargetContextErrorV1::InvalidCoherentAllocation);
            }
            if self.system_coherent_allocations.len() == MAX_PLIRON_SYSTEM_COHERENT_ALLOCATIONS_V1
                && !self.system_coherent_allocations.contains(&allocation)
            {
                return Err(PlironAtomicTargetContextErrorV1::CoherentAllocationLimitExceeded);
            }
            self.system_coherent_allocations.insert(allocation);
        }
        Ok(self)
    }

    pub fn capabilities(&self) -> &BTreeSet<PlironAtomicTargetCapabilityV1> {
        &self.capabilities
    }

    pub fn system_coherent_allocations(&self) -> &BTreeSet<u64> {
        &self.system_coherent_allocations
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    fn supports(
        &self,
        element_width: u32,
        memory_space: MemorySpaceAttr,
        scope: AtomicScopeAttr,
    ) -> bool {
        self.capabilities
            .iter()
            .any(|capability| capability.supports(element_width, memory_space, scope))
    }

    fn supports_system_coherence(&self, allocation_origin: u64) -> bool {
        self.system_coherent_allocations
            .contains(&allocation_origin)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironAtomicTargetContextErrorV1 {
    InvalidCapability,
    CapabilityLimitExceeded,
    InvalidCoherentAllocation,
    CoherentAllocationLimitExceeded,
}

impl fmt::Display for PlironAtomicTargetContextErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapability => formatter.write_str(
                "atomic target capability has an invalid memory-space/scope combination",
            ),
            Self::CapabilityLimitExceeded => formatter
                .write_str("atomic target capability count exceeds the bounded verifier limit"),
            Self::InvalidCoherentAllocation => {
                formatter.write_str("system-coherent allocation identity must be non-zero")
            }
            Self::CoherentAllocationLimitExceeded => formatter
                .write_str("system-coherent allocation count exceeds the bounded verifier limit"),
        }
    }
}

impl std::error::Error for PlironAtomicTargetContextErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironAtomicLegalityFindingV1 {
    MissingAccessKind {
        block: usize,
        operation: usize,
    },
    MissingContract {
        block: usize,
        operation: usize,
        kind: AccessKindAttr,
        ordering_missing: bool,
        scope_missing: bool,
    },
    UnexpectedContract {
        block: usize,
        operation: usize,
        kind: AccessKindAttr,
    },
    InvalidOrdering {
        block: usize,
        operation: usize,
        kind: AccessKindAttr,
        ordering: AtomicOrderingAttr,
    },
    InvalidScope {
        block: usize,
        operation: usize,
        memory_space: MemorySpaceAttr,
        scope: AtomicScopeAttr,
    },
    ViewProvenanceUnavailable {
        block: usize,
        operation: usize,
    },
    TargetCapabilityUnavailable {
        block: usize,
        operation: usize,
        element_width: u32,
        memory_space: MemorySpaceAttr,
        scope: AtomicScopeAttr,
    },
    SystemCoherenceUnproven {
        block: usize,
        operation: usize,
    },
    ResourceLimitExceeded,
}

impl PlironAtomicLegalityFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::MissingAccessKind { .. }
            | Self::MissingContract { .. }
            | Self::UnexpectedContract { .. }
            | Self::InvalidOrdering { .. }
            | Self::InvalidScope { .. } => KernelCheckStatusV1::Rejected,
            Self::ViewProvenanceUnavailable { .. }
            | Self::TargetCapabilityUnavailable { .. }
            | Self::SystemCoherenceUnproven { .. }
            | Self::ResourceLimitExceeded => KernelCheckStatusV1::Incomplete,
        }
    }
}

impl fmt::Display for PlironAtomicLegalityFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAccessKind { block, operation } => write!(
                formatter,
                "error[FE2O3-ATOMIC-001]: atomic legality rejected malformed access at block {block} op {operation}: access kind is missing",
            ),
            Self::MissingContract {
                block,
                operation,
                kind,
                ordering_missing,
                scope_missing,
            } => write!(
                formatter,
                "error[FE2O3-ATOMIC-001]: {kind:?} at block {block} op {operation} lacks an explicit atomic contract: ordering_missing={ordering_missing}, scope_missing={scope_missing}; help: retain the source ordering and synchronization scope in Kernel IR",
            ),
            Self::UnexpectedContract {
                block,
                operation,
                kind,
            } => write!(
                formatter,
                "error[FE2O3-ATOMIC-001]: non-atomic {kind:?} at block {block} op {operation} carries atomic ordering or scope metadata",
            ),
            Self::InvalidOrdering {
                block,
                operation,
                kind,
                ordering,
            } => write!(
                formatter,
                "error[FE2O3-ATOMIC-001]: invalid {ordering:?} ordering for {kind:?} at block {block} op {operation}; atomic loads cannot release and atomic stores cannot acquire",
            ),
            Self::InvalidScope {
                block,
                operation,
                memory_space,
                scope,
            } => write!(
                formatter,
                "error[FE2O3-ATOMIC-001]: invalid {scope:?} atomic scope for {memory_space:?} memory at block {block} op {operation}; workgroup memory requires Workgroup scope and private memory is not atomic",
            ),
            Self::ViewProvenanceUnavailable { block, operation } => write!(
                formatter,
                "error[FE2O3-ATOMIC-002]: atomic analysis is incomplete at block {block} op {operation}: ranked-view memory-space provenance is unavailable",
            ),
            Self::TargetCapabilityUnavailable {
                block,
                operation,
                element_width,
                memory_space,
                scope,
            } => write!(
                formatter,
                "error[FE2O3-ATOMIC-002]: atomic analysis is incomplete at block {block} op {operation}: no bound target capability supports a {element_width}-bit {memory_space:?} atomic at {scope:?} scope",
            ),
            Self::SystemCoherenceUnproven { block, operation } => write!(
                formatter,
                "error[FE2O3-ATOMIC-002]: atomic analysis is incomplete at block {block} op {operation}: system scope requires authenticated coherent-allocation provenance",
            ),
            Self::ResourceLimitExceeded => formatter.write_str(
                "error[FE2O3-ATOMIC-003]: atomic legality analysis resource limit exceeded",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironAtomicLegalityReportV1 {
    findings: Vec<PlironAtomicLegalityFindingV1>,
}

super::pliron_report_payload_receipt::impl_empty_findings_payload_v1!(PlironAtomicLegalityReportV1);

impl PlironAtomicLegalityReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::AtomicLegality
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[PlironAtomicLegalityFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironAtomicLegalityCheckErrorV1 {
    report: PlironAtomicLegalityReportV1,
}

impl PlironAtomicLegalityCheckErrorV1 {
    pub const fn report(&self) -> &PlironAtomicLegalityReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironAtomicLegalityCheckErrorV1 {
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

impl std::error::Error for PlironAtomicLegalityCheckErrorV1 {}

#[cfg(test)]
pub(crate) fn run_pliron_atomic_legality_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironAtomicLegalityReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_check_with_analyses(context, function, None, &mut analyses)
}

#[cfg(test)]
pub(crate) fn run_pliron_atomic_legality_check_with_target_v1(
    context: &Context,
    function: &FuncOp,
    target: &PlironAtomicTargetContextV1,
) -> PlironAtomicLegalityReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_check_with_analyses(context, function, Some(target), &mut analyses)
}

#[cfg(test)]
pub(crate) fn require_pliron_atomic_legality_with_target_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
    target: &PlironAtomicTargetContextV1,
) -> Result<PlironAtomicLegalityReportV1, PlironAtomicLegalityCheckErrorV1> {
    require_report(run_pliron_atomic_legality_check_with_target_v1(
        context, function, target,
    ))
}

fn require_report(
    report: PlironAtomicLegalityReportV1,
) -> Result<PlironAtomicLegalityReportV1, PlironAtomicLegalityCheckErrorV1> {
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironAtomicLegalityCheckErrorV1 { report })
    }
}

#[cfg(test)]
fn run_check_with_analyses(
    context: &Context,
    function: &FuncOp,
    target: Option<&PlironAtomicTargetContextV1>,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironAtomicLegalityReportV1 {
    run_pliron_atomic_legality_check_with_observation_v1(context, function, target, analyses, None)
}

type AtomicObservationV1<'o, 'p, 'r> = Option<&'o InvocationObserverV1<'p, 'r>>;

fn with_atomic_observation_v1<T>(
    observer: AtomicObservationV1<'_, '_, '_>,
    run: impl FnOnce(AtomicObservationV1<'_, '_, '_>) -> T,
) -> T {
    match observer {
        None => run(None),
        Some(observer) => observer.with_projection(&Ok, |nested| run(Some(nested))),
    }
}

pub(crate) fn require_pliron_atomic_legality_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    target: Option<&PlironAtomicTargetContextV1>,
    analyses: &mut PlironAnalysisManagerV1,
    observer: AtomicObservationV1<'_, '_, '_>,
) -> Result<PlironAtomicLegalityReportV1, PlironAtomicLegalityCheckErrorV1> {
    with_atomic_observation_v1(observer, |observer| {
        require_report(run_atomic_check_inner_v1(
            context, function, target, analyses, observer,
        ))
    })
}

#[cfg(test)]
pub(crate) fn run_pliron_atomic_legality_check_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    target: Option<&PlironAtomicTargetContextV1>,
    analyses: &mut PlironAnalysisManagerV1,
    observer: AtomicObservationV1<'_, '_, '_>,
) -> PlironAtomicLegalityReportV1 {
    with_atomic_observation_v1(observer, |observer| {
        run_atomic_check_inner_v1(context, function, target, analyses, observer)
    })
}

fn run_atomic_check_inner_v1(
    context: &Context,
    function: &FuncOp,
    target: Option<&PlironAtomicTargetContextV1>,
    analyses: &mut PlironAnalysisManagerV1,
    observer: AtomicObservationV1<'_, '_, '_>,
) -> PlironAtomicLegalityReportV1 {
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(failure) => {
            if let Some(observer) = observer {
                observer.deny(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::FunctionInventory,
                    resource: failure.resource(),
                });
            }
            return report(vec![PlironAtomicLegalityFindingV1::ResourceLimitExceeded]);
        }
    };
    let mut operation_count = 0_usize;
    let mut views = HashMap::<Value, (MemorySpaceAttr, u32, u64)>::new();
    let mut access_sites = Vec::new();
    for site in inventory.operations() {
        operation_count += 1;
        if operation_count > MAX_PLIRON_ATOMIC_OPERATIONS_V1 {
            if let Some(observer) = observer {
                observer.deny(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::AtomicLegality,
                    resource: "atomic operation limit",
                });
            }
            return report(vec![PlironAtomicLegalityFindingV1::ResourceLimitExceeded]);
        }
        let operation = Operation::get_op_dyn(site.pointer(), context);
        if let Some(view) = operation.downcast_ref::<RankedViewOp>()
            && let (Some(memory_space), Some(view_type)) =
                (view.memory_space(context), view.view_type(context))
            && let Some(allocation_origin) = view.allocation_origin(context)
        {
            views.insert(
                view.result(context),
                (
                    memory_space,
                    view_type.deref(context).element_width(),
                    allocation_origin,
                ),
            );
        }
        if operation.downcast_ref::<RankedAccessOp>().is_some() {
            access_sites.push(*site);
        }
    }

    let mut findings = Vec::new();
    for site in access_sites {
        let block_index = site.block();
        let operation_index = site.operation();
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(access) = operation.downcast_ref::<RankedAccessOp>() else {
            continue;
        };
        let Some(kind) = access.kind(context) else {
            if !push_atomic_finding_v1(
                &mut findings,
                PlironAtomicLegalityFindingV1::MissingAccessKind {
                    block: block_index,
                    operation: operation_index,
                },
            ) {
                return report(findings);
            }
            continue;
        };
        let ordering = access.atomic_ordering(context);
        let scope = access.atomic_scope(context);
        if !kind.is_atomic() {
            if (ordering.is_some() || scope.is_some())
                && !push_atomic_finding_v1(
                    &mut findings,
                    PlironAtomicLegalityFindingV1::UnexpectedContract {
                        block: block_index,
                        operation: operation_index,
                        kind,
                    },
                )
            {
                return report(findings);
            }
            continue;
        }
        let (Some(ordering), Some(scope)) = (ordering, scope) else {
            if !push_atomic_finding_v1(
                &mut findings,
                PlironAtomicLegalityFindingV1::MissingContract {
                    block: block_index,
                    operation: operation_index,
                    kind,
                    ordering_missing: ordering.is_none(),
                    scope_missing: scope.is_none(),
                },
            ) {
                return report(findings);
            }
            continue;
        };
        if !ordering_is_valid(kind, ordering) {
            if !push_atomic_finding_v1(
                &mut findings,
                PlironAtomicLegalityFindingV1::InvalidOrdering {
                    block: block_index,
                    operation: operation_index,
                    kind,
                    ordering,
                },
            ) {
                return report(findings);
            }
            continue;
        }
        let Some(&(memory_space, element_width, allocation_origin)) =
            views.get(&access.view(context))
        else {
            if !push_atomic_finding_v1(
                &mut findings,
                PlironAtomicLegalityFindingV1::ViewProvenanceUnavailable {
                    block: block_index,
                    operation: operation_index,
                },
            ) {
                return report(findings);
            }
            continue;
        };
        if !scope_is_valid(memory_space, scope) {
            if !push_atomic_finding_v1(
                &mut findings,
                PlironAtomicLegalityFindingV1::InvalidScope {
                    block: block_index,
                    operation: operation_index,
                    memory_space,
                    scope,
                },
            ) {
                return report(findings);
            }
            continue;
        }
        if target.is_none_or(|target| !target.supports(element_width, memory_space, scope))
            && !push_atomic_finding_v1(
                &mut findings,
                PlironAtomicLegalityFindingV1::TargetCapabilityUnavailable {
                    block: block_index,
                    operation: operation_index,
                    element_width,
                    memory_space,
                    scope,
                },
            )
        {
            return report(findings);
        }
        if scope == AtomicScopeAttr::System
            && target.is_none_or(|target| !target.supports_system_coherence(allocation_origin))
            && !push_atomic_finding_v1(
                &mut findings,
                PlironAtomicLegalityFindingV1::SystemCoherenceUnproven {
                    block: block_index,
                    operation: operation_index,
                },
            )
        {
            return report(findings);
        }
    }
    report(findings)
}

fn push_atomic_finding_v1(
    findings: &mut Vec<PlironAtomicLegalityFindingV1>,
    finding: PlironAtomicLegalityFindingV1,
) -> bool {
    if findings.len() >= MAX_PLIRON_ATOMIC_FINDINGS_V1 - 1 {
        findings.push(PlironAtomicLegalityFindingV1::ResourceLimitExceeded);
        return false;
    }
    findings.push(finding);
    true
}

fn report(findings: Vec<PlironAtomicLegalityFindingV1>) -> PlironAtomicLegalityReportV1 {
    PlironAtomicLegalityReportV1 { findings }
}

const fn ordering_is_valid(kind: AccessKindAttr, ordering: AtomicOrderingAttr) -> bool {
    match kind {
        AccessKindAttr::AtomicRead => matches!(
            ordering,
            AtomicOrderingAttr::Relaxed
                | AtomicOrderingAttr::Acquire
                | AtomicOrderingAttr::SequentiallyConsistent
        ),
        AccessKindAttr::AtomicWrite => matches!(
            ordering,
            AtomicOrderingAttr::Relaxed
                | AtomicOrderingAttr::Release
                | AtomicOrderingAttr::SequentiallyConsistent
        ),
        AccessKindAttr::AtomicReadModifyWrite => true,
        AccessKindAttr::Read | AccessKindAttr::Write => false,
    }
}

const fn scope_is_valid(memory_space: MemorySpaceAttr, scope: AtomicScopeAttr) -> bool {
    match memory_space {
        MemorySpaceAttr::Private => false,
        MemorySpaceAttr::Workgroup => {
            matches!(
                scope,
                AtomicScopeAttr::SingleThread | AtomicScopeAttr::Workgroup
            )
        }
        MemorySpaceAttr::Global => true,
    }
}

#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    #[test]
    fn atomic_bound_has_exact_and_one_under_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 17,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_atomic_legality_resource_upper_bound_v1(
            census,
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert_eq!(
            preflight_atomic_legality_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::AtomicLegality,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_atomic_legality_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound() - 1,
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::AtomicLegality,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn atomic_bound_rejects_overflow_before_analysis() {
        assert_eq!(
            preflight_atomic_legality_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    operations: usize::MAX,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                None,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(atomic_resource_overflow_v1())
        );
    }

    #[test]
    fn atomic_bound_covers_two_findings_per_access_and_the_exact_cap() {
        let unlimited = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        let two = preflight_atomic_legality_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                operations: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            None,
            unlimited,
        )
        .unwrap();
        assert_eq!(two.retained_storage_upper_bound(), 2 * 16);

        let capped = preflight_atomic_legality_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                operations: MAX_PLIRON_ATOMIC_OPERATIONS_V1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            None,
            unlimited,
        )
        .unwrap();
        assert_eq!(
            capped.retained_storage_upper_bound(),
            MAX_PLIRON_ATOMIC_FINDINGS_V1 * 16
        );
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    fn invalid_ordering() -> PlironAtomicLegalityFindingV1 {
        PlironAtomicLegalityFindingV1::InvalidOrdering {
            block: 0,
            operation: 0,
            kind: AccessKindAttr::AtomicRead,
            ordering: AtomicOrderingAttr::Release,
        }
    }

    #[test]
    fn every_atomic_finding_has_the_shared_status() {
        let rejected = [
            PlironAtomicLegalityFindingV1::MissingAccessKind {
                block: 0,
                operation: 0,
            },
            PlironAtomicLegalityFindingV1::MissingContract {
                block: 0,
                operation: 0,
                kind: AccessKindAttr::AtomicRead,
                ordering_missing: true,
                scope_missing: true,
            },
            PlironAtomicLegalityFindingV1::UnexpectedContract {
                block: 0,
                operation: 0,
                kind: AccessKindAttr::Read,
            },
            invalid_ordering(),
            PlironAtomicLegalityFindingV1::InvalidScope {
                block: 0,
                operation: 0,
                memory_space: MemorySpaceAttr::Private,
                scope: AtomicScopeAttr::SingleThread,
            },
        ];
        for finding in rejected {
            assert_eq!(finding.status(), KernelCheckStatusV1::Rejected);
        }

        let incomplete = [
            PlironAtomicLegalityFindingV1::ViewProvenanceUnavailable {
                block: 0,
                operation: 0,
            },
            PlironAtomicLegalityFindingV1::TargetCapabilityUnavailable {
                block: 0,
                operation: 0,
                element_width: 32,
                memory_space: MemorySpaceAttr::Global,
                scope: AtomicScopeAttr::Device,
            },
            PlironAtomicLegalityFindingV1::SystemCoherenceUnproven {
                block: 0,
                operation: 0,
            },
            PlironAtomicLegalityFindingV1::ResourceLimitExceeded,
        ];
        for finding in incomplete {
            assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
        }
    }

    #[test]
    fn rejected_atomic_finding_dominates_an_incomplete_finding() {
        let mixed = report(vec![
            PlironAtomicLegalityFindingV1::ResourceLimitExceeded,
            invalid_ordering(),
        ]);
        assert_eq!(mixed.status(), KernelCheckStatusV1::Rejected);
        assert!(!mixed.is_clean());
        assert_eq!(report(vec![]).status(), KernelCheckStatusV1::Clean);
    }
}
