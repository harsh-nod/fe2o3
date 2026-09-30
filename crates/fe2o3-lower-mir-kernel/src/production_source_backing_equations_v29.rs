// These equations select representations, not values, allocations or activity.
// Unifying equations never unifies the original-instance cells that use them.
struct SourceBackingEquationV29 {
    ty: SemanticTypeIdV1,
    variant: Option<u32>,
    parent: usize,
    rank: u32,
    original: bool,
    space: Option<AddressSpace>,
    children: BTreeMap<u32, usize>,
    pointee: Option<usize>,
}

struct SourceBackingEquationsV29 {
    rows: Vec<SourceBackingEquationV29>,
    nodes: Vec<Option<usize>>,
    cells: Vec<usize>,
    object_cells: Vec<bool>,
    stored_loans: Vec<bool>,
    pending: Vec<(usize, usize)>,
}

#[derive(Clone, Copy)]
enum SourceBackingWriteIndexV29 {
    Entry { state: usize, local: usize },
    Assignment(usize),
}

fn source_backing_index_write_v29(
    index: &mut BTreeMap<(usize, u32, u32), Vec<SourceBackingWriteIndexV29>>,
    key: (usize, u32, u32),
    source: SourceBackingWriteIndexV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(index.len(), budget)?;
    if !index.contains_key(&key) {
        reserve_execution_cfg_map_entry_v29::<(usize, u32, u32), Vec<SourceBackingWriteIndexV29>>(
            index.len(),
            budget,
        )?;
        index.insert(key, Vec::new());
    }
    emission_push_v1(
        index.get_mut(&key).ok_or_else(source_backing_error_v29)?,
        source,
        budget,
    )
}

fn source_backing_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "selected backing differs from its original source object or representation",
    )
}

impl SourceBackingEquationsV29 {
    fn new(
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let mut nodes = emission_vec_v1(plan.nodes.len(), budget)?;
        let mut stored_loans = emission_vec_v1(plan.loans.len(), budget)?;
        budget.charge_work(argument_sum_v1(&[plan.nodes.len(), plan.loans.len()])?)?;
        nodes.resize(plan.nodes.len(), None);
        stored_loans.resize(plan.loans.len(), false);
        Ok(Self {
            rows: Vec::new(),
            nodes,
            cells: Vec::new(),
            object_cells: Vec::new(),
            stored_loans,
            pending: Vec::new(),
        })
    }

    fn insert(
        &mut self,
        ty: SemanticTypeIdV1,
        variant: Option<u32>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let index = self.rows.len();
        emission_push_v1(
            &mut self.rows,
            SourceBackingEquationV29 {
                ty,
                variant,
                parent: index,
                rank: 0,
                original: false,
                space: None,
                children: BTreeMap::new(),
                pointee: None,
            },
            budget,
        )?;
        Ok(index)
    }

    fn root(
        &mut self,
        index: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let mut root = index;
        let mut traversed = 0;
        loop {
            budget.charge_work(2)?;
            let row = self.rows.get(root).ok_or_else(source_backing_error_v29)?;
            if row.parent == root {
                break;
            }
            traversed = argument_sum_v1(&[traversed, 1])?;
            if traversed >= self.rows.len() {
                return Err(source_backing_error_v29());
            }
            root = row.parent;
        }
        let mut current = index;
        while current != root {
            budget.charge_work(2)?;
            let row = self
                .rows
                .get_mut(current)
                .ok_or_else(source_backing_error_v29)?;
            current = row.parent;
            row.parent = root;
        }
        Ok(root)
    }

    fn join_space(left: Option<AddressSpace>, right: Option<AddressSpace>) -> Option<AddressSpace> {
        match (left, right) {
            (None, value) | (value, None) => value,
            (Some(left), Some(right)) if left == right => Some(left),
            (Some(_), Some(_)) => Some(AddressSpace::Generic),
        }
    }

    fn join(
        &mut self,
        left: usize,
        right: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.reserve_storage(std::mem::size_of::<BTreeMap<u32, usize>>())?;
        emission_push_v1(&mut self.pending, (left, right), budget)?;
        while let Some((left, right)) = self.pending.pop() {
            let mut left = self.root(left, budget)?;
            let mut right = self.root(right, budget)?;
            if left == right {
                continue;
            }
            budget.charge_work(7)?;
            if self.rows[left].ty != self.rows[right].ty
                || self.rows[left].variant != self.rows[right].variant
            {
                return Err(source_backing_error_v29());
            }
            if (self.rows[left].rank, std::cmp::Reverse(left))
                < (self.rows[right].rank, std::cmp::Reverse(right))
            {
                std::mem::swap(&mut left, &mut right);
            }
            if self.rows[left].rank == self.rows[right].rank {
                self.rows[left].rank = self.rows[left]
                    .rank
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            self.rows[right].parent = left;
            self.rows[left].original |= self.rows[right].original;
            self.rows[left].space = Self::join_space(self.rows[left].space, self.rows[right].space);
            match (self.rows[left].pointee, self.rows[right].pointee) {
                (None, value) => self.rows[left].pointee = value,
                (Some(left), Some(right)) => {
                    emission_push_v1(&mut self.pending, (left, right), budget)?
                }
                _ => {}
            }
            // Move each paid edge at most once per union-by-rank level. No
            // Cartesian combination of C1 enum alternatives is constructed.
            let children = std::mem::take(&mut self.rows[right].children);
            for (key, child) in children {
                charge_execution_cfg_lookup_v29(self.rows[left].children.len(), budget)?;
                if let Some(previous) = self.rows[left].children.get(&key).copied() {
                    emission_push_v1(&mut self.pending, (previous, child), budget)?;
                } else {
                    reserve_execution_cfg_map_entry_v29::<u32, usize>(
                        self.rows[left].children.len(),
                        budget,
                    )?;
                    self.rows[left].children.insert(key, child);
                }
            }
        }
        Ok(())
    }

    fn child(
        &mut self,
        parent: usize,
        key: u32,
        ty: SemanticTypeIdV1,
        variant: Option<u32>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let parent = self.root(parent, budget)?;
        charge_execution_cfg_lookup_v29(self.rows[parent].children.len(), budget)?;
        if let Some(child) = self.rows[parent].children.get(&key).copied() {
            let child = self.root(child, budget)?;
            if self.rows[child].ty != ty || self.rows[child].variant != variant {
                return Err(source_backing_error_v29());
            }
            return Ok(child);
        }
        let child = self.insert(ty, variant, budget)?;
        self.rows[child].original = self.rows[parent].original;
        reserve_execution_cfg_map_entry_v29::<u32, usize>(
            self.rows[parent].children.len(),
            budget,
        )?;
        self.rows[parent].children.insert(key, child);
        Ok(child)
    }

    fn copy_value(
        &mut self,
        source: usize,
        depth: usize,
        components: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        execution_cfg_charge_node_v29(components, budget)?;
        if depth >= 256 {
            return Err(source_backing_error_v29());
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<(u32, usize)>>(),
            std::mem::size_of::<Result<usize, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let source = self.root(source, budget)?;
        budget.charge_work(4)?;
        let row = &self.rows[source];
        let (ty, variant, original, space, pointee) =
            (row.ty, row.variant, row.original, row.space, row.pointee);
        let mut children = emission_vec_v1(row.children.len(), budget)?;
        budget.charge_work(row.children.len())?;
        children.extend(row.children.iter().map(|(&key, &value)| (key, value)));
        let output = self.insert(ty, variant, budget)?;
        self.rows[output].original = original;
        self.rows[output].space = space;
        // Value copies are independent representation uses. A pointer still
        // addresses the same original object, so only pointee equations alias.
        self.rows[output].pointee = pointee;
        for (key, child) in children {
            let child = self.copy_value(child, depth + 1, components, budget)?;
            reserve_execution_cfg_map_entry_v29::<u32, usize>(
                self.rows[output].children.len(),
                budget,
            )?;
            self.rows[output].children.insert(key, child);
        }
        Ok(output)
    }

    fn retain_recursive_original(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        // Leaf stripping visits the existing finite equation graph once. The
        // remainder contains cycles and their dependents; none needs a guessed
        // forward physical row. Original compatibility is checked separately.
        let count = self.rows.len();
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<usize>>() * 3,
            std::mem::size_of::<Vec<Vec<usize>>>(),
        ])?)?;
        let mut outgoing = emission_vec_v1(count, budget)?;
        let mut incoming = emission_vec_v1(count, budget)?;
        let mut pending = emission_vec_v1(count, budget)?;
        budget.charge_work(argument_product_v1(count, 2)?)?;
        outgoing.resize(count, 0_usize);
        incoming.resize_with(count, Vec::<usize>::new);
        for index in 0..count {
            budget.charge_work(1)?;
            if self.rows[index].parent != index {
                continue;
            }
            let mut children = emission_vec_v1(
                argument_sum_v1(&[self.rows[index].children.len(), 1])?,
                budget,
            )?;
            budget.charge_work(self.rows[index].children.len())?;
            children.extend(self.rows[index].children.values().copied());
            if let Some(child) = self.rows[index].pointee {
                children.push(child);
            }
            outgoing[index] = children.len();
            for child in children {
                let child = self.root(child, budget)?;
                emission_push_v1(&mut incoming[child], index, budget)?;
            }
            if outgoing[index] == 0 {
                pending.push(index);
            }
        }
        while let Some(child) = pending.pop() {
            budget.charge_work(1)?;
            for &parent in &incoming[child] {
                budget.charge_work(2)?;
                outgoing[parent] = outgoing[parent]
                    .checked_sub(1)
                    .ok_or_else(source_backing_error_v29)?;
                if outgoing[parent] == 0 {
                    pending.push(parent);
                }
            }
        }
        for (index, remaining) in outgoing.into_iter().enumerate() {
            budget.charge_work(1)?;
            if remaining != 0 {
                self.rows[index].original = true;
            }
        }
        Ok(())
    }

    fn join_value(
        &mut self,
        destination: usize,
        source: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let source = self.copy_value(source, 0, &mut 0, budget)?;
        self.join(destination, source, budget)
    }

    fn fields<'a>(
        types: &'a [SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        variant: Option<u32>,
    ) -> Result<&'a [SemanticTypeIdV1], ProductionSemanticKirErrorV1> {
        let declaration = types
            .get(ty.index() as usize)
            .ok_or_else(source_backing_error_v29)?;
        match (declaration.shape(), variant) {
            (
                SemanticTypeShapeV1::Tuple(fields)
                | SemanticTypeShapeV1::Aggregate(fields)
                | SemanticTypeShapeV1::Union(fields),
                None,
            ) => Ok(fields.fields()),
            (SemanticTypeShapeV1::Enum { variants, .. }, Some(variant)) => variants
                .get(variant as usize)
                .map(|variant| variant.fields().fields())
                .ok_or_else(source_backing_error_v29),
            _ => Err(source_backing_error_v29()),
        }
    }

    fn project(
        &mut self,
        types: &[SemanticTypeDeclV1],
        mut root: usize,
        projections: &[SemanticProjectionV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        for projection in projections {
            budget.charge_work(3)?;
            root = self.root(root, budget)?;
            let ty = self.rows[root].ty;
            let variant = self.rows[root].variant;
            let (key, child, child_variant) = match projection.kind() {
                SemanticProjectionKindV1::Downcast(variant) => {
                    let SemanticTypeShapeV1::Enum { variants, .. } =
                        types[ty.index() as usize].shape()
                    else {
                        return Err(source_backing_error_v29());
                    };
                    if variants.get(variant as usize).is_none() || self.rows[root].variant.is_some()
                    {
                        return Err(source_backing_error_v29());
                    }
                    (variant, ty, Some(variant))
                }
                SemanticProjectionKindV1::Field(field) => {
                    let fields = Self::fields(types, ty, variant)?;
                    (
                        field,
                        *fields
                            .get(field as usize)
                            .ok_or_else(source_backing_error_v29)?,
                        None,
                    )
                }
                SemanticProjectionKindV1::Index(_)
                | SemanticProjectionKindV1::ConstantIndex { .. } => {
                    let SemanticTypeShapeV1::Array { element, .. } =
                        types[ty.index() as usize].shape()
                    else {
                        return Err(source_backing_error_v29());
                    };
                    (0, *element, None)
                }
                _ => return Err(source_backing_error_v29()),
            };
            root = self.child(root, key, child, child_variant, budget)?;
            // Downcast keeps the enum source identity; all other projections
            // retain the admitted original terminal type.
            if projection.result_type() != child {
                return Err(source_backing_error_v29());
            }
        }
        self.root(root, budget)
    }

    fn ensure_cell(
        &mut self,
        plan: &mut SourceReferencePlanV29<'_, '_>,
        cells: &mut BTreeMap<(usize, u32, u32), usize>,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        generation: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, usize), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let ty = plan
            .instances
            .instance(instance)
            .and_then(|instance| instance.declaration().locals().get(local.index() as usize))
            .map(|local| local.ty())
            .ok_or_else(source_backing_error_v29)?;
        let key = (instance.index(), local.index(), generation);
        charge_execution_cfg_lookup_v29(cells.len(), budget)?;
        let index = if let Some(index) = cells.get(&key).copied() {
            let row = plan
                .cells
                .rows
                .get(index)
                .ok_or_else(source_backing_error_v29)?;
            if row.instance != instance
                || row.local != local
                || row.generation != generation
                || row.ty != ty
            {
                return Err(source_backing_error_v29());
            }
            index
        } else {
            let index = plan.cells.rows.len();
            emission_push_v1(
                &mut plan.cells.rows,
                SourceReferenceScalarCellV29 {
                    instance,
                    local,
                    generation,
                    ty,
                    kind: SourceBackingKindV29::Scalar,
                },
                budget,
            )?;
            reserve_execution_cfg_map_entry_v29::<(usize, u32, u32), usize>(cells.len(), budget)?;
            cells.insert(key, index);
            index
        };
        while self.cells.len() < plan.cells.rows.len() {
            budget.charge_work(1)?;
            let ty = plan.cells.rows[self.cells.len()].ty;
            let equation = self.insert(ty, None, budget)?;
            emission_push_v1(&mut self.cells, equation, budget)?;
            emission_push_v1(&mut self.object_cells, false, budget)?;
        }
        Ok((index, self.cells[index]))
    }

    fn ensure_node(
        &mut self,
        plan: &mut SourceReferencePlanV29<'_, '_>,
        cells: &mut BTreeMap<(usize, u32, u32), usize>,
        node: usize,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        if depth >= 256 {
            return Err(source_reference_error_v29(
                "selected backing node exceeds its typed depth bound",
            ));
        }
        if let Some(previous) = self.nodes.get(node).copied().flatten() {
            return self.root(previous, budget).map(Some);
        }
        let row = *plan.nodes.get(node).ok_or_else(source_backing_error_v29)?;
        let types = plan.instances.owner().source_semantic().types();
        if row.kind == SourceReferenceNodeKindV29::Absent
            || execution_cfg_nominal_kind_v29(types, row.ty)?.is_some()
        {
            return Ok(None);
        }
        let value = self.insert(row.ty, None, budget)?;
        *self
            .nodes
            .get_mut(node)
            .ok_or_else(source_backing_error_v29)? = Some(value);
        match row.kind {
            SourceReferenceNodeKindV29::Absent => return Err(source_backing_error_v29()),
            SourceReferenceNodeKindV29::Plain(_) | SourceReferenceNodeKindV29::Discriminant(_) => {
                self.rows[value].original = true;
                if let Some(descriptor) = row.descriptor.filter(|_| row.inactive.is_none()) {
                    if plan.descriptor_root.is_none() {
                        return Err(source_backing_error_v29());
                    }
                    let SemanticTypeShapeV1::Pointer(pointer) =
                        types[row.ty.index() as usize].shape()
                    else {
                        return Err(source_backing_error_v29());
                    };
                    if pointer.metadata() != SemanticPointerMetadataV1::SliceLength {
                        return Err(source_backing_error_v29());
                    }
                    let descriptor = plan
                        .descriptor_sets
                        .get(descriptor)
                        .ok_or_else(source_backing_error_v29)?;
                    self.rows[value].space = Some(descriptor.representation);
                    self.rows[value].original = false;
                }
            }
            SourceReferenceNodeKindV29::Loan(loan) => {
                budget.charge_work(5)?;
                let source = plan.loans.get(loan).ok_or_else(source_backing_error_v29)?;
                let origin = plan
                    .origins
                    .get(source.origin)
                    .ok_or_else(source_backing_error_v29)?;
                if source.source_type != row.ty || source.kind == SemanticBorrowKindV1::Fake {
                    return Err(source_backing_error_v29());
                }
                // A represented field cannot contain a promoted referent scalar.
                // Keep the original allocation coordinate, including projections.
                let (instance, local, generation, first, end, pointee_ty) = (
                    origin.instance,
                    origin.local,
                    origin.generation,
                    origin.projections.start,
                    origin.projections.end,
                    origin.ty,
                );
                let (cell, object) =
                    self.ensure_cell(plan, cells, instance, local, generation, budget)?;
                self.object_cells[cell] = true;
                let target = self.project(
                    types,
                    object,
                    plan.projections
                        .get(first..end)
                        .ok_or_else(source_backing_error_v29)?,
                    budget,
                )?;
                if self.rows[target].ty != pointee_ty {
                    return Err(source_backing_error_v29());
                }
                self.rows[value].space = Some(AddressSpace::Private);
                self.rows[value].pointee = Some(target);
                *self
                    .stored_loans
                    .get_mut(loan)
                    .ok_or_else(source_backing_error_v29)? = true;
            }
            SourceReferenceNodeKindV29::Address(set) => {
                budget.charge_work(3)?;
                let object = *plan
                    .raw_sets
                    .get(set)
                    .ok_or_else(source_backing_error_v29)?;
                if object.count == 0 {
                    return Err(source_backing_error_v29());
                }
                let end = argument_sum_v1(&[object.first, object.count])?;
                for ordinal in object.first..end {
                    budget.charge_work(4)?;
                    let choice = *plan
                        .raw_choices
                        .get(ordinal)
                        .ok_or_else(source_backing_error_v29)?;
                    let source = *plan
                        .raw_origins
                        .get(choice.origin)
                        .ok_or_else(source_backing_error_v29)?;
                    if source.pointer_type != row.ty
                        || source.instance != object.instance
                        || source.local != object.local
                        || source.ty != object.ty
                        || source.count != object.projection_count
                    {
                        return Err(source_backing_error_v29());
                    }
                    let (cell, root) = self.ensure_cell(
                        plan,
                        cells,
                        source.instance,
                        source.local,
                        source.generation,
                        budget,
                    )?;
                    self.object_cells[cell] = true;
                    let projections = plan
                        .projections
                        .get(source.first..argument_sum_v1(&[source.first, source.count])?)
                        .ok_or_else(source_backing_error_v29)?;
                    let target = self.project(types, root, projections, budget)?;
                    if self.rows[target].ty != source.ty {
                        return Err(source_backing_error_v29());
                    }
                    if let Some(previous) = self.rows[value].pointee {
                        self.join(previous, target, budget)?;
                    } else {
                        self.rows[value].pointee = Some(target);
                    }
                }
                self.rows[value].space = Some(AddressSpace::Private);
            }
            SourceReferenceNodeKindV29::Aggregate { first, count } => {
                let shape = types[row.ty.index() as usize].shape();
                let fields = match shape {
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                        if fields.fields().len() != count {
                            return Err(source_backing_error_v29());
                        }
                        Some(fields.fields())
                    }
                    SemanticTypeShapeV1::Array { length, .. } if *length == count as u64 => None,
                    _ => return Err(source_backing_error_v29()),
                };
                for offset in 0..count {
                    budget.charge_work(2)?;
                    let child = *plan
                        .children
                        .get(argument_sum_v1(&[first, offset])?)
                        .ok_or_else(source_backing_error_v29)?;
                    if child >= node {
                        return Err(source_backing_error_v29());
                    }
                    let expected = match fields {
                        Some(fields) => fields[offset],
                        None => match shape {
                            SemanticTypeShapeV1::Array { element, .. } => *element,
                            _ => return Err(source_backing_error_v29()),
                        },
                    };
                    if plan.nodes[child].ty != expected {
                        return Err(source_backing_error_v29());
                    }
                    if let Some(input) = self.ensure_node(plan, cells, child, depth + 1, budget)? {
                        let key = if fields.is_some() {
                            u32::try_from(offset).map_err(|_| ArgumentResourceV1::Arithmetic)?
                        } else {
                            0
                        };
                        let field = self.child(value, key, expected, None, budget)?;
                        self.join_value(field, input, budget)?;
                    }
                }
            }
            SourceReferenceNodeKindV29::Enum { first, count } => {
                if count == 0 {
                    return Err(source_backing_error_v29());
                }
                for offset in 0..count {
                    budget.charge_work(3)?;
                    let member = *plan
                        .enum_members
                        .get(argument_sum_v1(&[first, offset])?)
                        .ok_or_else(source_backing_error_v29)?;
                    let alternative = *plan
                        .enum_alternatives
                        .get(member)
                        .ok_or_else(source_backing_error_v29)?;
                    if alternative.ty != row.ty {
                        return Err(source_backing_error_v29());
                    }
                    let fields = Self::fields(types, row.ty, Some(alternative.variant))?;
                    let payload = self.child(
                        value,
                        alternative.variant,
                        row.ty,
                        Some(alternative.variant),
                        budget,
                    )?;
                    if let Some(source) = alternative.opaque {
                        let opaque = plan
                            .nodes
                            .get(source)
                            .ok_or_else(source_backing_error_v29)?;
                        if source >= node
                            || opaque.ty != row.ty
                            || opaque.inactive.is_some()
                            || !matches!(opaque.kind, SourceReferenceNodeKindV29::Plain(_))
                            || alternative.first != 0
                            || alternative.count != 0
                        {
                            return Err(source_backing_error_v29());
                        }
                        let payload = self.root(payload, budget)?;
                        self.rows[payload].original = true;
                        continue;
                    }
                    if fields.len() != alternative.count {
                        return Err(source_backing_error_v29());
                    }
                    for (offset, &ty) in fields.iter().enumerate() {
                        budget.charge_work(2)?;
                        let child = *plan
                            .children
                            .get(argument_sum_v1(&[alternative.first, offset])?)
                            .ok_or_else(source_backing_error_v29)?;
                        if child >= node || plan.nodes[child].ty != ty {
                            return Err(source_backing_error_v29());
                        }
                        if let Some(input) =
                            self.ensure_node(plan, cells, child, depth + 1, budget)?
                        {
                            let field = self.child(
                                payload,
                                u32::try_from(offset)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                ty,
                                None,
                                budget,
                            )?;
                            self.join_value(field, input, budget)?;
                        }
                    }
                }
            }
            SourceReferenceNodeKindV29::EnumView(view) => {
                let view = *plan
                    .enum_views
                    .get(view)
                    .ok_or_else(source_backing_error_v29)?;
                if view.source >= node || view.child_count == 0 {
                    return Err(source_backing_error_v29());
                }
                for offset in 0..view.child_count {
                    budget.charge_work(2)?;
                    let child = *plan
                        .children
                        .get(argument_sum_v1(&[view.children, offset])?)
                        .ok_or_else(source_backing_error_v29)?;
                    if child >= node || plan.nodes[child].ty != row.ty {
                        return Err(source_backing_error_v29());
                    }
                    if let Some(input) = self.ensure_node(plan, cells, child, depth + 1, budget)? {
                        self.join_value(value, input, budget)?;
                    }
                }
            }
        }
        self.root(value, budget).map(Some)
    }
}
