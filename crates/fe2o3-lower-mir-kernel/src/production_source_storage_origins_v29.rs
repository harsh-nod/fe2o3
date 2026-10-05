#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SourceStorageOriginKeyV29 {
    origin: usize,
    loan: usize,
}

impl SourceStorageOriginKeyV29 {
    pub(super) fn loan(self) -> usize {
        self.loan
    }
}

// This is a finite may-origin set, never the runtime selector or an access
// certificate. The A/C1 driver must freshly join each member to the actual
// source access, lifetime and emitted pointer selection.
pub(super) struct SourceStorageOriginsV29<'layout, 'source> {
    layouts: &'layout SourceStorageLayoutsV29<'source>,
    plan_slot: usize,
    root: ProductionCallInstanceIdV1,
    keys: Vec<SourceStorageOriginKeyV29>,
}

impl<'layout, 'source> SourceStorageOriginsV29<'layout, 'source> {
    pub(super) fn capture(
        layouts: &'layout SourceStorageLayoutsV29<'source>,
        plan: &SourceReferencePlanV29<'_, '_>,
        loan: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        layouts.check_owner(plan.instances.owner(), budget)?;
        plan.check_owner(plan.instances, budget)?;
        let found = plan
            .loans
            .get(loan)
            .ok_or_else(|| error("source storage alternative loan is absent"))?;
        plan.origins
            .get(found.origin)
            .ok_or_else(|| error("source storage alternative origin is absent"))?;
        layouts
            .lease
            .reserve(size_of::<Self>() + size_of::<Result<Self, Error>>(), budget)?;
        let mut keys = layouts.lease.vector(1, budget)?;
        layouts.lease.push(
            &mut keys,
            SourceStorageOriginKeyV29 {
                origin: found.origin,
                loan,
            },
            budget,
        )?;
        Ok(Self {
            layouts,
            plan_slot: plan as *const SourceReferencePlanV29<'_, '_> as usize,
            root: plan.root,
            keys,
        })
    }

    fn check(
        &self,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.layouts.check_owner(plan.instances.owner(), budget)?;
        plan.check_owner(plan.instances, budget)?;
        if self.plan_slot != plan as *const SourceReferencePlanV29<'_, '_> as usize
            || self.root != plan.root
        {
            return Err(error(
                "source storage alternatives changed their source instance plan",
            ));
        }
        for key in &self.keys {
            self.layouts.lease.work(1, budget)?;
            if plan
                .loans
                .get(key.loan)
                .is_none_or(|loan| loan.origin != key.origin)
                || plan.origins.get(key.origin).is_none()
            {
                return Err(error(
                    "source storage alternative no longer names its original loan",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn keys<'a>(
        &'a self,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<&'a [SourceStorageOriginKeyV29], Error> {
        self.check(plan, budget)?;
        Ok(&self.keys)
    }

    pub(super) fn copy(
        &self,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        self.check(plan, budget)?;
        self.layouts
            .lease
            .reserve(size_of::<Self>() + size_of::<Result<Self, Error>>(), budget)?;
        let mut keys = self.layouts.lease.vector(self.keys.len(), budget)?;
        self.layouts.lease.work(self.keys.len(), budget)?;
        keys.extend_from_slice(&self.keys);
        Ok(Self {
            layouts: self.layouts,
            plan_slot: self.plan_slot,
            root: self.root,
            keys,
        })
    }

    pub(super) fn merge(
        &mut self,
        other: &Self,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        self.check(plan, budget)?;
        other.check(plan, budget)?;
        if !std::ptr::eq(self.layouts, other.layouts)
            || self.plan_slot != other.plan_slot
            || self.root != other.root
        {
            return Err(error(
                "source storage alternatives belong to different owner domains",
            ));
        }
        let capacity = argument_sum_v1(&[self.keys.len(), other.keys.len()])?;
        self.layouts
            .lease
            .reserve(size_of::<Vec<SourceStorageOriginKeyV29>>(), budget)?;
        let mut merged = self.layouts.lease.vector(capacity, budget)?;
        let (mut left, mut right) = (0, 0);
        while left < self.keys.len() || right < other.keys.len() {
            self.layouts.lease.work(1, budget)?;
            let key = match (self.keys.get(left), other.keys.get(right)) {
                (Some(a), Some(b)) if a < b => {
                    left += 1;
                    *a
                }
                (Some(a), Some(b)) if a == b => {
                    left += 1;
                    right += 1;
                    *a
                }
                (Some(_), Some(b)) | (None, Some(b)) => {
                    right += 1;
                    *b
                }
                (Some(a), None) => {
                    left += 1;
                    *a
                }
                (None, None) => break,
            };
            self.layouts.lease.push(&mut merged, key, budget)?;
        }
        let changed = merged.len() != self.keys.len();
        let old = std::mem::replace(&mut self.keys, merged);
        self.layouts.lease.discard_vec(old, budget)?;
        self.layouts
            .lease
            .refund(size_of::<Vec<SourceStorageOriginKeyV29>>(), budget)?;
        Ok(changed)
    }

    pub(super) fn discard(self, budget: &mut Budget<'_>) -> Result<(), Error> {
        let lease = &self.layouts.lease;
        lease.discard_vec(self.keys, budget)?;
        lease.refund(size_of::<Self>() + size_of::<Result<Self, Error>>(), budget)
    }

    fn same_keys(&self, other: &Self, budget: &mut Budget<'_>) -> Result<bool, Error> {
        self.layouts
            .lease
            .work(argument_sum_v1(&[self.keys.len(), 1])?, budget)?;
        Ok(std::ptr::eq(self.layouts, other.layouts)
            && self.plan_slot == other.plan_slot
            && self.root == other.root
            && self.keys == other.keys)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceStorageDescriptorLengthV29 {
    Constant(u64),
    // An inert original source definition coordinate. The existing SSA/access
    // owner must authenticate it when the actual descriptor is emitted/read.
    Source {
        instance: ProductionCallInstanceIdV1,
        block: SemanticBlockIdV1,
        definition: u32,
    },
}

// Alternatives are complete correlated descriptors. The associated runtime
// selector must select the same data and length alternative, never a cross product.
pub(super) struct SourceStorageDescriptorAlternativeV29<'layout, 'source> {
    origins: SourceStorageOriginsV29<'layout, 'source>,
    length: Option<SourceStorageDescriptorLengthV29>,
}

impl<'layout, 'source> SourceStorageDescriptorAlternativeV29<'layout, 'source> {
    fn copy(
        &self,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        Ok(Self {
            origins: self.origins.copy(plan, budget)?,
            length: self.length,
        })
    }
    fn compare(&self, other: &Self, budget: &mut Budget<'_>) -> Result<std::cmp::Ordering, Error> {
        fn length_key(value: Option<SourceStorageDescriptorLengthV29>) -> (u8, u64, u32, u32) {
            match value {
                None => (0, 0, 0, 0),
                Some(SourceStorageDescriptorLengthV29::Constant(length)) => (1, length, 0, 0),
                Some(SourceStorageDescriptorLengthV29::Source {
                    instance,
                    block,
                    definition,
                }) => (2, instance.index() as u64, block.index(), definition),
            }
        }
        self.origins.layouts.lease.work(
            argument_sum_v1(&[self.origins.keys.len().min(other.origins.keys.len()), 1])?,
            budget,
        )?;
        Ok(length_key(self.length)
            .cmp(&length_key(other.length))
            .then_with(|| self.origins.keys.cmp(&other.origins.keys)))
    }
    pub(super) fn parts(
        &self,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            &SourceStorageOriginsV29<'layout, 'source>,
            Option<SourceStorageDescriptorLengthV29>,
        ),
        Error,
    > {
        self.origins.check(plan, budget)?;
        Ok((&self.origins, self.length))
    }
}

struct SourceStorageRelocationV29<'layout, 'source> {
    range: SourceStorageRangeV29,
    representation: StorageLayoutIdV1,
    alternatives: Vec<SourceStorageDescriptorAlternativeV29<'layout, 'source>>,
}

impl<'layout, 'source> SourceStorageRelocationV29<'layout, 'source> {
    fn copy(
        &self,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        let layouts = self
            .alternatives
            .first()
            .ok_or_else(|| error("source relocation has no alternatives"))?
            .origins
            .layouts;
        let mut alternatives = layouts.lease.vector(self.alternatives.len(), budget)?;
        for alternative in &self.alternatives {
            layouts.lease.work(1, budget)?;
            let copy = alternative.copy(plan, budget)?;
            layouts.lease.push(&mut alternatives, copy, budget)?;
        }
        Ok(Self {
            range: self.range,
            representation: self.representation,
            alternatives,
        })
    }
    fn discard(
        mut self,
        layouts: &SourceStorageLayoutsV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        for alternative in self.alternatives.drain(..) {
            alternative.origins.discard(budget)?;
        }
        layouts.lease.discard_vec(self.alternatives, budget)
    }
}

pub(super) struct SourceStorageRelocationsV29<'layout, 'source> {
    layouts: &'layout SourceStorageLayoutsV29<'source>,
    instances: &'layout ProductionCallInstancePlanV1<'source>,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    extent: u64,
    rows: Vec<SourceStorageRelocationV29<'layout, 'source>>,
}

impl<'layout, 'source> SourceStorageRelocationsV29<'layout, 'source> {
    pub(super) fn new(
        state: &SourceStorageStateV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        state
            .layouts
            .lease
            .reserve(size_of::<Self>() + size_of::<Result<Self, Error>>(), budget)?;
        let extent = state
            .layouts
            .declaration(state.ty)?
            .layout()
            .size_bytes()
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(Self {
            layouts: state.layouts,
            instances: state.instances,
            instance: state.instance,
            local: state.local,
            extent,
            rows: Vec::new(),
        })
    }

    fn check_plan(
        &self,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.layouts.check_owner(plan.instances.owner(), budget)?;
        plan.check_owner(plan.instances, budget)?;
        if !std::ptr::eq(self.instances, plan.instances) {
            return Err(error("source relocation changed its call-instance owner"));
        }
        Ok(())
    }

    // Stable linear compaction: overlapping representations are removed whole.
    // No truncated pointer, old length, or shifted tail acquires provenance.
    pub(super) fn invalidate(
        &mut self,
        range: SourceStorageRangeV29,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.layouts.lease.check(budget)?;
        range.check(self.extent)?;
        let mut removed = 0;
        for index in 0..self.rows.len() {
            self.layouts.lease.work(1, budget)?;
            if self.rows[index].range.overlaps(range) {
                removed += 1;
            } else if removed != 0 {
                self.rows.swap(index, index - removed);
            }
        }
        for _ in 0..removed {
            let row = self.rows.pop().ok_or(ArgumentResourceV1::Accounting)?;
            row.discard(self.layouts, budget)?;
        }
        Ok(())
    }

    fn insert(
        &mut self,
        row: SourceStorageRelocationV29<'layout, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.layouts.lease.push(&mut self.rows, row, budget)?;
        let mut index = self.rows.len() - 1;
        while index > 0 {
            self.layouts.lease.work(1, budget)?;
            if self.rows[index - 1].range <= self.rows[index].range {
                break;
            }
            self.rows.swap(index - 1, index);
            index -= 1;
        }
        Ok(())
    }

    pub(super) fn replace(
        &mut self,
        offset: u64,
        representation: StorageLayoutIdV1,
        origins: &SourceStorageOriginsV29<'layout, 'source>,
        length: Option<SourceStorageDescriptorLengthV29>,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.check_plan(plan, budget)?;
        origins.check(plan, budget)?;
        if !std::ptr::eq(self.layouts, origins.layouts) {
            return Err(error("source relocation uses foreign layout custody"));
        }
        let rows = self.layouts.rows(plan.instances.owner(), budget)?;
        let row = rows
            .get(representation.0 as usize)
            .ok_or_else(|| error("source relocation representation is absent"))?;
        if !matches!(
            (&row.kind, length),
            (StorageLayoutKindV1::Pointer(_), None) | (StorageLayoutKindV1::Slice { .. }, Some(_))
        ) {
            return Err(error(
                "source relocation does not preserve the complete pointer or descriptor",
            ));
        }
        let range = SourceStorageRangeV29::new(offset, row.size, self.extent)?;
        drop(rows);
        let mut alternatives = self.layouts.lease.vector(1, budget)?;
        let copy = origins.copy(plan, budget)?;
        self.layouts.lease.push(
            &mut alternatives,
            SourceStorageDescriptorAlternativeV29 {
                origins: copy,
                length,
            },
            budget,
        )?;
        self.invalidate(range, budget)?;
        self.insert(
            SourceStorageRelocationV29 {
                range,
                representation,
                alternatives,
            },
            budget,
        )
    }

    pub(super) fn read(
        &self,
        offset: u64,
        representation: StorageLayoutIdV1,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<&[SourceStorageDescriptorAlternativeV29<'layout, 'source>], Error> {
        self.check_plan(plan, budget)?;
        for row in &self.rows {
            self.layouts.lease.work(1, budget)?;
            if row.range.start == offset && row.representation == representation {
                for alternative in &row.alternatives {
                    alternative.origins.check(plan, budget)?;
                }
                return Ok(&row.alternatives);
            }
            if row.range.start > offset {
                break;
            }
        }
        Err(error(
            "source storage read has no complete symbolic relocation on every incoming path",
        ))
    }

    fn snapshot(
        &self,
        destination: u64,
        destination_extent: u64,
        range: SourceStorageRangeV29,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<Vec<SourceStorageRelocationV29<'layout, 'source>>, Error> {
        self.check_plan(plan, budget)?;
        range.check(self.extent)?;
        SourceStorageRangeV29::new(destination, range.length(), destination_extent)?;
        self.layouts
            .lease
            .reserve(size_of::<Vec<SourceStorageRelocationV29<'_, '_>>>(), budget)?;
        let mut snapshot = self.layouts.lease.vector(self.rows.len(), budget)?;
        for row in &self.rows {
            self.layouts.lease.work(1, budget)?;
            if range.contains(row.range) {
                let mut copy = row.copy(plan, budget)?;
                let offset = destination
                    .checked_add(row.range.start - range.start)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                copy.range =
                    SourceStorageRangeV29::new(offset, row.range.length(), destination_extent)?;
                self.layouts.lease.push(&mut snapshot, copy, budget)?;
            }
        }
        Ok(snapshot)
    }

    fn install_snapshot(
        &mut self,
        destination_range: SourceStorageRangeV29,
        mut snapshot: Vec<SourceStorageRelocationV29<'layout, 'source>>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.invalidate(destination_range, budget)?;
        for row in snapshot.drain(..) {
            self.insert(row, budget)?;
        }
        self.layouts.lease.discard_vec(snapshot, budget)?;
        self.layouts
            .lease
            .refund(size_of::<Vec<SourceStorageRelocationV29<'_, '_>>>(), budget)
    }

    pub(super) fn copy_from(
        &mut self,
        destination: u64,
        source: &Self,
        range: SourceStorageRangeV29,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.check_plan(plan, budget)?;
        if !std::ptr::eq(self.layouts, source.layouts) {
            return Err(error("source relocation copy crosses layout owners"));
        }
        let destination_range =
            SourceStorageRangeV29::new(destination, range.length(), self.extent)?;
        let snapshot = source.snapshot(destination, self.extent, range, plan, budget)?;
        self.install_snapshot(destination_range, snapshot, budget)
    }

    pub(super) fn copy_within(
        &mut self,
        destination: u64,
        range: SourceStorageRangeV29,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        let destination_range =
            SourceStorageRangeV29::new(destination, range.length(), self.extent)?;
        let snapshot = self.snapshot(destination, self.extent, range, plan, budget)?;
        self.install_snapshot(destination_range, snapshot, budget)
    }

    pub(super) fn join(
        &self,
        other: &Self,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        self.check_plan(plan, budget)?;
        other.check_plan(plan, budget)?;
        if !std::ptr::eq(self.layouts, other.layouts)
            || !std::ptr::eq(self.instances, other.instances)
            || self.instance != other.instance
            || self.local != other.local
            || self.extent != other.extent
        {
            return Err(error(
                "source relocation join changes original holder identity",
            ));
        }
        self.layouts
            .lease
            .reserve(size_of::<Self>() + size_of::<Result<Self, Error>>(), budget)?;
        let mut result = Self {
            layouts: self.layouts,
            instances: self.instances,
            instance: self.instance,
            local: self.local,
            extent: self.extent,
            rows: self
                .layouts
                .lease
                .vector(self.rows.len().min(other.rows.len()), budget)?,
        };
        let (mut left, mut right) = (0, 0);
        while left < self.rows.len() && right < other.rows.len() {
            self.layouts.lease.work(1, budget)?;
            let (a, b) = (&self.rows[left], &other.rows[right]);
            if a.range < b.range {
                left += 1;
                continue;
            }
            if b.range < a.range {
                right += 1;
                continue;
            }
            if a.representation == b.representation {
                let mut alternatives = self.layouts.lease.vector(
                    argument_sum_v1(&[a.alternatives.len(), b.alternatives.len()])?,
                    budget,
                )?;
                let (mut ai, mut bi) = (0, 0);
                while ai < a.alternatives.len() || bi < b.alternatives.len() {
                    self.layouts.lease.work(1, budget)?;
                    let choice = match (a.alternatives.get(ai), b.alternatives.get(bi)) {
                        (Some(av), Some(bv)) => match av.compare(bv, budget)? {
                            std::cmp::Ordering::Less => {
                                ai += 1;
                                av
                            }
                            std::cmp::Ordering::Equal => {
                                ai += 1;
                                bi += 1;
                                av
                            }
                            std::cmp::Ordering::Greater => {
                                bi += 1;
                                bv
                            }
                        },
                        (Some(av), None) => {
                            ai += 1;
                            av
                        }
                        (None, Some(bv)) => {
                            bi += 1;
                            bv
                        }
                        (None, None) => break,
                    };
                    let copy = choice.copy(plan, budget)?;
                    self.layouts.lease.push(&mut alternatives, copy, budget)?;
                }
                let row = SourceStorageRelocationV29 {
                    range: a.range,
                    representation: a.representation,
                    alternatives,
                };
                self.layouts.lease.push(&mut result.rows, row, budget)?;
            }
            left += 1;
            right += 1;
        }
        Ok(result)
    }

    pub(super) fn discard(mut self, budget: &mut Budget<'_>) -> Result<(), Error> {
        for row in self.rows.drain(..) {
            row.discard(self.layouts, budget)?;
        }
        self.layouts.lease.discard_vec(self.rows, budget)?;
        self.layouts
            .lease
            .refund(size_of::<Self>() + size_of::<Result<Self, Error>>(), budget)
    }
}
