struct InitializationFact<'layout, 'source> {
    place: SourceStorageSubobjectV29<'layout, 'source>,
    initialized: bool,
    guards: Vec<usize>,
}

struct EnumState<'layout, 'source> {
    place: SourceStorageSubobjectV29<'layout, 'source>,
    construction: Option<u32>,
    readable: Vec<u32>,
    guards: Vec<usize>,
}

// The original A/C1 planner remains the CFG and loan authority. This object is
// its field-sensitive transfer state, not a second source interpreter.
pub(super) struct SourceStorageStateV29<'layout, 'source> {
    layouts: &'layout SourceStorageLayoutsV29<'source>,
    instances: &'layout ProductionCallInstancePlanV1<'source>,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    // Optional representation identity, never allocation or nominal authority.
    physical_root: Option<StorageLayoutIdV1>,
    live: bool,
    // A finite transfer relation, not a dynamic incarnation number. Once this
    // transfer kills incoming loans, revisiting StorageLive cannot undo it.
    kills_incoming_loans: bool,
    has_selections: bool,
    facts: Vec<InitializationFact<'layout, 'source>>,
    initialized_bytes: Vec<SourceStorageRangeV29>,
    enums: Vec<EnumState<'layout, 'source>>,
}

fn prefix_path(
    prefix: &[SubobjectStep],
    value: &[SubobjectStep],
    lease: &Lease,
    budget: &mut Budget<'_>,
) -> Result<bool, Error> {
    lease.work(
        argument_sum_v1(&[prefix.len().min(value.len()), 1])?,
        budget,
    )?;
    Ok(value.starts_with(prefix))
}

impl<'layout, 'source> SourceStorageStateV29<'layout, 'source> {
    pub(super) fn new(
        layouts: &'layout SourceStorageLayoutsV29<'source>,
        instances: &'layout ProductionCallInstancePlanV1<'source>,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        layouts.check_owner(instances.owner(), budget)?;
        let ty = instances
            .instance(instance)
            .and_then(|row| row.declaration().locals().get(local.index() as usize))
            .ok_or_else(|| error("source storage object has no original call-instance local"))?
            .ty();
        let row = layouts.row_id(RowKey::ty(ty), budget)?;
        Self::new_root(layouts, instances, instance, local, ty, Some(row), budget)
    }

    fn new_source(
        layouts: &'layout SourceStorageLayoutsV29<'source>,
        instances: &'layout ProductionCallInstancePlanV1<'source>,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        layouts.check_owner(instances.owner(), budget)?;
        let ty = instances
            .instance(instance)
            .and_then(|row| row.declaration().locals().get(local.index() as usize))
            .ok_or_else(|| error("source logical object has no original call-instance local"))?
            .ty();
        layouts
            .declaration(ty)?
            .layout()
            .size_bytes()
            .ok_or_else(|| error("source logical object has no admitted sized geometry"))?;
        Self::new_root(layouts, instances, instance, local, ty, None, budget)
    }

    fn new_root(
        layouts: &'layout SourceStorageLayoutsV29<'source>,
        instances: &'layout ProductionCallInstancePlanV1<'source>,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        ty: SemanticTypeIdV1,
        physical_root: Option<StorageLayoutIdV1>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        layouts
            .lease
            .reserve(size_of::<Self>() + size_of::<Result<Self, Error>>(), budget)?;
        Ok(Self {
            layouts,
            instances,
            instance,
            local,
            ty,
            physical_root,
            live: true,
            kills_incoming_loans: false,
            has_selections: false,
            facts: Vec::new(),
            initialized_bytes: Vec::new(),
            enums: Vec::new(),
        })
    }

    fn empty_like(&self, budget: &mut Budget<'_>) -> Result<Self, Error> {
        if self.physical_root.is_some() {
            Self::new(
                self.layouts,
                self.instances,
                self.instance,
                self.local,
                budget,
            )
        } else {
            Self::new_source(
                self.layouts,
                self.instances,
                self.instance,
                self.local,
                budget,
            )
        }
    }

    fn root_subobject(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        if self.physical_root.is_some() {
            self.layouts
                .root_subobject(self.layouts.owner, self.ty, budget)
        } else {
            self.layouts
                .source_root_subobject(self.layouts.owner, self.ty, budget)
        }
    }

    fn projected_subobject(
        &self,
        projections: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        if self.physical_root.is_some() {
            self.layouts
                .projected_subobject(self.layouts.owner, self.ty, projections, budget)
        } else {
            self.layouts.source_projected_subobject(
                self.layouts.owner,
                self.ty,
                projections,
                budget,
            )
        }
    }

    fn check_place(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.layouts.lease.work(1, budget)?;
        if !std::ptr::eq(self.layouts, place.layouts) || self.ty != place.root {
            return Err(error(
                "source storage state belongs to a different original object type",
            ));
        }
        if !self.live {
            return Err(error("source storage object lifetime has ended"));
        }
        Ok(())
    }

    fn set_fact(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        initialized: bool,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.set_guarded_fact(place, initialized, &[], budget)
    }

    fn set_guarded_fact(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        initialized: bool,
        guards: &[usize],
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.replace_fact(place, initialized, guards, true, budget)
    }

    fn clear_prefix_default(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.replace_fact(place, false, &[], false, budget)
    }

    fn replace_fact(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        initialized: bool,
        guards: &[usize],
        replace_descendants: bool,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.check_place(place, budget)?;
        self.has_selections |= place.selected;
        self.check_guards(place, initialized, guards, budget)?;
        let inherited = self.initialization_proof(place, false, budget)?;
        let selected_override = initialized && self.selection_requires_override(place, budget)?;
        self.layouts
            .lease
            .reserve(2 * size_of::<Vec<InitializationFact<'_, '_>>>(), budget)?;
        let mut keep = self
            .layouts
            .lease
            .vector(argument_sum_v1(&[self.facts.len(), 1])?, budget)?;
        let old = std::mem::take(&mut self.facts);
        let old_capacity = old.capacity();
        for fact in old {
            let prefix = prefix_path(&place.path, &fact.place.path, &self.layouts.lease, budget)?;
            if prefix && (replace_descendants || place.path.len() == fact.place.path.len()) {
                fact.place.discard(budget)?;
                self.layouts.lease.discard_vec(fact.guards, budget)?;
            } else {
                self.layouts.lease.push(&mut keep, fact, budget)?;
            }
        }
        // IntoIterator has dropped the old vector backing before its refund.
        let bytes = argument_product_v1(old_capacity, size_of::<InitializationFact<'_, '_>>())?;
        self.layouts.lease.refund(bytes, budget)?;
        self.layouts.lease.work(guards.len(), budget)?;
        if inherited.initialized != initialized || inherited.guards != guards || selected_override {
            let copy = place.copy(budget)?;
            let copied_guards = self.copy_guards(guards, budget)?;
            self.layouts.lease.push(
                &mut keep,
                InitializationFact {
                    place: copy,
                    initialized,
                    guards: copied_guards,
                },
                budget,
            )?;
            let mut index = keep.len() - 1;
            while index > 0 {
                self.layouts.lease.work(
                    argument_sum_v1(&[
                        keep[index - 1]
                            .place
                            .path
                            .len()
                            .min(keep[index].place.path.len()),
                        1,
                    ])?,
                    budget,
                )?;
                if keep[index - 1].place.path <= keep[index].place.path {
                    break;
                }
                keep.swap(index - 1, index);
                index -= 1;
            }
        }
        self.facts = keep;
        inherited.discard(&self.layouts.lease, budget)?;
        self.layouts
            .lease
            .refund(2 * size_of::<Vec<InitializationFact<'_, '_>>>(), budget)
    }

    fn direct_fact(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(bool, bool), Error> {
        let (mut depth, mut value, mut descendant) = (0, false, false);
        for fact in &self.facts {
            if prefix_path(&fact.place.path, &place.path, &self.layouts.lease, budget)?
                && fact.place.path.len() >= depth
            {
                depth = fact.place.path.len();
                value = fact.initialized && fact.place.path.len() >= place.initialization_floor;
            }
            if fact.place.path.len() > place.path.len()
                && prefix_path(&place.path, &fact.place.path, &self.layouts.lease, budget)?
            {
                descendant = true;
            }
        }
        Ok((value, descendant))
    }

    fn enum_entry(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<Option<usize>, Error> {
        for (index, state) in self.enums.iter().enumerate() {
            self.layouts.lease.work(
                argument_sum_v1(&[place.path.len().min(state.place.path.len()), 1])?,
                budget,
            )?;
            if place.path == state.place.path {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    pub(super) fn is_initialized(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        enum Task<'a, 'b> {
            Query(SourceStorageSubobjectV29<'a, 'b>),
            Reduce {
                all: bool,
                empty: bool,
                count: usize,
            },
        }
        self.check_place(place, budget)?;
        let lease = &self.layouts.lease;
        let headers = size_of::<Vec<Task<'_, '_>>>() + size_of::<Vec<bool>>();
        lease.reserve(headers, budget)?;
        let mut tasks = lease.vector(1, budget)?;
        let mut answers = lease.vector(1, budget)?;
        lease.push(&mut tasks, Task::Query(place.copy(budget)?), budget)?;
        while let Some(task) = tasks.pop() {
            lease.work(1, budget)?;
            let current = match task {
                Task::Reduce { all, empty, count } => {
                    lease.work(count, budget)?;
                    let start = answers
                        .len()
                        .checked_sub(count)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let answer = if count == 0 {
                        empty
                    } else if all {
                        answers[start..].iter().all(|answer| *answer)
                    } else {
                        answers[start..].iter().any(|answer| *answer)
                    };
                    answers.truncate(start);
                    lease.push(&mut answers, answer, budget)?;
                    continue;
                }
                Task::Query(current) => current,
            };
            if self.selection_conflict(&current, budget)? {
                lease.push(&mut answers, false, budget)?;
                current.discard(budget)?;
                continue;
            }
            let (direct, descendant) = self.direct_fact(&current, budget)?;
            if direct && !descendant {
                lease.push(&mut answers, true, budget)?;
                current.discard(budget)?;
                continue;
            }
            if !direct && !descendant && current.variant.is_none() {
                if current.selected
                    && let Some((parent, depth, count)) =
                        self.concrete_selection_domain(&current, budget)?
                {
                    lease.push(
                        &mut tasks,
                        Task::Reduce {
                            all: true,
                            empty: false,
                            count,
                        },
                        budget,
                    )?;
                    for index in 0..count {
                        lease.work(1, budget)?;
                        let mut concrete = self.layouts.project_step(
                            &parent,
                            SubobjectStep::Element(index as u64),
                            budget,
                        )?;
                        for &step in &current.path[depth + 1..] {
                            lease.work(1, budget)?;
                            let next = self.layouts.project_step(&concrete, step, budget)?;
                            concrete.discard(budget)?;
                            concrete = next;
                        }
                        // Re-enter the ordinary typed query, including all
                        // may-alias holes, enum guards, and union floors.
                        lease.push(&mut tasks, Task::Query(concrete), budget)?;
                    }
                    parent.discard(budget)?;
                    current.discard(budget)?;
                    continue;
                }
                let mut active_descendant = false;
                for state in &self.enums {
                    lease.work(1, budget)?;
                    if !state.readable.is_empty()
                        && prefix_path(&current.path, &state.place.path, lease, budget)?
                    {
                        active_descendant = true;
                        break;
                    }
                }
                // An absent sparse fact cannot initialize any descendant.
                // Avoid expanding a repeated record DAG into physical objects.
                if !active_descendant {
                    lease.push(&mut answers, false, budget)?;
                    current.discard(budget)?;
                    continue;
                }
            }
            let reduce = tasks.len();
            lease.push(
                &mut tasks,
                Task::Reduce {
                    all: true,
                    empty: direct,
                    count: 0,
                },
                budget,
            )?;
            let (mut all, mut empty, mut children) = (true, direct, 0_usize);
            if direct {
                // Sparse overrides, not array cardinality, bound this walk.
                lease.reserve(size_of::<Vec<SubobjectStep>>(), budget)?;
                let mut affected = lease.vector(self.facts.len(), budget)?;
                for fact in &self.facts {
                    lease.work(1, budget)?;
                    if fact.place.path.len() > current.path.len()
                        && prefix_path(&current.path, &fact.place.path, lease, budget)?
                    {
                        lease.push(&mut affected, fact.place.path[current.path.len()], budget)?;
                    }
                }
                sort_rows(&mut affected, lease, budget)?;
                let mut previous = None;
                for &step in &affected {
                    lease.work(1, budget)?;
                    if previous == Some(step) {
                        continue;
                    }
                    previous = Some(step);
                    if let SubobjectStep::Variant(variant) = step
                        && let Some(index) = self.enum_entry(&current, budget)?
                        && !self.enums[index].readable.is_empty()
                    {
                        lease.work(self.enums[index].readable.len(), budget)?;
                        if !self.enums[index].readable.contains(&variant) {
                            continue;
                        }
                    }
                    let child = self.layouts.project_step(&current, step, budget)?;
                    lease.push(&mut tasks, Task::Query(child), budget)?;
                    children = children
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                }
                lease.discard_vec(affected, budget)?;
                lease.refund(size_of::<Vec<SubobjectStep>>(), budget)?;
            } else {
                let declaration = self.layouts.declaration(current.ty)?;
                let count = match declaration.shape() {
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                        fields.fields().len()
                    }
                    SemanticTypeShapeV1::Union(fields) => {
                        // A union value needs one initialized typed field, not
                        // all overlapping field interpretations at once.
                        all = false;
                        fields.fields().len()
                    }
                    SemanticTypeShapeV1::Array { length, .. } => {
                        if *length > self.facts.len() as u64 {
                            0
                        } else {
                            usize::try_from(*length).map_err(|_| ArgumentResourceV1::Arithmetic)?
                        }
                    }
                    SemanticTypeShapeV1::Enum { variants, .. } => {
                        if let Some(variant) = current.variant {
                            empty = true;
                            variants
                                .get(variant as usize)
                                .ok_or_else(|| error("source initialized variant is absent"))?
                                .fields()
                                .fields()
                                .len()
                        } else {
                            if let Some(entry) = self.enum_entry(&current, budget)? {
                                for &variant in &self.enums[entry].readable {
                                    let child = self.layouts.project_step(
                                        &current,
                                        SubobjectStep::Variant(variant),
                                        budget,
                                    )?;
                                    lease.push(&mut tasks, Task::Query(child), budget)?;
                                    children = children
                                        .checked_add(1)
                                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                                }
                            }
                            0
                        }
                    }
                    _ => 0,
                };
                for index in 0..count {
                    let step = if matches!(declaration.shape(), SemanticTypeShapeV1::Array { .. }) {
                        SubobjectStep::Element(index as u64)
                    } else {
                        SubobjectStep::Field(
                            u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        )
                    };
                    let child = self.layouts.project_step(&current, step, budget)?;
                    lease.push(&mut tasks, Task::Query(child), budget)?;
                    children = children
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                }
            }
            tasks[reduce] = Task::Reduce {
                all,
                empty,
                count: children,
            };
            current.discard(budget)?;
        }
        if answers.len() != 1 {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let result = answers[0];
        lease.discard_vec(answers, budget)?;
        lease.discard_vec(tasks, budget)?;
        lease.refund(headers, budget)?;
        Ok(result)
    }

    fn update_bytes(
        &mut self,
        range: SourceStorageRangeV29,
        initialized: bool,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        let extent = self
            .layouts
            .declaration(self.ty)?
            .layout()
            .size_bytes()
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        range.check(extent)?;
        if range.start == range.end {
            return Ok(());
        }
        let capacity = argument_sum_v1(&[self.initialized_bytes.len(), 2])?;
        self.layouts
            .lease
            .reserve(size_of::<Vec<SourceStorageRangeV29>>(), budget)?;
        let mut next = self.layouts.lease.vector(capacity, budget)?;
        for &old in &self.initialized_bytes {
            self.layouts.lease.work(1, budget)?;
            if !initialized && old.overlaps(range) {
                if old.start < range.start {
                    self.layouts.lease.push(
                        &mut next,
                        SourceStorageRangeV29 {
                            start: old.start,
                            end: range.start,
                        },
                        budget,
                    )?;
                }
                if range.end < old.end {
                    self.layouts.lease.push(
                        &mut next,
                        SourceStorageRangeV29 {
                            start: range.end,
                            end: old.end,
                        },
                        budget,
                    )?;
                }
            } else {
                self.layouts.lease.push(&mut next, old, budget)?;
            }
        }
        if initialized {
            self.layouts.lease.push(&mut next, range, budget)?;
        }
        sort_rows(&mut next, &self.layouts.lease, budget)?;
        let mut count = 0;
        for index in 0..next.len() {
            self.layouts.lease.work(1, budget)?;
            let row = next[index];
            if count > 0 && row.start <= next[count - 1].end {
                next[count - 1].end = next[count - 1].end.max(row.end);
            } else {
                next[count] = row;
                count += 1;
            }
        }
        next.truncate(count);
        let old = std::mem::replace(&mut self.initialized_bytes, next);
        self.layouts.lease.discard_vec(old, budget)?;
        self.layouts
            .lease
            .refund(size_of::<Vec<SourceStorageRangeV29>>(), budget)
    }

    pub(super) fn bytes_initialized(
        &self,
        range: SourceStorageRangeV29,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        self.layouts.lease.check(budget)?;
        range.check(
            self.layouts
                .declaration(self.ty)?
                .layout()
                .size_bytes()
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        )?;
        if !self.live {
            return Ok(false);
        }
        if self.has_selections {
            for fact in &self.facts {
                self.layouts.lease.work(1, budget)?;
                if fact.place.selected && fact.place.range.overlaps(range) {
                    // A symbolic envelope is never a physical byte mask.
                    return Ok(false);
                }
            }
        }
        if range.start == range.end {
            return Ok(true);
        }
        for &known in &self.initialized_bytes {
            self.layouts.lease.work(1, budget)?;
            if known.contains(range) {
                return Ok(true);
            }
            if known.start > range.start {
                break;
            }
        }
        for fact in &self.facts {
            self.layouts.lease.work(1, budget)?;
            if !fact.initialized
                || !fact.place.range.contains(range)
                || !self.guards_hold(&fact.place, &fact.guards, budget)?
            {
                continue;
            }
            let mut overwritten = false;
            for hole in &self.facts {
                self.layouts.lease.work(1, budget)?;
                if !hole.initialized
                    && hole.place.range.overlaps(range)
                    && prefix_path(
                        &fact.place.path,
                        &hole.place.path,
                        &self.layouts.lease,
                        budget,
                    )?
                {
                    overwritten = true;
                    break;
                }
            }
            if !overwritten
                && self.layouts.data_covers(
                    fact.place.ty,
                    SourceStorageRangeV29 {
                        start: range.start - fact.place.range.start,
                        end: range.end - fact.place.range.start,
                    },
                    budget,
                )?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn initialize(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        if place.has_selection(budget)? {
            self.invalidate_aliases(place, true, budget)?;
            self.update_bytes(place.range, false, budget)?;
            return self.set_fact(place, true, budget);
        }
        self.invalidate_aliases(place, true, budget)?;
        self.update_bytes(place.range, false, budget)?;
        self.set_fact(place, true, budget)?;
        // Aggregate construction and CopyObject retain independent physical
        // masks; assigning a logical whole value never initializes its padding.
        match self.layouts.declaration(place.ty)?.shape() {
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
                self.update_bytes(place.range, true, budget)
            }
            SemanticTypeShapeV1::Pointer(pointer)
                if pointer.metadata() == SemanticPointerMetadataV1::None =>
            {
                self.update_bytes(place.range, true, budget)
            }
            _ => Ok(()),
        }
    }

    pub(super) fn deinitialize(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.invalidate_selection_positives(place, budget)?;
        self.invalidate_aliases(place, false, budget)?;
        self.set_fact(place, false, budget)?;
        self.update_bytes(place.range, false, budget)?;
        Ok(())
    }

    fn invalidate_aliases(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        initialized_write: bool,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.check_place(place, budget)?;
        let mut parent = self.root_subobject(budget)?;
        for &step in &place.path {
            self.layouts.lease.work(1, budget)?;
            if let (SemanticTypeShapeV1::Union(fields), SubobjectStep::Field(selected)) =
                (self.layouts.declaration(parent.ty)?.shape(), step)
            {
                self.clear_prefix_default(&parent, budget)?;
                for index in 0..fields.fields().len() {
                    let index = u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                    if index != selected {
                        let alias = self.layouts.project_step(
                            &parent,
                            SubobjectStep::Field(index),
                            budget,
                        )?;
                        if alias.range.overlaps(place.range) {
                            self.invalidate_union_field_overlaps(&alias, place.range, budget)?;
                        }
                        alias.discard(budget)?;
                    }
                }
            }
            let next = self.layouts.project_step(&parent, step, budget)?;
            parent.discard(budget)?;
            parent = next;
        }
        parent.discard(budget)?;
        for state in &mut self.enums {
            self.layouts.lease.work(1, budget)?;
            let tag_overlap = self
                .layouts
                .enum_tag_range(&state.place)?
                .is_some_and(|tag| tag.overlaps(place.range));
            let preserves_niche =
                initialized_write && self.layouts.preserves_niche_tag(state, place, budget)?;
            if (tag_overlap && !preserves_niche)
                || prefix_path(&place.path, &state.place.path, &self.layouts.lease, budget)?
            {
                self.layouts.lease.work(state.readable.len(), budget)?;
                state.readable.clear();
                state.guards.clear();
                state.construction = None;
            }
        }
        Ok(())
    }

    pub(super) fn is_live(&self, budget: &mut Budget<'_>) -> Result<bool, Error> {
        self.layouts.lease.check(budget)?;
        self.layouts.lease.work(1, budget)?;
        Ok(self.live)
    }

    pub(super) fn storage_dead(&mut self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.layouts.lease.check(budget)?;
        self.live = false;
        self.kills_incoming_loans = true;
        self.has_selections = false;
        let mut facts = std::mem::take(&mut self.facts);
        for fact in facts.drain(..) {
            self.layouts.lease.work(1, budget)?;
            fact.place.discard(budget)?;
            self.layouts.lease.discard_vec(fact.guards, budget)?;
        }
        self.layouts.lease.discard_vec(facts, budget)?;
        self.layouts
            .lease
            .work(self.initialized_bytes.len(), budget)?;
        self.initialized_bytes.clear();
        for state in &mut self.enums {
            self.layouts
                .lease
                .work(argument_sum_v1(&[state.readable.len(), 1])?, budget)?;
            state.readable.clear();
            state.guards.clear();
            state.construction = None;
        }
        Ok(())
    }

    pub(super) fn storage_live(&mut self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.storage_dead(budget)?;
        self.live = true;
        // The A/C1 driver separately invalidates every pre-existing loan. A
        // repeated StorageLive site never restores a prior incarnation's loan.
        Ok(())
    }

    pub(super) fn discard(mut self, budget: &mut Budget<'_>) -> Result<(), Error> {
        let lease = &self.layouts.lease;
        for fact in self.facts.drain(..) {
            fact.place.discard(budget)?;
            lease.discard_vec(fact.guards, budget)?;
        }
        lease.discard_vec(self.facts, budget)?;
        lease.discard_vec(self.initialized_bytes, budget)?;
        for state in self.enums.drain(..) {
            state.place.discard(budget)?;
            lease.discard_vec(state.readable, budget)?;
            lease.discard_vec(state.guards, budget)?;
        }
        lease.discard_vec(self.enums, budget)?;
        lease.refund(size_of::<Self>() + size_of::<Result<Self, Error>>(), budget)
    }

    pub(super) fn copy(&self, budget: &mut Budget<'_>) -> Result<Self, Error> {
        self.layouts.lease.check(budget)?;
        let mut result = self.empty_like(budget)?;
        result.live = self.live;
        result.kills_incoming_loans = self.kills_incoming_loans;
        result.has_selections = self.has_selections;
        result.facts = self.layouts.lease.vector(self.facts.len(), budget)?;
        for fact in &self.facts {
            let copy = InitializationFact {
                place: fact.place.copy(budget)?,
                initialized: fact.initialized,
                guards: self.copy_guards(&fact.guards, budget)?,
            };
            self.layouts.lease.push(&mut result.facts, copy, budget)?;
        }
        result.initialized_bytes = self
            .layouts
            .lease
            .vector(self.initialized_bytes.len(), budget)?;
        self.layouts
            .lease
            .work(self.initialized_bytes.len(), budget)?;
        result
            .initialized_bytes
            .extend_from_slice(&self.initialized_bytes);
        result.enums = self.layouts.lease.vector(self.enums.len(), budget)?;
        for state in &self.enums {
            let mut readable = self.layouts.lease.vector(state.readable.len(), budget)?;
            self.layouts.lease.work(state.readable.len(), budget)?;
            readable.extend_from_slice(&state.readable);
            let copy = EnumState {
                place: state.place.copy(budget)?,
                construction: state.construction,
                readable,
                guards: self.copy_guards(&state.guards, budget)?,
            };
            self.layouts.lease.push(&mut result.enums, copy, budget)?;
        }
        Ok(result)
    }

    fn relocated_place(
        &self,
        destination: &SourceStorageSubobjectV29<'layout, 'source>,
        suffix: &[SubobjectStep],
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        let mut result = destination.copy(budget)?;
        for &step in suffix {
            let next = self.layouts.project_step(&result, step, budget)?;
            result.discard(budget)?;
            result = next;
        }
        Ok(result)
    }

    // A typed partial-object snapshot preserves logical holes, data masks and
    // enum state. Relocations are copied by the parallel correlated domain;
    // this method never supplies pointer or allocation authority.
    pub(super) fn copy_subobject_from(
        &mut self,
        destination: &SourceStorageSubobjectV29<'layout, 'source>,
        source: &Self,
        source_place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.check_place(destination, budget)?;
        source.check_place(source_place, budget)?;
        let selected = destination.has_selection(budget)? || source_place.has_selection(budget)?;
        if !std::ptr::eq(self.layouts, source.layouts)
            || !std::ptr::eq(self.instances, source.instances)
            || destination.ty != source_place.ty
            || destination.variant != source_place.variant
            || (!selected && destination.range.length() != source_place.range.length())
        {
            return Err(error(
                "source object copy changes its source domain or typed subobject",
            ));
        }
        if selected {
            if !source.is_initialized(source_place, budget)? {
                return Err(error("source selected copy reads a partial typed value"));
            }
            // Typed selected copies carry validity, not a guessed byte offset,
            // enum tag, padding mask or reference-origin identity.
            return self.initialize(destination, budget);
        }
        let base = source.initialization_proof(source_place, true, budget)?;
        let base = source.remap_proof(base, source_place, destination, budget)?;
        self.deinitialize(destination, budget)?;
        self.set_guarded_fact(destination, base.initialized, &base.guards, budget)?;
        base.discard(&self.layouts.lease, budget)?;
        for fact in &source.facts {
            self.layouts.lease.work(1, budget)?;
            if fact.place.path.len() > source_place.path.len()
                && prefix_path(
                    &source_place.path,
                    &fact.place.path,
                    &self.layouts.lease,
                    budget,
                )?
            {
                let moved = self.relocated_place(
                    destination,
                    &fact.place.path[source_place.path.len()..],
                    budget,
                )?;
                let proof = source.initialization_proof(&fact.place, true, budget)?;
                let proof = source.remap_proof(proof, source_place, destination, budget)?;
                self.set_guarded_fact(&moved, proof.initialized, &proof.guards, budget)?;
                proof.discard(&self.layouts.lease, budget)?;
                moved.discard(budget)?;
            }
        }
        for &known in &source.initialized_bytes {
            self.layouts.lease.work(1, budget)?;
            let intersection = SourceStorageRangeV29 {
                start: known.start.max(source_place.range.start),
                end: known.end.min(source_place.range.end),
            };
            if intersection.start < intersection.end {
                let start = destination
                    .range
                    .start
                    .checked_add(intersection.start - source_place.range.start)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                self.update_bytes(
                    SourceStorageRangeV29::new(
                        start,
                        intersection.length(),
                        self.layouts
                            .declaration(self.ty)?
                            .layout()
                            .size_bytes()
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                    )?,
                    true,
                    budget,
                )?;
            }
        }
        for state in &source.enums {
            self.layouts.lease.work(1, budget)?;
            if prefix_path(
                &source_place.path,
                &state.place.path,
                &self.layouts.lease,
                budget,
            )? {
                let moved = self.relocated_place(
                    destination,
                    &state.place.path[source_place.path.len()..],
                    budget,
                )?;
                let index = self.enum_state_mut(&moved, budget)?;
                let proof = InitializationProofV29::new(
                    !state.readable.is_empty(),
                    &state.guards,
                    &self.layouts.lease,
                    budget,
                )?;
                let proof = source.remap_proof(proof, source_place, destination, budget)?;
                let count = if proof.initialized {
                    state.readable.len()
                } else {
                    0
                };
                let mut readable = self.layouts.lease.vector(count, budget)?;
                self.layouts.lease.work(count, budget)?;
                readable.extend_from_slice(&state.readable[..count]);
                let old = std::mem::replace(&mut self.enums[index].readable, readable);
                self.layouts.lease.discard_vec(old, budget)?;
                let guards = self.copy_guards(&proof.guards, budget)?;
                let old = std::mem::replace(&mut self.enums[index].guards, guards);
                self.layouts.lease.discard_vec(old, budget)?;
                self.enums[index].construction = state.construction;
                proof.discard(&self.layouts.lease, budget)?;
                moved.discard(budget)?;
            }
        }
        Ok(())
    }

    pub(super) fn copy_subobject_within(
        &mut self,
        destination: &SourceStorageSubobjectV29<'layout, 'source>,
        source_place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        let snapshot = self.copy(budget)?;
        self.copy_subobject_from(destination, &snapshot, source_place, budget)?;
        snapshot.discard(budget)
    }

    pub(super) fn equivalent(&self, other: &Self, budget: &mut Budget<'_>) -> Result<bool, Error> {
        self.layouts.lease.check(budget)?;
        if !std::ptr::eq(self.layouts, other.layouts)
            || !std::ptr::eq(self.instances, other.instances)
            || self.instance != other.instance
            || self.local != other.local
            || self.physical_root != other.physical_root
            || self.live != other.live
            || self.kills_incoming_loans != other.kills_incoming_loans
            || self.facts.len() != other.facts.len()
            || self.initialized_bytes.len() != other.initialized_bytes.len()
            || self.enums.len() != other.enums.len()
        {
            return Ok(false);
        }
        for (left, right) in self.facts.iter().zip(&other.facts) {
            self.layouts.lease.work(
                argument_sum_v1(&[left.place.path.len().min(right.place.path.len()), 1])?,
                budget,
            )?;
            self.layouts
                .lease
                .work(left.guards.len().min(right.guards.len()), budget)?;
            if left.initialized != right.initialized
                || left.place.path != right.place.path
                || left.guards != right.guards
            {
                return Ok(false);
            }
        }
        self.layouts
            .lease
            .work(self.initialized_bytes.len(), budget)?;
        if self.initialized_bytes != other.initialized_bytes {
            return Ok(false);
        }
        for state in &self.enums {
            let Some(index) = other.enum_entry(&state.place, budget)? else {
                return Ok(false);
            };
            self.layouts
                .lease
                .work(argument_sum_v1(&[state.readable.len(), 1])?, budget)?;
            self.layouts.lease.work(state.guards.len(), budget)?;
            if state.construction != other.enums[index].construction
                || state.readable != other.enums[index].readable
                || state.guards != other.enums[index].guards
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn conditional_initialization_proof(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<InitializationProofV29, Error> {
        if self.selection_conflict(place, budget)? {
            return InitializationProofV29::new(false, &[], &self.layouts.lease, budget);
        }
        // Preserve the correlation between a variant choice and its payload.
        // A path that cannot select this variant contributes no counterexample
        // to initialization conditional on selecting the variant.
        if let Some(depth) = self.excluded_guard(place, budget)? {
            return InitializationProofV29::new(true, &[depth], &self.layouts.lease, budget);
        }
        // Join prefix defaults, then the finite union of overridden paths. A
        // whole-object read answer would erase initialized siblings whenever
        // a single descendant is moved on either predecessor.
        self.initialization_proof(place, true, budget)
    }

    pub(super) fn join(&self, other: &Self, budget: &mut Budget<'_>) -> Result<Self, Error> {
        self.layouts.lease.check(budget)?;
        if !std::ptr::eq(self.layouts, other.layouts)
            || !std::ptr::eq(self.instances, other.instances)
            || self.instance != other.instance
            || self.local != other.local
            || self.ty != other.ty
            || self.physical_root != other.physical_root
        {
            return Err(error(
                "source storage state join crosses source object layout custody",
            ));
        }
        let mut result = self.empty_like(budget)?;
        result.live = self.live && other.live;
        result.kills_incoming_loans = self.kills_incoming_loans || other.kills_incoming_loans;
        if !result.live {
            return Ok(result);
        }
        let capacity = argument_sum_v1(&[self.facts.len(), other.facts.len(), 1])?;
        self.layouts
            .lease
            .reserve(size_of::<Vec<(usize, usize)>>(), budget)?;
        let mut order = self.layouts.lease.vector(capacity, budget)?;
        self.layouts
            .lease
            .push(&mut order, (0_usize, 0_usize), budget)?;
        // Both fact rosters are path-sorted. Merge their union before ordering
        // parents before children; replaying duplicate paths grows spare backing.
        let (mut left, mut right) = (0, 0);
        while left < self.facts.len() || right < other.facts.len() {
            let (fact, index) = match (self.facts.get(left), other.facts.get(right)) {
                (Some(a), Some(b)) => {
                    self.layouts.lease.work(
                        argument_sum_v1(&[a.place.path.len().min(b.place.path.len()), 1])?,
                        budget,
                    )?;
                    match a.place.path.cmp(&b.place.path) {
                        std::cmp::Ordering::Less => {
                            left += 1;
                            (a, left)
                        }
                        std::cmp::Ordering::Equal => {
                            left += 1;
                            right += 1;
                            (a, left)
                        }
                        std::cmp::Ordering::Greater => {
                            right += 1;
                            (b, self.facts.len() + right)
                        }
                    }
                }
                (Some(a), None) => {
                    self.layouts.lease.work(1, budget)?;
                    left += 1;
                    (a, left)
                }
                (None, Some(b)) => {
                    self.layouts.lease.work(1, budget)?;
                    right += 1;
                    (b, self.facts.len() + right)
                }
                (None, None) => break,
            };
            if !fact.place.path.is_empty() {
                self.layouts
                    .lease
                    .push(&mut order, (fact.place.path.len(), index), budget)?;
            }
        }
        sort_rows(&mut order, &self.layouts.lease, budget)?;
        let root = self.root_subobject(budget)?;
        for &(_, index) in &order {
            let place = if index == 0 {
                &root
            } else if index - 1 < self.facts.len() {
                &self.facts[index - 1].place
            } else {
                &other.facts[index - 1 - self.facts.len()].place
            };
            let a = self.conditional_initialization_proof(place, budget)?;
            let b = other.conditional_initialization_proof(place, budget)?;
            let proof = a.intersect(&b, &self.layouts.lease, budget)?;
            result.set_guarded_fact(place, proof.initialized, &proof.guards, budget)?;
            proof.discard(&self.layouts.lease, budget)?;
            a.discard(&self.layouts.lease, budget)?;
            b.discard(&self.layouts.lease, budget)?;
        }
        root.discard(budget)?;
        self.layouts.lease.discard_vec(order, budget)?;
        self.layouts
            .lease
            .refund(size_of::<Vec<(usize, usize)>>(), budget)?;
        let (mut left, mut right) = (0, 0);
        result.initialized_bytes = self.layouts.lease.vector(
            argument_sum_v1(&[self.initialized_bytes.len(), other.initialized_bytes.len()])?,
            budget,
        )?;
        while left < self.initialized_bytes.len() && right < other.initialized_bytes.len() {
            self.layouts.lease.work(1, budget)?;
            let (a, b) = (self.initialized_bytes[left], other.initialized_bytes[right]);
            let intersection = SourceStorageRangeV29 {
                start: a.start.max(b.start),
                end: a.end.min(b.end),
            };
            if intersection.start < intersection.end {
                self.layouts
                    .lease
                    .push(&mut result.initialized_bytes, intersection, budget)?;
            }
            if a.end <= b.end {
                left += 1;
            } else {
                right += 1;
            }
        }
        for state in self.enums.iter().chain(&other.enums) {
            result.join_enum_readability(self, other, &state.place, budget)?;
        }
        Ok(result)
    }
}

include!("production_source_storage_initialization_v29.rs");
