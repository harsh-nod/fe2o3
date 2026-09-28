impl SourceStorageLayoutsV29<'_> {
    // Move the actual source-bound row backing once. K remains charged to the
    // candidate; table headers, keys and interrupted scratch are destroyed first.
    pub(super) fn install_rows(
        self,
        owner: &ProductionSemanticSsaOwnerV1,
        candidate: &mut Module,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Error> {
        let checked = (|| {
            self.lease.check(budget)?;
            if self.lease.root.get().is_some() {
                return Err(error("source storage rows still have an active root scope"));
            }
            self.check_owner(owner, budget)?;
            self.lease.reserve(
                argument_sum_v1(&[
                    size_of::<Result<usize, Error>>(),
                    size_of::<Option<Error>>(),
                    3 * size_of::<usize>(),
                ])?,
                budget,
            )?;
            if candidate.storage_layouts.capacity() != 0 {
                return Err(error(
                    "source storage row installation requires an empty candidate",
                ));
            }
            let physical = self
                .physical
                .try_borrow()
                .map_err(|_| error("source storage installation has an active schema builder"))?;
            if !physical.selected_keys.is_empty() {
                fe2o3_kernel_ir::check_storage_layouts_v1(&physical.rows, self.limits, budget)
                    .map_err(source_storage_limits_error_v29)?;
            }
            self.lease.work(physical.rows.len(), budget)?;
            let mut retained =
                argument_product_v1(physical.rows.capacity(), size_of::<StorageLayoutV1>())?;
            for row in &physical.rows {
                let bytes = match &row.kind {
                    StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
                        argument_product_v1(fields.len(), size_of::<StorageFieldV1>())?
                    }
                    StorageLayoutKindV1::Variants { variants, .. } => {
                        argument_product_v1(variants.len(), size_of::<StorageVariantV1>())?
                    }
                    StorageLayoutKindV1::Scalar(_)
                    | StorageLayoutKindV1::Vector(_)
                    | StorageLayoutKindV1::Pointer(_)
                    | StorageLayoutKindV1::Array { .. }
                    | StorageLayoutKindV1::Slice { .. } => 0,
                };
                retained = retained
                    .checked_add(bytes)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            self.lease
                .owned
                .get()
                .checked_sub(retained)
                .ok_or(ArgumentResourceV1::Accounting)?;
            Ok(retained)
        })();
        match checked {
            Ok(retained) => {
                let owned = self.lease.owned.get();
                let refund = owned
                    .checked_sub(retained)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                // No user code or fallible traversal can run after this final
                // custody check and before the ownership transfer/refund pair.
                self.lease.custody(budget)?;
                let Self {
                    owner: _,
                    lease: _,
                    keys,
                    physical,
                    limits: _,
                } = self;
                drop(keys);
                let SourceStoragePhysicalV29 {
                    rows,
                    selected,
                    selected_keys,
                    depths,
                    edges: _,
                    original_extensions,
                } = physical.into_inner();
                drop((selected, selected_keys, depths, original_extensions));
                candidate.storage_layouts = rows;
                budget.release_storage(refund)?;
                Ok(retained)
            }
            Err(first) => {
                self.lease.failure.record(first);
                let custody = self.lease.custody(budget).is_ok() && self.lease.root.get().is_none();
                let owned = self.lease.owned.get();
                let Self {
                    owner: _,
                    lease,
                    keys,
                    physical,
                    limits: _,
                } = self;
                let first = lease
                    .failure
                    .into_first()
                    .unwrap_or_else(|| ArgumentResourceV1::Accounting.into());
                drop(keys);
                drop(physical);
                if custody {
                    // A cleanup error cannot replace the selected source error.
                    let _ = budget.release_storage(owned);
                }
                Err(first)
            }
        }
    }
}
