// Address-use checking only. This grants no bounds, initialized-read,
// activation-lifetime, allocation-equivalence or relocation authority.
use super::origin_worklist_v1::{OriginStateV1, OriginWorkErrorV1, OriginWorkV1};
use super::*;

#[cfg(test)]
#[path = "production_scoped_slot_uses_v29_tests.rs"]
mod tests;

type Origin = OriginStateV1<Option<usize>>;
type UseResult<T> = Result<T, ProductionSemanticKirErrorV1>;

fn invalid(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

fn graph_error(error: CallInstanceEmissionErrorV1) -> ProductionSemanticKirErrorV1 {
    match error {
        CallInstanceEmissionErrorV1::Resource(error) => error.into(),
        _ => invalid("scoped source-slot address graph is inconsistent"),
    }
}

fn origin_error(error: OriginWorkErrorV1) -> ProductionSemanticKirErrorV1 {
    match error {
        OriginWorkErrorV1::Resource(error) => error.into(),
        OriginWorkErrorV1::Shape => invalid("scoped source-slot origin graph is inconsistent"),
    }
}

struct SlotUseGraphV29<'a> {
    index: CallSpliceIndexV1<'a>,
    blocks: Vec<(BlockId, &'a BasicBlock)>,
    origins: Vec<Origin>,
}

impl<'a> SlotUseGraphV29<'a> {
    fn value(&self, value: ValueId, budget: &mut ArgumentBudgetV1<'_>) -> UseResult<usize> {
        budget.charge_work(call_splice_search_work_v1(self.index.values.len()))?;
        self.index
            .values
            .binary_search_by_key(&value, |(id, _)| *id)
            .map_err(|_| invalid("scoped source-slot use has no definition"))
    }

    fn ty(&self, value: ValueId, budget: &mut ArgumentBudgetV1<'_>) -> UseResult<&'a Type> {
        Ok(self.index.values[self.value(value, budget)?].1)
    }

    fn exact(&self, value: ValueId, budget: &mut ArgumentBudgetV1<'_>) -> UseResult<Option<usize>> {
        match self.origins[self.value(value, budget)?] {
            Origin::Exact(origin) => Ok(origin),
            Origin::Unknown | Origin::Pending => {
                Err(invalid("scoped source-slot pointer transport is ambiguous"))
            }
        }
    }

    fn unrelated(&self, value: ValueId, budget: &mut ArgumentBudgetV1<'_>) -> UseResult<()> {
        if self.exact(value, budget)?.is_some() {
            return Err(invalid(
                "scoped source-slot address escapes through an unsupported operand",
            ));
        }
        Ok(())
    }

    fn target(&self, id: BlockId, budget: &mut ArgumentBudgetV1<'_>) -> UseResult<&'a BasicBlock> {
        budget.charge_work(call_splice_search_work_v1(self.blocks.len()))?;
        self.blocks
            .binary_search_by_key(&id, |(id, _)| *id)
            .map(|index| self.blocks[index].1)
            .map_err(|_| invalid("scoped source-slot CFG target is missing"))
    }

    fn same_type(
        &self,
        left: &Type,
        right: &Type,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<()> {
        if !call_splice_type_eq_v1(left, right, budget).map_err(graph_error)? {
            return Err(invalid(
                "scoped source-slot pointer access has an incompatible type",
            ));
        }
        Ok(())
    }

    fn new(
        function: &'a Function,
        slots: &[ScopedSourceSlotV29],
        first_slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<Self> {
        let mut scratch = 0;
        let index = call_splice_index_v1(function, budget, &mut scratch).map_err(graph_error)?;
        call_splice_check_body_v1(function, &index, false, budget).map_err(graph_error)?;
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| invalid("scoped source-slot body is missing"))?;
        let mut blocks =
            call_splice_vec_v1(body.blocks.len(), budget, &mut scratch).map_err(graph_error)?;
        let mut origins =
            call_splice_vec_v1(index.values.len(), budget, &mut scratch).map_err(graph_error)?;
        budget.charge_work(argument_sum_v1(&[body.blocks.len(), index.values.len()])?)?;
        origins.resize(index.values.len(), Origin::Exact(None));
        blocks.extend(body.blocks.iter().map(|block| (block.id, block)));
        call_splice_sort_work_v1(blocks.len(), budget).map_err(graph_error)?;
        blocks.sort_unstable_by_key(|(id, _)| *id);
        let mut graph = Self {
            index,
            blocks,
            origins,
        };
        for (ordinal, block) in body.blocks.iter().enumerate() {
            budget.charge_work(argument_sum_v1(&[
                1,
                block.parameters.len(),
                block.operations.len(),
            ])?)?;
            for parameter in &block.parameters {
                if matches!(parameter.ty, Type::Pointer(_)) {
                    let index = graph.value(parameter.id, budget)?;
                    graph.origins[index] = if ordinal == 0 {
                        Origin::Unknown
                    } else {
                        Origin::Pending
                    };
                }
            }
            for operation in &block.operations {
                if let Some(result) = transport_result(operation) {
                    let index = graph.value(result.id, budget)?;
                    graph.origins[index] = Origin::Pending;
                }
            }
        }
        budget.charge_work(slots.len())?;
        for (ordinal, slot) in slots.iter().enumerate() {
            let index = graph.value(slot.origin.pointer, budget)?;
            if graph.origins[index] != Origin::Exact(None) {
                return Err(invalid(
                    "scoped source-slot seed is not a unique allocation",
                ));
            }
            graph.origins[index] = Origin::Exact(Some(argument_sum_v1(&[first_slot, ordinal])?));
        }
        Ok(graph)
    }

    fn dependencies(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl FnMut(usize, usize, &mut ArgumentBudgetV1<'_>) -> UseResult<()>,
    ) -> UseResult<()> {
        for (_, block) in &self.blocks {
            budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
            for operation in &block.operations {
                let Some(result) = transport_result(operation) else {
                    continue;
                };
                let target = self.value(result.id, budget)?;
                match &operation.kind {
                    OperationKind::GetElementPointer { base, .. }
                    | OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value: base,
                        ..
                    } => {
                        visit(self.value(*base, budget)?, target, budget)?;
                    }
                    OperationKind::Select {
                        true_value,
                        false_value,
                        ..
                    } => {
                        visit(self.value(*true_value, budget)?, target, budget)?;
                        visit(self.value(*false_value, budget)?, target, budget)?;
                    }
                    _ => unreachable!(),
                }
            }
            let term = block
                .terminator
                .as_ref()
                .ok_or_else(|| invalid("scoped source-slot terminator is missing"))?;
            term.try_visit_edges_v1(|target, arguments| {
                let target = self.target(target, budget)?;
                budget.charge_work(argument_sum_v1(&[1, arguments.len()])?)?;
                if arguments.len() != target.parameters.len() {
                    return Err(invalid(
                        "scoped source-slot CFG transport has an incompatible signature",
                    ));
                }
                for (&argument, parameter) in arguments.iter().zip(&target.parameters) {
                    self.same_type(self.ty(argument, budget)?, &parameter.ty, budget)?;
                    if matches!(parameter.ty, Type::Pointer(_)) {
                        visit(
                            self.value(argument, budget)?,
                            self.value(parameter.id, budget)?,
                            budget,
                        )?;
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    fn solve(&mut self, budget: &mut ArgumentBudgetV1<'_>) -> UseResult<()> {
        let mut count = 0;
        self.dependencies(budget, |_, _, _| {
            count = argument_sum_v1(&[count, 1])?;
            Ok(())
        })?;
        let mut work =
            OriginWorkV1::new(self.origins.len(), count, budget).map_err(origin_error)?;
        for &origin in &self.origins {
            work.seed_next(origin, budget).map_err(origin_error)?;
        }
        self.dependencies(budget, |source, target, budget| {
            work.add_link(source, target, budget).map_err(origin_error)
        })?;
        self.origins = work.solve(budget).map_err(origin_error)?;
        Ok(())
    }

    fn pointer(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<&'a fe2o3_kernel_ir::PointerType> {
        match self.ty(value, budget)? {
            Type::Pointer(pointer) if pointer.address_space == AddressSpace::Private => Ok(pointer),
            _ => Err(invalid("scoped source-slot pointer is not private")),
        }
    }

    fn result<'o>(&self, operation: &'o Operation) -> UseResult<&'o ValueDef> {
        match operation.results.as_slice() {
            [result] => Ok(result),
            _ => Err(invalid(
                "scoped source-slot operation has incompatible results",
            )),
        }
    }

    fn access(
        &self,
        pointer: ValueId,
        access: &MemoryAccess,
        writing: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<&'a Type> {
        let ty = self.pointer(pointer, budget)?;
        if access.address_space != ty.address_space
            || !access.alignment.is_power_of_two()
            || (writing && ty.access != AccessMode::ReadWrite)
            || (!writing && ty.access == AccessMode::WriteOnly)
        {
            return Err(invalid("scoped source-slot memory access is incompatible"));
        }
        Ok(&ty.pointee)
    }

    fn check_candidate_operation(
        &self,
        operation: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<()> {
        match &operation.kind {
            OperationKind::GetElementPointer { base, offset } => {
                self.pointer(*base, budget)?;
                self.same_type(self.ty(*base, budget)?, &self.result(operation)?.ty, budget)?;
                if !matches!(self.ty(*offset, budget)?, Type::Scalar(scalar) if scalar.is_integer())
                {
                    return Err(invalid("scoped source-slot offset is not an integer"));
                }
            }
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value,
                to,
            } => {
                let from = self.pointer(*value, budget)?;
                let Type::Pointer(to_pointer) = to else {
                    return Err(invalid(
                        "scoped source-slot restriction has a nonpointer result",
                    ));
                };
                self.same_type(&self.result(operation)?.ty, to, budget)?;
                self.same_type(&from.pointee, &to_pointer.pointee, budget)?;
                if from.address_space != to_pointer.address_space
                    || from.access != AccessMode::ReadWrite
                    || to_pointer.access != AccessMode::ReadOnly
                {
                    return Err(invalid(
                        "scoped source-slot restriction changes address space or widens access",
                    ));
                }
            }
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => {
                let result = self.result(operation)?;
                self.pointer(*true_value, budget)?;
                self.same_type(self.ty(*condition, budget)?, &Type::BOOL, budget)?;
                self.same_type(
                    self.ty(*true_value, budget)?,
                    self.ty(*false_value, budget)?,
                    budget,
                )?;
                self.same_type(self.ty(*true_value, budget)?, &result.ty, budget)?;
                let origin = self.exact(*true_value, budget)?;
                if origin != self.exact(*false_value, budget)?
                    || origin != self.exact(result.id, budget)?
                {
                    return Err(invalid(
                        "scoped source-slot selection mixes allocation origins",
                    ));
                }
            }
            OperationKind::Load { pointer, access }
            | OperationKind::GuardedLoad {
                pointer, access, ..
            } => {
                let pointee = self.access(*pointer, access, false, budget)?;
                self.same_type(pointee, &self.result(operation)?.ty, budget)?;
                if let OperationKind::GuardedLoad {
                    predicate,
                    fallback,
                    ..
                } = &operation.kind
                {
                    self.same_type(self.ty(*predicate, budget)?, &Type::BOOL, budget)?;
                    self.same_type(self.ty(*fallback, budget)?, pointee, budget)?;
                }
            }
            OperationKind::Store {
                pointer,
                value,
                access,
            }
            | OperationKind::GuardedStore {
                pointer,
                value,
                access,
                ..
            } => {
                let pointee = self.access(*pointer, access, true, budget)?;
                self.same_type(pointee, self.ty(*value, budget)?, budget)?;
                if !operation.results.is_empty() {
                    return Err(invalid("scoped source-slot store has a result"));
                }
                if let OperationKind::GuardedStore { predicate, .. } = &operation.kind {
                    self.same_type(self.ty(*predicate, budget)?, &Type::BOOL, budget)?;
                }
            }
            _ => {
                return Err(invalid(
                    "scoped source-slot address escapes through an unsupported operand",
                ));
            }
        }
        Ok(())
    }

    fn check_uses(&self, budget: &mut ArgumentBudgetV1<'_>) -> UseResult<()> {
        for (_, block) in &self.blocks {
            budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
            for operation in &block.operations {
                let mut ordinal = 0;
                let mut candidate = false;
                if let OperationKind::InlineAssembly(assembly) = &operation.kind {
                    budget.charge_work(assembly.operands.len())?;
                }
                operation.kind.try_visit_operands(|value| {
                    let allowed = match &operation.kind {
                        OperationKind::GetElementPointer { .. }
                        | OperationKind::Cast {
                            kind: CastKind::RestrictPointerAccess,
                            ..
                        }
                        | OperationKind::Load { .. }
                        | OperationKind::GuardedLoad { .. }
                        | OperationKind::Store { .. }
                        | OperationKind::GuardedStore { .. } => ordinal == 0,
                        OperationKind::Select { .. } if transport_result(operation).is_some() => {
                            ordinal == 1 || ordinal == 2
                        }
                        _ => false,
                    };
                    ordinal = argument_sum_v1(&[ordinal, 1])?;
                    if self.exact(value, budget)?.is_some() {
                        if !allowed {
                            return Err(invalid(
                                "scoped source-slot address escapes through an unsupported operand",
                            ));
                        }
                        candidate = true;
                    }
                    Ok(())
                })?;
                if candidate {
                    self.check_candidate_operation(operation, budget)?;
                }
            }
            let term = block
                .terminator
                .as_ref()
                .ok_or_else(|| invalid("scoped source-slot terminator is missing"))?;
            match term {
                Terminator::Return { values } => {
                    budget.charge_work(values.len())?;
                    for &value in values {
                        self.unrelated(value, budget)?;
                    }
                }
                Terminator::ConditionalBranch { condition, .. } => {
                    self.unrelated(*condition, budget)?
                }
                Terminator::Switch { selector, .. }
                | Terminator::IntegerSwitch { selector, .. } => {
                    self.unrelated(*selector, budget)?;
                }
                Terminator::Branch { .. } | Terminator::Unreachable => {}
            }
            term.try_visit_edges_v1(|target, arguments| {
                let target = self.target(target, budget)?;
                budget.charge_work(arguments.len())?;
                for (&argument, parameter) in arguments.iter().zip(&target.parameters) {
                    if self.exact(argument, budget)? != self.exact(parameter.id, budget)? {
                        return Err(invalid(
                            "scoped source-slot CFG transport mixes allocation origins",
                        ));
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }
}

fn transport_result(operation: &Operation) -> Option<&ValueDef> {
    let [result] = operation.results.as_slice() else {
        return None;
    };
    if !matches!(result.ty, Type::Pointer(_)) {
        return None;
    }
    matches!(
        operation.kind,
        OperationKind::GetElementPointer { .. }
            | OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                ..
            }
            | OperationKind::Select { .. }
    )
    .then_some(result)
}

fn check_function(
    function: &Function,
    slots: &[ScopedSourceSlotV29],
    first_slot: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    let mut graph = SlotUseGraphV29::new(function, slots, first_slot, budget)?;
    graph.solve(budget)?;
    graph.check_uses(budget)
}

pub(super) fn check_scoped_source_slot_uses_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    max_elements: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    if slots.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    with_canonical_call_scratch_v1(budget, |budget| {
        // Reuse the complete census against the same live owners, including any
        // changes made by the test-only preassembly observer.
        let fresh = derive_scoped_source_slots_v29(instances, emitted, max_elements, budget)?;
        budget.charge_work(argument_sum_v1(&[
            5,
            argument_product_v1(slots.instances.len(), 7)?,
            argument_product_v1(slots.slots.len(), 18)?,
        ])?)?;
        let bytes = argument_sum_v1(&[
            argument_product_v1(
                slots.instances.capacity(),
                std::mem::size_of::<ScopedSourceSlotInstanceV29>(),
            )?,
            argument_product_v1(
                slots.slots.capacity(),
                std::mem::size_of::<ScopedSourceSlotV29>(),
            )?,
        ])?;
        if slots.source != fresh.source
            || slots.ledger != fresh.ledger
            || slots.instances != fresh.instances
            || slots.slots != fresh.slots
            || slots.retained_storage != bytes
        {
            return Err(scoped_slot_error_v29());
        }
        for row in &slots.instances {
            budget.charge_work(1)?;
            let candidates = slots
                .slots
                .get(row.slots.clone())
                .ok_or_else(scoped_slot_error_v29)?;
            if candidates.is_empty() {
                continue;
            }
            let lowered = emitted
                .get(row.instance.index())
                .and_then(Option::as_ref)
                .ok_or_else(scoped_slot_error_v29)?;
            with_canonical_call_scratch_v1(budget, |budget| {
                check_function(&lowered.function, candidates, row.slots.start, budget)
            })
            .map_err(|error| match error {
                ProductionSemanticKirErrorV1::Unsupported { detail, .. } => {
                    unsupported(row.function.index(), None, None, detail)
                }
                other => other,
            })?;
        }
        Ok(())
    })
}
