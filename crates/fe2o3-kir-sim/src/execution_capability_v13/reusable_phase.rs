//! A physical replay schedule from the completed ordered audit, not a second
//! loan graph. Full phase operations are retained and compared before erasure.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVersionV1, Function, ReusablePhaseCheckLimitsV1, ReusablePhaseCheckUsageV1,
    ReusablePhasePhysicalOriginV1, analyze_control_flow, project_reusable_phase_function_v1,
};

type Result<T> = std::result::Result<T, ExecutionCapabilityProjectionErrorV13>;
fn invalid() -> ExecutionCapabilityProjectionErrorV13 {
    ExecutionCapabilityProjectionErrorV13::Invalid(
        "phase physical replay differs from the ordered audit",
    )
}
fn budget() -> ExecutionCapabilityProjectionErrorV13 {
    ExecutionCapabilityProjectionErrorV13::Invalid(
        "phase physical replay exceeded its existing resource ceiling",
    )
}

struct Step {
    block: BlockId,
    index: usize,
    original: Operation,
    origins: Vec<Option<ReusablePhasePhysicalOriginV1>>,
}

/// Private, move-only and not serializable. Preparing a schedule makes no
/// aliases; legacy allocation/conversion checks see only prior actual uses.
pub(super) struct Replay {
    steps: Vec<Step>,
    cursor: usize,
    usage: ReusablePhaseCheckUsageV1,
    work_limit: usize,
}

pub(super) fn prepare(
    function: &Function,
    version: CanonicalKernelIrVersionV1,
    limits: ReusablePhaseCheckLimitsV1,
) -> Result<Option<Replay>> {
    let Some(body) = &function.body else {
        return Ok(None);
    };
    let mut operations = 0usize;
    let mut count = 0usize;
    for block in &body.blocks {
        for operation in &block.operations {
            operations = operations.checked_add(1).ok_or_else(budget)?;
            count = count
                .checked_add(usize::from(matches!(
                    operation.kind,
                    OperationKind::ReusablePhase(_)
                )))
                .ok_or_else(budget)?;
        }
    }
    if count == 0 {
        return Ok(None);
    }
    if version != CanonicalKernelIrVersionV1::V14 {
        return Err(invalid());
    }
    let cfg = analyze_control_flow(function).map_err(|_| invalid())?;
    let mut plan =
        project_reusable_phase_function_v1(function, &cfg, limits).map_err(|_| invalid())?;
    plan.charge_adapter_work(
        operations
            .checked_add(body.blocks.len())
            .ok_or_else(budget)?,
    )
    .map_err(|_| budget())?;
    let mut steps = Vec::new();
    plan.observe_adapter_peak(count.checked_mul(size_of::<Step>()).ok_or_else(budget)?)
        .map_err(|_| budget())?;
    steps
        .try_reserve_exact(count)
        .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
    let mut held = steps
        .capacity()
        .checked_mul(size_of::<Step>())
        .ok_or_else(budget)?;
    plan.observe_adapter_peak(held).map_err(|_| budget())?;
    for block in &body.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            plan.charge_adapter_work(1).map_err(|_| budget())?;
            if !matches!(operation.kind, OperationKind::ReusablePhase(_)) {
                continue;
            }
            let rows = plan
                .operation_results(block.id, index)
                .map_err(|_| invalid())?;
            if rows.len() != operation.results.len() || rows[0].operation() != operation {
                return Err(invalid());
            }
            // Only closed phase output types occur here; no recursive runtime
            // type clone or arbitrary user container enters this schedule.
            let requested = phase_heap_bytes(operation)?
                .checked_add(
                    rows.len()
                        .checked_mul(size_of::<Option<ReusablePhasePhysicalOriginV1>>())
                        .ok_or_else(budget)?,
                )
                .ok_or_else(budget)?;
            plan.observe_adapter_peak(held.checked_add(requested).ok_or_else(budget)?)
                .map_err(|_| budget())?;
            // Use the exact sorted range, not a per-operation whole-roster scan.
            plan.charge_adapter_work(requested).map_err(|_| budget())?;
            let roster = plan.results();
            let key = (block.id, index);
            let start = roster.partition_point(|r| (r.block(), r.operation_index()) < key);
            let end = roster.partition_point(|r| (r.block(), r.operation_index()) <= key);
            let mut origins = Vec::new();
            origins
                .try_reserve_exact(end - start)
                .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
            origins.extend(roster[start..end].iter().map(|r| r.storage()));
            let original = operation.clone();
            let allocated = phase_heap_bytes(&original)?
                .checked_add(
                    origins
                        .capacity()
                        .checked_mul(size_of::<Option<ReusablePhasePhysicalOriginV1>>())
                        .ok_or_else(budget)?,
                )
                .ok_or_else(budget)?;
            held = held.checked_add(allocated).ok_or_else(budget)?;
            plan.observe_adapter_peak(held).map_err(|_| budget())?;
            plan.charge_adapter_work(
                operation
                    .results
                    .len()
                    .checked_add(2 * (1 + plan.results().len().max(1).ilog2() as usize))
                    .ok_or_else(budget)?,
            )
            .map_err(|_| budget())?;
            steps.push(Step {
                block: block.id,
                index,
                original,
                origins,
            });
        }
    }
    let usage = plan.finish().map_err(|_| invalid())?;
    Ok(Some(Replay {
        steps,
        cursor: 0,
        usage,
        work_limit: limits.work.min(ReusablePhaseCheckLimitsV1::DEFAULT.work),
    }))
}

fn phase_heap_bytes(operation: &Operation) -> Result<usize> {
    let OperationKind::ReusablePhase(p) = &operation.kind else {
        return Err(invalid());
    };
    let mut bytes = operation
        .results
        .capacity()
        .checked_mul(size_of::<ValueDef>())
        .ok_or_else(budget)?
        .checked_add(
            p.operands
                .capacity()
                .checked_mul(size_of::<ValueId>())
                .ok_or_else(budget)?,
        )
        .ok_or_else(budget)?
        .checked_add(p.provenance.root.retained_capacity_bytes())
        .ok_or_else(budget)?;
    for result in &operation.results {
        let root = match &result.ty {
            Type::ExecutionCapability(c) => &c.provenance.root,
            Type::ReusablePhaseToken(t) => &t.provenance.root,
            _ => return Err(invalid()),
        };
        bytes = bytes
            .checked_add(root.retained_capacity_bytes())
            .ok_or_else(budget)?;
    }
    Ok(bytes)
}

impl Replay {
    fn charge(&mut self, work: usize) -> Result<()> {
        self.usage.work = self.usage.work.checked_add(work).ok_or_else(budget)?;
        if self.usage.work > self.work_limit {
            return Err(budget());
        }
        Ok(())
    }
    pub(super) fn project(
        &mut self,
        block: BlockId,
        index: usize,
        operation: &Operation,
        scalars: &BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarType>,
        aliases: &mut Vec<(ValueId, ValueId)>,
        promoted: &mut BTreeMap<ValueId, Type>,
    ) -> Result<()> {
        let work = aliases
            .len()
            .checked_add(16)
            .and_then(|n| n.checked_add(4 * (1 + promoted.len().max(1).ilog2() as usize)))
            .and_then(|n| n.checked_add(4 * (1 + scalars.len().max(1).ilog2() as usize)))
            .and_then(|n| n.checked_mul(operation.results.len()))
            .ok_or_else(budget)?;
        self.charge(
            work.checked_add(phase_heap_bytes(operation)?)
                .ok_or_else(budget)?,
        )?;
        let step = self.steps.get(self.cursor).ok_or_else(invalid)?;
        if step.block != block || step.index != index || step.original != *operation {
            return Err(invalid());
        }
        let mut pointers = 0usize;
        for (result, origin) in operation.results.iter().zip(&step.origins) {
            if promoted.contains_key(&result.id)
                || aliases.iter().any(|(from, _)| *from == result.id)
            {
                return Err(invalid());
            }
            if !pointer_result(&result.ty) {
                continue;
            }
            pointers = pointers.checked_add(1).ok_or_else(budget)?;
            let origin = origin.ok_or_else(invalid)?;
            let scalar = *scalars.get(&origin.element()).ok_or_else(invalid)?;
            validate_layout(origin.layout(), scalar)?;
            u32::try_from(origin.elements()).map_err(|_| invalid())?;
            let pointer = Type::pointer(
                Type::Scalar(scalar),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            );
            if promoted
                .get(&origin.allocation())
                .is_some_and(|old| old != &pointer)
            {
                return Err(invalid());
            }
        }
        aliases
            .try_reserve(pointers)
            .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
        for (result, origin) in operation.results.iter().zip(&step.origins) {
            if !pointer_result(&result.ty) {
                continue;
            }
            let origin = origin.ok_or_else(invalid)?;
            let scalar = *scalars.get(&origin.element()).ok_or_else(invalid)?;
            promoted.insert(
                result.id,
                Type::pointer(
                    Type::Scalar(scalar),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            );
            aliases.push((result.id, origin.allocation()));
        }
        self.cursor += 1;
        Ok(())
    }
    pub(super) fn finish(mut self) -> Result<ReusablePhaseCheckUsageV1> {
        self.charge(1)?;
        if self.cursor != self.steps.len() {
            return Err(invalid());
        }
        Ok(self.usage)
    }
}
fn pointer_result(ty: &Type) -> bool {
    matches!(ty, Type::ExecutionCapability(c) if matches!(c.role,
        fe2o3_kernel_ir::ExecutionCapabilityRoleV1::Lds { .. }
        | fe2o3_kernel_ir::ExecutionCapabilityRoleV1::ReusableLds { .. }))
}

pub(super) fn requires_v14(module: &Module) -> bool {
    fn ty(mut ty: &Type) -> bool {
        loop {
            ty =
                match ty {
                    Type::ReusablePhaseToken(_) => return true,
                    Type::ExecutionCapability(c) => {
                        return matches!(c.role,
                    fe2o3_kernel_ir::ExecutionCapabilityRoleV1::ReusableWorkgroup
                    | fe2o3_kernel_ir::ExecutionCapabilityRoleV1::ReusablePhaseCompletion);
                    }
                    Type::Pointer(p) => &p.pointee,
                    Type::Slice(s) => &s.element,
                    Type::Unit
                    | Type::Scalar(_)
                    | Type::KernelContext(_)
                    | Type::GlobalCapability(_) => return false,
                };
        }
    }
    module.functions.iter().any(|function| {
        function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
            .any(ty)
            || function.body.as_ref().is_some_and(|body| {
                body.blocks.iter().any(|block| {
                    block.parameters.iter().any(|v| ty(&v.ty))
                        || block.operations.iter().any(|op| {
                            matches!(op.kind, OperationKind::ReusablePhase(_))
                                || op.results.iter().any(|v| ty(&v.ty))
                        })
                })
            })
    })
}
