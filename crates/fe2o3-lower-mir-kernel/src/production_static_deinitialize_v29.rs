impl SemanticFunctionLoweringV1<'_, '_> {
    fn lower_static_deinitialize_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let function = self.semantic_function.index();
        let error = || {
            unsupported(
                function,
                Some(block.index()),
                statement,
                "semantic deinitialize requires an exact promoted static source place",
            )
        };
        let local = self.require_local(block, statement, place.local().index())?;
        let site = execution_site_v29(block, statement);
        // No runtime store is implied by removing an SSA value. Retained
        // storage and execution-capability destruction need their own rules.
        self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(argument_sum_v1(&[
                6,
                argument_product_v1(place.projections().len(), 2)?,
            ])?)?;
            charge_execution_cfg_lookup_v29(this.control_flow_ssa.ssa_value_locals.len(), budget)?;
            if !this
                .control_flow_ssa
                .ssa_value_locals
                .contains(&place.local().index())
                || !place.projections().iter().all(|projection| {
                    matches!(projection.kind(), SemanticProjectionKindV1::Field(_))
                })
                || !scoped_source_place_v29(
                    this.function,
                    site,
                    ExecutionOperandV29::StatementPlace,
                )
                .is_some_and(|original| std::ptr::eq(original, place))
            {
                return Err(error());
            }
            let cursor = this
                .execution
                .as_mut()
                .ok_or_else(execution_availability_error_v29)?;
            cursor.check_ledger(budget)?;
            if !std::ptr::eq(cursor.function, this.function)
                || cursor.cfg.nominal_locals.get(local) != Some(&0)
            {
                return Err(execution_availability_error_v29());
            }
            let mut held = this
                .locals
                .get(local)
                .and_then(Option::as_ref)
                .ok_or_else(execution_availability_error_v29)?;
            for projection in place.projections() {
                let (
                    SemanticProjectionKindV1::Field(field),
                    SemanticValueBindingV1::Aggregate(fields),
                ) = (projection.kind(), held)
                else {
                    return Err(error());
                };
                held = fields
                    .get(field as usize)
                    .ok_or_else(execution_availability_error_v29)?;
            }
            if !static_deinitialize_binding_is_inert_v29(held, 0, budget)? {
                return Err(error());
            }
            // This is the existing owner's resolved original occurrence, not
            // a read reconstructed from type, physical values or metadata.
            let definition = cursor.use_place(
                site,
                ExecutionOperandV29::StatementPlace,
                place,
                false,
                budget,
            )?;
            check_execution_archive_v29(
                &this.locals,
                &this.semantic_ssa_bindings,
                place,
                definition,
                budget,
            )
        })?;
        if place.projections().is_empty() {
            self.locals[local] = None;
        }
        // Partial removals keep their physical sibling representation. The
        // source certificate and reference snapshots prohibit removed reads.
        Ok(())
    }
}

fn static_deinitialize_binding_is_inert_v29(
    binding: &SemanticValueBindingV1,
    depth: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    if depth > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Ok(false);
    }
    Ok(match binding {
        SemanticValueBindingV1::Unit | SemanticValueBindingV1::SourceReference(_) => true,
        SemanticValueBindingV1::Value { ty, .. } => matches!(
            ty,
            Type::Unit | Type::Scalar(_) | Type::Pointer(_) | Type::Vector(_)
        ),
        SemanticValueBindingV1::Aggregate(fields) => {
            for field in fields {
                if !static_deinitialize_binding_is_inert_v29(field, depth + 1, budget)? {
                    return Ok(false);
                }
            }
            true
        }
        _ => false,
    })
}

#[cfg(test)]
mod static_deinitialize_tests_v29 {
    use super::*;

    #[test]
    fn logical_removal_shape_is_closed_and_charges_before_visiting_children() {
        let scalar = || SemanticValueBindingV1::Value {
            id: ValueId(7),
            ty: Type::Scalar(ScalarType::U32),
        };
        let accepted = SemanticValueBindingV1::Aggregate(vec![
            SemanticValueBindingV1::Unit,
            SemanticValueBindingV1::Aggregate(vec![scalar(), SemanticValueBindingV1::Unit]),
        ]);
        for limit in [20, 19] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 43);
            budget.reserve_storage(43).unwrap();
            let result = static_deinitialize_binding_is_inert_v29(&accepted, 0, &mut budget);
            if limit == 20 {
                assert!(result.unwrap());
            } else {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
            }
            assert_eq!(budget.work(), if limit == 20 { 20 } else { 16 });
            assert_eq!(budget.storage(), 43);
        }
        for denied in [
            SemanticValueBindingV1::MathContext,
            SemanticValueBindingV1::CollectiveContext,
            SemanticValueBindingV1::MatrixContext,
            SemanticValueBindingV1::WorkgroupLdsScope,
            SemanticValueBindingV1::MovedExecution,
            SemanticValueBindingV1::Unmaterialized,
            SemanticValueBindingV1::Value {
                id: ValueId(8),
                ty: Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Context),
            },
        ] {
            let denied = SemanticValueBindingV1::Aggregate(vec![scalar(), denied]);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(12);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            assert!(!static_deinitialize_binding_is_inert_v29(&denied, 0, &mut budget).unwrap());
            assert_eq!(budget.work(), 12);
            assert_eq!(budget.storage(), 0);
        }
    }
}
