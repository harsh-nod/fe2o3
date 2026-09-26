use std::marker::PhantomData;

impl SourceStorageLayoutsV29<'_> {
    pub(super) fn permits_root_emission_refund(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        bytes: usize,
        budget: &Budget<'_>,
    ) -> bool {
        std::ptr::eq(self.owner, owner)
            && self.lease.root.get().is_none()
            && self.lease.custody(budget).is_ok()
            && self
                .lease
                .floor
                .checked_add(self.lease.owned.get())
                .is_some_and(|floor| {
                    budget
                        .storage()
                        .checked_sub(bytes)
                        .is_some_and(|after| after >= floor)
                })
    }
}

pub(super) struct SourceStorageRootArenaV29<'root, 'source> {
    layouts: &'root SourceStorageLayoutsV29<'source>,
    instances: &'root ExecutionInstancesV29<'source>,
    states: RefCell<Vec<SourceStorageStateV29<'root, 'source>>>,
    paths: RefCell<Vec<SourceStorageSubobjectV29<'root, 'source>>>,
    origins: RefCell<Vec<SourceStorageOriginsV29<'root, 'source>>>,
    snapshots: RefCell<Vec<SourceStorageStateV29<'root, 'source>>>,
    snapshot_index: RefCell<SourceStorageSnapshotIndexV29>,
}

include!("production_source_storage_snapshot_index_v29.rs");

pub(super) struct SourceStorageRootV29<'view, 'root, 'source> {
    arena: &'view SourceStorageRootArenaV29<'root, 'source>,
}

// A concrete read-only borrow, not an implementation-supplied custody claim.
// Its private constructor borrows the stable scope-owned arena. No mutable C2
// state appears in this view, so the existing C1 plan remains covariant.
pub(super) struct SourceStorageRootCustodyViewV29<'arena, 'source> {
    layouts: &'arena SourceStorageLayoutsV29<'source>,
    instances: &'arena ExecutionInstancesV29<'source>,
    arena: usize,
}

// Accounting only: this borrows the concrete original view and cannot create a
// root, access, or refund permit. Its caller must also check concrete custody.
pub(super) struct SourceStorageRootGrowthV29<'view, 'arena, 'source> {
    custody: &'view SourceStorageRootCustodyViewV29<'arena, 'source>,
    owned: usize,
    table_owned: usize,
}

// Four constant capture groups and four cleanup replay/arithmetic groups. The
// entire amount is prepaid; cleanup never spends work over a selected failure.
pub(super) const SOURCE_STORAGE_ROOT_GROWTH_WORK_V29: usize = 4 + 4;

impl SourceStorageRootGrowthV29<'_, '_, '_> {
    pub(super) fn permits_refund(
        &self,
        entry: usize,
        retained: usize,
        current: usize,
        bytes: usize,
    ) -> bool {
        let Some(owned) = self.custody.growth_owned(self.table_owned) else {
            return false;
        };
        let Some(growth) = owned.checked_sub(self.owned) else {
            return false;
        };
        retained
            .checked_add(growth)
            .is_some_and(|floor| current >= floor)
            && entry.checked_add(growth).is_some_and(|floor| {
                current
                    .checked_sub(bytes)
                    .is_some_and(|after| after >= floor)
            })
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SourceStorageStateHandleV29<'view> {
    arena: usize,
    index: usize,
    view: PhantomData<&'view ()>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SourceStoragePathHandleV29<'view> {
    arena: usize,
    index: usize,
    view: PhantomData<&'view ()>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SourceStorageOriginsHandleV29<'view> {
    arena: usize,
    index: usize,
    view: PhantomData<&'view ()>,
}

// Unlike mutable callback handles, these select immutable source-flow states.
// Only the arena can construct one; no mutable-state API accepts this type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SourceStorageSnapshotV29<'view> {
    arena: usize,
    index: usize,
    view: PhantomData<&'view ()>,
}

#[derive(Clone, Copy)]
pub(super) enum SourceStorageRootMutationV29 {
    Initialize,
    Deinitialize,
    BeginVariant(u32),
    SetDiscriminant(u32),
}

fn source_storage_arena_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<SourceStorageRootArenaV29<'_, '_>>(),
        size_of::<Result<SourceStorageRootArenaV29<'_, '_>, Error>>(),
        // Arena operations are non-reentrant. All three handle types have the
        // same two-word representation; bool/unit results are smaller. Keep both
        // the protected operation and typed observation envelopes prepaid.
        2 * size_of::<Result<Option<SourceStorageSnapshotV29<'_>>, Error>>(),
        2 * size_of::<Result<Option<SourceStorageSnapshotV29<'_>>, RecordedStorageFailureV29<'_>>>(
        ),
        size_of::<std::cell::RefMut<'_, Vec<SourceStorageStateV29<'_, '_>>>>(),
        size_of::<std::cell::RefMut<'_, SourceStorageSnapshotIndexV29>>(),
        size_of::<Vec<Vec<SourceStorageSnapshotBucketV29>>>(),
        size_of::<Vec<SourceStorageSnapshotBucketV29>>(),
        size_of::<SourceStorageSnapshotBucketV29>(),
        size_of::<std::cell::Ref<'_, Vec<SourceStorageSubobjectV29<'_, '_>>>>(),
        2 * size_of::<usize>(),
    ])
}

impl<'root, 'source> SourceStorageRootArenaV29<'root, 'source> {
    fn new(
        layouts: &'root SourceStorageLayoutsV29<'source>,
        instances: &'root ExecutionInstancesV29<'source>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        layouts.check_owner(instances.owner(), budget)?;
        let active =
            layouts.lease.root.get().ok_or_else(|| {
                error("source storage arena requires an active original root scope")
            })?;
        if active.instances != instances as *const ExecutionInstancesV29<'_> as usize
            || active.root != instances.root()
        {
            return Err(error(
                "source storage arena changed its original instance plan",
            ));
        }
        layouts
            .lease
            .reserve(source_storage_arena_headers_v29()?, budget)?;
        Ok(Self {
            layouts,
            instances,
            states: RefCell::new(Vec::new()),
            paths: RefCell::new(Vec::new()),
            origins: RefCell::new(Vec::new()),
            snapshots: RefCell::new(Vec::new()),
            snapshot_index: RefCell::new(SourceStorageSnapshotIndexV29::default()),
        })
    }

    pub(super) fn custody_view(&self) -> SourceStorageRootCustodyViewV29<'_, 'source> {
        SourceStorageRootCustodyViewV29 {
            layouts: self.layouts,
            instances: self.instances,
            arena: self.identity(),
        }
    }

    pub(super) fn view(&self) -> SourceStorageRootV29<'_, 'root, 'source> {
        SourceStorageRootV29 { arena: self }
    }

    fn identity(&self) -> usize {
        self as *const Self as usize
    }

    fn check(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.layouts.lease.check(budget)?;
        if !self.layouts.lease.root.get().is_some_and(|active| {
            active.instances == self.instances as *const ExecutionInstancesV29<'_> as usize
                && active.root == self.instances.root()
                && !active.poisoned
        }) {
            return Err(error(
                "source storage handle is outside its original root scope",
            ));
        }
        self.layouts.check_owner(self.instances.owner(), budget)
    }

    fn record<'view, T>(
        &'view self,
        result: Result<T, Error>,
    ) -> Result<T, RecordedStorageFailureV29<'view>> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.layouts.lease.failure.record(error);
                Err(RecordedStorageFailureV29 {
                    cell: &self.layouts.lease.failure,
                })
            }
        }
    }

    fn handle(&self, owner: usize) -> Result<(), Error> {
        if owner != self.identity() {
            return Err(error(
                "source storage handle changed its original root arena",
            ));
        }
        Ok(())
    }
}

impl<'arena, 'source> SourceStorageRootCustodyViewV29<'arena, 'source> {
    pub(super) fn source_layouts(
        &self,
        instances: &ExecutionInstancesV29<'source>,
        budget: &mut Budget<'_>,
    ) -> Result<&'arena SourceStorageLayoutsV29<'source>, Error> {
        self.layouts.lease.work(1, budget)?;
        if !std::ptr::eq(self.instances, instances)
            || !std::ptr::eq(self.layouts.owner, instances.owner())
            || !self.layouts.lease.root.get().is_some_and(|active| {
                !active.poisoned
                    && active.instances == instances as *const ExecutionInstancesV29<'_> as usize
                    && active.root == instances.root()
            })
        {
            return Err(error("source schema selection has no current original root owner"));
        }
        Ok(self.layouts)
    }

    fn growth_owned(&self, table_owned: usize) -> Option<usize> {
        let lease = &self.layouts.lease;
        let active = lease.root.get()?;
        let owned = lease.owned.get();
        (std::ptr::eq(self.layouts.owner, self.instances.owner())
            && active.instances == self.instances as *const ExecutionInstancesV29<'_> as usize
            && active.root == self.instances.root()
            && active.table_owned == table_owned
            && active.required(owned).is_some())
        .then_some(owned)
    }

    pub(super) fn capture_retained_growth(
        &self,
    ) -> Option<SourceStorageRootGrowthV29<'_, 'arena, 'source>> {
        let table_owned = self.layouts.lease.root.get()?.table_owned;
        Some(SourceStorageRootGrowthV29 {
            custody: self,
            owned: self.growth_owned(table_owned)?,
            table_owned,
        })
    }

    pub(super) fn failure(&self) -> SourceReferenceFailureV29<'arena> {
        SourceReferenceFailureV29::borrowed(&self.layouts.lease.failure)
    }

    pub(super) fn retains_custody(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        failure: &SourceReferenceFailureV29<'_>,
        budget: &Budget<'_>,
    ) -> bool {
        std::ptr::eq(self.instances, instances)
            && std::ptr::eq(self.layouts.owner, instances.owner())
            && failure.borrows(&self.layouts.lease.failure)
            && self.layouts.lease.custody(budget).is_ok()
            && self.layouts.lease.root.get().is_some_and(|active| {
                !active.poisoned
                    && active.instances == instances as *const ExecutionInstancesV29<'_> as usize
                    && active.root == instances.root()
            })
    }

    pub(super) fn retains_after_refund(&self, bytes: usize, budget: &Budget<'_>) -> bool {
        let after = budget.storage().checked_sub(bytes);
        self.layouts.lease.root.get().is_some_and(|active| {
            active
                .required(self.layouts.lease.owned.get())
                .is_some_and(|required| after.is_some_and(|after| after >= required))
        })
    }

    pub(super) fn permits_cleanup_refund(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        failure: &SourceReferenceFailureV29<'_>,
        bytes: usize,
        budget: &Budget<'_>,
    ) -> bool {
        // The genuine view keeps the original lease borrowed. Observing it
        // during cleanup must not record a later error over the selected one.
        let lease = &self.layouts.lease;
        std::ptr::eq(self.instances, instances)
            && std::ptr::eq(self.layouts.owner, instances.owner())
            && failure.borrows(&lease.failure)
            && lease.ledger == budget.work_ledger_identity_v1()
            && lease.slot == budget as *const Budget<'_> as usize
            && budget.storage().checked_sub(bytes).is_some_and(|after| {
                lease
                    .floor
                    .checked_add(lease.owned.get())
                    .is_some_and(|floor| after >= floor)
                    && lease.root.get().is_some_and(|active| {
                        active.instances == instances as *const ExecutionInstancesV29<'_> as usize
                            && active.root == instances.root()
                            && active
                                .required(lease.owned.get())
                                .is_some_and(|floor| after >= floor)
                    })
            })
    }

    pub(super) fn deny_active_root_refund(&self) {
        // This changes only the original borrowed lease, never fabricates an
        // arena view, and cannot reset poison or replace its first diagnostic.
        if let Some(mut active) = self.layouts.lease.root.get() {
            active.poisoned = true;
            self.layouts.lease.root.set(Some(active));
        }
    }
}

impl<'view, 'root, 'source> SourceStorageRootV29<'view, 'root, 'source> {
    pub(super) fn record_callback_failure(&self, error: Error) -> RecordedStorageFailureV29<'view> {
        // Recording a selected diagnostic is not a query or refund permit.
        // Move it before cleanup can observe a later custody failure.
        self.arena.layouts.lease.failure.record(error);
        RecordedStorageFailureV29 {
            cell: &self.arena.layouts.lease.failure,
        }
    }

    pub(super) fn permits_cleanup_refund(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        failure: &SourceReferenceFailureV29<'_>,
        bytes: usize,
        budget: &Budget<'_>,
    ) -> bool {
        self.arena
            .custody_view()
            .permits_cleanup_refund(instances, failure, bytes, budget)
    }

    pub(super) fn deny_active_root_refund(&self) {
        self.arena.custody_view().deny_active_root_refund();
    }

    fn check_selection_plan(
        &self,
        references: &SourceReferencePlanV29<'_, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.arena.check(budget)?;
        references.check_owner(self.arena.instances, budget)?;
        if references.storage_root.as_ref().is_none_or(|view| {
            view.arena != self.arena.identity()
                || !std::ptr::eq(view.layouts, self.arena.layouts)
                || !std::ptr::eq(view.instances, self.arena.instances)
        }) {
            return Err(error(
                "source selector changed its concrete root storage arena",
            ));
        }
        Ok(())
    }

    pub(super) fn snapshot_selected_readable(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        projections: &[SemanticProjectionV1],
        references: &SourceReferencePlanV29<'_, 'source>,
        source: Option<(SourceReferenceSiteV29, usize)>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            self.check_selection_plan(references, budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena.snapshots.try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let state = snapshots.get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            let path = state.selected_subobject(projections, references, source, budget)?;
            let readable = state.is_readable(&path, budget)?;
            path.discard(budget)?;
            Ok(readable)
        })())
    }

    pub(super) fn snapshot_selected_initialized(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        projections: &[SemanticProjectionV1],
        references: &SourceReferencePlanV29<'_, 'source>,
        source: Option<(SourceReferenceSiteV29, usize)>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            self.check_selection_plan(references, budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let state = snapshots
                .get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            let path = state.selected_subobject(projections, references, source, budget)?;
            let initialized = state.is_initialized(&path, budget)?;
            path.discard(budget)?;
            Ok(initialized)
        })())
    }

    pub(super) fn mutate_selected_snapshot(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        projections: &[SemanticProjectionV1],
        references: &SourceReferencePlanV29<'_, 'source>,
        source: Option<(SourceReferenceSiteV29, usize)>,
        mutation: SourceStorageRootMutationV29,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSnapshotV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            self.check_selection_plan(references, budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let mut value = snapshots
                .get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?
                .copy(budget)?;
            drop(snapshots);
            let path = value.selected_subobject(projections, references, source, budget)?;
            match mutation {
                SourceStorageRootMutationV29::Initialize => value.initialize(&path, budget)?,
                SourceStorageRootMutationV29::Deinitialize => value.deinitialize(&path, budget)?,
                _ => {
                    return Err(error(
                        "source selected enum tag requires guarded alias correspondence",
                    ));
                }
            }
            path.discard(budget)?;
            self.retain_snapshot(value, budget)
        })())
    }

    pub(super) fn snapshot_selected_copy_from(
        &self,
        destination: SourceStorageSnapshotV29<'view>,
        destination_path: &[SemanticProjectionV1],
        destination_source: Option<(SourceReferenceSiteV29, usize)>,
        source: SourceStorageSnapshotV29<'view>,
        source_path: &[SemanticProjectionV1],
        source_source: Option<(SourceReferenceSiteV29, usize)>,
        references: &SourceReferencePlanV29<'_, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSnapshotV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            self.check_selection_plan(references, budget)?;
            arena.handle(destination.arena)?;
            arena.handle(source.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let mut destination = snapshots
                .get(destination.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?
                .copy(budget)?;
            let source = snapshots
                .get(source.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            let a = destination.selected_subobject(
                destination_path,
                references,
                destination_source,
                budget,
            )?;
            let b = source.selected_subobject(source_path, references, source_source, budget)?;
            destination.copy_subobject_from(&a, source, &b, budget)?;
            a.discard(budget)?;
            b.discard(budget)?;
            drop(snapshots);
            self.retain_snapshot(destination, budget)
        })())
    }

    pub(super) fn forget_snapshot_selectors(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        keys: &[usize],
        references: &SourceReferencePlanV29<'_, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSnapshotV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            self.check_selection_plan(references, budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let original = snapshots
                .get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            if !original.has_selections || keys.is_empty() {
                return Ok(state);
            }
            let mut value = original.copy(budget)?;
            drop(snapshots);
            value.forget_selectors(keys, budget)?;
            self.retain_snapshot(value, budget)
        })())
    }

    #[cfg(test)]
    pub(super) fn snapshot_statistics(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(usize, usize), RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.layouts.lease.work(1, budget)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            Ok((snapshots.len(), arena.layouts.lease.owned.get()))
        })())
    }

    pub(super) fn custody_view(&self) -> SourceStorageRootCustodyViewV29<'view, 'source> {
        self.arena.custody_view()
    }

    pub(super) fn snapshot_ordinal(
        &self,
        snapshot: SourceStorageSnapshotV29<'view>,
        budget: &mut Budget<'_>,
    ) -> Result<usize, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(snapshot.arena)?;
            arena.layouts.lease.work(1, budget)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            snapshots
                .get(snapshot.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            Ok(snapshot.index)
        })())
    }

    fn retain_snapshot(
        &self,
        value: SourceStorageStateV29<'root, 'source>,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSnapshotV29<'view>, Error> {
        let arena = self.arena;
        let mut snapshots = arena
            .snapshots
            .try_borrow_mut()
            .map_err(|_| error("source storage snapshots are already borrowed"))?;
        let mut index = arena
            .snapshot_index
            .try_borrow_mut()
            .map_err(|_| error("source storage snapshot index is already borrowed"))?;
        arena.layouts.lease.work(3, budget)?;
        if !std::ptr::eq(value.layouts, arena.layouts)
            || !std::ptr::eq(value.instances, arena.instances)
            || index.next.len() != snapshots.len()
        {
            return Err(error(
                "source storage snapshot index changed its original arena",
            ));
        }
        let bucket = index.prepare_object(arena, value.instance, value.local, budget)?;
        if bucket.first.is_none() != bucket.last.is_none() {
            return Err(error("source storage snapshot bucket is incomplete"));
        }
        if let Some(last) = bucket.last {
            arena.layouts.lease.work(3, budget)?;
            let tail = snapshots
                .get(last)
                .ok_or_else(|| error("source storage snapshot tail is outside its arena"))?;
            if bucket.first.is_none_or(|first| first > last)
                || index.next.get(last) != Some(&None)
                || tail.instance != value.instance
                || tail.local != value.local
            {
                return Err(error(
                    "source storage snapshot tail changed its original object",
                ));
            }
        }
        // Compare before publication. Equivalent loop/call transfers refund
        // their temporary copy and retain only the original canonical state.
        let mut candidate = bucket.first;
        while let Some(ordinal) = candidate {
            arena.layouts.lease.work(4, budget)?;
            let existing = snapshots
                .get(ordinal)
                .ok_or_else(|| error("source storage snapshot link is outside its arena"))?;
            let next = *index
                .next
                .get(ordinal)
                .ok_or_else(|| error("source storage snapshot link is absent"))?;
            if existing.instance != value.instance
                || existing.local != value.local
                || next.is_some_and(|next| next <= ordinal || Some(next) > bucket.last)
                || (next.is_none() != (bucket.last == Some(ordinal)))
            {
                return Err(error(
                    "source storage snapshot link changed its original object",
                ));
            }
            if let Some(next) = next {
                arena.layouts.lease.work(3, budget)?;
                let successor = &snapshots[next];
                if successor.instance != value.instance || successor.local != value.local {
                    return Err(error(
                        "source storage snapshot successor changed its original object",
                    ));
                }
            }
            if existing.equivalent(&value, budget)? {
                value.discard(budget)?;
                return Ok(SourceStorageSnapshotV29 {
                    arena: arena.identity(),
                    index: ordinal,
                    view: PhantomData,
                });
            }
            candidate = next;
        }
        arena.layouts.lease.work(4, budget)?;
        let ordinal = snapshots.len();
        let instance = value.instance.index();
        let local = value.local.index() as usize;
        // A failed second append leaves only poisoned, scope-owned backing.
        // No link/handle is published until both concrete rosters exist.
        arena.layouts.lease.push(&mut index.next, None, budget)?;
        arena.layouts.lease.push(&mut snapshots, value, budget)?;
        if let Some(last) = bucket.last {
            index.next[last] = Some(ordinal);
        }
        index.objects[instance][local] = SourceStorageSnapshotBucketV29 {
            first: bucket.first.or(Some(ordinal)),
            last: Some(ordinal),
        };
        Ok(SourceStorageSnapshotV29 {
            arena: arena.identity(),
            index: ordinal,
            view: PhantomData,
        })
    }

    pub(super) fn snapshot_local(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        initialized: bool,
        budget: &mut Budget<'_>,
    ) -> Result<Option<SourceStorageSnapshotV29<'view>>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            let ty = arena
                .instances
                .instance(instance)
                .and_then(|row| row.declaration().locals().get(local.index() as usize))
                .ok_or_else(|| error("source snapshot local is outside its original instance"))?
                .ty();
            let key = RowKey::ty(ty);
            let (mut low, mut high) = (0, arena.layouts.keys.len());
            let mut present = false;
            while low < high {
                arena.layouts.lease.work(1, budget)?;
                let mid = low + (high - low) / 2;
                match arena.layouts.keys[mid].cmp(&key) {
                    std::cmp::Ordering::Less => low = mid + 1,
                    std::cmp::Ordering::Greater => high = mid,
                    std::cmp::Ordering::Equal => {
                        present = true;
                        break;
                    }
                }
            }
            let mut value = if present {
                SourceStorageStateV29::new(arena.layouts, arena.instances, instance, local, budget)?
            } else {
                // Logical presence needs no physical whole-object layout.
                // It grants neither backing nor nominal identity/provenance.
                SourceStorageStateV29::new_source(
                    arena.layouts,
                    arena.instances,
                    instance,
                    local,
                    budget,
                )?
            };
            if initialized {
                let root = value.root_subobject(budget)?;
                // An admitted by-value argument is a valid typed value, not a
                // certificate that its padding bytes were physically written.
                value.set_fact(&root, true, budget)?;
                root.discard(budget)?;
            }
            self.retain_snapshot(value, budget).map(Some)
        })())
    }

    pub(super) fn snapshots_equivalent(
        &self,
        left: SourceStorageSnapshotV29<'view>,
        right: SourceStorageSnapshotV29<'view>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(left.arena)?;
            arena.handle(right.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let left = snapshots
                .get(left.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            let right = snapshots
                .get(right.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            left.equivalent(right, budget)
        })())
    }

    pub(super) fn join_snapshots(
        &self,
        left: SourceStorageSnapshotV29<'view>,
        right: SourceStorageSnapshotV29<'view>,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSnapshotV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(left.arena)?;
            arena.handle(right.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let a = snapshots
                .get(left.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            let b = snapshots
                .get(right.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            if a.equivalent(b, budget)? {
                return Ok(left);
            }
            let value = a.join(b, budget)?;
            drop(snapshots);
            self.retain_snapshot(value, budget)
        })())
    }

    pub(super) fn snapshot_live(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            snapshots
                .get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?
                .is_live(budget)
        })())
    }

    pub(super) fn snapshot_readable(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        projections: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<bool, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena.snapshots.try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let state = snapshots.get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            let path = state.projected_subobject(projections, budget)?;
            let readable = state.is_readable(&path, budget)?;
            path.discard(budget)?;
            Ok(readable)
        })())
    }

    pub(super) fn snapshot_initialized(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        projections: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<bool, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let state = snapshots
                .get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            let path = state.projected_subobject(projections, budget)?;
            let initialized = state.is_initialized(&path, budget)?;
            path.discard(budget)?;
            Ok(initialized)
        })())
    }

    pub(super) fn snapshot_discriminant_initialized(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        projections: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<bool, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena.snapshots.try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let state = snapshots.get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            let path = state.projected_subobject(projections, budget)?;
            let initialized = state.discriminant_is_initialized(&path, budget)?;
            path.discard(budget)?;
            Ok(initialized)
        })())
    }

    pub(super) fn snapshot_discriminant_edge(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        plan: &SourceReferencePlanV29<'_, '_>,
        receipt: &SourceReferenceEnumEdgeV29,
        budget: &mut Budget<'_>,
    ) -> Result<Option<SourceStorageSnapshotV29<'view>>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            if !std::ptr::eq(plan.instances, arena.instances) {
                return Err(error("source discriminator edge changed its original instance owner"));
            }
            let allowed = plan.enum_edge_variants(receipt, budget)?;
            let observed = plan.enum_observations.get(receipt.observation)
                .ok_or_else(source_reference_enum_error_v29)?;
            let snapshots = arena.snapshots.try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let original = snapshots.get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            if original.instance != observed.instance || original.local != observed.local {
                return Err(error("source discriminator edge changed its observed object"));
            }
            let mut value = original.copy(budget)?;
            drop(snapshots);
            let projections = plan.projections.get(observed.first..argument_sum_v1(&[observed.first, observed.count])?)
                .ok_or_else(source_reference_enum_error_v29)?;
            let path = value.projected_subobject(projections, budget)?;
            let reachable = value.restrict_discriminant(&path, &allowed, budget)?;
            path.discard(budget)?;
            if reachable {
                self.retain_snapshot(value, budget).map(Some)
            } else {
                value.discard(budget)?;
                Ok(None)
            }
        })())
    }

    pub(super) fn mutate_snapshot(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        projections: &[SemanticProjectionV1],
        mutation: SourceStorageRootMutationV29,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSnapshotV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let mut value = snapshots
                .get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?
                .copy(budget)?;
            drop(snapshots);
            let path = value.projected_subobject(projections, budget)?;
            match mutation {
                SourceStorageRootMutationV29::Initialize => value.initialize(&path, budget)?,
                SourceStorageRootMutationV29::Deinitialize => value.deinitialize(&path, budget)?,
                SourceStorageRootMutationV29::BeginVariant(variant) => {
                    value.begin_variant(&path, variant, budget)?
                }
                SourceStorageRootMutationV29::SetDiscriminant(variant) => {
                    value.set_discriminant(&path, variant, budget)?
                }
            }
            path.discard(budget)?;
            self.retain_snapshot(value, budget)
        })())
    }

    pub(super) fn snapshot_lifetime(
        &self,
        state: SourceStorageSnapshotV29<'view>,
        live: bool,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSnapshotV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let mut value = snapshots
                .get(state.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?
                .copy(budget)?;
            drop(snapshots);
            if live {
                value.storage_live(budget)?;
            } else {
                value.storage_dead(budget)?;
            }
            self.retain_snapshot(value, budget)
        })())
    }

    pub(super) fn snapshot_copy_from(
        &self,
        destination: SourceStorageSnapshotV29<'view>,
        destination_path: &[SemanticProjectionV1],
        source: SourceStorageSnapshotV29<'view>,
        source_path: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSnapshotV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(destination.arena)?;
            arena.handle(source.arena)?;
            let snapshots = arena
                .snapshots
                .try_borrow()
                .map_err(|_| error("source storage snapshots are already borrowed"))?;
            let mut destination = snapshots
                .get(destination.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?
                .copy(budget)?;
            let source = snapshots
                .get(source.index)
                .ok_or_else(|| error("source storage snapshot is absent"))?;
            let a = destination.projected_subobject(destination_path, budget)?;
            let b = source.projected_subobject(source_path, budget)?;
            destination.copy_subobject_from(&a, source, &b, budget)?;
            a.discard(budget)?;
            b.discard(budget)?;
            drop(snapshots);
            self.retain_snapshot(destination, budget)
        })())
    }

    pub(super) fn new_state(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageStateHandleV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            let value = SourceStorageStateV29::new(
                arena.layouts,
                arena.instances,
                instance,
                local,
                budget,
            )?;
            let mut states = arena
                .states
                .try_borrow_mut()
                .map_err(|_| error("source storage state arena is already borrowed"))?;
            let index = states.len();
            arena.layouts.lease.push(&mut states, value, budget)?;
            Ok(SourceStorageStateHandleV29 {
                arena: arena.identity(),
                index,
                view: PhantomData,
            })
        })())
    }

    pub(super) fn copy_state(
        &self,
        state: SourceStorageStateHandleV29<'view>,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageStateHandleV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            let mut states = arena
                .states
                .try_borrow_mut()
                .map_err(|_| error("source storage state arena is already borrowed"))?;
            let value = states
                .get(state.index)
                .ok_or_else(|| error("source storage state handle is absent"))?
                .copy(budget)?;
            let index = states.len();
            arena.layouts.lease.push(&mut states, value, budget)?;
            Ok(SourceStorageStateHandleV29 {
                arena: arena.identity(),
                index,
                view: PhantomData,
            })
        })())
    }

    pub(super) fn root_path(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStoragePathHandleV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            let value = arena
                .layouts
                .root_subobject(arena.instances.owner(), ty, budget)?;
            let mut paths = arena
                .paths
                .try_borrow_mut()
                .map_err(|_| error("source storage path arena is already borrowed"))?;
            let index = paths.len();
            arena.layouts.lease.push(&mut paths, value, budget)?;
            Ok(SourceStoragePathHandleV29 {
                arena: arena.identity(),
                index,
                view: PhantomData,
            })
        })())
    }

    pub(super) fn project(
        &self,
        root: SemanticTypeIdV1,
        projections: &[SemanticProjectionV1],
        budget: &mut Budget<'_>,
    ) -> Result<SourceStoragePathHandleV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            let value = arena.layouts.projected_subobject(
                arena.instances.owner(),
                root,
                projections,
                budget,
            )?;
            let mut paths = arena
                .paths
                .try_borrow_mut()
                .map_err(|_| error("source storage path arena is already borrowed"))?;
            let index = paths.len();
            arena.layouts.lease.push(&mut paths, value, budget)?;
            Ok(SourceStoragePathHandleV29 {
                arena: arena.identity(),
                index,
                view: PhantomData,
            })
        })())
    }

    pub(super) fn mutate(
        &self,
        state: SourceStorageStateHandleV29<'view>,
        path: SourceStoragePathHandleV29<'view>,
        mutation: SourceStorageRootMutationV29,
        budget: &mut Budget<'_>,
    ) -> Result<(), RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            arena.handle(path.arena)?;
            let mut states = arena
                .states
                .try_borrow_mut()
                .map_err(|_| error("source storage state arena is already borrowed"))?;
            let paths = arena
                .paths
                .try_borrow()
                .map_err(|_| error("source storage path arena is already borrowed"))?;
            let value = states
                .get_mut(state.index)
                .ok_or_else(|| error("source storage state handle is absent"))?;
            let place = paths
                .get(path.index)
                .ok_or_else(|| error("source storage path handle is absent"))?;
            match mutation {
                SourceStorageRootMutationV29::Initialize => value.initialize(place, budget),
                SourceStorageRootMutationV29::Deinitialize => value.deinitialize(place, budget),
                SourceStorageRootMutationV29::BeginVariant(variant) => {
                    value.begin_variant(place, variant, budget)
                }
                SourceStorageRootMutationV29::SetDiscriminant(variant) => {
                    value.set_discriminant(place, variant, budget)
                }
            }
        })())
    }

    pub(super) fn is_initialized(
        &self,
        state: SourceStorageStateHandleV29<'view>,
        path: SourceStoragePathHandleV29<'view>,
        budget: &mut Budget<'_>,
    ) -> Result<bool, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(state.arena)?;
            arena.handle(path.arena)?;
            let states = arena
                .states
                .try_borrow()
                .map_err(|_| error("source storage state arena is already borrowed"))?;
            let paths = arena
                .paths
                .try_borrow()
                .map_err(|_| error("source storage path arena is already borrowed"))?;
            let value = states
                .get(state.index)
                .ok_or_else(|| error("source storage state handle is absent"))?;
            let place = paths
                .get(path.index)
                .ok_or_else(|| error("source storage path handle is absent"))?;
            value.is_initialized(place, budget)
        })())
    }

    pub(super) fn capture_origin(
        &self,
        plan: &SourceReferencePlanV29<'_, '_>,
        loan: usize,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageOriginsHandleV29<'view>, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            if !std::ptr::eq(arena.instances, plan.instances)
                || plan
                    .storage_root
                    .as_ref()
                    .is_none_or(|owner| owner.arena != arena.identity())
            {
                return Err(error(
                    "source storage origin changed its stable reference plan",
                ));
            }
            let value = SourceStorageOriginsV29::capture(arena.layouts, plan, loan, budget)?;
            let mut origins = arena
                .origins
                .try_borrow_mut()
                .map_err(|_| error("source storage origin arena is already borrowed"))?;
            let index = origins.len();
            arena.layouts.lease.push(&mut origins, value, budget)?;
            Ok(SourceStorageOriginsHandleV29 {
                arena: arena.identity(),
                index,
                view: PhantomData,
            })
        })())
    }

    pub(super) fn origin_count(
        &self,
        origins: SourceStorageOriginsHandleV29<'view>,
        plan: &SourceReferencePlanV29<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<usize, RecordedStorageFailureV29<'view>> {
        let arena = self.arena;
        arena.record((|| {
            arena.check(budget)?;
            arena.handle(origins.arena)?;
            let rows = arena
                .origins
                .try_borrow()
                .map_err(|_| error("source storage origin arena is already borrowed"))?;
            let value = rows
                .get(origins.index)
                .ok_or_else(|| error("source storage origin handle is absent"))?;
            Ok(value.keys(plan, budget)?.len())
        })())
    }
}
