//! Physical views of an already ordered, canonical V14 phase function.
//! This child does not authenticate source or discharge target requirements.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVersionV1, ReusablePhasePhysicalProjectionV1, ReusablePhasePhysicalResultV1,
    ReusablePhaseCheckLimitsV1, analyze_control_flow, project_reusable_phase_function_v1,
    ReusablePhaseCheckUsageV1,
};

// Function plans are dropped between functions; work is cumulative across the
// module, while the existing temporary-storage ceiling bounds each live plan.
pub(super) struct ModuleBudget { remaining: ReusablePhaseCheckLimitsV1 }
impl ModuleBudget {
    pub(super) fn new(limits: ReusablePhaseCheckLimitsV1) -> Self {
        Self { remaining: ReusablePhaseCheckLimitsV1 {
            work: limits.work.min(ReusablePhaseCheckLimitsV1::DEFAULT.work),
            temporary_bytes: limits.temporary_bytes.min(ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes),
        } }
    }
    pub(super) fn remaining(&self) -> ReusablePhaseCheckLimitsV1 { self.remaining }
    pub(super) fn consume(&mut self, module: &Module, usage: ReusablePhaseCheckUsageV1) -> Result<(), LoweringErrors> {
        self.remaining.work = self.remaining.work.checked_sub(usage.work)
            .ok_or_else(|| incomplete(module, "phase module physical work ceiling exceeded"))?;
        Ok(())
    }
}

pub(super) fn is_phase_type(ty: &Type) -> bool {
    matches!(ty, Type::ReusablePhaseToken(_)) || matches!(ty,
        Type::ExecutionCapability(c) if matches!(c.role,
            ExecutionCapabilityRoleV1::ReusableWorkgroup |
            ExecutionCapabilityRoleV1::ReusablePhaseCompletion))
}

fn contains_phase(function: &Function) -> bool {
    function.signature.parameters.iter().chain(&function.signature.results).any(is_phase_type)
        || function.body.as_ref().is_some_and(|body| body.blocks.iter().any(|block| {
            block.parameters.iter().any(|p| is_phase_type(&p.ty))
                || block.operations.iter().any(|op| matches!(op.kind, OperationKind::ReusablePhase(_))
                    || op.results.iter().any(|r| is_phase_type(&r.ty)))
        }))
}

pub(super) fn check_declared_version(
    module: &Module,
    declared_version: CanonicalKernelIrVersionV1,
) -> Result<(), LoweringErrors> {
    if declared_version == CanonicalKernelIrVersionV1::V13
        && module.functions.iter().any(contains_phase)
    {
        return Err(incomplete(module, "V13 physical lowering cannot consume V14 phase custody"));
    }
    Ok(())
}

pub(super) fn prepare<'a>(
    module: &Module,
    original: &'a Function,
    declared_version: CanonicalKernelIrVersionV1,
    limits: ReusablePhaseCheckLimitsV1,
) -> Result<Option<ReusablePhasePhysicalProjectionV1<'a>>, LoweringErrors> {
    if !contains_phase(original) { return Ok(None); }
    if declared_version != CanonicalKernelIrVersionV1::V14 {
        return Err(incomplete(module, "V13 physical lowering cannot consume V14 phase custody"));
    }
    let cfg = analyze_control_flow(original)
        .map_err(|e| incomplete(module, format!("phase physical CFG rejected: {e:?}")))?;
    let plan = project_reusable_phase_function_v1(original, &cfg, limits)
        .map_err(|e| incomplete(module, format!("phase physical projection rejected: {e:?}")))?;
    Ok(Some(plan))
}

pub(super) fn reject_legacy_results(
    module: &Module,
    operation: &Operation,
) -> Result<(), LoweringErrors> {
    if operation.results.iter().any(|r| is_phase_type(&r.ty)) {
        return Err(incomplete(module, "phase result lacks its exact lifecycle adapter"));
    }
    Ok(())
}

/// Called at the exact original phase operation, before replacing that
/// operation in the existing physical lowering loop. No memory op is emitted.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    module: &Module,
    declared_version: CanonicalKernelIrVersionV1,
    plan: &mut ReusablePhasePhysicalProjectionV1<'_>,
    original: &Function,
    block: fe2o3_kernel_ir::BlockId,
    index: usize,
    operation: &Operation,
    elements: &BTreeMap<ExecutionTypeIdentityV1, Type>,
    lowered: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
) -> Result<(), LoweringErrors> {
    let error = || {
        incomplete(
            module,
            "phase physical lowering lost its exact ordered V14 operation",
        )
    };
    if declared_version != CanonicalKernelIrVersionV1::V14
        || !std::ptr::eq(plan.function(), original)
        || !matches!(operation.kind, OperationKind::ReusablePhase(_))
    {
        return Err(error());
    }
    let lookup = [elements.len(), lowered.len(), aliases.len()]
        .into_iter()
        .try_fold(4usize, |n, len| {
            n.checked_add(1 + len.max(1).ilog2() as usize)
        })
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_mul(operation.results.len()))
        .ok_or_else(error)?;
    // Full equality below also compares the bounded source record and retained
    // result provenance. Charge their bytes, not only the number of map keys.
    let OperationKind::ReusablePhase(contract) = &operation.kind else { return Err(error()); };
    let mut equality = std::mem::size_of_val(contract)
        .checked_add(contract.provenance.root.as_str().len())
        .and_then(|n| n.checked_add(contract.operands.len().checked_mul(std::mem::size_of::<ValueId>())?))
        .ok_or_else(error)?;
    for result in &operation.results {
        let root = match &result.ty {
            Type::ExecutionCapability(c) => c.provenance.root.as_str(),
            Type::ReusablePhaseToken(t) => t.provenance.root.as_str(),
            _ => return Err(error()),
        };
        equality = equality.checked_add(std::mem::size_of_val(result))
            .and_then(|n| n.checked_add(root.len())).ok_or_else(error)?;
    }
    let work = equality.checked_mul(2).and_then(|n| n.checked_add(lookup)).ok_or_else(error)?;
    plan.charge_adapter_work(work).map_err(|_| error())?;
    let rows = plan
        .operation_results(block, index)
        .map_err(|e| incomplete(module, format!("phase physical projection rejected: {e:?}")))?;
    if rows.len() != operation.results.len() || rows[0].operation() != operation {
        return Err(error());
    }
    // Preflight the whole operation before changing the existing physical maps.
    // These maps are physical views only; ordered SSA has already consumed the
    // input loans and proved their matching Close/End on every relevant edge.
    for (row, result) in rows.iter().zip(&operation.results) {
        if row.result() != result || aliases.contains_key(&result.id) {
            return Err(error());
        }
        if let Some((physical, pointer)) = pointer_view(module, row, elements)? {
            if lowered
                .get(&physical.value)
                .is_some_and(|t| !matches!(t, Type::ExecutionCapability(_)) && t != &pointer)
            {
                return Err(error());
            }
        }
    }
    for row in rows {
        if let Some((physical, pointer)) = pointer_view(module, row, elements)? {
            lowered.insert(row.result().id, pointer);
            aliases.insert(row.result().id, ExecutionAliasV1::Physical(physical));
        } else {
            aliases.insert(row.result().id, ExecutionAliasV1::Erased);
        }
    }
    Ok(())
}

fn pointer_view(
    module: &Module,
    row: &ReusablePhasePhysicalResultV1<'_>,
    elements: &BTreeMap<ExecutionTypeIdentityV1, Type>,
) -> Result<Option<(PhysicalValueV1, Type)>, LoweringErrors> {
    if !row.materializes_pointer() {
        return Ok(None);
    }
    let origin = row
        .storage()
        .ok_or_else(|| incomplete(module, "phase LDS has no original allocation"))?;
    let element = elements.get(&origin.element()).ok_or_else(|| {
        incomplete(
            module,
            "phase LDS element has no structural physical KIR type witness",
        )
    })?;
    if !matches!(element, Type::Scalar(_)) {
        return Err(incomplete(
            module,
            "phase LDS currently requires an exact scalar physical witness",
        ));
    }
    validate_physical_layout(module, element, origin.layout())?;
    u32::try_from(origin.elements()).map_err(|_| {
        incomplete(
            module,
            "phase LDS extent exceeds the existing physical allocation width",
        )
    })?;
    let pointer = Type::pointer(
        element.clone(),
        fe2o3_kernel_ir::AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    Ok(Some((
        PhysicalValueV1::view(
            origin.allocation(),
            PhysicalExtentV1::Static(origin.elements()),
        ),
        pointer,
    )))
}
