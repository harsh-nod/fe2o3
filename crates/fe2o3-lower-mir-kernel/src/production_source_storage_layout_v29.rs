// Source-bound physical layout ownership. Active-root and partial-state
// consumers are separate prerequisites; a table row is not access authority.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source admission remains gated")
)]
mod source_storage_v29 {
    use super::production_call_instances_v1::ProductionCallInstancePlanV1;
    use super::*;
    use fe2o3_kernel_ir::{
        FixedVectorTypeV12, StorageFieldV1, StorageLayoutIdV1, StorageLayoutKindV1,
        StorageLayoutV1, StoragePointerV1, StorageVariantEncodingV1, StorageVariantV1,
        VectorLayoutV12,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBackendPrimitiveV1, SemanticBackendReprV1, SemanticEnumVariantLayoutV1,
        SemanticNichePathComponentV1, SemanticPointerTypeV1, SemanticTargetArchitectureV1,
    };
    use std::cell::{Cell, RefCell};
    use std::mem::size_of;

    type Error = ProductionSemanticKirErrorV1;
    type Budget<'a> = ArgumentBudgetV1<'a>;

    fn error(detail: &'static str) -> Error {
        unsupported(0, None, None, detail)
    }

    // This lease accounts only its own reservations. Caller-owned extra credit
    // is preserved when the dependent layout/state values have all been dropped.
    struct Lease {
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        slot: usize,
        floor: usize,
        owned: Cell<usize>,
        persistent: Cell<usize>,
        schema_allocation: Cell<bool>,
        failure: SourceStorageFailureCellV29,
        root: Cell<Option<SourceStorageRootCustodyV29>>,
    }

    impl Lease {
        fn new(budget: &Budget<'_>) -> Self {
            Self {
                ledger: budget.work_ledger_identity_v1(),
                slot: budget as *const Budget<'_> as usize,
                floor: budget.storage(),
                owned: Cell::new(0),
                persistent: Cell::new(0),
                schema_allocation: Cell::new(false),
                failure: SourceStorageFailureCellV29::new(),
                root: Cell::new(None),
            }
        }

        fn record(&self, error: &Error) {
            if let Error::ArgumentCorrespondenceResource(resource) = error {
                self.failure.record((*resource).into());
            }
        }

        fn custody(&self, budget: &Budget<'_>) -> Result<(), Error> {
            let expected = self.floor.checked_add(self.owned.get());
            let root = self.root.get().map(|root| root.required(self.owned.get()));
            if self.ledger != budget.work_ledger_identity_v1()
                || self.slot != budget as *const Budget<'_> as usize
                || expected.is_none_or(|floor| budget.storage() < floor)
                || root
                    .is_some_and(|required| required.is_none_or(|floor| budget.storage() < floor))
            {
                let error = Error::from(ArgumentResourceV1::Accounting);
                self.record(&error);
                return Err(self
                    .failure
                    .first_error()
                    .unwrap_or_else(|| ArgumentResourceV1::Accounting.into()));
            }
            Ok(())
        }

        fn check(&self, budget: &Budget<'_>) -> Result<(), Error> {
            if let Some(first) = self.failure.first_error() {
                return Err(first);
            }
            self.custody(budget)
        }

        fn work(&self, count: usize, budget: &mut Budget<'_>) -> Result<(), Error> {
            self.check(budget)?;
            budget
                .charge_work(count)
                .map_err(Error::from)
                .inspect_err(|e| self.record(e))
        }

        fn observe(&self, before: usize, after: usize) -> Result<(), Error> {
            let owned = if after >= before {
                self.owned.get().checked_add(after - before)
            } else {
                self.owned.get().checked_sub(before - after)
            }
            .ok_or_else(|| {
                let error = Error::from(ArgumentResourceV1::Accounting);
                self.record(&error);
                error
            })?;
            let persistent = if self.schema_allocation.get() {
                if after >= before {
                    self.persistent.get().checked_add(after - before)
                } else {
                    self.persistent.get().checked_sub(before - after)
                }
                .ok_or_else(|| {
                    let error = Error::from(ArgumentResourceV1::Accounting);
                    self.record(&error);
                    error
                })?
            } else {
                self.persistent.get()
            };
            self.owned.set(owned);
            self.persistent.set(persistent);
            Ok(())
        }

        fn refund(&self, bytes: usize, budget: &mut Budget<'_>) -> Result<(), Error> {
            self.custody(budget)?;
            let owned = self.owned.get().checked_sub(bytes).ok_or_else(|| {
                let error = Error::from(ArgumentResourceV1::Accounting);
                self.record(&error);
                error
            })?;
            let persistent = if self.schema_allocation.get() {
                self.persistent.get().checked_sub(bytes)
            } else {
                (owned >= self.persistent.get()).then_some(self.persistent.get())
            }
            .ok_or_else(|| {
                let error = Error::from(ArgumentResourceV1::Accounting);
                self.record(&error);
                error
            })?;
            budget
                .release_storage(bytes)
                .map_err(Error::from)
                .inspect_err(|e| self.record(e))?;
            self.owned.set(owned);
            self.persistent.set(persistent);
            Ok(())
        }

        fn reserve(&self, bytes: usize, budget: &mut Budget<'_>) -> Result<(), Error> {
            self.check(budget)?;
            let before = budget.storage();
            let result = budget.reserve_storage(bytes).map_err(Error::from);
            self.observe(before, budget.storage())?;
            result.inspect_err(|e| self.record(e))
        }

        fn vector<T>(&self, count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, Error> {
            self.check(budget)?;
            let before = budget.storage();
            let result = emission_vec_v1(count, budget);
            self.observe(before, budget.storage())?;
            result.inspect_err(|e| self.record(e))
        }

        fn push<T>(
            &self,
            rows: &mut Vec<T>,
            value: T,
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            self.check(budget)?;
            let before = budget.storage();
            let result = emission_push_v1(rows, value, budget);
            self.observe(before, budget.storage())?;
            result.inspect_err(|e| self.record(e))
        }

        fn discard_vec<T>(&self, rows: Vec<T>, budget: &mut Budget<'_>) -> Result<(), Error> {
            self.custody(budget)?;
            let bytes = argument_product_v1(rows.capacity(), size_of::<T>())?;
            drop(rows);
            self.refund(bytes, budget)
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    enum RowRole {
        Type,
        Payload(u32),
        Tag,
        DescriptorData,
        DescriptorLength,
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    struct RowKey {
        ty: SemanticTypeIdV1,
        role: RowRole,
    }

    impl RowKey {
        fn ty(ty: SemanticTypeIdV1) -> Self {
            Self {
                ty,
                role: RowRole::Type,
            }
        }
    }

    pub(super) struct SourceStorageLayoutsV29<'source> {
        owner: &'source ProductionSemanticSsaOwnerV1,
        lease: Lease,
        keys: Vec<RowKey>,
        physical: RefCell<SourceStoragePhysicalV29>,
        limits: fe2o3_kernel_ir::StorageLayoutLimitsV1,
    }

    // Iterative heapsort has metered comparisons/swaps and no hidden scratch or
    // recursive stack. Source row order is independent of root demand order.
    fn sort_rows<T: Copy + Ord>(
        rows: &mut [T],
        lease: &Lease,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        fn sift<T: Copy + Ord>(
            rows: &mut [T],
            mut parent: usize,
            end: usize,
            lease: &Lease,
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            loop {
                lease.work(1, budget)?;
                let Some(mut child) = parent
                    .checked_mul(2)
                    .and_then(|v| v.checked_add(1))
                    .filter(|v| *v < end)
                else {
                    break;
                };
                if child + 1 < end {
                    lease.work(1, budget)?;
                    if rows[child] < rows[child + 1] {
                        child += 1;
                    }
                }
                lease.work(1, budget)?;
                if rows[parent] >= rows[child] {
                    break;
                }
                lease.work(1, budget)?;
                rows.swap(parent, child);
                parent = child;
            }
            Ok(())
        }
        for parent in (0..rows.len() / 2).rev() {
            sift(rows, parent, rows.len(), lease, budget)?;
        }
        for end in (1..rows.len()).rev() {
            lease.work(1, budget)?;
            rows.swap(0, end);
            sift(rows, 0, end, lease, budget)?;
        }
        Ok(())
    }

    impl<'source> SourceStorageLayoutsV29<'source> {
        fn construct(
            owner: &'source ProductionSemanticSsaOwnerV1,
            demands: &[SemanticTypeIdV1],
            limits: fe2o3_kernel_ir::StorageLayoutLimitsV1,
            budget: &mut Budget<'_>,
        ) -> Result<Self, Error> {
            let mut binding = Self {
                owner,
                lease: Lease::new(budget),
                keys: Vec::new(),
                physical: RefCell::new(SourceStoragePhysicalV29::default()),
                limits,
            };
            let result = binding.build(demands, budget);
            match result {
                Ok(()) => {
                    binding.lease.persistent.set(binding.lease.owned.get());
                    Ok(binding)
                }
                Err(first) => {
                    // All fields have built-in destructors; no generic callback
                    // or user panic payload is accepted by this constructor.
                    let owned = binding.lease.owned.get();
                    binding.lease.custody(budget)?;
                    drop(binding);
                    budget.release_storage(owned)?;
                    Err(first)
                }
            }
        }

        fn build(
            &mut self,
            demands: &[SemanticTypeIdV1],
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            self.lease.work(1, budget)?;
            self.lease.reserve(
                argument_sum_v1(&[
                    size_of::<Self>(),
                    size_of::<Result<Self, Error>>(),
                    size_of::<Vec<bool>>(),
                    size_of::<Vec<StorageFieldV1>>(),
                    size_of::<Vec<StorageVariantV1>>(),
                    size_of::<Vec<StorageLayoutV1>>(),
                    size_of::<Result<StorageLayoutV1, Error>>(),
                    6 * size_of::<usize>(),
                ])?,
                budget,
            )?;
            let mut selected = self
                .lease
                .vector::<bool>(self.owner.source_semantic().types().len(), budget)?;
            self.lease
                .work(self.owner.source_semantic().types().len(), budget)?;
            selected.resize(self.owner.source_semantic().types().len(), false);
            for &ty in demands {
                self.enqueue(ty, &mut selected, budget)?;
            }
            let mut next = 0;
            while next < self.keys.len() {
                self.lease.work(1, budget)?;
                let key = self.keys[next];
                if key.role == RowRole::Type {
                    self.follow_type(key.ty, &mut selected, budget)?;
                }
                next += 1;
            }
            sort_rows(&mut self.keys, &self.lease, budget)?;
            self.initialize_schema_metrics(budget)?;
            self.physical.get_mut().rows = self.lease.vector(self.keys.len(), budget)?;
            for index in 0..self.keys.len() {
                self.lease.work(1, budget)?;
                let row = self.lower_row(self.keys[index], budget)?;
                self.lease
                    .push(&mut self.physical.get_mut().rows, row, budget)?;
            }
            self.lease.discard_vec(selected, budget)
        }

        // Preflight original geometry before allocating physical row payloads.
        // The demand closure is an index, not another layout or CFG owner.
        fn original_row_metrics(&self, key: RowKey) -> Result<(u64, usize), Error> {
            let declaration = self.declaration(key.ty)?;
            let layout = declaration.layout();
            let sized = || {
                layout
                    .size_bytes()
                    .ok_or_else(|| error("source storage object is unsized"))
            };
            Ok(match key.role {
                RowRole::DescriptorData | RowRole::DescriptorLength => {
                    let (data, length, _) = self.descriptor_parts(key.ty)?;
                    let data_row = key.role == RowRole::DescriptorData;
                    let primitive = if data_row { data } else { length };
                    (
                        primitive
                            .size_bytes()
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                        usize::from(data_row),
                    )
                }
                RowRole::Tag => {
                    let SemanticRustcVariantsV1::Multiple(enumeration) = layout.variants() else {
                        return Err(error("source tag lacks its original enum encoding"));
                    };
                    let primitive = match enumeration.encoding() {
                        SemanticEnumEncodingV1::Direct(tag) => tag.tag().primitive(),
                        SemanticEnumEncodingV1::Niche(tag) => tag.tag().primitive(),
                    };
                    (
                        primitive
                            .size_bytes()
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                        usize::from(matches!(
                            primitive,
                            SemanticBackendPrimitiveV1::Pointer { .. }
                        )),
                    )
                }
                RowRole::Payload(index) => {
                    let (_, variants) = self.enum_parts(key.ty)?;
                    let fields = variants
                        .get(index as usize)
                        .ok_or_else(|| error("source payload variant is absent"))?
                        .fields()
                        .fields();
                    (
                        self.enum_variant_layout(key.ty, index)?.rustc_size_bytes(),
                        fields.len(),
                    )
                }
                RowRole::Type => {
                    (
                        sized()?,
                        match declaration.shape() {
                            SemanticTypeShapeV1::Tuple(fields)
                            | SemanticTypeShapeV1::Aggregate(fields)
                            | SemanticTypeShapeV1::Union(fields) => fields.fields().len(),
                            SemanticTypeShapeV1::Array { .. } => 1,
                            SemanticTypeShapeV1::Pointer(pointer) => match pointer.metadata() {
                                SemanticPointerMetadataV1::None => 1,
                                SemanticPointerMetadataV1::SliceLength => 3,
                                SemanticPointerMetadataV1::VTable => {
                                    return Err(error("source vtable lacks a physical schema"));
                                }
                            },
                            SemanticTypeShapeV1::Enum { variants, .. } => match layout.variants() {
                                SemanticRustcVariantsV1::Empty => 0,
                                SemanticRustcVariantsV1::Single { index } => variants
                                    .get(*index as usize)
                                    .ok_or_else(|| error("source single variant is absent"))?
                                    .fields()
                                    .fields()
                                    .len(),
                                SemanticRustcVariantsV1::Multiple(_) => variants
                                    .len()
                                    .checked_add(1)
                                    .ok_or(ArgumentResourceV1::Arithmetic)?,
                            },
                            _ => 0,
                        },
                    )
                }
            })
        }

        fn original_contained_key(
            &self,
            key: RowKey,
            index: usize,
        ) -> Result<Option<RowKey>, Error> {
            let declaration = self.declaration(key.ty)?;
            let fields = match key.role {
                RowRole::Tag | RowRole::DescriptorData | RowRole::DescriptorLength => {
                    return Ok(None);
                }
                RowRole::Payload(variant) => {
                    let (_, variants) = self.enum_parts(key.ty)?;
                    variants
                        .get(variant as usize)
                        .ok_or_else(|| error("source payload variant is absent"))?
                        .fields()
                        .fields()
                }
                RowRole::Type => match declaration.shape() {
                    SemanticTypeShapeV1::Tuple(fields)
                    | SemanticTypeShapeV1::Aggregate(fields)
                    | SemanticTypeShapeV1::Union(fields) => fields.fields(),
                    SemanticTypeShapeV1::Array { element, .. } => {
                        return Ok((index == 0).then_some(RowKey::ty(*element)));
                    }
                    SemanticTypeShapeV1::Pointer(pointer)
                        if pointer.metadata() == SemanticPointerMetadataV1::SliceLength =>
                    {
                        return Ok(match index {
                            0 => Some(RowKey {
                                ty: key.ty,
                                role: RowRole::DescriptorData,
                            }),
                            1 => Some(RowKey {
                                ty: key.ty,
                                role: RowRole::DescriptorLength,
                            }),
                            _ => None,
                        });
                    }
                    SemanticTypeShapeV1::Enum { variants, .. } => {
                        match declaration.layout().variants() {
                            SemanticRustcVariantsV1::Empty => return Ok(None),
                            SemanticRustcVariantsV1::Single { index: variant } => variants
                                .get(*variant as usize)
                                .ok_or_else(|| error("source single variant is absent"))?
                                .fields()
                                .fields(),
                            SemanticRustcVariantsV1::Multiple(_) => {
                                return Ok(if index < variants.len() {
                                    Some(RowKey {
                                        ty: key.ty,
                                        role: RowRole::Payload(
                                            u32::try_from(index)
                                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                        ),
                                    })
                                } else if index == variants.len() {
                                    Some(RowKey {
                                        ty: key.ty,
                                        role: RowRole::Tag,
                                    })
                                } else {
                                    None
                                });
                            }
                        }
                    }
                    _ => return Ok(None),
                },
            };
            Ok(fields.get(index).copied().map(RowKey::ty))
        }

        fn enqueue(
            &mut self,
            ty: SemanticTypeIdV1,
            selected: &mut [bool],
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            self.lease.work(1, budget)?;
            let seen = selected
                .get_mut(ty.index() as usize)
                .ok_or_else(|| error("source storage demand type is absent"))?;
            if !*seen {
                self.check_row_capacity(self.keys.len(), budget)?;
                *seen = true;
                self.lease.push(&mut self.keys, RowKey::ty(ty), budget)?;
            }
            Ok(())
        }

        fn auxiliary(
            &mut self,
            ty: SemanticTypeIdV1,
            role: RowRole,
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            self.check_row_capacity(self.keys.len(), budget)?;
            self.lease.push(&mut self.keys, RowKey { ty, role }, budget)
        }

        fn declaration(&self, ty: SemanticTypeIdV1) -> Result<&'source SemanticTypeDeclV1, Error> {
            self.owner
                .source_semantic()
                .types()
                .get(ty.index() as usize)
                .ok_or_else(|| error("source storage type is absent"))
        }

        fn row_id(&self, key: RowKey, budget: &mut Budget<'_>) -> Result<StorageLayoutIdV1, Error> {
            let (mut low, mut high) = (0, self.keys.len());
            while low < high {
                self.lease.work(1, budget)?;
                let mid = low + (high - low) / 2;
                match self.keys[mid].cmp(&key) {
                    std::cmp::Ordering::Less => low = mid + 1,
                    std::cmp::Ordering::Greater => high = mid,
                    std::cmp::Ordering::Equal => {
                        return Ok(StorageLayoutIdV1(
                            u32::try_from(mid).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ));
                    }
                }
            }
            Err(error(
                "source storage row was not included in the module demand closure",
            ))
        }

        pub(super) fn row_for(
            &self,
            owner: &ProductionSemanticSsaOwnerV1,
            ty: SemanticTypeIdV1,
            budget: &mut Budget<'_>,
        ) -> Result<StorageLayoutIdV1, Error> {
            self.check_owner(owner, budget)?;
            self.row_id(RowKey::ty(ty), budget)
        }

        pub(super) fn check_owner(
            &self,
            owner: &ProductionSemanticSsaOwnerV1,
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            self.lease.work(1, budget)?;
            if !std::ptr::eq(self.owner, owner) {
                return Err(error(
                    "source storage layout belongs to a different source SSA owner",
                ));
            }
            Ok(())
        }

        pub(super) fn rows<'a>(
            &'a self,
            owner: &ProductionSemanticSsaOwnerV1,
            budget: &mut Budget<'_>,
        ) -> Result<std::cell::Ref<'a, [StorageLayoutV1]>, Error> {
            self.check_owner(owner, budget)?;
            let physical = self
                .physical
                .try_borrow()
                .map_err(|_| error("source storage physical table has an active schema builder"))?;
            Ok(std::cell::Ref::map(physical, |physical| {
                physical.rows.as_slice()
            }))
        }

        pub(super) fn release(self, budget: &mut Budget<'_>) -> Result<(), Error> {
            self.lease.custody(budget)?;
            if self.lease.root.get().is_some() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let owned = self.lease.owned.get();
            let Self {
                owner: _,
                lease,
                keys,
                physical,
                limits: _,
            } = self;
            let first = lease.failure.into_first();
            drop(keys);
            drop(physical);
            budget.release_storage(owned)?;
            first.map_or(Ok(()), Err)
        }
    }

    include!("production_source_storage_limits_v29.rs");
    include!("production_source_storage_representation_v29.rs");
    include!("production_source_storage_original_graph_v29.rs");
    include!("production_source_storage_original_closure_v29.rs");
    include!("production_source_storage_table_custody_v29.rs");
    include!("production_source_storage_root_custody_v29.rs");
    include!("production_source_storage_root_arena_v29.rs");
    include!("production_source_storage_rows_transfer_v29.rs");
    include!("production_source_storage_rows_v29.rs");
    include!("production_source_storage_enum_v29.rs");
    include!("production_source_storage_state_v29.rs");
    include!("production_source_storage_selectors_v29.rs");
    include!("production_source_storage_origins_v29.rs");

    #[cfg(test)]
    mod limits_tests {
        use super::*;
        include!("production_source_storage_limits_v29_tests.rs");
    }
    #[cfg(test)]
    mod layout_tests {
        use super::*;
        include!("production_source_storage_layout_v29_tests.rs");
    }
    #[cfg(test)]
    mod rows_transfer_tests {
        use super::*;
        include!("production_source_storage_rows_transfer_v29_tests.rs");
    }
    #[cfg(test)]
    mod state_tests {
        use super::*;
        include!("production_source_storage_state_v29_tests.rs");
    }
    #[cfg(test)]
    mod origin_tests {
        use super::*;
        include!("production_source_storage_origins_v29_tests.rs");
    }
    #[cfg(test)]
    mod resource_tests {
        use super::*;
        include!("production_source_storage_resources_v29_tests.rs");
    }
    #[cfg(test)]
    mod logical_resource_tests {
        use super::*;
        include!("production_source_storage_logical_resources_v29_tests.rs");
    }
    #[cfg(test)]
    mod root_custody_tests {
        use super::*;
        include!("production_source_storage_root_custody_v29_tests.rs");
    }
    #[cfg(test)]
    mod root_arena_tests {
        use super::*;
        include!("production_source_storage_root_arena_v29_tests.rs");
    }
    #[cfg(test)]
    mod snapshot_index_tests {
        use super::*;
        include!("production_source_storage_snapshot_index_v29_tests.rs");
    }
    #[cfg(test)]
    mod selector_tests {
        use super::*;
        include!("production_source_storage_selectors_v29_tests.rs");
    }
    #[cfg(test)]
    mod selector_resource_tests {
        use super::*;
        include!("production_source_storage_selector_resources_v29_tests.rs");
    }
    #[cfg(test)]
    mod representation_tests {
        use super::*;
        include!("production_source_storage_representation_v29_tests.rs");
    }
    #[cfg(fe2o3_source_storage_root_custody_ui)]
    mod root_custody_ui {
        use super::*;
        include!("production_source_storage_root_custody_v29_ui.rs");
    }
}
