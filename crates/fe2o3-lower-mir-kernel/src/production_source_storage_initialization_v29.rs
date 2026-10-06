struct InitializationProofV29 {
    initialized: bool,
    guards: Vec<usize>,
}

impl InitializationProofV29 {
    fn header() -> usize {
        size_of::<Self>() + size_of::<Result<Self, Error>>()
    }

    fn new(
        initialized: bool,
        guards: &[usize],
        lease: &Lease,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        lease.reserve(Self::header(), budget)?;
        let count = if initialized { guards.len() } else { 0 };
        let mut copied = lease.vector(count, budget)?;
        lease.work(count, budget)?;
        copied.extend_from_slice(&guards[..count]);
        Ok(Self {
            initialized,
            guards: copied,
        })
    }

    fn intersect(
        &self,
        other: &Self,
        lease: &Lease,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        if !self.initialized || !other.initialized {
            return Self::new(false, &[], lease, budget);
        }
        lease.reserve(Self::header(), budget)?;
        let mut guards = lease.vector(
            argument_sum_v1(&[self.guards.len(), other.guards.len()])?,
            budget,
        )?;
        let (mut a, mut b) = (0, 0);
        while a < self.guards.len() || b < other.guards.len() {
            lease.work(1, budget)?;
            let value = match (self.guards.get(a), other.guards.get(b)) {
                (Some(left), Some(right)) if left == right => {
                    a += 1;
                    b += 1;
                    *left
                }
                (Some(left), Some(right)) if left < right => {
                    a += 1;
                    *left
                }
                (Some(_), Some(right)) | (None, Some(right)) => {
                    b += 1;
                    *right
                }
                (Some(left), None) => {
                    a += 1;
                    *left
                }
                (None, None) => break,
            };
            lease.push(&mut guards, value, budget)?;
        }
        Ok(Self {
            initialized: true,
            guards,
        })
    }

    fn discard(self, lease: &Lease, budget: &mut Budget<'_>) -> Result<(), Error> {
        lease.discard_vec(self.guards, budget)?;
        lease.refund(Self::header(), budget)
    }
}

impl<'layout, 'source> SourceStorageStateV29<'layout, 'source> {
    pub(super) fn is_readable(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        if !self.is_initialized(place, budget)? {
            return Ok(false);
        }
        self.layouts.lease.work(place.path.len(), budget)?;
        let Some(depth) = place
            .path
            .iter()
            .rposition(|step| matches!(step, SubobjectStep::Variant(_)))
        else {
            return Ok(true);
        };
        // The deepest selector checks its complete enclosing prefix. Logical
        // initialization alone deliberately retains conditional payload facts.
        self.guard_holds(place, depth, budget)
    }

    fn copy_guards(&self, guards: &[usize], budget: &mut Budget<'_>) -> Result<Vec<usize>, Error> {
        let mut copied = self.layouts.lease.vector(guards.len(), budget)?;
        self.layouts.lease.work(guards.len(), budget)?;
        copied.extend_from_slice(guards);
        Ok(copied)
    }

    fn check_guards(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        initialized: bool,
        guards: &[usize],
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        if !initialized && !guards.is_empty() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let mut previous = None;
        for &depth in guards {
            self.layouts.lease.work(1, budget)?;
            if previous.is_some_and(|before| before >= depth)
                || !matches!(place.path.get(depth), Some(SubobjectStep::Variant(_)))
            {
                return Err(error(
                    "source initialization guard is not an original variant path",
                ));
            }
            previous = Some(depth);
        }
        Ok(())
    }

    fn initialization_proof(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        include_exact: bool,
        budget: &mut Budget<'_>,
    ) -> Result<InitializationProofV29, Error> {
        self.check_place(place, budget)?;
        let mut selected: Option<&InitializationFact<'_, '_>> = None;
        for fact in &self.facts {
            self.layouts.lease.work(1, budget)?;
            if (!include_exact && fact.place.path.len() >= place.path.len())
                || !prefix_path(&fact.place.path, &place.path, &self.layouts.lease, budget)?
            {
                continue;
            }
            if selected.is_none_or(|old| old.place.path.len() <= fact.place.path.len()) {
                selected = Some(fact);
            }
        }
        let Some(fact) = selected
            .filter(|fact| fact.initialized && fact.place.path.len() >= place.initialization_floor)
        else {
            return InitializationProofV29::new(false, &[], &self.layouts.lease, budget);
        };
        let mut result =
            InitializationProofV29::new(true, &fact.guards, &self.layouts.lease, budget)?;
        // A valid whole enum only proves the payload of its actual variant.
        for depth in fact.place.path.len()..place.path.len() {
            self.layouts.lease.work(1, budget)?;
            if matches!(place.path[depth], SubobjectStep::Variant(_)) {
                self.layouts.lease.push(&mut result.guards, depth, budget)?;
            }
        }
        Ok(result)
    }

    fn guard_holds(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        if !matches!(place.path.get(depth), Some(SubobjectStep::Variant(_))) {
            return Err(error("source initialization guard depth changed"));
        }
        // An inner selector is meaningful only under its enclosing selectors.
        for at in 0..=depth {
            self.layouts.lease.work(1, budget)?;
            let SubobjectStep::Variant(variant) = place.path[at] else {
                continue;
            };
            let mut found = false;
            for state in &self.enums {
                self.layouts.lease.work(
                    argument_sum_v1(&[at.min(state.place.path.len()), 1])?,
                    budget,
                )?;
                if state.place.path == place.path[..at] {
                    found = state.readable.as_slice() == [variant];
                    break;
                }
            }
            if !found {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn guards_hold(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        guards: &[usize],
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        for &depth in guards {
            self.layouts.lease.work(1, budget)?;
            if !self.guard_holds(place, depth, budget)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn excluded_guard(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<Option<usize>, Error> {
        for (depth, step) in place.path.iter().enumerate() {
            self.layouts.lease.work(1, budget)?;
            let SubobjectStep::Variant(variant) = step else {
                continue;
            };
            for state in &self.enums {
                self.layouts
                    .lease
                    .work(argument_sum_v1(&[depth, 1])?, budget)?;
                if state.place.path == place.path[..depth] && !state.readable.is_empty() {
                    self.layouts.lease.work(state.readable.len(), budget)?;
                    if !state.readable.contains(variant) {
                        return Ok(Some(depth));
                    }
                }
            }
        }
        Ok(None)
    }

    fn enum_readability_proof(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<InitializationProofV29, Error> {
        if let Some(depth) = self.excluded_guard(place, budget)? {
            return InitializationProofV29::new(true, &[depth], &self.layouts.lease, budget);
        }
        match self.enum_entry(place, budget)? {
            Some(index) => InitializationProofV29::new(
                !self.enums[index].readable.is_empty(),
                &self.enums[index].guards,
                &self.layouts.lease,
                budget,
            ),
            None => InitializationProofV29::new(false, &[], &self.layouts.lease, budget),
        }
    }

    fn join_enum_readability(
        &mut self,
        left: &Self,
        right: &Self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        let a = left.enum_readability_proof(place, budget)?;
        let b = right.enum_readability_proof(place, budget)?;
        let proof = a.intersect(&b, &self.layouts.lease, budget)?;
        let index = self.enum_state_mut(place, budget)?;
        self.enums[index].readable.clear();
        self.enums[index].construction = None;
        if proof.initialized {
            for source in [left, right] {
                self.layouts.lease.work(1, budget)?;
                if source.excluded_guard(place, budget)?.is_some() {
                    continue;
                }
                if let Some(from) = source.enum_entry(place, budget)? {
                    for &variant in &source.enums[from].readable {
                        self.layouts.lease.push(
                            &mut self.enums[index].readable,
                            variant,
                            budget,
                        )?;
                    }
                }
            }
            sort_rows(&mut self.enums[index].readable, &self.layouts.lease, budget)?;
            self.layouts
                .lease
                .work(self.enums[index].readable.len(), budget)?;
            self.enums[index].readable.dedup();
        }
        let guards = if self.enums[index].readable.is_empty() {
            self.copy_guards(&[], budget)?
        } else {
            self.copy_guards(&proof.guards, budget)?
        };
        let old = std::mem::replace(&mut self.enums[index].guards, guards);
        self.layouts.lease.discard_vec(old, budget)?;
        let a_index = left.enum_entry(place, budget)?;
        let b_index = right.enum_entry(place, budget)?;
        self.enums[index].construction = match (a_index, b_index) {
            (Some(a), Some(b)) if left.enums[a].construction == right.enums[b].construction => {
                left.enums[a].construction
            }
            _ => None,
        };
        proof.discard(&self.layouts.lease, budget)?;
        a.discard(&self.layouts.lease, budget)?;
        b.discard(&self.layouts.lease, budget)
    }

    fn remap_proof(
        &self,
        mut proof: InitializationProofV29,
        source: &SourceStorageSubobjectV29<'layout, 'source>,
        destination: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<InitializationProofV29, Error> {
        let mut kept = 0;
        for index in 0..proof.guards.len() {
            self.layouts.lease.work(1, budget)?;
            let depth = proof.guards[index];
            if depth < source.path.len() {
                if !self.guard_holds(source, depth, budget)? {
                    proof.initialized = false;
                    proof.guards.clear();
                    return Ok(proof);
                }
            } else {
                proof.guards[kept] = destination
                    .path
                    .len()
                    .checked_add(depth - source.path.len())
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                kept += 1;
            }
        }
        proof.guards.truncate(kept);
        Ok(proof)
    }

    fn assignment_proof(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        enum_depth: usize,
        budget: &mut Budget<'_>,
    ) -> Result<InitializationProofV29, Error> {
        let proof = self.initialization_proof(place, true, budget)?;
        self.discharge_assignment_guards(proof, place, enum_depth, budget)
    }

    fn discharge_assignment_guards(
        &self,
        mut proof: InitializationProofV29,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        enum_depth: usize,
        budget: &mut Budget<'_>,
    ) -> Result<InitializationProofV29, Error> {
        let mut kept = 0;
        for index in 0..proof.guards.len() {
            self.layouts.lease.work(1, budget)?;
            let depth = proof.guards[index];
            if depth >= enum_depth {
                if !self.guard_holds(place, depth, budget)? {
                    proof.initialized = false;
                    proof.guards.clear();
                    return Ok(proof);
                }
            } else {
                proof.guards[kept] = depth;
                kept += 1;
            }
        }
        proof.guards.truncate(kept);
        Ok(proof)
    }

    fn prepare_tag_assignment(
        &mut self,
        snapshot: &Self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        variant: u32,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        let exact = snapshot
            .enum_entry(place, budget)?
            .is_some_and(|index| snapshot.enums[index].readable.as_slice() == [variant]);
        if exact {
            return Ok(());
        }
        // Forget a valid-whole-enum default before selecting a different tag.
        // Restore finite payload proofs using only the incoming selectors.
        self.set_fact(place, false, budget)?;
        for fact in &snapshot.facts {
            self.layouts.lease.work(1, budget)?;
            if fact.place.path.len() > place.path.len()
                && prefix_path(&place.path, &fact.place.path, &self.layouts.lease, budget)?
            {
                let proof = snapshot.assignment_proof(&fact.place, place.path.len(), budget)?;
                self.set_guarded_fact(&fact.place, proof.initialized, &proof.guards, budget)?;
                proof.discard(&self.layouts.lease, budget)?;
            }
        }
        for old in &snapshot.enums {
            self.layouts.lease.work(1, budget)?;
            if old.place.path.len() <= place.path.len()
                || !prefix_path(&place.path, &old.place.path, &self.layouts.lease, budget)?
            {
                continue;
            }
            let proof = InitializationProofV29::new(
                !old.readable.is_empty(),
                &old.guards,
                &self.layouts.lease,
                budget,
            )?;
            let proof = snapshot.discharge_assignment_guards(
                proof,
                &old.place,
                place.path.len(),
                budget,
            )?;
            let index = self.enum_state_mut(&old.place, budget)?;
            if !proof.initialized {
                self.enums[index].readable.clear();
                self.enums[index].construction = None;
            }
            let guards = self.copy_guards(&proof.guards, budget)?;
            let replaced = std::mem::replace(&mut self.enums[index].guards, guards);
            self.layouts.lease.discard_vec(replaced, budget)?;
            proof.discard(&self.layouts.lease, budget)?;
        }
        Ok(())
    }

    fn invalidate_tag_payload(
        &mut self,
        snapshot: &Self,
        payload: &SourceStorageSubobjectV29<'layout, 'source>,
        tag: SourceStorageRangeV29,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.set_fact(payload, false, budget)?;
        for state in &mut self.enums {
            self.layouts.lease.work(1, budget)?;
            if prefix_path(
                &payload.path,
                &state.place.path,
                &self.layouts.lease,
                budget,
            )? {
                state.readable.clear();
                state.guards.clear();
                state.construction = None;
            }
        }
        // Other variant paths may physically alias the overwritten niche.
        for fact in &snapshot.facts {
            self.layouts.lease.work(1, budget)?;
            if fact.place.range.overlaps(tag)
                && !prefix_path(&fact.place.path, &payload.path, &self.layouts.lease, budget)?
            {
                self.set_fact(&fact.place, false, budget)?;
            }
        }
        Ok(())
    }

    fn invalidate_tag_aliases(
        &mut self,
        snapshot: &Self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        tag: SourceStorageRangeV29,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.clear_enclosing_union_defaults(place, budget)?;
        // Only the exact tag bytes are written. Sibling payloads outside that
        // range keep their existing sparse proofs, including through a union.
        for fact in &snapshot.facts {
            self.layouts.lease.work(1, budget)?;
            if prefix_path(&fact.place.path, &place.path, &self.layouts.lease, budget)?
                || prefix_path(&place.path, &fact.place.path, &self.layouts.lease, budget)?
            {
                continue;
            }
            if fact.place.range.overlaps(tag) {
                self.set_fact(&fact.place, false, budget)?;
            } else {
                self.set_guarded_fact(&fact.place, fact.initialized, &fact.guards, budget)?;
            }
        }
        for state in &mut self.enums {
            self.layouts.lease.work(1, budget)?;
            if self
                .layouts
                .enum_tag_range(&state.place)?
                .is_some_and(|range| range.overlaps(tag))
            {
                state.readable.clear();
                state.guards.clear();
                state.construction = None;
            }
        }
        Ok(())
    }

    fn clear_enclosing_union_defaults(
        &mut self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        let mut parent = self.root_subobject(budget)?;
        for &step in &place.path {
            self.layouts.lease.work(1, budget)?;
            if matches!(
                self.layouts.declaration(parent.ty)?.shape(),
                SemanticTypeShapeV1::Union(_)
            ) {
                self.clear_prefix_default(&parent, budget)?;
            }
            let next = self.layouts.project_step(&parent, step, budget)?;
            parent.discard(budget)?;
            parent = next;
        }
        parent.discard(budget)
    }

    fn invalidate_union_field_overlaps(
        &mut self,
        alias: &SourceStorageSubobjectV29<'layout, 'source>,
        written: SourceStorageRangeV29,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        // The enclosing union default has already been cleared. Its original
        // field boundary is an initialization floor, so dropping overlapping
        // proofs cannot reveal an initialized ancestor for this alternative.
        self.check_place(alias, budget)?;
        let lease = &self.layouts.lease;
        let headers = 2 * size_of::<Vec<InitializationFact<'_, '_>>>();
        lease.reserve(headers, budget)?;
        let mut keep = lease.vector(self.facts.len(), budget)?;
        let old = std::mem::take(&mut self.facts);
        let capacity = old.capacity();
        for fact in old {
            lease.work(1, budget)?;
            if fact.place.range.overlaps(written)
                && prefix_path(&alias.path, &fact.place.path, lease, budget)?
            {
                fact.place.discard(budget)?;
                lease.discard_vec(fact.guards, budget)?;
            } else {
                lease.push(&mut keep, fact, budget)?;
            }
        }
        lease.refund(
            argument_product_v1(capacity, size_of::<InitializationFact<'_, '_>>())?,
            budget,
        )?;
        self.facts = keep;
        lease.refund(headers, budget)
    }

    fn niche_payload(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        niche: &fe2o3_mir_model::semantic_mir_v1::SemanticNicheEnumEncodingV1,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
        let mut current = self.layouts.project_step(
            place,
            SubobjectStep::Variant(niche.untagged_variant()),
            budget,
        )?;
        for part in niche.source().path() {
            self.layouts.lease.work(1, budget)?;
            let step = match part {
                SemanticNichePathComponentV1::Field(field) => SubobjectStep::Field(*field),
                SemanticNichePathComponentV1::ArrayElement(index) => SubobjectStep::Element(*index),
            };
            let next = self.layouts.project_step(&current, step, budget)?;
            current.discard(budget)?;
            current = next;
        }
        Ok(current)
    }

    fn untagged_payload_is_valid(
        &self,
        place: &SourceStorageSubobjectV29<'layout, 'source>,
        payload: &SourceStorageSubobjectV29<'layout, 'source>,
        niche: &fe2o3_mir_model::semantic_mir_v1::SemanticNicheEnumEncodingV1,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        let proof = self.initialization_proof(payload, true, budget)?;
        let initialized = proof.initialized
            && self.guards_hold(payload, &proof.guards, budget)?
            && self.is_initialized(payload, budget)?;
        proof.discard(&self.layouts.lease, budget)?;
        if !initialized {
            return Ok(false);
        }
        let terminal = self.layouts.declaration(payload.ty)?;
        let relative = place
            .range
            .start
            .checked_add(niche.source().expected_offset_bytes())
            .and_then(|tag| tag.checked_sub(payload.range.start))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let scalar = match terminal.layout().backend_repr() {
            SemanticBackendReprV1::Scalar(scalar) if relative == 0 => Some(*scalar),
            SemanticBackendReprV1::ScalarPair { first, second } => {
                let size = first
                    .primitive()
                    .size_bytes()
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let align = second.primitive().alignment_bytes();
                let offset = size
                    .checked_add(align - 1)
                    .map(|v| v & !(align - 1))
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                if relative == 0 {
                    Some(*first)
                } else if relative == offset {
                    Some(*second)
                } else {
                    None
                }
            }
            SemanticBackendReprV1::Memory { .. }
                if matches!(terminal.shape(), SemanticTypeShapeV1::Enum { .. }) =>
            {
                terminal
                    .layout()
                    .largest_niche()
                    .filter(|source| source.offset_bytes() == relative)
                    .map(|source| {
                        SemanticBackendScalarV1::initialized(
                            source.primitive(),
                            source.valid_range(),
                        )
                    })
            }
            _ => None,
        };
        let Some(scalar) = scalar else {
            return Ok(false);
        };
        let source = niche.source_niche();
        if scalar.primitive() != source.primitive()
            || scalar.valid_range() != Some(source.valid_range())
        {
            return Ok(false);
        }
        niche_values_excluded_v29(
            source,
            niche.niche_variant_range(),
            niche.niche_start(),
            &self.layouts.lease,
            budget,
        )
    }
}

fn niche_values_excluded_v29(
    source: fe2o3_mir_model::semantic_mir_v1::SemanticLayoutNicheV1,
    variants: (u32, u32),
    start: u128,
    lease: &Lease,
    budget: &mut Budget<'_>,
) -> Result<bool, Error> {
    lease.work(4, budget)?;
    let bits = source
        .primitive()
        .size_bytes()
        .and_then(|size| size.checked_mul(8))
        .filter(|bits| (1..=128).contains(bits))
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let max = if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    };
    let count = variants
        .1
        .checked_sub(variants.0)
        .and_then(|n| u64::from(n).checked_add(1))
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let valid = source.valid_range();
    if start > max
        || valid.start() > max
        || valid.end() > max
        || (bits < 128 && u128::from(count) > max + 1)
    {
        return Ok(false);
    }
    // Both intervals are modular in the admitted primitive's exact width.
    let end = start.wrapping_add(u128::from(count - 1)) & max;
    let split = |first, last| {
        if first <= last {
            [(first, last), (1, 0)]
        } else {
            [(first, max), (0, last)]
        }
    };
    let valid = split(valid.start(), valid.end());
    let encoded = split(start, end);
    Ok(valid.iter().all(|&(a, b)| {
        encoded
            .iter()
            .all(|&(c, d)| a > b || c > d || b < c || d < a)
    }))
}
