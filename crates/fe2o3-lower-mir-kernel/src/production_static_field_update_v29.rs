fn static_field_update_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("static field update differs from its original SSA holder")
}

// The old holder is a representation, not a read of each represented leaf.
// Cloning it keeps copied holders and earlier SSA definitions immutable.
fn rebuild_static_field_update_v29(
    types: &[SemanticTypeDeclV1],
    root_type: SemanticTypeIdV1,
    original: &SemanticValueBindingV1,
    destination: &SemanticPlaceV1,
    replacement: SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<SemanticValueBindingV1>(),
        std::mem::size_of::<Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1>>(),
    ])?)?;
    budget.charge_work(2)?;
    if destination.projections().is_empty() {
        return Err(static_field_update_error_v29());
    }
    let mut rebuilt = clone_execution_cfg_binding_v29(original, &mut 0, budget)?;
    let mut target = &mut rebuilt;
    let mut ty = root_type;
    for projection in destination.projections() {
        budget.charge_work(6)?;
        let SemanticProjectionKindV1::Field(field) = projection.kind() else {
            return Err(static_field_update_error_v29());
        };
        let Some(SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields)) =
            types
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape)
        else {
            return Err(static_field_update_error_v29());
        };
        let SemanticValueBindingV1::Aggregate(values) = target else {
            return Err(static_field_update_error_v29());
        };
        if values.len() != fields.fields().len() {
            return Err(static_field_update_error_v29());
        }
        ty = *fields
            .fields()
            .get(field as usize)
            .ok_or_else(static_field_update_error_v29)?;
        if ty != projection.result_type() {
            return Err(static_field_update_error_v29());
        }
        target = values
            .get_mut(field as usize)
            .ok_or_else(static_field_update_error_v29)?;
    }
    if ty != destination.ty() {
        return Err(static_field_update_error_v29());
    }
    *target = replacement;
    Ok(rebuilt)
}

#[cfg(test)]
mod static_field_update_tests {
    use super::*;
    include!("production_static_field_update_v29_tests.rs");
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn assign_static_field_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        destination: &SemanticPlaceV1,
        value: SemanticValueBindingV1,
        volatility: SemanticVolatilityV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let local = destination.local();
        let rebuilt = self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_mut()
                .ok_or_else(static_field_update_error_v29)?;
            cursor.check_ledger(budget)?;
            if let Some(references) = cursor.references {
                references.check(budget)?;
            }
            budget.charge_work(8)?;
            let statement_index = statement.ok_or_else(static_field_update_error_v29)?;
            let original = this
                .function
                .blocks()
                .get(block.index() as usize)
                .and_then(|block| block.statements().get(statement_index as usize))
                .ok_or_else(static_field_update_error_v29)?;
            if volatility != SemanticVolatilityV1::NonVolatile
                || !matches!(original.kind(), SemanticStatementKindV1::Assign(assignment)
                    if std::ptr::eq(assignment.destination(), destination)
                        && assignment.value().result_type() == destination.ty())
                || !this
                    .control_flow_ssa
                    .ssa_value_locals
                    .contains(&local.index())
                || !std::ptr::eq(cursor.function, this.function)
            {
                return Err(static_field_update_error_v29());
            }
            let definition = cursor.use_place(
                execution_site_v29(block, statement),
                ExecutionOperandV29::Destination,
                destination,
                false,
                budget,
            )?;
            charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
            let original = this
                .semantic_ssa_bindings
                .get(&definition)
                .ok_or_else(static_field_update_error_v29)?;
            let root_type = this
                .function
                .locals()
                .get(local.index() as usize)
                .ok_or_else(static_field_update_error_v29)?
                .ty();
            rebuild_static_field_update_v29(
                this.types,
                root_type,
                original,
                destination,
                value,
                budget,
            )
        })?;
        self.bind_local_definition_v29(block, statement, local, rebuilt, true)
    }
}
