// Paid original-assignment locators, captured before temporary archives expire.
// Unmodeled whole bindings are explicit refusals in the new scalar proof query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceRvalueEndpointV30 {
    Unit,
    Scalar { value: ValueId, scalar: ScalarType },
    Unmodeled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceRvalueRowV30 {
    instance: usize,
    block: u32,
    statement: u32,
    ty: SemanticTypeIdV1,
    endpoint: SourceRvalueEndpointV30,
}

struct OwnedSourceRvaluesV30 {
    source: ExecutionCallSourceV29,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    rows: Vec<SourceRvalueRowV30>,
    storage: usize,
}

fn source_rvalue_endpoint_v30(binding: &SemanticValueBindingV1) -> SourceRvalueEndpointV30 {
    match binding {
        SemanticValueBindingV1::Unit => SourceRvalueEndpointV30::Unit,
        SemanticValueBindingV1::Value {
            id,
            ty: Type::Scalar(scalar),
        } => SourceRvalueEndpointV30::Scalar {
            value: *id,
            scalar: *scalar,
        },
        _ => SourceRvalueEndpointV30::Unmodeled,
    }
}

fn source_rvalue_headers_v30() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<OwnedSourceRvaluesV30>()?,
        h::<Vec<SourceRvalueRowV30>>()?,
        h::<SourceRvalueRowV30>()?,
        h::<SourceRvalueEndpointV30>()?,
        h::<&ExecutionArchiveV29>()?,
        h::<&ExecutionInstancesV29<'_>>()?,
        h::<&SourceReferencePlanV29<'_, '_>>()?,
        h::<&mut PendingScopedRootEmissionV29>()?,
        h::<&mut ArgumentBudgetV1<'_>>()?,
        h::<&ProductionSourceCorrespondenceV18<'_>>()?,
        h::<&SemanticValueBindingV1>()?,
        h::<&ScopedModuleRootV29>()?,
        h::<&OwnedSourceRvaluesV30>()?,
        h::<&SourceRvalueRowV30>()?,
        h::<&SemanticFunctionDeclV1>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()?,
        h::<Option<usize>>()?,
        h::<Type>()?,
        h::<(usize, u32, u32)>()?,
        h::<ExecutionCallSourceV29>()?,
        h::<ExecutionSiteV29>()?,
        h::<std::slice::Iter<'_, SourceRvalueRowV30>>()?,
        argument_product_v1(8, h::<usize>()?)?,
        h::<()>()?,
    ])
}

fn retain_source_rvalues_v30(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_root_execution_archives_v29(pending, instances, plan, budget)?;
    if pending.rvalue_results.is_some() {
        return Err(execution_archive_error_v29());
    }
    let floor = budget.storage();
    budget.reserve_storage(source_rvalue_headers_v30()?)?;
    let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
    let mut count = 0;
    for selected in &pending.active_instances.rows {
        budget.charge_work(1)?;
        let Some(selected) = *selected else { continue };
        let archive = pending
            .sidecars
            .rows
            .get(selected)
            .and_then(|row| row.execution_observation.as_ref())
            .ok_or_else(execution_archive_error_v29)?;
        count = argument_sum_v1(&[count, archive.rvalues.len()])?;
    }
    let mut rows = emission_vec_v1(count, budget)?;
    for (ordinal, selected) in pending.active_instances.rows.iter().enumerate() {
        budget.charge_work(1)?;
        let Some(selected) = *selected else { continue };
        let instance = instances
            .id_at(ordinal)
            .ok_or_else(execution_archive_error_v29)?;
        let function = instances
            .instance(instance)
            .ok_or_else(execution_archive_error_v29)?
            .declaration();
        let archive = pending
            .sidecars
            .rows
            .get(selected)
            .and_then(|row| row.execution_observation.as_ref())
            .ok_or_else(execution_archive_error_v29)?;
        let start = rows.len();
        for (block, original) in function.blocks().iter().enumerate() {
            budget.charge_work(2)?;
            let block = u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            match instances.block_reachable(instance, SemanticBlockIdV1::from_index(block)) {
                Some(false) => continue,
                Some(true) => {}
                None => return Err(execution_archive_error_v29()),
            }
            for (statement, original) in original.statements().iter().enumerate() {
                budget.charge_work(2)?;
                let SemanticStatementKindV1::Assign(assignment) = original.kind() else {
                    continue;
                };
                let statement =
                    u32::try_from(statement).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                let site =
                    execution_site_v29(SemanticBlockIdV1::from_index(block), Some(statement));
                let ty = assignment.value().result_type();
                let binding =
                    archive.lookup_rvalue_original_v30(instances, instance, site, ty, budget)?;
                if rows.len() == rows.capacity() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                rows.push(SourceRvalueRowV30 {
                    instance: ordinal,
                    block,
                    statement,
                    ty,
                    endpoint: source_rvalue_endpoint_v30(binding),
                });
            }
        }
        if rows.len().checked_sub(start) != Some(archive.rvalues.len()) {
            return Err(execution_archive_error_v29());
        }
    }
    if rows.len() != count {
        return Err(execution_archive_error_v29());
    }
    let storage = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let total = argument_sum_v1(&[pending.additional_storage_bytes, storage])?;
    pending.rvalue_results = Some(OwnedSourceRvaluesV30 {
        source,
        ledger: budget.work_ledger_identity_v1(),
        rows,
        storage,
    });
    pending.additional_storage_bytes = total;
    Ok(())
}

impl OwnedSourceRvaluesV30 {
    fn matches_replay_v30(
        &self,
        other: &Self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(
                argument_sum_v1(&[self.rows.len(), other.rows.len()])?,
                size_of::<SourceRvalueRowV30>(),
            )?,
            argument_product_v1(2, size_of::<ExecutionCallSourceV29>())?,
            argument_product_v1(
                2,
                size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
            )?,
            4,
        ])?)?;
        Ok(self.source == other.source && self.ledger == other.ledger && self.rows == other.rows)
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Returns the exact captured assignment result in this inventory's dense
    /// definition roster, or None for Unit. This is an emission locator, not
    /// expression equivalence. Unsupported whole bindings fail explicitly.
    pub fn assignment_scalar_definition_v30(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        let result = self.assignment_scalar_contents_v30(root, instance, block, statement, budget);
        self.retain_query(result)
    }

    fn assignment_scalar_contents_v30(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        self.query(budget)?;
        let owner = self.source.root_row(root)?;
        let results =
            owner
                .rvalue_results
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original assignment result roster is absent",
                ))?;
        budget.charge_work(8)?;
        let source = self.source.source_ssa(budget)?;
        if results.source.semantic != *source.source_semantic_sha256()
            || results.source.ssa != source.identity()
            || results.source.root != owner.coordinates.root
            || results.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < results.storage
        {
            return self
                .source
                .missing("original assignment result owner differs");
        }
        let (function, _) = self.source.instance(root, instance, budget)?;
        if !self.source.instance_active(root, instance, budget)? {
            return self
                .source
                .missing("original assignment result instance is inactive");
        }
        let assignment = source
            .source_semantic()
            .functions()
            .get(function.index() as usize)
            .and_then(|function| function.blocks().get(block.index() as usize))
            .and_then(|block| block.statements().get(statement as usize))
            .and_then(|statement| match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => Some(assignment),
                _ => None,
            })
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "original assignment result site differs",
            ))?;
        budget.charge_work(argument_product_v1(
            results.rows.len().checked_ilog2().unwrap_or(0) as usize + 2,
            16,
        )?)?;
        let key = (instance, block.index(), statement);
        let index = results
            .rows
            .binary_search_by_key(&key, |row| (row.instance, row.block, row.statement))
            .map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding("original assignment result is missing")
            })?;
        let row = &results.rows[index];
        if row.ty != assignment.value().result_type() {
            return self
                .source
                .missing("original assignment result type differs");
        }
        match row.endpoint {
            SourceRvalueEndpointV30::Unmodeled => self
                .source
                .missing("original assignment whole binding is not scalar"),
            SourceRvalueEndpointV30::Unit => {
                if !matches!(
                    source
                        .source_semantic()
                        .types()
                        .get(row.ty.index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                    Some(fe2o3_mir_model::semantic_mir_v1::SemanticTypeShapeV1::Unit)
                ) {
                    return self.source.missing("original assignment Unit type differs");
                }
                Ok(None)
            }
            SourceRvalueEndpointV30::Scalar { value, scalar } => {
                let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                    u32::try_from(owner.function_ordinal)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                let index = self
                    .inventory
                    .definition_index_for_value(function, value, budget)
                    .map_err(|error| {
                        ProductionSourceOwnedViewErrorV18::from(
                            fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                        )
                    })?
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "original assignment canonical result is absent",
                    ))?;
                let actual = &self.inventory.definitions()[index];
                let expected = lower_scalar_type(source.source_semantic().types(), row.ty)
                    .map_err(|_| {
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "original assignment scalar type is not modeled",
                        )
                    })?;
                if actual.ty != &Type::Scalar(scalar) || actual.ty != &expected {
                    return self
                        .source
                        .missing("original assignment canonical result type differs");
                }
                Ok(Some(index))
            }
        }
    }
}
