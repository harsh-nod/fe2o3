fn prepay_scalar_enum_emission_shape_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(MAX_SSA_VALUE_COMPONENTS_V1)?;
    let nodes = visit_scalar_enum_result_v1(types, ty, |_, _, _| Ok(()))?;
    budget.charge_work(argument_product_v1(nodes, 12)?)?;
    budget.reserve_storage(argument_product_v1(nodes, 1024)?)?;
    Ok(())
}

impl SemanticFunctionLoweringV1<'_> {
    fn scalar_enum_result_local_v1(&self, local: u32) -> bool {
        self.control_flow_ssa
            .promoted
            .get(&local)
            .is_some_and(|promoted| {
                promoted.transport == SemanticPromotedTransportV1::ScalarEnumResult
            })
    }

    fn complete_scalar_enum_result_v1(
        &mut self,
        semantic_type: SemanticTypeIdV1,
        binding: SemanticValueBindingV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        if let Some(budget) = self.emission_work.as_deref_mut() {
            prepay_scalar_enum_emission_shape_v1(self.types, semantic_type, budget)?;
            emission_prepay_binding_scan_v1(&binding, budget)?;
        }
        let shape = scalar_enum_result_shape_v1(self.types, semantic_type)?;
        let SemanticValueBindingV1::Enum {
            discriminant,
            discriminant_ty,
            semantic_type: actual,
            variant,
            mut payloads,
        } = binding
        else {
            return Err(scalar_enum_result_producer_error_v1());
        };
        let (_, variants) = semantic_enum_shape(self.types, semantic_type)?;
        if actual != semantic_type
            || discriminant_ty != shape.components[0].kernel_type
            || variant.is_some_and(|variant| variant as usize >= variants.len())
            || payloads
                .keys()
                .any(|variant| *variant as usize >= variants.len())
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        for (variant_index, declaration) in variants.iter().enumerate() {
            let variant_index = variant_index as u32;
            let field_types = declaration.fields().fields();
            if let Some(fields) = payloads.get(&variant_index) {
                if fields.len() != field_types.len() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                for (field, field_type) in fields.iter().zip(field_types) {
                    let expected = lower_ssa_value_components_v1(self.types, *field_type)?;
                    let values = field
                        .values()
                        .map_err(|_| scalar_enum_result_producer_error_v1())?;
                    if values.len() != expected.len()
                        || values
                            .iter()
                            .zip(&expected)
                            .any(|((_, actual), (_, expected))| actual != expected)
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
                continue;
            }
            if variant.is_none() || variant == Some(variant_index) {
                return Err(scalar_enum_result_producer_error_v1());
            }
            // These words are defined carrier padding, not an initialized inactive variant.
            let mut fields = argument_vec_v1(field_types.len())?;
            for field_type in field_types {
                let expected = lower_ssa_value_components_v1(self.types, *field_type)?;
                let mut values = argument_vec_v1(expected.len())?;
                for (_, ty) in expected {
                    let zero = scalar_enum_result_zero_v1(&ty)?;
                    let (id, actual) = self
                        .emit(operations, ty, OperationKind::Constant(zero))?
                        .value()
                        .map_err(|_| scalar_enum_result_producer_error_v1())?;
                    values.push(ValueDef::new(id, actual));
                }
                fields.push(binding_from_value_defs_with_validation(
                    self.types,
                    *field_type,
                    &values,
                    false,
                )?);
            }
            payloads.insert(variant_index, fields);
        }
        let binding = SemanticValueBindingV1::Enum {
            discriminant,
            discriminant_ty,
            semantic_type,
            variant,
            payloads,
        };
        let expected = shape
            .components
            .iter()
            .map(|component| component.kernel_type.clone())
            .collect::<Vec<_>>();
        scalar_enum_result_values_v1(self.types, semantic_type, &binding, &expected)
            .map_err(|_| scalar_enum_result_producer_error_v1())?;
        Ok(binding)
    }
}
