// Exact emission locators for every original assignment, including assignments
// whose destination remains memory. These are not expression-equivalence proofs.
type ExecutionRvalueBindingsV30 = BTreeMap<(u32, u32), Box<ExecutionRvalueBindingV30>>;

struct ExecutionRvalueBindingV30 {
    ty: SemanticTypeIdV1,
    binding: SemanticValueBindingV1,
}

fn execution_rvalue_headers_v30() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            argument_product_v1(
                2,
                std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
            )?,
        ])
    }
    argument_sum_v1(&[
        h::<ExecutionArchiveCreditV29>()?,
        h::<Option<ExecutionArchiveCreditV29>>()?,
        h::<ExecutionSiteV29>()?,
        h::<(u32, u32)>()?,
        h::<SemanticTypeIdV1>()?,
        h::<SemanticValueBindingV1>()?,
        h::<Box<ExecutionRvalueBindingV30>>()?,
        h::<&ExecutionRvalueBindingV30>()?,
        h::<&SemanticValueBindingV1>()?,
        h::<&ExecutionArchiveV29>()?,
        h::<&ExecutionAvailabilityV29<'_>>()?,
        h::<&ExecutionInstancesV29<'_>>()?,
        h::<&SemanticFunctionDeclV1>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1>()?,
        h::<&mut ExecutionRvalueBindingsV30>()?,
        h::<&mut Option<ExecutionArchiveCreditV29>>()?,
        h::<&mut dyn SemanticEmissionBudgetV1>()?,
        h::<SourceIssuedActualValueV29<'_>>()?,
        h::<&Type>()?,
        h::<&Operation>()?,
        h::<ValueId>()?,
        argument_product_v1(4, h::<usize>()?)?,
        h::<()>()?,
    ])
}

fn execution_rvalue_key_v30(
    site: ExecutionSiteV29,
) -> Result<(u32, u32), ProductionSemanticKirErrorV1> {
    match site {
        ExecutionSiteV29::Statement { block, statement } => Ok((block.get(), statement)),
        ExecutionSiteV29::Terminator { .. } => Err(execution_archive_error_v29()),
    }
}

fn execution_rvalue_entry_storage_v30(count: usize) -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        execution_archive_map_growth_v30::<(u32, u32), Box<ExecutionRvalueBindingV30>>(count)?,
        std::mem::size_of::<ExecutionRvalueBindingV30>(),
        // Captures are sequential. One retained frame covers every append to
        // this map; per-entry rows and binding payloads remain separately paid.
        if count == 0 {
            execution_rvalue_headers_v30()?
        } else {
            0
        },
    ])
}

fn archive_owned_rvalue_v30(
    rvalues: &mut ExecutionRvalueBindingsV30,
    credit: &mut Option<ExecutionArchiveCreditV29>,
    site: ExecutionSiteV29,
    ty: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut owned = match *credit {
        Some(owned) => {
            owned.check(budget)?;
            owned
        }
        None => ExecutionArchiveCreditV29::new(budget)?,
    };
    let key = execution_rvalue_key_v30(site)?;
    charge_execution_cfg_lookup_v29(rvalues.len(), budget)?;
    if rvalues.contains_key(&key) {
        return Err(execution_archive_error_v29());
    }
    let bytes = execution_rvalue_entry_storage_v30(rvalues.len())?;
    let next = argument_sum_v1(&[owned.bytes, bytes])?;
    budget.reserve_storage(bytes)?;
    owned.bytes = next;
    *credit = Some(owned);
    let before = budget.storage();
    let binding = emission_clone_binding_v1(binding, budget)?;
    owned.check(budget)?;
    let payload = budget
        .storage()
        .checked_sub(before)
        .ok_or(ArgumentResourceV1::Accounting)?;
    owned.bytes = argument_sum_v1(&[owned.bytes, payload])?;
    *credit = Some(owned);
    rvalues.insert(key, Box::new(ExecutionRvalueBindingV30 { ty, binding }));
    Ok(())
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn archive_rvalue_result_v30(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        binding: &SemanticValueBindingV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.execution.is_none() {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this.execution.as_ref().ok_or_else(execution_archive_error_v29)?;
            cursor.check_ledger(budget)?;
            if let Some(references) = cursor.references {
                budget.source_reference_owner_v29(references.plan)?;
            }
            budget.charge_work(8)?;
            let site = execution_site_v29(block, statement);
            if !std::ptr::eq(cursor.function, this.function)
                || cursor.block != Some(SsaBlockIdV1::new(block.index()))
                || !cursor.source_block_reachable_v29(block, budget)?
                || !matches!(scoped_source_statement_v29(this.function, site),
                    Some(SemanticStatementKindV1::Assign(original)) if std::ptr::eq(original, assignment))
            {
                return Err(execution_archive_error_v29());
            }
            archive_owned_rvalue_v30(
                &mut this.semantic_rvalue_bindings,
                &mut this.semantic_ssa_archive_credit,
                site,
                assignment.value().result_type(),
                binding,
                budget,
            )
        })
    }
}

impl ExecutionArchiveV29 {
    fn lookup_rvalue_original_v30<'archive>(
        &'archive self,
        instances: &ExecutionInstancesV29<'_>,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'archive SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.check_original_v29(instances, instance, budget)?;
        let header = execution_rvalue_headers_v30()?;
        budget.reserve_storage(header)?;
        let result = self.lookup_rvalue_contents_v30(instances, instance, site, ty, budget);
        // No callback, allocation, or ledger mutation occurs in this lookup.
        budget.release_storage(header)?;
        result
    }

    fn lookup_rvalue_contents_v30<'archive>(
        &'archive self,
        instances: &ExecutionInstancesV29<'_>,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'archive SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        let function = instances
            .instance(instance)
            .ok_or_else(execution_archive_error_v29)?
            .declaration();
        let Some(SemanticStatementKindV1::Assign(assignment)) =
            scoped_source_statement_v29(function, site)
        else {
            return Err(execution_archive_error_v29());
        };
        if assignment.value().result_type() != ty {
            return Err(execution_archive_error_v29());
        }
        charge_execution_cfg_lookup_v29(self.rvalues.len(), budget)?;
        let record = self
            .rvalues
            .get(&execution_rvalue_key_v30(site)?)
            .ok_or_else(execution_archive_error_v29)?;
        if record.ty != ty {
            return Err(execution_archive_error_v29());
        }
        Ok(&record.binding)
    }
}

fn check_issued_assignment_binding_v30(
    binding: &SemanticValueBindingV1,
    value: ValueId,
    actual_type: &Type,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(6)?;
    if !matches!(binding, SemanticValueBindingV1::Value { id, ty }
        if *id == value && ty == actual_type)
        || !matches!(operation.kind, OperationKind::Store { value: stored, .. } if stored == value)
    {
        return Err(source_issued_error_v29());
    }
    Ok(())
}
