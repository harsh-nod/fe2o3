// A checked pair's fields are computed results, not original RHS operands.
// The original rvalue archive is captured before either field is written.
fn source_object_checked_types_v44<'a>(
    types: &'a [SemanticTypeDeclV1],
    result: SemanticTypeIdV1,
    checked: &fe2o3_mir_model::semantic_mir_v1::SemanticCheckedBinaryRvalueV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'a [SemanticTypeIdV1], ProductionSemanticKirErrorV1> {
    budget.charge_work(10)?;
    let fields = source_object_aggregate_field_types_v29(
        types,
        result,
        &SemanticAggregateKindV1::Tuple,
        2,
        budget,
    )?;
    if fields.len() != 2
        || checked.left().ty() != fields[0]
        || checked.right().ty() != fields[0]
        || lower_scalar_type(types, fields[1])? != Type::BOOL
    {
        return Err(scoped_object_allocation_error_v29());
    }
    let value = checked_binary_result_type(types, fields[0], result)
        .map_err(|_| scoped_object_allocation_error_v29())?;
    if !matches!(
        value,
        Type::Scalar(
            ScalarType::I8
                | ScalarType::I16
                | ScalarType::I32
                | ScalarType::I64
                | ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
        )
    ) {
        return Err(scoped_object_allocation_error_v29());
    }
    Ok(fields)
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn prepare_source_object_checked_fields_v44(
        &self,
        site: ExecutionSiteV29,
        assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        destination: ScopedObjectEndpointV29,
        fields: &[SemanticValueBindingV1],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Vec<SourceObjectAggregateFieldV29>, ProductionSemanticKirErrorV1> {
        let cursor = self
            .execution
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let plan = cursor
            .references
            .ok_or(ArgumentResourceV1::Accounting)?
            .plan;
        budget.source_reference_owner_v29(plan)?;
        source_reference_owned_prepay_v29::<(
            &ExecutionRvalueBindingV30,
            &[SemanticValueBindingV1],
            &[SemanticTypeIdV1],
            SemanticProjectionV1,
            Type,
            Option<SourceObjectAggregateFieldV29>,
        )>(plan, budget)?;
        let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
            return Err(scoped_object_allocation_error_v29());
        };
        let result_type = assignment.value().result_type();
        let types = source_object_checked_types_v44(self.types, result_type, checked, budget)?;
        charge_execution_cfg_lookup_v29(self.semantic_rvalue_bindings.len(), budget)?;
        let archived = self
            .semantic_rvalue_bindings
            .get(&execution_rvalue_key_v30(site)?)
            .ok_or_else(scoped_object_allocation_error_v29)?;
        let SemanticValueBindingV1::Aggregate(original) = &archived.binding else {
            return Err(scoped_object_allocation_error_v29());
        };
        if archived.ty != result_type || fields.len() != 2 || original.len() != 2 {
            return Err(scoped_object_allocation_error_v29());
        }
        let mut prepared = source_reference_owned_vec_v29(plan, 2, budget)?;
        for (index, &ty) in types.iter().enumerate() {
            budget.source_reference_charge_v29(plan, 14)?;
            let (
                SemanticValueBindingV1::Value { id, ty: actual },
                SemanticValueBindingV1::Value {
                    id: saved,
                    ty: saved_type,
                },
            ) = (&fields[index], &original[index])
            else {
                return Err(scoped_object_allocation_error_v29());
            };
            if id != saved
                || !invocation_equal_types_v1(actual, saved_type, budget)?
                || !invocation_equal_types_v1(actual, &lower_scalar_type(self.types, ty)?, budget)?
            {
                return Err(scoped_object_allocation_error_v29());
            }
            let ordinal = index as u32;
            let projection =
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(ordinal), ty)
                    .map_err(|_| ArgumentResourceV1::Accounting)?;
            let path = [projection];
            let components = budget.source_object_projection_v29(
                plan,
                result_type,
                destination.root_schema,
                &path,
            )?;
            let [component] = components.as_slice() else {
                return Err(scoped_object_allocation_error_v29());
            };
            let source_storage_v29::SourceSelectedComponentKindV29::Field {
                original,
                physical,
                ..
            } = component.kind
            else {
                return Err(scoped_object_allocation_error_v29());
            };
            if original != ordinal
                || component.source_type != result_type
                || component.source_schema != destination.root_schema
                || component.result_type != ty
            {
                return Err(scoped_object_allocation_error_v29());
            }
            prepared.push(SourceObjectAggregateFieldV29 {
                operand: ordinal,
                computed: true,
                projection: ScopedObjectViewProjectionV29::Field(
                    u32::try_from(physical).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ),
                ty,
                schema: component
                    .result_schema
                    .ok_or(ArgumentResourceV1::Accounting)?,
                value: *id,
                source: ScopedMemoryStoreSourceV29::AssignmentComponent {
                    site,
                    component: ordinal,
                    ty,
                },
            });
        }
        Ok(prepared)
    }
}
