include!("production_source_backing_equations_v29.rs");
include!("production_source_boundary_schemas_v29.rs");

impl SourceBackingEquationsV29 {
    fn propagate_original(
        &mut self,
        types: &[SemanticTypeDeclV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.reserve_storage(2 * std::mem::size_of::<Vec<usize>>())?;
        let mut pending = emission_vec_v1(self.rows.len(), budget)?;
        for index in 0..self.rows.len() {
            budget.charge_work(1)?;
            if self.rows[index].parent == index && self.rows[index].original {
                pending.push(index);
            }
        }
        while let Some(index) = pending.pop() {
            budget.charge_work(2)?;
            if let SemanticTypeShapeV1::Pointer(pointer) =
                types[self.rows[index].ty.index() as usize].shape()
            {
                let original_space = lower_address_space(pointer.address_space())?;
                if original_space != AddressSpace::Generic
                    && self.rows[index]
                        .space
                        .is_some_and(|actual| actual != original_space)
                {
                    return Err(source_reference_error_v29(
                        "recursive backing cannot discard an incompatible original pointer representation",
                    ));
                }
                if let Some(pointee) = self.rows[index].pointee {
                    let pointee = self.root(pointee, budget)?;
                    let expected = match pointer.metadata() {
                        SemanticPointerMetadataV1::None => pointer.pointee(),
                        SemanticPointerMetadataV1::SliceLength => {
                            match types[pointer.pointee().index() as usize].shape() {
                                SemanticTypeShapeV1::Slice { element } => *element,
                                _ => return Err(source_backing_error_v29()),
                            }
                        }
                        _ => return Err(source_backing_error_v29()),
                    };
                    if self.rows[pointee].ty != expected || self.rows[pointee].variant.is_some() {
                        return Err(source_backing_error_v29());
                    }
                }
                self.rows[index].space =
                    Self::join_space(self.rows[index].space, Some(original_space));
            }
            // Only existing finite equations are visited. Original recursive
            // pointer types remain closed original rows; no type-tree expansion.
            let count = self.rows[index].children.len();
            let mut children = emission_vec_v1(argument_sum_v1(&[count, 1])?, budget)?;
            budget.charge_work(count)?;
            children.extend(self.rows[index].children.values().copied());
            if let Some(pointee) = self.rows[index].pointee {
                children.push(pointee);
            }
            for child in children {
                let child = self.root(child, budget)?;
                if !self.rows[child].original {
                    self.rows[child].original = true;
                    emission_push_v1(&mut pending, child, budget)?;
                }
            }
        }
        Ok(())
    }

    fn materialize(
        &mut self,
        plan: &SourceReferencePlanV29<'_, '_>,
        layouts: &source_storage_v29::SourceStorageLayoutsV29<'_>,
        equation: usize,
        selected: &mut Vec<Option<fe2o3_kernel_ir::StorageLayoutIdV1>>,
        colors: &mut Vec<u8>,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<fe2o3_kernel_ir::StorageLayoutIdV1, ProductionSemanticKirErrorV1> {
        use source_storage_v29::SourceStorageSelectionV29 as Selection;
        if depth >= 256 {
            return Err(source_reference_error_v29(
                "selected backing schema exceeds its typed depth bound",
            ));
        }
        let equation = self.root(equation, budget)?;
        while selected.len() < self.rows.len() {
            emission_push_v1(selected, None, budget)?;
            emission_push_v1(colors, 0, budget)?;
        }
        if let Some(id) = selected[equation] {
            return Ok(id);
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<(u32, fe2o3_kernel_ir::StorageLayoutIdV1)>>(),
            std::mem::size_of::<Selection<'_>>(),
            std::mem::size_of::<Result<fe2o3_kernel_ir::StorageLayoutIdV1, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        if colors[equation] != 0 {
            return Err(source_reference_error_v29(
                "selected backing equations contain an unclosed recursive object representation",
            ));
        }
        colors[equation] = 1;
        let owner = plan.instances.owner();
        let types = owner.source_semantic().types();
        let ty = self.rows[equation].ty;
        let variant = self.rows[equation].variant;
        let declaration = types
            .get(ty.index() as usize)
            .ok_or_else(source_backing_error_v29)?;
        let original_space = match declaration.shape() {
            SemanticTypeShapeV1::Pointer(pointer) => {
                Some(lower_address_space(pointer.address_space())?)
            }
            _ => None,
        };
        if variant.is_none()
            && (self.rows[equation].original
                || (self.rows[equation].children.is_empty()
                    && self.rows[equation].pointee.is_none()
                    && (self.rows[equation].space.is_none()
                        || self.rows[equation].space == original_space)))
        {
            let id = match layouts.original_schema(owner, ty, budget)? {
                Some(id) => id,
                None => layouts.select_original_closure(owner, ty, budget)?,
            };
            colors[equation] = 2;
            selected[equation] = Some(id);
            return Ok(id);
        }
        let id = match (declaration.shape(), variant) {
            (SemanticTypeShapeV1::Pointer(pointer), None) => {
                let pointee_ty = match pointer.metadata() {
                    SemanticPointerMetadataV1::None => pointer.pointee(),
                    SemanticPointerMetadataV1::SliceLength => {
                        match types[pointer.pointee().index() as usize].shape() {
                            SemanticTypeShapeV1::Slice { element } => *element,
                            _ => return Err(source_backing_error_v29()),
                        }
                    }
                    _ => return Err(source_backing_error_v29()),
                };
                let pointee = if let Some(pointee) = self.rows[equation].pointee {
                    let pointee = self.root(pointee, budget)?;
                    if self.rows[pointee].ty != pointee_ty || self.rows[pointee].variant.is_some() {
                        return Err(source_backing_error_v29());
                    }
                    self.materialize(plan, layouts, pointee, selected, colors, depth + 1, budget)?
                } else {
                    match layouts.original_schema(owner, pointee_ty, budget)? {
                        Some(id) => id,
                        None => layouts.select_original_closure(owner, pointee_ty, budget)?,
                    }
                };
                let value_space = self.rows[equation]
                    .space
                    .or(original_space)
                    .ok_or_else(source_backing_error_v29)?;
                let access = match pointer.mutability() {
                    SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                    SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
                };
                let selection = if pointer.metadata() == SemanticPointerMetadataV1::SliceLength {
                    Selection::Slice {
                        element: pointee,
                        value_space,
                        access,
                    }
                } else {
                    Selection::Pointer {
                        pointee,
                        value_space,
                        access,
                    }
                };
                layouts.select_schema(owner, ty, selection, budget)?
            }
            (
                SemanticTypeShapeV1::Tuple(_)
                | SemanticTypeShapeV1::Aggregate(_)
                | SemanticTypeShapeV1::Union(_),
                None,
            )
            | (SemanticTypeShapeV1::Enum { .. }, Some(_)) => {
                let fields = Self::fields(types, ty, variant)?;
                let mut represented = emission_vec_v1(fields.len(), budget)?;
                for (ordinal, &field_ty) in fields.iter().enumerate() {
                    budget.charge_work(2)?;
                    if execution_cfg_nominal_kind_v29(types, field_ty)?.is_some() {
                        continue;
                    }
                    let ordinal =
                        u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                    let child = self.child(equation, ordinal, field_ty, None, budget)?;
                    let id = self.materialize(
                        plan,
                        layouts,
                        child,
                        selected,
                        colors,
                        depth + 1,
                        budget,
                    )?;
                    represented.push((ordinal, id));
                }
                layouts.select_schema(
                    owner,
                    ty,
                    Selection::Aggregate {
                        variant,
                        fields: &represented,
                    },
                    budget,
                )?
            }
            (SemanticTypeShapeV1::Array { element, .. }, None) => {
                let child = self.child(equation, 0, *element, None, budget)?;
                let element =
                    self.materialize(plan, layouts, child, selected, colors, depth + 1, budget)?;
                layouts.select_schema(owner, ty, Selection::Array { element }, budget)?
            }
            (SemanticTypeShapeV1::Enum { variants, .. }, None) => {
                if let SemanticRustcVariantsV1::Single { index } = declaration.layout().variants() {
                    let child = self.child(equation, *index, ty, Some(*index), budget)?;
                    self.materialize(plan, layouts, child, selected, colors, depth + 1, budget)?
                } else {
                    let mut represented = emission_vec_v1(variants.len(), budget)?;
                    for index in 0..variants.len() {
                        budget.charge_work(1)?;
                        let index =
                            u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                        let child = self.child(equation, index, ty, Some(index), budget)?;
                        let id = self.materialize(
                            plan,
                            layouts,
                            child,
                            selected,
                            colors,
                            depth + 1,
                            budget,
                        )?;
                        represented.push((index, id));
                    }
                    layouts.select_schema(
                        owner,
                        ty,
                        Selection::Enum {
                            variants: &represented,
                        },
                        budget,
                    )?
                }
            }
            _ => layouts.select_original_leaf_schema(owner, ty, budget)?,
        };
        colors[equation] = 2;
        selected[equation] = Some(id);
        Ok(id)
    }
}

fn source_backing_original_request_v29(
    requests: &[source_storage_demands_v29::DemandV29],
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let key = (instance.index(), local.index());
    let mut start = 0;
    let mut end = requests.len();
    while start < end {
        budget.charge_work(2)?;
        let middle = start + (end - start) / 2;
        let row = &requests[middle];
        if (row.instance.index(), row.local.index()) < key {
            start = middle + 1;
        } else {
            end = middle;
        }
    }
    budget.charge_work(1)?;
    Ok(requests.get(start).is_some_and(|row| {
        (row.instance.index(), row.local.index()) == key
            && row.path.is_empty()
            && matches!(
                row.kind,
                source_storage_demands_v29::DemandKindV29::WholeBackingCandidate
                    | source_storage_demands_v29::DemandKindV29::UnresolvedNominalBacking
            )
    }))
}

// A legacy scalar slot is local-wide, while typed allocations are generation
// qualified. Once any generation needs typed backing, every generation of the
// same original local must use its own selected typed schema. This changes only
// representation selection, never generation identity or lifetime/currentness.
fn source_reference_reconcile_object_generations_v29(
    rows: &[SourceReferenceScalarCellV29],
    cells: &BTreeMap<(usize, u32, u32), usize>,
    objects: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    if rows.len() != cells.len() || objects.len() != rows.len() {
        return Err(source_backing_error_v29());
    }
    // The owning builder's ordered index already groups exact original locals.
    // Forward/backward propagation visits each row twice, without scratch or a
    // generation cross product. Any refusal invalidates the owning plan.
    let mut previous = None;
    for (&(instance, local, generation), &index) in cells {
        budget.charge_work(9)?;
        let row = rows.get(index).ok_or_else(source_backing_error_v29)?;
        if (row.instance.index(), row.local.index(), row.generation)
            != (instance, local, generation)
        {
            return Err(source_backing_error_v29());
        }
        let required = objects.get_mut(index).ok_or_else(source_backing_error_v29)?;
        if let Some((key, ty, earlier)) = previous
            && key == (instance, local)
        {
            if row.ty != ty {
                return Err(source_backing_error_v29());
            }
            *required |= earlier;
        }
        previous = Some(((instance, local), row.ty, *required));
    }
    let mut next = None;
    for (&(instance, local, _), &index) in cells.iter().rev() {
        budget.charge_work(4)?;
        let required = objects.get_mut(index).ok_or_else(source_backing_error_v29)?;
        if let Some((key, later)) = next
            && key == (instance, local)
        {
            *required |= later;
        }
        next = Some(((instance, local), *required));
    }
    Ok(())
}

fn source_reference_select_backing_v29(
    builder: &mut SourceReferenceBuilderV29<'_, '_, '_>,
    cells: &mut BTreeMap<(usize, u32, u32), usize>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let result = (|| {
        let plan = &mut builder.plan;
        let demands = plan.storage_demands.ok_or_else(|| {
            source_reference_error_v29(
                "selected backing requires the original root storage demand lens",
            )
        })?;
        let (requests, _) = demands.requests(plan.instances, budget)?;
        let root = plan
            .storage_root
            .as_ref()
            .ok_or_else(source_backing_error_v29)?;
        let layouts = root.source_layouts(plan.instances, budget)?;
        let has_schema_inputs = source_reference_has_schema_inputs_v29(plan, budget)?;
        if requests.is_empty() {
            plan.charge(plan.cells.strategies.len(), budget)?;
            if !plan.raw_origins.is_empty()
                || !plan.storage_activations.is_empty()
                || !plan.representation_demands.is_empty()
                || plan.cells.strategies.iter().any(|strategy| {
                    matches!(strategy, SourceReferenceCellStrategyV29::NeedsStorage)
                })
            {
                return Err(source_backing_error_v29());
            }
            if !plan.cells.rows.is_empty() {
                return Err(source_backing_error_v29());
            }
            if !has_schema_inputs {
                return Ok(());
            }
        }
        let types = plan.instances.owner().source_semantic().types();
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<Option<fe2o3_kernel_ir::StorageLayoutIdV1>>>(),
            std::mem::size_of::<Vec<u8>>(),
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let mut equations = SourceBackingEquationsV29::new(plan, budget)?;
        // The collector orders original requests by instance/local. Validate
        // that invariant once before binary queries; never rescan the CFG.
        let mut previous = None;
        for request in requests {
            budget.charge_work(2)?;
            let key = (request.instance.index(), request.local.index());
            if previous.is_some_and(|previous| previous > key) {
                return Err(source_backing_error_v29());
            }
            previous = Some(key);
        }
        let existing_receivers = source_existing_receiver_rows_v29(plan, budget)?;
        // Activations can have no value write at all (for example a tag-only
        // empty variant). An original entry receiver may already have its
        // checked ABI representation; all actual storage restarts still need
        // backing. Neither disposition establishes an initialized value.
        for ordinal in 0..plan.storage_activations.len() {
            budget.charge_work(2)?;
            let source = plan.storage_activations[ordinal];
            if plan.instances.instance_reachable(source.instance) != Some(true)
                || !source_backing_original_request_v29(
                    requests, source.instance, source.local, budget,
                )?
            {
                return Err(source_backing_error_v29());
            }
            if !source_existing_receiver_entry_v29(plan, &existing_receivers, source, budget)? {
                equations.ensure_cell(
                    plan, cells, source.instance, source.local, source.generation, budget,
                )?;
            }
        }
        for loan in 0..plan.loans.len() {
            budget.charge_work(2)?;
            if matches!(
                plan.cells.strategies.get(loan),
                Some(SourceReferenceCellStrategyV29::Promoted)
            ) {
                continue;
            }
            let source = &plan.origins[plan.loans[loan].origin];
            let (instance, local, generation) = (source.instance, source.local, source.generation);
            equations.ensure_cell(plan, cells, instance, local, generation, budget)?;
        }
        // All original raw formations retain a backing row, including projected
        // addresses. Legacy raw scalar queries remain closed to Object rows.
        plan.cells.raw_origins = emission_vec_v1(plan.raw_origins.len(), budget)?;
        for ordinal in 0..plan.raw_origins.len() {
            budget.charge_work(1)?;
            let origin = plan.raw_origins[ordinal];
            let (index, _) = equations.ensure_cell(
                plan,
                cells,
                origin.instance,
                origin.local,
                origin.generation,
                budget,
            )?;
            plan.cells.raw_origins.push(index);
        }
        // This paid borrowed index contains only coordinates into the existing
        // incoming values and original-write census, not a second demand owner
        // or CFG. Late addressable pointees consume the same original rows.
        budget.reserve_storage(std::mem::size_of::<
            BTreeMap<(usize, u32, u32), Vec<SourceBackingWriteIndexV29>>,
        >())?;
        let mut writes = BTreeMap::new();
        for ordinal in 0..plan.entries.len() {
            budget.charge_work(2)?;
            let Some(state) = plan.entries[ordinal] else {
                continue;
            };
            let instance = plan
                .instances
                .id_at(ordinal)
                .ok_or_else(source_backing_error_v29)?;
            let count = plan
                .states
                .get(state)
                .ok_or_else(source_backing_error_v29)?
                .len();
            for local in 0..count {
                budget.charge_work(1)?;
                let source = plan.states[state][local];
                let Some(_) = source.node else {
                    continue;
                };
                let local = SemanticLocalIdV1::from_index(
                    u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                let key = (instance.index(), local.index(), source.generation);
                source_backing_index_write_v29(
                    &mut writes,
                    key,
                    SourceBackingWriteIndexV29::Entry {
                        state,
                        local: local.index() as usize,
                    },
                    budget,
                )?;
                if source_backing_original_request_v29(requests, instance, local, budget)?
                    && !source_existing_receiver_v29(&existing_receivers, instance, local, budget)? {
                    equations.ensure_cell(
                        plan,
                        cells,
                        instance,
                        local,
                        source.generation,
                        budget,
                    )?;
                }
            }
        }
        for ordinal in 0..plan.representation_demands.len() {
            budget.charge_work(2)?;
            let source = &plan.representation_demands[ordinal];
            let (instance, local, generation) = (source.instance, source.local, source.generation);
            let key = (instance.index(), local.index(), generation);
            source_backing_index_write_v29(
                &mut writes,
                key,
                SourceBackingWriteIndexV29::Assignment(ordinal),
                budget,
            )?;
            if source_backing_original_request_v29(requests, instance, local, budget)? {
                equations.ensure_cell(plan, cells, instance, local, generation, budget)?;
            }
        }
        if has_schema_inputs {
            source_reference_seed_schema_inputs_v29(plan, &mut equations, cells, budget)?;
        }
        for node in 0..plan.nodes.len() {
            budget.charge_work(1)?;
            if matches!(plan.nodes[node].kind, SourceReferenceNodeKindV29::Address(_)) {
                equations.ensure_node(plan, cells, node, 0, budget)?;
            }
        }
        source_reference_check_boundary_write_census_v29(plan, requests, budget)?;
        let mut next_cell = 0;
        while next_cell < plan.cells.rows.len() {
            budget.charge_work(2)?;
            let cell = plan.cells.rows[next_cell];
            let object = *equations
                .cells
                .get(next_cell)
                .ok_or_else(source_backing_error_v29)?;
            next_cell += 1;
            let key = (cell.instance.index(), cell.local.index(), cell.generation);
            charge_execution_cfg_lookup_v29(writes.len(), budget)?;
            let Some(inputs) = writes.get(&key) else {
                continue;
            };
            for &input in inputs {
                budget.charge_work(2)?;
                let (node, target) = match input {
                    SourceBackingWriteIndexV29::Entry { state, local } => {
                        let incoming = plan
                            .states
                            .get(state)
                            .and_then(|state| state.get(local))
                            .ok_or_else(source_backing_error_v29)?;
                        if incoming.generation != cell.generation
                            || local != cell.local.index() as usize
                        {
                            return Err(source_backing_error_v29());
                        }
                        (incoming.node.ok_or_else(source_backing_error_v29)?, object)
                    }
                    SourceBackingWriteIndexV29::Assignment(ordinal) => {
                        let source = plan
                            .representation_demands
                            .get(ordinal)
                            .ok_or_else(source_backing_error_v29)?;
                        if source.instance != cell.instance
                            || source.local != cell.local
                            || source.generation != cell.generation
                        {
                            return Err(source_backing_error_v29());
                        }
                        let target = equations.project(
                            types,
                            object,
                            plan.projections
                                .get(source.projections.clone())
                                .ok_or_else(source_backing_error_v29)?,
                            budget,
                        )?;
                        (source.node, target)
                    }
                };
                if let Some(value) = equations.ensure_node(plan, cells, node, 0, budget)? {
                    equations.join_value(target, value, budget)?;
                }
            }
        }
        equations.retain_recursive_original(budget)?;
        equations.propagate_original(types, budget)?;
        source_reference_reconcile_object_generations_v29(
            &plan.cells.rows, cells, &mut equations.object_cells, budget,
        )?;
        let mut selected = emission_vec_v1(equations.rows.len(), budget)?;
        let mut colors = emission_vec_v1(equations.rows.len(), budget)?;
        budget.charge_work(argument_product_v1(equations.rows.len(), 2)?)?;
        selected.resize(equations.rows.len(), None);
        colors.resize(equations.rows.len(), 0);
        for index in 0..plan.cells.rows.len() {
            budget.charge_work(2)?;
            let equation = *equations
                .cells
                .get(index)
                .ok_or_else(source_backing_error_v29)?;
            let schema = equations.materialize(
                plan,
                layouts,
                equation,
                &mut selected,
                &mut colors,
                0,
                budget,
            )?;
            let row = &mut plan.cells.rows[index];
            layouts.check_selected_schema(plan.instances.owner(), row.ty, schema, budget)?;
            let declaration = &types[row.ty.index() as usize];
            row.kind = if !equations.object_cells[index]
                && matches!(
                    declaration.shape(),
                    SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
                )
                && private_retained_slot_facts_v1(types, row.ty, budget)?.is_some()
            {
                SourceBackingKindV29::Scalar
            } else {
                SourceBackingKindV29::Object(schema)
            };
        }
        for loan in 0..plan.loans.len() {
            budget.charge_work(4)?;
            let strategy = plan.cells.strategies.get(loan)
                .ok_or_else(source_backing_error_v29)?;
            if matches!(strategy, SourceReferenceCellStrategyV29::Promoted)
                && !equations.stored_loans[loan]
            {
                continue;
            }
            let source = &plan.origins[plan.loans[loan].origin];
            let key = (
                source.instance.index(),
                source.local.index(),
                source.generation,
            );
            charge_execution_cfg_lookup_v29(cells.len(), budget)?;
            let Some(&index) = cells.get(&key) else {
                if equations.stored_loans[loan] {
                    return Err(source_backing_error_v29());
                }
                continue;
            };
            plan.cells.strategies[loan] = match plan.cells.rows[index].kind {
                SourceBackingKindV29::Scalar => SourceReferenceCellStrategyV29::Scalar(index),
                SourceBackingKindV29::Object(_) => SourceReferenceCellStrategyV29::Object(index),
            };
        }
        plan.selected_storage = emission_vec_v1(plan.nodes.len(), budget)?;
        budget.charge_work(plan.nodes.len())?;
        plan.selected_storage.resize(plan.nodes.len(), None);
        for node in 0..equations.nodes.len() {
            budget.charge_work(1)?;
            if let Some(equation) = equations.nodes[node] {
                let id = equations.materialize(
                    plan,
                    layouts,
                    equation,
                    &mut selected,
                    &mut colors,
                    0,
                    budget,
                )?;
                plan.selected_storage[node] = Some(id);
            }
        }
        Ok(())
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(&builder.plan, error))
}

fn source_reference_address_value_origin_v29(
    nodes: &[SourceReferenceNodeV29],
    node: usize,
    canonical: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let row = nodes.get(node).ok_or(ArgumentResourceV1::Accounting)?;
    let original = nodes.get(canonical).ok_or(ArgumentResourceV1::Accounting)?;
    if !matches!(original.kind, SourceReferenceNodeKindV29::Address(_))
        || original.value_origin.is_some()
        || original.storage.is_some()
        || original.inactive.is_some()
        || original.descriptor.is_some()
        || row.ty != original.ty
        || row.kind != original.kind
        || row.inactive != original.inactive
        || row.descriptor != original.descriptor
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    match row.value_origin {
        None if node == canonical => Ok(()),
        Some(origin) if origin == canonical && origin < node && row.storage.is_some() => Ok(()),
        _ => Err(ArgumentResourceV1::Accounting.into()),
    }
}

include!("production_source_address_formation_epochs_v29.rs");

fn source_reference_selected_pointer_type_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    expected: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<Type>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(plan.instances, budget)?;
        source_reference_owned_prepay_v29::<Option<Type>>(plan, budget)?;
        plan.charge(7, budget)?;
        let row = plan.nodes.get(node).ok_or(ArgumentResourceV1::Accounting)?;
        if row.ty != expected { return Err(ArgumentResourceV1::Accounting.into()); }
        let SourceReferenceNodeKindV29::Address(set) = row.kind else { return Ok(None); };
        if row.descriptor.is_some() { return Err(ArgumentResourceV1::Accounting.into()); }
        let owner = plan.instances.owner();
        let types = owner.source_semantic().types();
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = types.get(expected.index() as usize)
            .map(SemanticTypeDeclV1::shape) else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
        let choices = plan.raw_sets.get(set).ok_or(ArgumentResourceV1::Accounting)?;
        let mutable = pointer.mutability() == SemanticMutabilityV1::Mutable;
        if pointer.kind() != SemanticPointerKindV1::Raw
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || choices.ty != pointer.pointee() || choices.mutable != mutable
        { return Err(ArgumentResourceV1::Accounting.into()); }
        charge_execution_cfg_lookup_v29(plan.raw_nodes.len(), budget)?;
        let canonical = *plan.raw_nodes.get(&(expected.index(), set)).ok_or(ArgumentResourceV1::Accounting)?;
        source_reference_address_value_origin_v29(&plan.nodes, node, canonical, budget)?;
        let path = plan.raw_projection_range(set, pointer.pointee(), budget)?;
        let schema = plan.selected_storage.get(node).copied().flatten()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let layouts = plan.storage_root.as_ref().ok_or(ArgumentResourceV1::Accounting)?
            .source_layouts(plan.instances, budget)?;
        layouts.check_selected_schema(owner, expected, schema, budget)?;
        plan.charge(2, budget)?;
        let selected = {
            let rows = layouts.rows(owner, budget)?;
            let Some(fe2o3_kernel_ir::StorageLayoutV1 {
                kind: fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer), ..
            }) = rows.get(schema.0 as usize) else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            *pointer
        };
        let access = if mutable { AccessMode::ReadWrite } else { AccessMode::ReadOnly };
        if selected.access != access || selected.value_space != AddressSpace::Private {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        layouts.check_selected_schema(owner, pointer.pointee(), selected.pointee, budget)?;
        let end = argument_sum_v1(&[choices.first, choices.count])?;
        for choice in plan.raw_choices.get(choices.first..end).ok_or(ArgumentResourceV1::Accounting)? {
            plan.charge(18, budget)?;
            let origin = plan.raw_origins.get(choice.origin).ok_or(ArgumentResourceV1::Accounting)?;
            let statement = origin.site.statement.ok_or(ArgumentResourceV1::Accounting)?;
            charge_execution_cfg_lookup_v29(plan.raw_origin_sites.len(), budget)?;
            if origin.pointer_type != expected || origin.mutable != mutable
                || plan.raw_origin_sites.get(&(origin.site.instance.index(), origin.site.block.index(), statement, origin.generation)) != Some(&choice.origin)
                || plan.instances.instance_reachable(origin.site.instance) != Some(true)
                || plan.instances.instance_reachable(origin.instance) != Some(true)
            { return Err(ArgumentResourceV1::Accounting.into()); }
            let original = plan.instances.instance(origin.site.instance)
                .and_then(|instance| instance.declaration().blocks().get(origin.site.block.index() as usize))
                .and_then(|block| block.statements().get(statement))
                .ok_or(ArgumentResourceV1::Accounting)?;
            let SemanticStatementKindV1::Assign(assign) = original.kind() else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            if assign.value().result_type() != expected { return Err(ArgumentResourceV1::Accounting.into()); }
            match (origin.formation, assign.value().kind()) {
                (SourceReferenceRawFormationV29::AddressOf, SemanticRvalueKindV1::AddressOf { place, mutability }) => {
                    if origin.source != place as *const SemanticPlaceV1 as usize
                        || place.ty() != origin.ty || *mutability != pointer.mutability()
                    { return Err(ArgumentResourceV1::Accounting.into()); }
                    let source = source_reference_access_at_v29(plan, origin.site, place, SourceReferenceAccessV29::Address, budget)?;
                    plan.charge(argument_sum_v1(&[7, path.len()])?, budget)?;
                    if source.instance != origin.instance || source.local != origin.local
                        || source.loan != origin.parent
                        || source.ty != origin.ty || (mutable && source.shared_path)
                        || plan.projections.get(source.projections.clone()) != plan.projections.get(path.clone())
                    { return Err(ArgumentResourceV1::Accounting.into()); }
                    if source.generation != origin.generation
                        && !source_reference_address_epoch_matches_v29(plan, origin.site, place,
                            &source, choice.origin, budget)?
                    { return Err(ArgumentResourceV1::Accounting.into()); }
                }
                (SourceReferenceRawFormationV29::ReferenceCast, SemanticRvalueKindV1::Cast { kind: SemanticCastKindV1::Pointer, operand }) => {
                    let parent = origin.parent.ok_or(ArgumentResourceV1::Accounting)?;
                    let loan = plan.loans.get(parent).ok_or(ArgumentResourceV1::Accounting)?;
                    if origin.source != operand as *const SemanticOperandV1 as usize
                        || loan.source_type != operand.ty()
                        || (mutable && loan.kind != SemanticBorrowKindV1::Mutable)
                    { return Err(ArgumentResourceV1::Accounting.into()); }
                }
                _ => return Err(ArgumentResourceV1::Accounting.into()),
            }
            let cell = *plan.cells.raw_origins.get(choice.origin).ok_or(ArgumentResourceV1::Accounting)?;
            let cell = plan.cells.rows.get(cell).ok_or(ArgumentResourceV1::Accounting)?;
            let SourceBackingKindV29::Object(root_schema) = cell.kind else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            if cell.instance != origin.instance || cell.local != origin.local
                || cell.generation != origin.generation
                || plan.instances.instance(cell.instance)
                    .and_then(|instance| instance.declaration().locals().get(cell.local.index() as usize))
                    .map(|local| local.ty()) != Some(cell.ty)
            { return Err(ArgumentResourceV1::Accounting.into()); }
            let (ty, pointee) = layouts.project_selected_schema(owner, cell.ty, root_schema,
                plan.projections.get(path.clone()).ok_or(ArgumentResourceV1::Accounting)?, budget)?;
            if ty != pointer.pointee() || pointee != selected.pointee {
                return Err(ArgumentResourceV1::Accounting.into());
            }
        }
        budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
        Ok(Some(Type::pointer(Type::StorageObject(selected.pointee), selected.value_space, selected.access)))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_object_pointer_type_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    loan: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(plan.instances, budget)?;
        source_reference_owned_prepay_v29::<Type>(plan, budget)?;
        plan.charge(7, budget)?;
        let (_, cell) = plan
            .backing_cell(loan, budget)?
            .ok_or_else(source_backing_error_v29)?;
        let SourceBackingKindV29::Object(schema) = cell.kind else {
            return Err(source_backing_error_v29());
        };
        let record = plan.loans.get(loan).ok_or_else(source_backing_error_v29)?;
        let origin = plan
            .origins
            .get(record.origin)
            .ok_or_else(source_backing_error_v29)?;
        let types = plan.instances.owner().source_semantic().types();
        let SemanticTypeShapeV1::Pointer(pointer) = types
            .get(record.source_type.index() as usize)
            .ok_or_else(source_backing_error_v29)?
            .shape()
        else {
            return Err(source_backing_error_v29());
        };
        let (access, mutability) = match record.kind {
            SemanticBorrowKindV1::Shared => (AccessMode::ReadOnly, SemanticMutabilityV1::Immutable),
            SemanticBorrowKindV1::Mutable => (AccessMode::ReadWrite, SemanticMutabilityV1::Mutable),
            SemanticBorrowKindV1::Fake => return Err(source_backing_error_v29()),
        };
        if pointer.metadata() != SemanticPointerMetadataV1::None
            || pointer.pointee() != origin.ty
            || pointer.mutability() != mutability
            || origin.instance != cell.instance
            || origin.local != cell.local
            || origin.generation != cell.generation
        {
            return Err(source_backing_error_v29());
        }
        let layouts = plan
            .storage_root
            .as_ref()
            .ok_or_else(source_backing_error_v29)?
            .source_layouts(plan.instances, budget)?;
        let (ty, selected) = layouts.project_selected_schema(
            plan.instances.owner(),
            cell.ty,
            schema,
            plan.projections
                .get(origin.projections.clone())
                .ok_or_else(source_backing_error_v29)?,
            budget,
        )?;
        if ty != origin.ty {
            return Err(source_backing_error_v29());
        }
        budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
        Ok(Type::pointer(
            Type::StorageObject(selected),
            AddressSpace::Private,
            access,
        ))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_check_selected_backing_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(plan.instances, budget)?;
        let Some(lens) = plan.storage_demands else {
            // Component fixtures have no production demand authority. They may
            // use only the unchanged scalar owner, never a selected object row.
            plan.charge(plan.cells.rows.len(), budget)?;
            if !plan.selected_storage.is_empty()
                || !plan.storage_activations.is_empty()
                || !plan.representation_demands.is_empty()
                || plan
                    .cells
                    .rows
                    .iter()
                    .any(|row| matches!(row.kind, SourceBackingKindV29::Object(_)))
            {
                return Err(source_backing_error_v29());
            }
            return Ok(false);
        };
        let (requests, _) = lens.requests(plan.instances, budget)?;
        let layouts = plan
            .storage_root
            .as_ref()
            .ok_or_else(source_backing_error_v29)?
            .source_layouts(plan.instances, budget)?;
        let has_schema_inputs = source_reference_has_schema_inputs_v29(plan, budget)?;
        let selected = !requests.is_empty() || has_schema_inputs;
        if plan.has_storage_demands != !requests.is_empty()
            || (!selected && !plan.selected_storage.is_empty())
            || (requests.is_empty() && !plan.storage_activations.is_empty())
            || (requests.is_empty() && !plan.cells.rows.is_empty())
            || (selected && plan.selected_storage.len() != plan.nodes.len())
        {
            return Err(source_backing_error_v29());
        }
        for row in &plan.cells.rows {
            plan.charge(4, budget)?;
            if plan.instances.instance_reachable(row.instance) != Some(true)
                || plan
                    .instances
                    .instance(row.instance)
                    .and_then(|instance| {
                        instance
                            .declaration()
                            .locals()
                            .get(row.local.index() as usize)
                    })
                    .map(|local| local.ty())
                    != Some(row.ty)
            {
                return Err(source_backing_error_v29());
            }
            if let SourceBackingKindV29::Object(schema) = row.kind {
                layouts.check_selected_schema(plan.instances.owner(), row.ty, schema, budget)?;
            }
        }
        for (node, schema) in plan.nodes.iter().zip(&plan.selected_storage) {
            plan.charge(1, budget)?;
            if let Some(schema) = schema {
                if node.kind == SourceReferenceNodeKindV29::Absent {
                    return Err(source_backing_error_v29());
                }
                layouts.check_selected_schema(plan.instances.owner(), node.ty, *schema, budget)?;
            }
        }
        Ok(true)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

#[cfg(test)]
mod generation_representation_tests_v29 {
    use super::*;

    fn row(instance: usize, local: u32, generation: u32) -> SourceReferenceScalarCellV29 {
        SourceReferenceScalarCellV29 {
            instance: ProductionCallInstanceIdV1(instance),
            local: SemanticLocalIdV1::from_index(local),
            generation,
            ty: SemanticTypeIdV1::from_index(3),
            kind: SourceBackingKindV29::Scalar,
        }
    }

    fn index(rows: &[SourceReferenceScalarCellV29]) -> BTreeMap<(usize, u32, u32), usize> {
        rows.iter().enumerate().map(|(index, row)|
            ((row.instance.index(), row.local.index(), row.generation), index)).collect()
    }

    #[test]
    fn object_generation_reconciliation_preserves_original_coordinates_and_other_locals() {
        // The original rows need not be ordered; only their already-owned key
        // index is ordered. Repeated helpers keep distinct instance identities.
        let rows = vec![row(1, 2, 0), row(0, 2, u32::MAX), row(0, 7, 0),
            row(0, 2, 0), row(1, 2, 19), row(0, 2, 17), row(0, 7, 5)];
        let original = rows.clone();
        let cells = index(&rows);
        for seed in [1, 3, 5] {
            let mut objects = vec![false; rows.len()];
            objects[seed] = true;
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 41);
            budget.reserve_storage(41).unwrap();
            source_reference_reconcile_object_generations_v29(&rows, &cells, &mut objects, &mut budget).unwrap();
            assert_eq!(objects, [false, true, false, true, false, true, false]);
            assert_eq!(rows, original);
            assert_eq!(cells, index(&original));
            assert_eq!(budget.work(), 3 + 13 * rows.len());
            assert_eq!((budget.storage(), budget.peak_storage()), (41, 41));
        }
        let mut objects = vec![false; rows.len()];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        source_reference_reconcile_object_generations_v29(&rows, &cells, &mut objects, &mut budget).unwrap();
        assert!(objects.iter().all(|required| !required), "scalar-only groups remain scalar");
    }

    #[test]
    fn object_generation_reconciliation_rejects_wrong_instance_local_generation_type_and_census() {
        for fault in 0..9 {
            let mut rows = vec![row(0, 2, 0), row(0, 2, 1), row(1, 2, 0)];
            let mut cells = index(&rows);
            let mut objects = vec![false, true, false];
            match fault {
                0 => { cells.remove(&(0, 2, 0)); }
                1 => { cells.insert((0, 2, 2), 1); }
                2 => { cells.insert((0, 2, 1), usize::MAX); }
                3 => { cells.insert((0, 2, 1), 0); }
                4 => rows[0].instance = ProductionCallInstanceIdV1(2),
                5 => rows[0].local = SemanticLocalIdV1::from_index(3),
                6 => rows[0].generation = 8,
                7 => rows[1].ty = SemanticTypeIdV1::from_index(4),
                8 => objects.push(false),
                _ => unreachable!(),
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 41);
            budget.reserve_storage(41).unwrap();
            let result = source_reference_reconcile_object_generations_v29(&rows, &cells, &mut objects, &mut budget);
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { .. })), "fault={fault}: {result:?}");
            assert_eq!((budget.storage(), budget.peak_storage()), (41, 41));
        }
    }

    #[test]
    fn object_generation_reconciliation_has_exact_linear_work_and_no_scratch() {
        for count in [0, 1, 4, 257] {
            let rows: Vec<_> = (0..count).map(|generation| row(0, 2, generation as u32)).collect();
            let cells = index(&rows);
            let required = 3 + 13 * count;
            for limit in [required, required - 1] {
                let mut objects = vec![false; count];
                if count != 0 { objects[count / 2] = true; }
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, 41);
                budget.reserve_storage(41).unwrap();
                let result = source_reference_reconcile_object_generations_v29(&rows, &cells, &mut objects, &mut budget);
                if limit == required {
                    result.unwrap();
                    assert_eq!(budget.work(), required);
                    assert!(objects.iter().all(|required| *required));
                } else {
                    assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))), "{result:?}");
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (41, 41));
            }
        }
    }
}
