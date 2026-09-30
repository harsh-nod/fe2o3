// A borrowed description of the actual installation, not a value-availability,
// initialization, or capability certificate. It cannot outlive its cursor.
struct PreparedInputTransportV1<'view, 'source> {
    cursor: &'view ExecutionAvailabilityV29<'source>,
    values: &'view [ValueId],
    types: &'view [Type],
    locals: &'view [Option<SemanticValueBindingV1>],
    slot: usize,
}

#[allow(clippy::too_many_arguments)]
fn with_prepared_input_transport_v1<'view, 'source, 'work, R>(
    cursor: Option<&'view ExecutionAvailabilityV29<'source>>,
    function: &SemanticFunctionDeclV1,
    ssa: &ProductionSemanticSsaFunctionPlanV1,
    values: &'view [ValueId],
    types: &'view [Type],
    locals: &'view [Option<SemanticValueBindingV1>],
    budget: Option<&mut (dyn SemanticEmissionBudgetV1 + 'work)>,
    consume: impl FnOnce(
        Option<&PreparedInputTransportV1<'view, 'source>>,
        Option<&mut (dyn SemanticEmissionBudgetV1 + 'work)>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    let Some(cursor) = cursor
        .filter(|cursor| cursor.invocation_inputs.is_some() || !cursor.retained_seeds.is_empty())
    else {
        return consume(None, budget);
    };
    let budget = budget.ok_or(ArgumentResourceV1::Accounting)?;
    cursor.check_ledger(budget)?;
    let slot = budget
        .prepared_input_slot_v1()
        .ok_or(ArgumentResourceV1::Accounting)?;
    let floor = budget.storage();
    let header = argument_sum_v1(&[
        std::mem::size_of::<PreparedInputTransportV1<'_, '_>>(),
        std::mem::size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>(),
    ])?;
    let required = argument_sum_v1(&[floor, header])?;
    budget.reserve_storage(header)?;
    let mut growth = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        if let Some(root) = cursor
            .references
            .and_then(|row| row.plan.storage_root.as_ref())
        {
            budget.charge_work(source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29)?;
            growth = Some(
                root.capture_retained_growth()
                    .ok_or(ArgumentResourceV1::Accounting)?,
            );
        }
        let prepared = PreparedInputTransportV1 {
            cursor,
            values,
            types,
            locals,
            slot,
        };
        prepared.check(cursor, function, ssa, budget)?;
        budget.charge_work(cursor.visited.len())?;
        if cursor.block.is_some()
            || cursor.visited.iter().any(|visited| *visited)
            || locals.len() != function.locals().len()
            || values.len() != types.len()
        {
            return Err(invocation_entry_error_v1());
        }
        if let Some(inputs) = cursor.invocation_inputs.as_deref() {
            invocation_check_inputs_v1(function, inputs, values.len(), budget)?;
        } else {
            prepared.check_retained_installation(budget)?;
        }
        consume(Some(&prepared), Some(budget))
    }));
    match result {
        Ok(Ok(value)) => {
            if !prepared_input_refund_v1(
                cursor,
                slot,
                floor,
                required,
                growth,
                Some(header),
                budget,
            ) {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok(value)
        }
        Ok(Err(error)) => {
            let bytes = budget.storage().checked_sub(floor);
            prepared_input_refund_v1(cursor, slot, floor, required, growth, bytes, budget);
            Err(error)
        }
        Err(payload) => {
            let bytes = budget.storage().checked_sub(floor);
            prepared_input_refund_v1(cursor, slot, floor, required, growth, bytes, budget);
            resume_unwind(payload)
        }
    }
}

fn prepared_input_refund_v1(
    cursor: &ExecutionAvailabilityV29<'_>,
    slot: usize,
    floor: usize,
    required: usize,
    growth: Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>,
    bytes: Option<usize>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> bool {
    let source = cursor.references.map(|row| row.plan);
    scoped_emission_refund_v29(
        source,
        cursor.ledger,
        slot,
        floor,
        required,
        growth,
        bytes,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn scoped_emission_refund_v29(
    source: Option<&SourceReferencePlanV29<'_, '_>>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    floor: usize,
    required: usize,
    growth: Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>,
    bytes: Option<usize>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> bool {
    let growth_permits = bytes.is_some_and(|bytes| {
        growth
            .as_ref()
            .is_none_or(|growth| growth.permits_refund(floor, required, budget.storage(), bytes))
    });
    drop(growth);
    if let Some(bytes) = bytes
        && growth_permits
        && budget.permits_prepared_input_refund_v1(source, slot, ledger, required, bytes)
        && budget.release_storage(bytes).is_ok()
    {
        return true;
    }
    if let Some(root) = source.and_then(|plan| plan.storage_root.as_ref()) {
        root.deny_active_root_refund();
    }
    false
}

impl PreparedInputTransportV1<'_, '_> {
    fn check(
        &self,
        cursor: &ExecutionAvailabilityV29<'_>,
        function: &SemanticFunctionDeclV1,
        ssa: &ProductionSemanticSsaFunctionPlanV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        cursor.check_ledger(budget)?;
        cursor.check_source(function, ssa)?;
        budget.charge_work(1)?;
        if !std::ptr::eq(
            self.cursor as *const _ as *const (),
            cursor as *const _ as *const (),
        ) || budget.prepared_input_slot_v1() != Some(self.slot)
        {
            return Err(execution_availability_error_v29());
        }
        Ok(())
    }

    fn check_retained_installation(
        &self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let cursor = self.cursor;
        let (identities, instance) = cursor.identities.ok_or_else(execution_identity_error_v1)?;
        identities.check_cursor(cursor, budget)?;
        budget.charge_work(3)?;
        if instance != cursor.instance
            || cursor.references.is_none()
            || cursor.retained_seeds.len() != self.locals.len()
        {
            return Err(execution_identity_error_v1());
        }
        charge_execution_cfg_lookup_v29(identities.index.retained.len(), budget)?;
        let mut count = 0usize;
        for (&(_, local), _) in identities
            .index
            .retained
            .range((instance.index(), 0)..=(instance.index(), u32::MAX))
        {
            budget.charge_work(2)?;
            if !cursor.retained_installed_slot_omission_v1(local, self, budget)? {
                return Err(execution_identity_error_v1());
            }
            count = argument_sum_v1(&[count, 1])?;
        }
        budget.charge_work(cursor.retained_seeds.len())?;
        if count == 0
            || cursor
                .retained_seeds
                .iter()
                .filter(|row| row.is_some())
                .count()
                != count
        {
            return Err(execution_identity_error_v1());
        }
        Ok(())
    }

    fn ordinary(
        &self,
        local: u32,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<SemanticTypeIdV1>, ProductionSemanticKirErrorV1> {
        self.cursor.check_ledger(budget)?;
        if budget.prepared_input_slot_v1() != Some(self.slot) {
            return Err(execution_availability_error_v29());
        }
        let Some(inputs) = self.cursor.invocation_inputs.as_deref() else {
            // The retained installation has no physical ordinal authority.
            return Ok(None);
        };
        for row in inputs {
            budget.charge_work(1)?;
            if row.local != local {
                continue;
            }
            let binding = self
                .locals
                .get(local as usize)
                .and_then(Option::as_ref)
                .ok_or_else(invocation_entry_error_v1)?;
            if !prepared_input_ordinary_shape_v1(binding, &mut 0, budget)? {
                return Ok(None);
            }
            let end = argument_sum_v1(&[row.first_parameter, row.parameter_count])?;
            let mut visitor = PreparedInputComponentsV1 {
                values: self
                    .values
                    .get(row.first_parameter..end)
                    .ok_or_else(invocation_entry_error_v1)?,
                types: self
                    .types
                    .get(row.first_parameter..end)
                    .ok_or_else(invocation_entry_error_v1)?,
                next: 0,
                nodes: 0,
                budget,
            };
            binding.visit_values_v1(&mut visitor)?;
            if visitor.next != row.parameter_count {
                return Err(invocation_entry_error_v1());
            }
            return Ok(Some(row.ty));
        }
        Ok(None)
    }
}

fn prepared_input_ordinary_shape_v1(
    binding: &SemanticValueBindingV1,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)?;
    match binding {
        SemanticValueBindingV1::Unit => Ok(true),
        SemanticValueBindingV1::Value { ty, .. } => Ok(!matches!(ty, Type::Execution(_))),
        SemanticValueBindingV1::Aggregate(fields) => {
            for field in fields {
                if !prepared_input_ordinary_shape_v1(field, nodes, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

struct PreparedInputComponentsV1<'a> {
    values: &'a [ValueId],
    types: &'a [Type],
    next: usize,
    nodes: usize,
    budget: &'a mut dyn SemanticEmissionBudgetV1,
}

impl SemanticTransportVisitorV1 for PreparedInputComponentsV1<'_> {
    type Error = ProductionSemanticKirErrorV1;

    fn node(&mut self) -> Result<(), Self::Error> {
        execution_cfg_charge_node_v29(&mut self.nodes, self.budget)
    }

    fn component(
        &mut self,
        value: ValueId,
        ty: BorrowedTransportTypeV1<'_>,
    ) -> Result<(), Self::Error> {
        self.budget.charge_work(1)?;
        if self.values.get(self.next) != Some(&value) {
            return Err(invocation_entry_error_v1());
        }
        let expected = self
            .types
            .get(self.next)
            .ok_or_else(invocation_entry_error_v1)?;
        let BorrowedTransportTypeV1::Existing(ty) = ty else {
            // The ordinary shape check admits no synthesized pointer carrier.
            return Err(invocation_entry_error_v1());
        };
        if !invocation_equal_types_v1(expected, ty, self.budget)? {
            return Err(invocation_entry_error_v1());
        }
        self.next = argument_sum_v1(&[self.next, 1])?;
        Ok(())
    }

    fn invalid(_: &'static str) -> Self::Error {
        invocation_entry_error_v1()
    }

    fn equal_types(&mut self, left: &Type, right: &Type) -> Result<bool, Self::Error> {
        invocation_equal_types_v1(left, right, self.budget)
    }
}
