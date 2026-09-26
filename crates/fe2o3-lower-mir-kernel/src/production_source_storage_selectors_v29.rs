// Selection IDs are canonical within the original C1 plan. Only the concrete
// root's checked Plan adapter can turn an original IndexUse into this path step.
const UNKNOWN_SELECTION_V29: usize = usize::MAX;

impl SourceStorageSubobjectV29<'_, '_> {
    fn has_selection(&self, _budget: &mut Budget<'_>) -> Result<bool, Error> {
        Ok(self.selected)
    }
}

fn selected_paths_overlap(
    left: &[SubobjectStep],
    right: &[SubobjectStep],
    lease: &Lease,
    budget: &mut Budget<'_>,
) -> Result<bool, Error> {
    lease.work(argument_sum_v1(&[left.len().min(right.len()), 1])?, budget)?;
    for (&a, &b) in left.iter().zip(right) {
        if a == b {
            continue;
        }
        match (a, b) {
            (
                SubobjectStep::Selection(_),
                SubobjectStep::Selection(_) | SubobjectStep::Element(_),
            )
            | (SubobjectStep::Element(_), SubobjectStep::Selection(_)) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

impl<'layout, 'source> SourceStorageStateV29<'layout, 'source> {
    // The sparse constructor can prove an array one concrete element at a
    // time, without creating a whole-array default. Only expand a selection
    // when those existing facts cover its entire concrete domain.
    fn concrete_selection_domain(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(SourceStorageSubobjectV29<'layout, 'source>, usize, usize)>, Error> {
        self.layouts.lease.work(place.path.len(), budget)?;
        let Some(depth) = place.path.iter().position(|step| matches!(step, SubobjectStep::Selection(_))) else {
            return Ok(None);
        };
        let mut parent = self.root_subobject(budget)?;
        for &step in &place.path[..depth] {
            self.layouts.lease.work(1, budget)?;
            let next = self.layouts.project_step(&parent, step, budget)?;
            parent.discard(budget)?;
            parent = next;
        }
        let SemanticTypeShapeV1::Array { length, .. } = self.layouts.declaration(parent.ty)?.shape() else {
            return Err(error("source selection coverage lost its original array"));
        };
        let length = *length;
        if length == 0 || length > self.facts.len() as u64 {
            parent.discard(budget)?;
            return Ok(None);
        }
        let mut covered = 0_u64;
        for fact in &self.facts {
            self.layouts.lease.work(1, budget)?;
            if !fact.initialized || !prefix_path(&parent.path, &fact.place.path, &self.layouts.lease, budget)? {
                continue;
            }
            let Some(SubobjectStep::Element(index)) = fact.place.path.get(depth) else {
                continue;
            };
            // Facts are canonical path-sorted. Several fields can cover the
            // same element; a missing element cannot be filled by another.
            if *index == covered {
                covered += 1;
            } else if *index > covered {
                break;
            }
        }
        if covered != length {
            parent.discard(budget)?;
            return Ok(None);
        }
        let count = usize::try_from(length).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        Ok(Some((parent, depth, count)))
    }

    fn selected_subobject(
        &self,
        projections: &[SemanticProjectionV1],
        references: &SourceReferencePlanV29<'_, 'source>,
        source: Option<(SourceReferenceSiteV29, usize)>,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        if !std::ptr::eq(self.instances, references.instances) {
            return Err(error(
                "source selection changed its original call-instance owner",
            ));
        }
        let mut place = self.root_subobject(budget)?;
        let mut alias_path = false;
        let mut selected = false;
        for (ordinal, projection) in projections.iter().enumerate() {
            self.layouts.lease.work(2, budget)?;
            alias_path |= matches!(
                self.layouts.declaration(place.ty)?.shape(),
                SemanticTypeShapeV1::Union(_) | SemanticTypeShapeV1::Enum { .. }
            );
            let next = if let SemanticProjectionKindV1::Index(local) = projection.kind() {
                if alias_path {
                    return Err(error(
                        "source dynamic enum or union selection needs guarded alias correspondence",
                    ));
                }
                let row = references.selector_for_path(source, ordinal, budget)?;
                let original = row.check(self.instances, budget)?;
                if row.instance != self.instance
                    || original.local() != self.local
                    || row.array != place.ty
                    || row.element != projection.result_type()
                    || row.local != local
                {
                    return Err(error(
                        "source selected path changed its original array projection",
                    ));
                }
                selected = true;
                self.layouts.project_step(
                    &place,
                    SubobjectStep::Selection(row.canonical),
                    budget,
                )?
            } else {
                if selected && alias_path {
                    return Err(error(
                        "source dynamic enum or union selection needs guarded alias correspondence",
                    ));
                }
                self.layouts.project_subobject(
                    place.copy(budget)?,
                    std::slice::from_ref(projection),
                    budget,
                )?
            };
            if next.ty != projection.result_type() {
                return Err(error(
                    "source selected path changed its original result type",
                ));
            }
            place.discard(budget)?;
            place = next;
        }
        Ok(place)
    }

    // Incompatible index expressions can denote the same element. Exact prefix
    // holes remain handled by the existing sparse descendant reduction.
    fn selection_conflict(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        if !self.has_selections && !place.selected {
            return Ok(false);
        }
        for fact in &self.facts {
            self.layouts.lease.work(1, budget)?;
            if fact.initialized
                || (!fact.place.has_selection(budget)? && !place.has_selection(budget)?)
            {
                continue;
            }
            if !prefix_path(&fact.place.path, &place.path, &self.layouts.lease, budget)?
                && !prefix_path(&place.path, &fact.place.path, &self.layouts.lease, budget)?
                && selected_paths_overlap(
                    &fact.place.path,
                    &place.path,
                    &self.layouts.lease,
                    budget,
                )?
            {
                let mut repaired = false;
                for positive in &self.facts {
                    self.layouts.lease.work(1, budget)?;
                    if positive.initialized
                        && positive.guards.is_empty()
                        && prefix_path(
                            &positive.place.path,
                            &place.path,
                            &self.layouts.lease,
                            budget,
                        )?
                        && !prefix_path(
                            &positive.place.path,
                            &fact.place.path,
                            &self.layouts.lease,
                            budget,
                        )?
                    {
                        // Later may-overlapping deinitialization removes this
                        // positive. An ancestor default does not prove repair.
                        repaired = true;
                        break;
                    }
                }
                if !repaired {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn selection_requires_override(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        if !self.has_selections && !place.selected {
            return Ok(false);
        }
        for fact in &self.facts {
            self.layouts.lease.work(1, budget)?;
            if !fact.initialized
                && (fact.place.selected || place.selected)
                && !prefix_path(&fact.place.path, &place.path, &self.layouts.lease, budget)?
                && !prefix_path(&place.path, &fact.place.path, &self.layouts.lease, budget)?
                && selected_paths_overlap(
                    &fact.place.path,
                    &place.path,
                    &self.layouts.lease,
                    budget,
                )?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn invalidate_selection_positives(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        if !self.has_selections && !place.selected {
            return Ok(());
        }
        let mut index = 0;
        while index < self.facts.len() {
            self.layouts.lease.work(1, budget)?;
            let fact = &self.facts[index];
            let remove = fact.initialized
                && (fact.place.has_selection(budget)? || place.has_selection(budget)?)
                && !prefix_path(&fact.place.path, &place.path, &self.layouts.lease, budget)?
                && !prefix_path(&place.path, &fact.place.path, &self.layouts.lease, budget)?
                && selected_paths_overlap(
                    &fact.place.path,
                    &place.path,
                    &self.layouts.lease,
                    budget,
                )?;
            if remove {
                self.layouts.lease.work(self.facts.len() - index, budget)?;
                let fact = self.facts.remove(index);
                fact.place.discard(budget)?;
                self.layouts.lease.discard_vec(fact.guards, budget)?;
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    fn forget_selectors(&mut self, keys: &[usize], budget: &mut Budget<'_>) -> Result<(), Error> {
        let mut index = 0;
        while index < self.facts.len() {
            let mut affected = false;
            for step in &mut self.facts[index].place.path {
                self.layouts
                    .lease
                    .work(argument_sum_v1(&[keys.len(), 1])?, budget)?;
                if let SubobjectStep::Selection(key) = step {
                    if *key != UNKNOWN_SELECTION_V29 && keys.contains(key) {
                        *key = UNKNOWN_SELECTION_V29;
                        affected = true;
                    }
                }
            }
            if affected && self.facts[index].initialized {
                self.layouts.lease.work(self.facts.len() - index, budget)?;
                let fact = self.facts.remove(index);
                fact.place.discard(budget)?;
                self.layouts.lease.discard_vec(fact.guards, budget)?;
            } else {
                index += 1;
            }
        }
        // Widening a negative selector must not restore an outer true default.
        // Canonicalize in place; no history or per-element facts are appended.
        for index in 1..self.facts.len() {
            let mut at = index;
            while at > 0 {
                self.layouts.lease.work(
                    argument_sum_v1(&[
                        self.facts[at - 1]
                            .place
                            .path
                            .len()
                            .min(self.facts[at].place.path.len()),
                        1,
                    ])?,
                    budget,
                )?;
                if self.facts[at - 1].place.path <= self.facts[at].place.path {
                    break;
                }
                self.facts.swap(at - 1, at);
                at -= 1;
            }
        }
        let mut index = 1;
        while index < self.facts.len() {
            self.layouts.lease.work(
                argument_sum_v1(&[
                    self.facts[index - 1]
                        .place
                        .path
                        .len()
                        .min(self.facts[index].place.path.len()),
                    1,
                ])?,
                budget,
            )?;
            if self.facts[index - 1].place.path == self.facts[index].place.path {
                self.facts[index - 1].initialized &= self.facts[index].initialized;
                self.layouts.lease.work(self.facts.len() - index, budget)?;
                let fact = self.facts.remove(index);
                fact.place.discard(budget)?;
                self.layouts.lease.discard_vec(fact.guards, budget)?;
            } else {
                index += 1;
            }
        }
        Ok(())
    }
}
