// Paid original-assignment locators, captured before temporary archives expire.
// Unmodeled whole bindings are explicit refusals in the new scalar proof query.
include!("production_source_descriptor_operand_v30.rs");
include!("production_source_index_computation_v35.rs");

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
    descriptor: Option<SourceDescriptorOperandV30>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceSsaRowV30 {
    instance: usize,
    original: SsaValueV1,
    endpoint: SourceRvalueEndpointV30,
}

struct OwnedSourceRvaluesV30 {
    source: ExecutionCallSourceV29,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    rows: Vec<SourceRvalueRowV30>,
    values: Vec<SourceSsaRowV30>,
    index_readers: Vec<SourceIndexReaderRowV35>,
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
        h::<Vec<SourceSsaRowV30>>()?,
        h::<SourceSsaRowV30>()?,
        h::<&SourceSsaRowV30>()?,
        h::<SsaValueV1>()?,
        h::<(usize, SsaValueV1)>()?,
        h::<&fe2o3_pliron::ProductionSemanticSsaFunctionPlanV1>()?,
        h::<std::collections::btree_map::Iter<'_, SsaValueV1, Box<SemanticValueBindingV1>>>()?,
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
        h::<(
            &SourceRvalueRowV30,
            &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        )>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()?,
        h::<Option<usize>>()?,
        h::<Type>()?,
        h::<(usize, u32, u32)>()?,
        h::<ExecutionCallSourceV29>()?,
        h::<ExecutionSiteV29>()?,
        source_descriptor_operand_headers_v30()?,
        source_index_reader_headers_v35()?,
        h::<std::slice::Iter<'_, SourceRvalueRowV30>>()?,
        argument_product_v1(8, h::<usize>()?)?,
        h::<()>()?,
    ])
}

fn retain_source_rvalues_v30(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    check_root_execution_archives_v29(pending, instances, plan, budget)?;
    if pending.rvalue_results.is_some() {
        return Err(execution_archive_error_v29());
    }
    let floor = budget.storage();
    budget.reserve_storage(source_rvalue_headers_v30()?)?;
    let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
    let mut count = 0;
    let mut value_count = 0;
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
        value_count = argument_sum_v1(&[value_count, archive.bindings.len()])?;
    }
    let mut rows = emission_vec_v1(count, budget)?;
    let mut values = emission_vec_v1(value_count, budget)?;
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
        // The complete archive has already passed original instance/plan and
        // definition-site checks. Preserve its exact sorted SSA identities
        // before the temporary owning bindings are destroyed.
        archive.check_original_v29(instances, instance, budget)?;
        for (original, binding) in &archive.bindings.owned {
            budget.charge_work(3)?;
            if values.len() == values.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            values.push(SourceSsaRowV30 {
                instance: ordinal,
                original: *original,
                endpoint: source_rvalue_endpoint_v30(binding),
            });
        }
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
                let descriptor = retain_source_descriptor_operand_v30(
                    instances, instance, archive, site, assignment, budget,
                )?;
                if rows.len() == rows.capacity() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                rows.push(SourceRvalueRowV30 {
                    instance: ordinal,
                    block,
                    statement,
                    ty,
                    endpoint: source_rvalue_endpoint_v30(binding),
                    descriptor,
                });
            }
        }
        if rows.len().checked_sub(start) != Some(archive.rvalues.len()) {
            return Err(execution_archive_error_v29());
        }
    }
    if rows.len() != count || values.len() != value_count {
        return Err(execution_archive_error_v29());
    }
    let index_readers = retain_source_index_readers_v35(pending, instances, references, budget)?;
    let storage = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let total = argument_sum_v1(&[pending.additional_storage_bytes, storage])?;
    pending.rvalue_results = Some(OwnedSourceRvaluesV30 {
        source,
        ledger: budget.work_ledger_identity_v1(),
        rows,
        values,
        index_readers,
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
            argument_product_v1(
                argument_sum_v1(&[self.values.len(), other.values.len()])?,
                size_of::<SourceSsaRowV30>(),
            )?,
            argument_product_v1(
                argument_sum_v1(&[self.index_readers.len(), other.index_readers.len()])?,
                size_of::<SourceIndexReaderRowV35>(),
            )?,
            argument_product_v1(2, size_of::<ExecutionCallSourceV29>())?,
            argument_product_v1(
                2,
                size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
            )?,
            4,
        ])?)?;
        Ok(self.source == other.source
            && self.ledger == other.ledger
            && self.rows == other.rows
            && self.values == other.values
            && self.index_readers == other.index_readers)
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Returns one exact original SSA value's scalar canonical definition, or
    /// None for its whole Unit binding. This is a source-owned emission locator,
    /// not a semantic/type-equivalence or memory-provenance proof. A consumer
    /// must independently compare the original source type and interpretation.
    pub fn ssa_scalar_definition_v30(
        &self,
        root: usize,
        instance: usize,
        original: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        let result = self.ssa_scalar_contents_v30(root, instance, original, budget);
        self.retain_query(result)
    }

    fn ssa_scalar_contents_v30(
        &self,
        root: usize,
        instance: usize,
        original: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        self.query(budget)?;
        let owner = self.source.root_row(root)?;
        let results =
            owner
                .rvalue_results
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original SSA result roster is absent",
                ))?;
        budget.charge_work(8)?;
        let source = self.source.source_ssa(budget)?;
        if results.source.semantic != *source.source_semantic_sha256()
            || results.source.ssa != source.identity()
            || results.source.root != owner.coordinates.root
            || results.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < results.storage
        {
            return self.source.missing("original SSA result owner differs");
        }
        let (function, _) = self.source.instance(root, instance, budget)?;
        if !self.source.instance_active(root, instance, budget)? {
            return self
                .source
                .missing("original SSA result instance is inactive");
        }
        let plan = source
            .plan_for_function(function)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "original SSA result plan is absent",
            ))?
            .plan();
        match original {
            SsaValueV1::Definition(value) if value.get() as usize >= plan.definition_count() => {
                return self
                    .source
                    .missing("original SSA result definition is foreign");
            }
            SsaValueV1::BlockArgument { block, variable } => {
                let variables = plan.transport_variables(block).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "original SSA result block is absent",
                    ),
                )?;
                budget.charge_work(variables.len().checked_ilog2().unwrap_or(0) as usize + 2)?;
                if variables.binary_search(&variable).is_err() {
                    return self
                        .source
                        .missing("original SSA result block argument is foreign");
                }
            }
            _ => {}
        }
        budget.charge_work(argument_product_v1(
            results.values.len().checked_ilog2().unwrap_or(0) as usize + 2,
            16,
        )?)?;
        let key = (instance, original);
        let index = results
            .values
            .binary_search_by_key(&key, |row| (row.instance, row.original))
            .map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding("original SSA result is missing")
            })?;
        match results.values[index].endpoint {
            SourceRvalueEndpointV30::Unmodeled => self
                .source
                .missing("original SSA whole binding is not scalar"),
            SourceRvalueEndpointV30::Unit => Ok(None),
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
                        "original SSA canonical result is absent",
                    ))?;
                if self.inventory.definitions()[index].ty != &Type::Scalar(scalar) {
                    return self
                        .source
                        .missing("original SSA canonical result type differs");
                }
                Ok(Some(index))
            }
        }
    }

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

    fn assignment_result_row_v30(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        &SourceRvalueRowV30,
        &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    )> {
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
        Ok((row, assignment))
    }

    fn assignment_scalar_contents_v30(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        let (row, _) = self.assignment_result_row_v30(root, instance, block, statement, budget)?;
        let source = self.source.source_ssa(budget)?;
        let owner = self.source.root_row(root)?;
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
