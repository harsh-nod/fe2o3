//! Fixed policy execution over the exact storage-aware typed output graph.

use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalRankedViewV18;
use fe2o3_kernel_ir::{StorageLayoutLimitsV1, VerifiedCanonicalKernelIrModuleV18};

#[path = "canonical_ranked_pending_v18.rs"]
mod pending;
pub use pending::{
    PendingCanonicalGlobalAccessesV18,
    PendingCanonicalRankedPoliciesV18, PendingCanonicalRankedSourceRolesV18,
    PendingCanonicalPrivateMemoryPoliciesV18,
    with_pending_canonical_ranked_source_roles_v18,
};

/// Closed role whose source/output recipe is required before policy admission.
/// This diagnostic is not an annotation, proof, or permission to skip a check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRankedSourceRequirementV18 {
    /// Address derivation, lifetime, access, provenance, or synchronization.
    Memory,
    /// A call requires the exact original caller and argument-role joins.
    Call,
    /// A source verification event requires its original contract recipe.
    Contract,
    /// Device execution, wave, or pipeline control requires its source recipe.
    Execution,
    /// Matrix and vector operations require exact tensor and layout recipes.
    Tensor,
    /// Target assembly requires the original authenticated ordered recipe.
    Assembly,
    /// A semantic intrinsic requires its registered source recipe.
    Intrinsic,
}

/// One unresolved source-role occurrence, not a discharged recipe or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRankedSourceObligationV18 {
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    requirement: CanonicalRankedSourceRequirementV18,
}

impl CanonicalRankedSourceObligationV18 {
    /// Exact occurrence in the immutable canonical output owner.
    pub const fn coordinate(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        self.coordinate
    }
    /// The closed source role still required for this occurrence.
    pub const fn requirement(&self) -> CanonicalRankedSourceRequirementV18 {
        self.requirement
    }
}

fn require_profile(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'_>,
) -> Result<(), Failure> {
    visit_source_requirements(owner, budget, |obligation, _| {
        Err(Failure::SourceRequirementV18 {
            coordinate: obligation.coordinate,
            requirement: obligation.requirement,
        })
    })
}

fn visit_source_requirements(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'_>,
    mut consume: impl FnMut(CanonicalRankedSourceObligationV18, &mut Budget<'_>) -> Result<(), Failure>,
) -> Result<(), Failure> {
    use CanonicalRankedSourceRequirementV18 as Need;
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
        CanonicalKirOperationCoordinateV1 as Coordinate, CastKind, OperationKind as Op, Terminator,
        Type,
    };
    for (fi, function) in owner.module().functions.iter().enumerate() {
        budget.charge_work(1)?;
        let Some(body) = &function.body else {
            continue;
        };
        for (bi, block) in body.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            for (oi, operation) in block.operations.iter().enumerate() {
                budget.charge_work(1)?;
                let requirement = match operation.kind {
                    Op::Constant(_) | Op::Unary { .. } | Op::Binary { .. } | Op::Compare { .. } => {
                        continue;
                    }
                    // The canonical verifier's non-address cast rule requires
                    // both endpoints scalar, including Bitcast. Address-space
                    // and capability casts still need original source recipes.
                    Op::Cast { kind, .. } => match kind {
                        CastKind::Truncate
                        | CastKind::ZeroExtend
                        | CastKind::SignExtend
                        | CastKind::FloatExtend
                        | CastKind::FloatTruncate
                        | CastKind::IntegerToFloat
                        | CastKind::FloatToInteger
                        | CastKind::Bitcast => continue,
                        CastKind::RestrictPointerAccess
                        | CastKind::PointerToGeneric
                        | CastKind::SliceToGeneric => Need::Memory,
                    },
                    Op::Select { .. } => {
                        budget.charge_work(operation.results.len())?;
                        match operation.results.as_slice() {
                            [result] => match result.ty {
                                Type::Unit | Type::Scalar(_) => continue,
                                Type::Pointer(_) | Type::Slice(_) | Type::StorageObject(_) => {
                                    Need::Memory
                                }
                                Type::Vector(_) => Need::Tensor,
                                Type::Execution(_) => Need::Execution,
                            },
                            _ => return Err(Failure::ExactGraph),
                        }
                    }
                    Op::Call { .. } => Need::Call,
                    Op::VerificationContract(_) => Need::Contract,
                    Op::Execution(_) | Op::Wave(_) => Need::Execution,
                    Op::VectorLoad(_)
                    | Op::VectorStore(_)
                    | Op::VectorLayoutConvert(_)
                    | Op::Matrix(_) => Need::Tensor,
                    Op::InlineAssembly(_)
                    | Op::Gfx942OrderedRegion(_)
                    | Op::Gfx942OrderedProgram(_) => Need::Assembly,
                    Op::Intrinsic(_) => Need::Intrinsic,
                    Op::Storage(_)
                    | Op::MemoryIntrinsic(_)
                    | Op::Alloca { .. }
                    | Op::SliceLength { .. }
                    | Op::SliceData { .. }
                    | Op::GetElementPointer { .. }
                    | Op::Load { .. }
                    | Op::GuardedLoad { .. }
                    | Op::GuardedStore { .. }
                    | Op::Store { .. }
                    | Op::Barrier(_)
                    | Op::Atomic(_)
                    | Op::Fence(_)
                    | Op::WorkgroupBarrier(_)
                    | Op::WorkgroupMemory(_)
                    | Op::Gfx950LdsTranspose(_) => Need::Memory,
                };
                consume(
                    CanonicalRankedSourceObligationV18 {
                        coordinate: Coordinate {
                            block: Block {
                                function: Function(
                                    u32::try_from(fi).map_err(|_| Resource::Arithmetic)?,
                                ),
                                block: u32::try_from(bi).map_err(|_| Resource::Arithmetic)?,
                            },
                            operation: u32::try_from(oi).map_err(|_| Resource::Arithmetic)?,
                        },
                        requirement,
                    },
                    budget,
                )?;
            }
            if !matches!(
                block.terminator,
                Some(
                    Terminator::Branch { .. }
                        | Terminator::ConditionalBranch { .. }
                        | Terminator::Switch { .. }
                        | Terminator::IntegerSwitch { .. }
                        | Terminator::Return { .. }
                        | Terminator::Unreachable
                )
            ) {
                return Err(Failure::UnsupportedGraph {
                    function: fi,
                    block: Some(bi),
                    operation: None,
                });
            }
        }
    }
    Ok(())
}

/// Reports from the fixed policy on the actual V18 typed CFG. These reports do
/// not discharge source annotations, currentness, or complete ranked admission.
/// Neither a legacy owner nor a replacement recipe graph can be obtained here.
///
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalRankedPoliciesV18;
/// fn clone_it(x: &CheckedCanonicalRankedPoliciesV18<'_, '_>) { let _ = x.clone(); let _: CheckedCanonicalRankedPoliciesV18<'_, '_> = x.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalRankedPoliciesV18;
/// fn edit(x: &CheckedCanonicalRankedPoliciesV18<'_, '_>) { let _ = x.context(); }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalRankedPoliciesV18;
/// fn promote(x: &CheckedCanonicalRankedPoliciesV18<'_, '_>) { let _ = x.into_verified_ranked(); }
/// ```
pub struct CheckedCanonicalRankedPoliciesV18<'s, 'g> {
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    reports: &'s [Option<ReportRow>],
    observation: CanonicalRankedPolicyResourceObservationV1,
    guard: &'s Guard,
}

impl<'g> CheckedCanonicalRankedPoliciesV18<'_, 'g> {
    /// The exact immutable storage-aware graph checked by this policy invocation.
    pub fn owner(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&'g VerifiedCanonicalKernelIrModuleV18, Failure> {
        self.guard.query(budget)?;
        Ok(self.owner)
    }
    /// The full declaration-order roster, including external declarations.
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.reports.len())
    }
    /// None is an actual declaration, never an empty successful policy report.
    pub fn report(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&ProductionPlironPreloweringReportV2>, Failure> {
        self.guard.query(budget)?;
        self.reports
            .get(function)
            .map(|row| row.as_ref().map(|row| &row.outcome.report))
            .ok_or_else(|| self.guard.invalid(function))
    }
    /// Accepted fixed-policy history; declarations have no invocation history.
    pub fn history(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalRankedPolicyHistoryV1>, Failure> {
        self.guard.query(budget)?;
        self.reports
            .get(function)
            .map(|row| row.as_ref().map(|row| row.history))
            .ok_or_else(|| self.guard.invalid(function))
    }
    /// Cumulative accepted analysis-domain accounting, not KIR byte credits.
    pub fn observation(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalRankedPolicyResourceObservationV1, Failure> {
        self.guard.query(budget)?;
        Ok(self.observation)
    }
    /// Source, semantic, currentness, and target obligations remain open.
    pub const fn pending_obligations(&self) -> Obligations {
        pending_obligations()
    }
    /// These reports alone are not complete ranked verification.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// No artifact or device-launch authority is granted by this scope.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn exact_snapshot(
    graph: &mut crate::KirPlironGraphV18<'_>,
    limits: StorageLayoutLimitsV1,
    epoch: u64,
    budget: &mut Budget<'_>,
) -> Result<(), Failure> {
    graph.check_ranked_policy_epoch_v18(epoch)?;
    let (owner, report, storage) = graph.extract_canonical_v18_o0(limits, budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    drop((owner, report));
    budget.release_storage(storage.retained_storage())?;
    graph.check_ranked_policy_epoch_v18(epoch)
}

/// Runs the existing fixed nine-stage policy with shared invocation accounting.
/// The actual V18 emitter preserves pointer/storage block parameters, successor
/// occurrences, switch selectors/cases, and return payloads. No source metadata
/// claim is interpreted as permission to omit a check or change policy order.
/// The callback has the same exact-floor and pre-reserved-result contract as V1.
pub fn with_canonical_ranked_policy_checks_v18<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'w>,
    callback: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalRankedPoliciesV18<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
    let mut analysis = AnalysisState::new(Limits::production_hard_ceiling());
    let result = protected(budget, |budget| {
        let owner = checked.inventory(budget)?.owner();
        // Policy implementations may ignore unrelated dialect operations. Do
        // not turn that behavior into a clean report for an unjoined source role.
        require_profile(owner, budget)?;
        with_fixed_native_reports_v18(
            owner,
            layouts,
            &mut analysis,
            budget,
            |owner, reports, observation, guard, budget| {
                let view = CheckedCanonicalRankedPoliciesV18 {
                    owner,
                    reports,
                    observation,
                    guard,
                };
                callback(&view, budget)
            },
        )
    });
    result.map_err(|failure| CanonicalRankedPolicyChecksErrorV1 {
        failure,
        observation: analysis.observation(),
        last_invocation: analysis.last,
    })
}

// The shared executor observes native policies but never decides source roles.
// Its two public entrances construct distinct, non-convertible scoped views.
fn with_fixed_native_reports_v18<'g, 'w, T>(
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    layouts: StorageLayoutLimitsV1,
    analysis: &mut AnalysisState,
    budget: &mut Budget<'w>,
    callback: impl for<'s> FnOnce(
        &'g VerifiedCanonicalKernelIrModuleV18,
        &'s [Option<ReportRow>],
        CanonicalRankedPolicyResourceObservationV1,
        &'s Guard,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, Failure> {
    let mut reports = prepare_native_report_rows_v18::<T>(owner, budget)?;
    let (mut graph, storage) = crate::KirPlironGraphV18::import(owner, budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let epoch = graph.ranked_policy_epoch_v18()?;
    let result = execute_fixed_native_reports_v18(
        owner,
        layouts,
        analysis,
        &mut graph,
        epoch,
        &mut reports,
        budget,
        callback,
    )?;
    drop(graph);
    drop(reports);
    analysis.release_reports()?;
    result
}

fn prepare_native_report_rows_v18<T>(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'_>,
) -> Result<Vec<Option<ReportRow>>, Failure> {
    budget.reserve_storage(checked_add(
        size_of::<Result<Vec<Option<ReportRow>>, Failure>>(),
        size_of::<Result<Result<T, Failure>, Failure>>(),
    )?)?;
    budget.reserve_storage(checked_add(
        size_of::<AnalysisState>(),
        checked_add(
            size_of::<Guard>(),
            checked_add(
                size_of::<CheckedCanonicalRankedPoliciesV18<'_, '_>>(),
                checked_add(
                    size_of::<std::thread::Result<Result<T, Failure>>>(),
                    drain_header(),
                )?,
            )?,
        )?,
    )?)?;
    let mut reports = reserve_rows::<Option<ReportRow>>(owner.module().functions.len(), budget)?;
    budget.charge_work(owner.module().functions.len())?;
    reports.resize_with(owner.module().functions.len(), || None);
    Ok(reports)
}

fn execute_fixed_native_reports_v18<'g, 'w, T>(
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    layouts: StorageLayoutLimitsV1,
    analysis: &mut AnalysisState,
    graph: &mut crate::KirPlironGraphV18<'g>,
    epoch: u64,
    reports: &mut [Option<ReportRow>],
    budget: &mut Budget<'w>,
    callback: impl for<'s> FnOnce(
        &'g VerifiedCanonicalKernelIrModuleV18,
        &'s [Option<ReportRow>],
        CanonicalRankedPolicyResourceObservationV1,
        &'s Guard,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<Result<T, Failure>, Failure> {
    exact_snapshot(graph, layouts, epoch, budget)?;
    graph.visit_ranked_policy_functions_v18(epoch, budget, |ordinal, identity| {
        let slot = reports.get_mut(ordinal).ok_or(Failure::ExactGraph)?;
        if slot.is_some() {
            return Err(Failure::ExactGraph);
        }
        #[cfg(test)]
        pending::before_function();
        let outcome = analysis.invoke_lifecycle_v18(ordinal, identity)?;
        let history = analysis.last.ok_or(Failure::InvocationAccounting)?;
        *slot = Some(ReportRow { outcome, history });
        #[cfg(test)]
        tests::after_function(&graph, ordinal);
        #[cfg(test)]
        pending::after_function(&graph);
        Ok(())
    })?;
    #[cfg(test)]
    pending::before_post_native_snapshot(budget);
    exact_snapshot(graph, layouts, epoch, budget)?;
    let guard = Guard::new(budget);
    let result = guard.callback(budget, |budget| {
        callback(owner, &reports, analysis.observation(), &guard, budget)
    });
    if let Err(error) = graph.check_ranked_policy_epoch_v18(epoch) {
        drop(result);
        return Err(error);
    }
    Ok(result)
}

#[cfg(test)]
#[path = "canonical_ranked_checks_v18_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "canonical_ranked_lifecycle_identity_v18_tests.rs"]
mod lifecycle_identity_tests;
