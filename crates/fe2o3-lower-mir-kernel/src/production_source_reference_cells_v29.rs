// Source-bound cell requirements. No row is an allocation or pointer permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceStorageV29 {
    PromotedOnly,
    ScalarCells,
}

// The complete production root always selects this cell-capable owner. The
// promoted-only constructor remains a private component-query compatibility API.
fn with_optional_source_reference_cell_plan_v29<'work, R>(
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl FnOnce(
        Option<&SourceReferencePlanV29<'_, '_>>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<R>(budget)?;
    if source_reference_borrows_present_v29(instances, budget)? {
        with_source_reference_storage_plan_v29(
            instances,
            SourceReferenceStorageV29::ScalarCells,
            budget,
            |plan, budget| consume(Some(plan), budget),
        )
    } else {
        consume(None, budget)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceBackingKindV29 {
    Scalar,
    Object(fe2o3_kernel_ir::StorageLayoutIdV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceScalarCellV29 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    ty: SemanticTypeIdV1,
    kind: SourceBackingKindV29,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceCellStrategyV29 {
    Promoted,
    Scalar(usize),
    Object(usize),
    NeedsStorage,
}

#[derive(Default)]
struct SourceReferenceCellsV29 {
    rows: Vec<SourceReferenceScalarCellV29>,
    strategies: Vec<SourceReferenceCellStrategyV29>,
    raw_origins: Vec<usize>,
    physical_backings: Vec<SourceReferencePhysicalBackingV29>,
}

include!("production_source_physical_backing_v29.rs");
include!("production_compiler_enum_reference_cells_v55.rs");

fn source_reference_cell_scratch_v29<T>(
    capacity: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    budget.reserve_storage(argument_product_v1(
        3,
        std::mem::size_of::<Result<Vec<T>, ProductionSemanticKirErrorV1>>(),
    )?)?;
    source_reference_scratch_v29(capacity, budget)
}

fn source_reference_group_root_v29(
    parents: &[usize],
    mut index: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    loop {
        budget.charge_work(2)?;
        let parent = *parents.get(index).ok_or(ArgumentResourceV1::Accounting)?;
        if parent == index {
            return Ok(index);
        }
        if parent > index {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        index = parent;
    }
}

fn source_reference_group_union_v29(
    parents: &mut [usize],
    left: usize,
    right: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let left = source_reference_group_root_v29(parents, left, budget)?;
    let right = source_reference_group_root_v29(parents, right, budget)?;
    parents[left.max(right)] = left.min(right);
    Ok(())
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn scalar_cell_candidate(
        &self,
        loan: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceReferenceScalarCellV29>, ProductionSemanticKirErrorV1> {
        budget.charge_work(12)?;
        let loan = self
            .plan
            .loans
            .get(loan)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let origin = &self.plan.origins[loan.origin];
        if !origin.projections.is_empty()
            || !matches!(
                loan.kind,
                SemanticBorrowKindV1::Shared | SemanticBorrowKindV1::Mutable
            )
        {
            return Ok(None);
        }
        let row = self
            .plan
            .instances
            .instance(origin.instance)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if row
            .declaration()
            .locals()
            .get(origin.local.index() as usize)
            .map(|local| local.ty())
            != Some(origin.ty)
        {
            return Ok(None);
        }
        let declaration =
            &self.plan.instances.owner().source_semantic().types()[origin.ty.index() as usize];
        if !matches!(
            declaration.shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        ) || !matches!(
            declaration.layout().backend_repr(),
            SemanticBackendReprV1::Scalar(_)
        ) || declaration.layout().is_uninhabited()
            || declaration
                .layout()
                .size_bytes()
                .is_none_or(|size| size == 0)
        {
            return Ok(None);
        }
        Ok(Some(SourceReferenceScalarCellV29 {
            instance: origin.instance,
            local: origin.local,
            generation: origin.generation,
            ty: origin.ty,
            kind: SourceBackingKindV29::Scalar,
        }))
    }

    fn plan_scalar_cells(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        use std::mem::size_of;
        // Local map/return envelopes coexist with the embedded paid owner rows.
        budget.reserve_storage(argument_sum_v1(&[
            size_of::<BTreeMap<(usize, u32, u32), usize>>(),
            argument_product_v1(2, size_of::<Result<Option<SourceReferenceScalarCellV29>, ProductionSemanticKirErrorV1>>())?,
            size_of::<Option<SourceReferenceScalarCellV29>>(),
            argument_product_v1(2, size_of::<Result<Vec<SourceReferenceCellStrategyV29>, ProductionSemanticKirErrorV1>>())?,
        ])?)?;
        let count = self.plan.loans.len();
        let mut parents = source_reference_cell_scratch_v29(count, budget)?;
        let mut required = source_reference_cell_scratch_v29(count, budget)?;
        let mut supported = source_reference_cell_scratch_v29(count, budget)?;
        budget.charge_work(argument_product_v1(count, 3)?)?;
        parents.extend(0..count);
        required.resize(count, false);
        supported.resize(count, true);
        let mut same_origin = BTreeMap::new();
        for index in 0..count {
            budget.charge_work(3)?;
            let origin = &self.plan.origins[self.plan.loans[index].origin];
            if origin.projections.is_empty() {
                let key = (
                    origin.instance.index(),
                    origin.local.index(),
                    origin.generation,
                );
                charge_execution_cfg_lookup_v29(same_origin.len(), budget)?;
                if let Some(&previous) = same_origin.get(&key) {
                    source_reference_group_union_v29(&mut parents, previous, index, budget)?;
                } else {
                    reserve_execution_cfg_map_entry_v29::<(usize, u32, u32), usize>(
                        same_origin.len(),
                        budget,
                    )?;
                    same_origin.insert(key, index);
                }
            }
        }
        // Physical call layouts are instance-qualified. Sharing a Rust function
        // does not join distinct original objects' representation strategies.
        for current in 0..self.plan.instances.instances().len() {
            budget.charge_work(2)?;
            let instance = self
                .plan
                .instances
                .id_at(current)
                .ok_or(ArgumentResourceV1::Accounting)?;
            match self.plan.instances.instance_reachable(instance) {
                Some(false)
                    if self.plan.entries[current].is_none()
                        && self.plan.returns[current].is_none() => {}
                Some(true) if self.plan.entries[current].is_some() => {}
                _ => return Err(source_reference_cfg_obligation_v29()),
            }
        }
        for index in 0..count {
            let root = source_reference_group_root_v29(&parents, index, budget)?;
            let candidate = self.scalar_cell_candidate(index, budget)?;
            budget.charge_work(2)?;
            required[root] |= matches!(
                self.plan.loans[index].representation,
                SourceReferenceRepresentationV29::NeedsAddressable(_)
            );
            supported[root] &= candidate.is_some();
        }
        // The same source storage can be reused across disjoint generations,
        // but every binding retains its generation and is checked at occurrence.
        same_origin.clear();
        self.plan.cells.strategies = emission_vec_v1(count, budget)?;
        for index in 0..count {
            let root = source_reference_group_root_v29(&parents, index, budget)?;
            let strategy = if !required[root] {
                SourceReferenceCellStrategyV29::Promoted
            } else if !supported[root] {
                SourceReferenceCellStrategyV29::NeedsStorage
            } else {
                let cell = self
                    .scalar_cell_candidate(index, budget)?
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let key = (cell.instance.index(), cell.local.index(), cell.generation);
                charge_execution_cfg_lookup_v29(same_origin.len(), budget)?;
                let cell_index = if let Some(&previous) = same_origin.get(&key) {
                    previous
                } else {
                    let next = self.plan.cells.rows.len();
                    emission_push_v1(&mut self.plan.cells.rows, cell, budget)?;
                    reserve_execution_cfg_map_entry_v29::<(usize, u32, u32), usize>(
                        same_origin.len(),
                        budget,
                    )?;
                    same_origin.insert(key, next);
                    next
                };
                SourceReferenceCellStrategyV29::Scalar(cell_index)
            };
            budget.charge_work(1)?;
            self.plan.cells.strategies.push(strategy);
        }
        if self.plan.storage_demands.is_some() {
            source_reference_select_backing_v29(self, &mut same_origin, budget)?;
            self.plan_physical_backings(&same_origin, budget)?;
        } else {
            self.plan_raw_scalar_cells(&mut same_origin, budget)?;
        }
        Ok(())
    }

    fn plan_raw_scalar_cells(
        &mut self,
        cells: &mut BTreeMap<(usize, u32, u32), usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.plan.raw_origins.is_empty() {
            return Ok(());
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<
                Result<Option<PrivateRetainedSlotFactsV1>, ProductionSemanticKirErrorV1>,
            >(),
            std::mem::size_of::<Option<PrivateRetainedSlotFactsV1>>(),
            std::mem::size_of::<SourceReferenceScalarCellV29>(),
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        ])?)?;
        self.plan.cells.raw_origins = emission_vec_v1(self.plan.raw_origins.len(), budget)?;
        let types = self.plan.instances.owner().source_semantic().types();
        for origin in &self.plan.raw_origins {
            budget.charge_work(7)?;
            let declaration = self
                .plan
                .instances
                .instance(origin.instance)
                .and_then(|instance| {
                    instance
                        .declaration()
                        .locals()
                        .get(origin.local.index() as usize)
                })
                .ok_or(ArgumentResourceV1::Accounting)?;
            if origin.count != 0
                || declaration.ty() != origin.ty
                || private_retained_slot_facts_v1(types, origin.ty, budget)?.is_none()
            {
                return Err(source_reference_error_v29(
                    "source raw address requires general table-aware backing",
                ));
            }
            let cell = SourceReferenceScalarCellV29 {
                instance: origin.instance,
                local: origin.local,
                generation: origin.generation,
                kind: SourceBackingKindV29::Scalar,
                ty: origin.ty,
            };
            let key = (cell.instance.index(), cell.local.index(), cell.generation);
            charge_execution_cfg_lookup_v29(cells.len(), budget)?;
            let index = if let Some(&index) = cells.get(&key) {
                if self.plan.cells.rows.get(index) != Some(&cell) {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                index
            } else {
                let index = self.plan.cells.rows.len();
                emission_push_v1(&mut self.plan.cells.rows, cell, budget)?;
                reserve_execution_cfg_map_entry_v29::<(usize, u32, u32), usize>(
                    cells.len(),
                    budget,
                )?;
                cells.insert(key, index);
                index
            };
            // Exactly one entry per original address occurrence/activation row.
            self.plan.cells.raw_origins.push(index);
        }
        Ok(())
    }
}

impl SourceReferencePlanV29<'_, '_> {
    // This is a common physical object coordinate, not an activation permit.
    // Every possible raw origin is checked; access uses the separate source
    // activation/expiry relation and final census joins every cell to one slot.
    fn raw_object_cell(
        &self,
        set: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self)?;
        budget.source_reference_charge_v29(self, 4)?;
        let object = self
            .raw_sets
            .get(set)
            .ok_or_else(execution_call_error_v29)?;
        let end = argument_sum_v1(&[object.first, object.count])?;
        if object.count == 0 || object.projection_count != 0 || end > self.raw_choices.len() {
            return Err(execution_call_error_v29());
        }
        let mut cell = None;
        let mut kind = None;
        for choice in &self.raw_choices[object.first..end] {
            budget.source_reference_charge_v29(self, 9)?;
            let source = self
                .raw_origins
                .get(choice.origin)
                .ok_or_else(execution_call_error_v29)?;
            let candidate = *self
                .cells
                .raw_origins
                .get(choice.origin)
                .ok_or_else(execution_call_error_v29)?;
            let row = self
                .cells
                .rows
                .get(candidate)
                .ok_or_else(execution_call_error_v29)?;
            if source.instance != object.instance
                || source.local != object.local
                || source.ty != object.ty
                || source.count != 0
                || row.instance != source.instance
                || row.local != source.local
                || row.ty != source.ty
                || row.generation != source.generation
                || kind.is_some_and(|previous| previous != row.kind)
            {
                return Err(execution_call_error_v29());
            }
            let candidate = if let SourceBackingKindV29::Object(schema) = row.kind {
                if !budget.source_object_storage_matches_v29(
                    self,
                    candidate,
                    row.instance,
                    row.local,
                    row.generation,
                    schema,
                    None,
                )? {
                    return Err(execution_call_error_v29());
                }
                let (representative, _, _) = self.physical_object_cell(candidate, budget)?;
                // Logical generations are still authenticated independently.
                // Only the checked same-local allocation family may share this
                // pending ABI origin; final currentness remains mandatory.
                if cell.is_some_and(|previous| previous != representative) {
                    return Err(execution_call_error_v29());
                }
                representative
            } else {
                candidate
            };
            kind = Some(row.kind);
            cell = Some(cell.map_or(candidate, |previous: usize| previous.min(candidate)));
        }
        cell.ok_or_else(execution_call_error_v29)
    }

    fn scalar_cell(
        &self,
        loan: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<(usize, SourceReferenceScalarCellV29)>, ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        self.charge(3, budget)?;
        if self.storage == SourceReferenceStorageV29::PromotedOnly {
            return Ok(None);
        }
        match self.cells.strategies.get(loan).copied() {
            Some(SourceReferenceCellStrategyV29::Promoted) => Ok(None),
            Some(SourceReferenceCellStrategyV29::Scalar(index)) => self
                .cells
                .rows
                .get(index)
                .copied()
                .filter(|cell| cell.kind == SourceBackingKindV29::Scalar)
                .map(|cell| Some((index, cell)))
                .ok_or_else(|| ArgumentResourceV1::Accounting.into()),
            Some(SourceReferenceCellStrategyV29::Object(_)) => Err(source_reference_error_v29(
                "typed object backing is not a scalar cell",
            )),
            Some(SourceReferenceCellStrategyV29::NeedsStorage) => Err(source_reference_error_v29(
                "source reference requires general addressable storage",
            )),
            None => Err(source_reference_error_v29(
                "source reference loan is outside its owner",
            )),
        }
    }

    fn backing_cell(
        &self,
        loan: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<(usize, SourceReferenceScalarCellV29)>, ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        self.charge(8, budget)?;
        if self.storage == SourceReferenceStorageV29::PromotedOnly {
            return Ok(None);
        }
        let strategy = self
            .cells
            .strategies
            .get(loan)
            .copied()
            .ok_or_else(execution_call_error_v29)?;
        let index = match strategy {
            SourceReferenceCellStrategyV29::Promoted => return Ok(None),
            SourceReferenceCellStrategyV29::Scalar(index)
            | SourceReferenceCellStrategyV29::Object(index) => index,
            SourceReferenceCellStrategyV29::NeedsStorage => {
                return Err(source_reference_error_v29(
                    "source reference requires an admitted selected backing schema",
                ));
            }
        };
        let row = *self
            .cells
            .rows
            .get(index)
            .ok_or_else(execution_call_error_v29)?;
        let source = self
            .loans
            .get(loan)
            .and_then(|loan| self.origins.get(loan.origin))
            .ok_or_else(execution_call_error_v29)?;
        let declaration = self
            .instances
            .instance(source.instance)
            .and_then(|instance| {
                instance
                    .declaration()
                    .locals()
                    .get(source.local.index() as usize)
            })
            .ok_or_else(execution_call_error_v29)?;
        if row.instance != source.instance
            || row.local != source.local
            || row.generation != source.generation
            || row.ty != declaration.ty()
            || !matches!(
                (strategy, row.kind),
                (
                    SourceReferenceCellStrategyV29::Scalar(_),
                    SourceBackingKindV29::Scalar
                ) | (
                    SourceReferenceCellStrategyV29::Object(_),
                    SourceBackingKindV29::Object(_)
                )
            )
        {
            return Err(execution_call_error_v29());
        }
        if let SourceBackingKindV29::Object(schema) = row.kind {
            let root = self
                .storage_root
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            root.source_layouts(self.instances, budget)?
                .check_selected_schema(self.instances.owner(), row.ty, schema, budget)?;
        }
        Ok(Some((index, row)))
    }
}

fn source_reference_append_cell_payload_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    output: &mut Vec<Option<usize>>,
    nodes: &mut usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)?;
    let row = plan.nodes.get(node).ok_or_else(execution_call_error_v29)?;
    match row.kind {
        SourceReferenceNodeKindV29::Absent => {
            return Err(source_reference_error_v29(
                "source reference cell payload cannot materialize an absent holder",
            ));
        }
        SourceReferenceNodeKindV29::Loan(loan) => {
            if let Some((cell, _)) = plan.backing_cell(loan, budget)? {
                source_reference_owned_push_v29(plan, output, Some(cell), budget)?;
            } else {
                for _ in source_reference_payload_types_v29(plan, loan, budget)? {
                    source_reference_owned_push_v29(plan, output, None, budget)?;
                }
            }
        }
        SourceReferenceNodeKindV29::Address(set) => {
            let cell = plan.raw_object_cell(set, budget)?;
            source_reference_owned_push_v29(plan, output, Some(cell), budget)?;
        }
        SourceReferenceNodeKindV29::Plain(_) | SourceReferenceNodeKindV29::Discriminant(_) => {
            for _ in source_execution_cfg_types_v29(
                plan.instances.owner().source_semantic().types(),
                row.ty,
                budget,
            )? {
                source_reference_owned_push_v29(plan, output, None, budget)?;
            }
        }
        SourceReferenceNodeKindV29::Enum { .. } => {
            let (_, alternative) = source_reference_enum_single_v29(plan, node, budget)?;
            source_reference_owned_push_v29(plan, output, None, budget)?;
            for offset in 0..alternative.count {
                budget.charge_work(1)?;
                let child = plan.children[argument_sum_v1(&[alternative.first, offset])?];
                source_reference_append_cell_payload_v29(plan, child, output, nodes, budget)?;
            }
        }
        SourceReferenceNodeKindV29::EnumView(_) => {
            return Err(source_reference_error_v29(
                "correlated enum payload requires checked choice transport",
            ));
        }
        SourceReferenceNodeKindV29::Aggregate { first, count } => {
            for offset in 0..count {
                budget.charge_work(1)?;
                let child = *plan
                    .children
                    .get(argument_sum_v1(&[first, offset])?)
                    .ok_or_else(execution_call_error_v29)?;
                if child >= node {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                source_reference_append_cell_payload_v29(plan, child, output, nodes, budget)?;
            }
        }
    }
    Ok(())
}

fn source_reference_cell_parameters_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    root_arity: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<Option<usize>>, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let mut cells = source_reference_owned_vec_v29(plan, root_arity, budget)?;
    if instance == plan.root {
        budget.charge_work(root_arity)?;
        cells.resize(root_arity, None);
        return Ok(cells);
    }
    let row = plan
        .instances
        .instance(instance)
        .ok_or_else(execution_call_error_v29)?;
    let semantic = plan.instances.owner().source_semantic();
    for (ordinal, local) in row.declaration().locals().iter().enumerate() {
        budget.charge_work(1)?;
        if !local.role().is_entry_argument() {
            continue;
        }
        let id = SemanticLocalIdV1::from_index(
            u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        let selector = plan
            .instances
            .parameter_source(instance, id, budget)
            .map_err(|error| match error {
                production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_call_error_v29(),
            })?;
        if execution_direct_parameter_v29(
            semantic,
            row.function(),
            selector.source_argument,
            selector.tuple_field,
            selector.ty,
            budget,
        )?
        .is_some()
        {
            source_reference_owned_push_v29(plan, &mut cells, None, budget)?;
            continue;
        }
        let node = source_reference_entry_node_v29(plan, instance, id, None, budget)?
            .ok_or_else(execution_call_error_v29)?;
        if plan.nodes[node].ty != selector.ty {
            return Err(execution_call_error_v29());
        }
        source_reference_append_cell_payload_v29(plan, node, &mut cells, &mut 0, budget)?;
    }
    Ok(cells)
}

impl SourceReferenceCellPointerProofV29<'_, '_, '_> {
    fn bind_operand(
        &mut self,
        cell: usize,
        key: SourceReferenceCellOperandKeyV29,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.require_payload_backing(cell, (key.0, value), budget)?;
        reserve_execution_cfg_map_entry_v29::<SourceReferenceCellOperandKeyV29, usize>(
            self.operands.len(),
            budget,
        )?;
        if self.operands.insert(key, cell).is_some() {
            return Err(execution_call_error_v29());
        }
        Ok(())
    }

    fn check_payloads(
        &mut self,
        emitted: &[Option<LoweredFunctionResultV1>],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let plan = self.plan;
        for (ordinal, lowered) in emitted.iter().enumerate() {
            budget.charge_work(4)?;
            let instance = plan
                .instances
                .id_at(ordinal)
                .ok_or_else(execution_call_error_v29)?;
            let Some(lowered) =
                source_reference_active_emitted_v29(plan, ordinal, lowered, budget)?
            else {
                continue;
            };
            let body = lowered
                .function
                .body
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            let cells = source_reference_cell_parameters_v29(
                plan,
                instance,
                body.parameters.len(),
                budget,
            )?;
            if cells.len() != body.parameters.len() {
                return Err(execution_call_error_v29());
            }
            for (component, (&value, cell)) in body.parameters.iter().zip(cells).enumerate() {
                budget.charge_work(2)?;
                if let Some(cell) = cell {
                    self.require_payload_backing(cell, (ordinal, value), budget)?;
                    let (caller, block, operation, call) =
                        self.index.calls[ordinal].ok_or_else(execution_call_error_v29)?;
                    let OperationKind::Call { arguments, .. } = &call.kind else {
                        return Err(execution_call_error_v29());
                    };
                    self.bind_operand(
                        cell,
                        (caller, block, Some(operation), component),
                        *arguments
                            .get(component)
                            .ok_or_else(execution_call_error_v29)?,
                        budget,
                    )?;
                } else {
                    self.require_not_cell((ordinal, value), budget)?;
                }
            }
            let mut results = source_reference_owned_vec_v29(plan, 0, budget)?;
            if let Some(node) = plan.returns.get(ordinal).copied().flatten()
                && (source_reference_node_has_loan_v29(plan, node, budget)?
                    || source_reference_node_has_selected_pointer_v29(plan, node, budget)?)
            {
                // Raw selected pointers are Address nodes, not Loans. Bind
                // their original return payload; a None slot cannot certify it.
                source_reference_append_cell_payload_v29(plan, node, &mut results, &mut 0, budget)?;
            } else {
                for _ in &lowered.function.signature.results {
                    source_reference_owned_push_v29(plan, &mut results, None, budget)?;
                }
            }
            if results.len() != lowered.function.signature.results.len() {
                return Err(execution_call_error_v29());
            }
            for block in &body.blocks {
                budget.charge_work(1)?;
                if let Some(Terminator::Return { values }) = &block.terminator {
                    if values.len() != results.len() {
                        return Err(execution_call_error_v29());
                    }
                    for (component, (&value, cell)) in values.iter().zip(&results).enumerate() {
                        budget.charge_work(1)?;
                        if let Some(cell) = *cell {
                            self.bind_operand(
                                cell,
                                (ordinal, block.id, None, component),
                                value,
                                budget,
                            )?;
                        } else {
                            self.require_not_cell((ordinal, value), budget)?;
                        }
                    }
                }
            }
            if let Some((caller, _, _, operation)) = self.index.calls[ordinal] {
                if operation.results.len() != results.len() {
                    return Err(execution_call_error_v29());
                }
                for (value, cell) in operation.results.iter().zip(results) {
                    budget.charge_work(1)?;
                    if let Some(cell) = cell {
                        self.require_payload_backing(cell, (caller, value.id), budget)?;
                    } else {
                        self.require_not_cell((caller, value.id), budget)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn check_all_uses(
        &self,
        emitted: &[Option<LoweredFunctionResultV1>],
        slots: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        for (instance, lowered) in emitted.iter().enumerate() {
            let Some(lowered) =
                source_reference_active_emitted_v29(self.plan, instance, lowered, budget)?
            else {
                continue;
            };
            let body = lowered
                .function
                .body
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            with_canonical_call_scratch_v1(budget, |budget| {
                let mut prologue = None;
                for block in &body.blocks {
                    budget.charge_work(1)?;
                    for (position, operation) in block.operations.iter().enumerate() {
                        budget.charge_work(1)?;
                        let mut ordinal = 0_usize;
                        operation.kind.try_visit_operands(|value| {
                            let component = ordinal;
                            ordinal = argument_sum_v1(&[ordinal, 1])?;
                            let Some(slot) = self.cell_slot((instance, value), budget)? else {
                                return Ok(());
                            };
                            if self.operand(
                                (instance, block.id, Some(position), component),
                                value,
                                budget,
                            )? {
                                return Ok(());
                            }
                            match &operation.kind {
                                OperationKind::Load { .. } | OperationKind::Store { .. }
                                    if component == 0 =>
                                {
                                    self.ordinary_local_memory(
                                        &mut prologue,
                                        slots,
                                        instance,
                                        lowered,
                                        block.id,
                                        position,
                                        operation,
                                        slot,
                                        budget,
                                    )
                                }
                                OperationKind::Cast {
                                    kind: CastKind::RestrictPointerAccess,
                                    value: source,
                                    to,
                                } if component == 0 && *source == value => {
                                    let [result] = operation.results.as_slice() else {
                                        return Err(execution_call_error_v29());
                                    };
                                    let (Type::Pointer(from), _) =
                                        source_reference_pointer_definition_lookup_v29(
                                            &self.index.definitions,
                                            (instance, value),
                                            budget,
                                        )?
                                        .ok_or_else(execution_call_error_v29)?
                                    else {
                                        return Err(execution_call_error_v29());
                                    };
                                    let Type::Pointer(to_pointer) = to else {
                                        return Err(execution_call_error_v29());
                                    };
                                    budget.charge_work(7)?;
                                    if from.address_space != AddressSpace::Private
                                        || to_pointer.address_space != AddressSpace::Private
                                        || from.access != AccessMode::ReadWrite
                                        || to_pointer.access != AccessMode::ReadOnly
                                        || !matches!(from.pointee.as_ref(), Type::Scalar(_))
                                        || from.pointee != to_pointer.pointee
                                        || result.ty != *to
                                        || self.cell_slot((instance, result.id), budget)?
                                            != Some(slot)
                                    {
                                        return Err(execution_call_error_v29());
                                    }
                                    Ok(())
                                }
                                _ => Err(source_reference_error_v29(
                                    "source reference cell pointer has an unbound emitted use",
                                )),
                            }
                        })?;
                    }
                    let terminator = block
                        .terminator
                        .as_ref()
                        .ok_or_else(execution_call_error_v29)?;
                    match terminator {
                        Terminator::Return { values } => {
                            for (component, &value) in values.iter().enumerate() {
                                budget.charge_work(1)?;
                                if self.cell_slot((instance, value), budget)?.is_some()
                                    && !self.operand(
                                        (instance, block.id, None, component),
                                        value,
                                        budget,
                                    )?
                                {
                                    return Err(execution_call_error_v29());
                                }
                            }
                        }
                        Terminator::ConditionalBranch { condition, .. } => {
                            self.require_not_cell((instance, *condition), budget)?
                        }
                        Terminator::Switch { selector, .. }
                        | Terminator::IntegerSwitch { selector, .. } => {
                            self.require_not_cell((instance, *selector), budget)?
                        }
                        Terminator::Branch { .. } | Terminator::Unreachable => {}
                    }
                    terminator.try_visit_edges_v1(|target, arguments| {
                        budget.charge_work(body.blocks.len())?;
                        let target = body
                            .blocks
                            .iter()
                            .find(|block| block.id == target)
                            .ok_or_else(execution_call_error_v29)?;
                        if target.parameters.len() != arguments.len() {
                            return Err(execution_call_error_v29());
                        }
                        for (&value, parameter) in arguments.iter().zip(&target.parameters) {
                            budget.charge_work(1)?;
                            let from = self.cell_slot((instance, value), budget)?;
                            let to = self.cell_slot((instance, parameter.id), budget)?;
                            if from != to {
                                return Err(execution_call_error_v29());
                            }
                            if from.is_some() {
                                let (ty, _) = source_reference_pointer_definition_lookup_v29(
                                    &self.index.definitions,
                                    (instance, value),
                                    budget,
                                )?
                                .ok_or_else(execution_call_error_v29)?;
                                if !call_splice_type_eq_v1(ty, &parameter.ty, budget).map_err(
                                    |error| match error {
                                        CallInstanceEmissionErrorV1::Resource(error) => {
                                            error.into()
                                        }
                                        _ => execution_call_error_v29(),
                                    },
                                )? {
                                    return Err(execution_call_error_v29());
                                }
                            }
                        }
                        Ok(())
                    })?;
                }
                Ok(())
            })
            .inspect_err(|error| source_reference_record_failure_v29(self.plan, error))?;
        }
        Ok(())
    }
}

fn checked_source_reference_cell_pointers_v29<'plan, 'source, 'kir>(
    references: &SourceReferenceEmissionV29<'plan, 'source>,
    emitted: &'kir [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceReferenceCellPointerProofV29<'plan, 'source, 'kir>, ProductionSemanticKirErrorV1>
{
    let mut proof = pending_source_reference_cell_claims_v29(
        references,
        emitted,
        slots,
        SourceReferencePointerClaimsV29::ScalarOnly,
        budget,
    )?;
    let result = proof.check_all_uses(emitted, slots, budget);
    result.inspect_err(|error| source_reference_record_failure_v29(references.plan, error))?;
    proof.floor = budget.storage();
    Ok(proof)
}

fn check_pending_source_reference_claims_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // The internal origin hypotheses are used only to check original claims.
    // They never escape as the ordinary checked pointer proof on this route.
    let pending = pending_source_reference_cell_claims_v29(
        references,
        emitted,
        slots,
        SourceReferencePointerClaimsV29::SelectedBacking,
        budget,
    )?;
    drop(pending);
    Ok(())
}

fn pending_source_reference_cell_claims_v29<'plan, 'source, 'kir>(
    references: &SourceReferenceEmissionV29<'plan, 'source>,
    emitted: &'kir [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    claims: SourceReferencePointerClaimsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceReferenceCellPointerProofV29<'plan, 'source, 'kir>, ProductionSemanticKirErrorV1>
{
    references.check(budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<SourceReferenceCellPointerProofV29<'_, '_, '_>>(
            references.plan,
            budget,
        )?;
        let mut proof = match claims {
            SourceReferencePointerClaimsV29::ScalarOnly => {
                source_reference_cell_pointer_proof_v29(references, emitted, slots, budget)?
            }
            SourceReferencePointerClaimsV29::SelectedBacking => {
                source_reference_backing_pointer_claims_v29(
                    references, emitted, slots, claims, budget,
                )?
            }
        };
        proof.check_payloads(emitted, budget)?;
        proof.check_source_accesses(references, emitted, budget)?;
        proof.check_compiler_enum_reference_stores_v55(references, emitted, budget)?;
        proof.selectors = source_reference_owned_vec_v29(
            references.plan,
            references.plan.selectors.len(),
            budget,
        )?;
        references.check_source_selectors(emitted, &mut proof.selectors, budget)?;
        proof.floor = budget.storage();
        Ok(proof)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(references.plan, error))
}
