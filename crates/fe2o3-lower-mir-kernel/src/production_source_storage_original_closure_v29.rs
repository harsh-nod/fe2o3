// Only this source-bound builder can resolve forward IDs. No source shape,
// validity or runtime pointer fact is supplied by a selected physical row.
enum SourceStorageOriginalResolverV29<'a> {
    Original,
    Completed,
    Constructing(&'a SourceStorageOriginalGraphV29),
}

struct SourceStorageOriginalStagedV29 {
    node: usize,
    id: StorageLayoutIdV1,
    row: StorageLayoutV1,
    hash: u64,
    backing: usize,
}

impl SourceStorageLayoutsV29<'_> {
    fn original_map_reserve<K, V>(
        &self,
        count: usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.lease.check(budget)?;
        let before = budget.storage();
        let result = reserve_execution_cfg_map_entry_v29::<K, V>(count, budget);
        self.lease.observe(before, budget.storage())?;
        result.inspect_err(|error| self.lease.record(error))
    }

    fn original_completed_id(
        &self,
        key: RowKey,
        budget: &mut Budget<'_>,
    ) -> Result<Option<StorageLayoutIdV1>, Error> {
        match self.row_id(key, budget) {
            Ok(id) => return Ok(Some(id)),
            Err(Error::Unsupported { .. }) => {}
            Err(error) => return Err(error),
        }
        let id = {
            let physical = self
                .physical
                .try_borrow()
                .map_err(|_| error("original schema lookup has an active builder"))?;
            charge_execution_cfg_lookup_v29(physical.original_extensions.len(), budget)?;
            physical.original_extensions.get(&key).copied()
        };
        if let Some(id) = id {
            if self.schema_key(id, budget)? != key {
                return Err(error("original schema extension changed source identity"));
            }
            let physical = self
                .physical
                .try_borrow()
                .map_err(|_| error("original schema lookup has an active builder"))?;
            if id.0 as usize >= physical.rows.len() || id.0 as usize >= physical.depths.len() {
                return Err(error("original schema extension is not closed"));
            }
        }
        Ok(id)
    }

    fn original_resolve(
        &self,
        key: RowKey,
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutIdV1, Error> {
        match resolver {
            SourceStorageOriginalResolverV29::Original => self.row_id(key, budget),
            SourceStorageOriginalResolverV29::Completed => self
                .original_completed_id(key, budget)?
                .ok_or_else(|| error("original schema has a missing completed dependency")),
            SourceStorageOriginalResolverV29::Constructing(graph) => {
                self.lease.work(1, budget)?;
                charge_execution_cfg_lookup_v29(graph.keys.len(), budget)?;
                let index = *graph
                    .keys
                    .get(&key)
                    .ok_or_else(|| error("original schema dependency was not discovered"))?;
                let node = graph
                    .nodes
                    .get(index)
                    .ok_or_else(|| error("original schema dependency index is absent"))?;
                if node.key != key {
                    return Err(error("original schema dependency source key changed"));
                }
                node.id
                    .ok_or_else(|| error("original schema dependency is not assigned"))
            }
        }
    }

    fn original_checked_id(
        &self,
        key: RowKey,
        budget: &mut Budget<'_>,
    ) -> Result<Option<StorageLayoutIdV1>, Error> {
        self.lease.work(1, budget)?;
        let Some(id) = self.original_completed_id(key, budget)? else {
            return Ok(None);
        };
        if (id.0 as usize) < self.keys.len() {
            return Ok(Some(id));
        }
        // A same-type selected row is not necessarily the original schema.
        // Reuse the source builder for an exact shallow comparison, never a hash
        // or physical-row-derived interpretation of the source declaration.
        let expected =
            self.lower_row_resolved(key, &SourceStorageOriginalResolverV29::Completed, budget)?;
        let backing = Self::row_backing(&expected)?;
        self.lease
            .work(argument_sum_v1(&[1, Self::row_edges(&expected)?])?, budget)?;
        let equal = self
            .physical
            .try_borrow()
            .map_err(|_| error("original schema comparison has an active builder"))?
            .rows
            .get(id.0 as usize)
            == Some(&expected);
        drop(expected);
        self.lease.refund(backing, budget)?;
        if !equal {
            return Err(error(
                "original schema extension is a different representation",
            ));
        }
        Ok(Some(id))
    }

    fn original_reference_key(
        &self,
        key: RowKey,
        budget: &mut Budget<'_>,
    ) -> Result<Option<RowKey>, Error> {
        self.lease.work(1, budget)?;
        let declaration = self.declaration(key.ty)?;
        if key.role == RowRole::Tag {
            let SemanticRustcVariantsV1::Multiple(layout) = declaration.layout().variants() else {
                return Err(error("original tag dependency lacks its enum layout"));
            };
            let primitive = match layout.encoding() {
                SemanticEnumEncodingV1::Direct(tag) => tag.tag().primitive(),
                SemanticEnumEncodingV1::Niche(tag) => tag.tag().primitive(),
            };
            return if matches!(primitive, SemanticBackendPrimitiveV1::Pointer { .. }) {
                Ok(Some(RowKey::ty(
                    self.niche_pointer_target(key.ty, primitive, budget)?.1,
                )))
            } else {
                Ok(None)
            };
        }
        if !matches!(key.role, RowRole::Type | RowRole::DescriptorData) {
            return Ok(None);
        }
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return if key.role == RowRole::Type {
                Ok(None)
            } else {
                Err(error(
                    "original descriptor dependency has a non-pointer type",
                ))
            };
        };
        Ok(Some(RowKey::ty(match pointer.metadata() {
            SemanticPointerMetadataV1::None if key.role == RowRole::Type => pointer.pointee(),
            SemanticPointerMetadataV1::SliceLength => self.slice_element(pointer)?,
            _ => return Err(error("original pointer dependency lacks admitted metadata")),
        })))
    }

    fn original_grow<T>(
        &self,
        rows: &mut Vec<T>,
        additional: usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.lease.work(1, budget)?;
        let needed = argument_sum_v1(&[rows.len(), additional])?;
        if needed <= rows.capacity() {
            return Ok(());
        }
        let capacity = needed.max(argument_product_v1(rows.capacity().max(2), 2)?);
        let mut replacement = self.lease.vector(capacity, budget)?;
        self.lease.work(rows.len(), budget)?;
        let old = argument_product_v1(rows.capacity(), size_of::<T>())?;
        replacement.append(rows);
        *rows = replacement;
        self.lease.refund(old, budget)
    }

    fn original_promote_payload(&self, bytes: usize, budget: &Budget<'_>) -> Result<(), Error> {
        self.lease.custody(budget)?;
        if !self.lease.schema_allocation.get() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let retained = self
            .lease
            .persistent
            .get()
            .checked_add(bytes)
            .filter(|&retained| retained <= self.lease.owned.get())
            .ok_or(ArgumentResourceV1::Accounting)?;
        self.lease.persistent.set(retained);
        Ok(())
    }

    fn original_publish_single(
        &self,
        key: RowKey,
        row: StorageLayoutV1,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutIdV1, Error> {
        let backing = Self::row_backing(&row)?;
        let before = {
            let physical = self
                .physical
                .try_borrow()
                .map_err(|_| error("original publication has an active builder"))?;
            charge_execution_cfg_lookup_v29(physical.original_extensions.len(), budget)?;
            if physical.original_extensions.contains_key(&key) {
                return Err(error("original publication repeated a completed key"));
            }
            physical.rows.len()
        };
        let _persistent = self.schema_allocation(budget)?;
        self.original_promote_payload(backing, budget)?;
        let result = self.intern_schema(key, row, budget);
        if result.is_err() {
            let unchanged = self
                .physical
                .try_borrow()
                .map_err(|_| error("original publication lost its table guard"))?
                .rows
                .len()
                == before;
            if unchanged {
                // intern_schema consumed/dropped the unpublished row. Only its
                // payload credit was promoted; table capacity credit stays live.
                self.lease.refund(backing, budget)?;
            }
        }
        let id = result?;
        let mut physical = self
            .physical
            .try_borrow_mut()
            .map_err(|_| error("original publication has an outstanding row view"))?;
        self.original_map_reserve::<RowKey, StorageLayoutIdV1>(
            physical.original_extensions.len(),
            budget,
        )?;
        physical.original_extensions.insert(key, id);
        Ok(id)
    }

    fn original_validate_staged(
        &self,
        graph: &SourceStorageOriginalGraphV29,
        order: &[(usize, RowKey, usize)],
        base: usize,
        node: usize,
        row: &StorageLayoutV1,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.lease.work(1, budget)?;
        let expected = graph
            .nodes
            .get(node)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let (size, edges) = self.original_row_metrics(expected.key)?;
        if row.size != size || Self::row_edges(row)? != edges {
            return Err(error(
                "original publication changed source geometry or edge roster",
            ));
        }
        let physical = self
            .physical
            .try_borrow()
            .map_err(|_| error("original publication has an active builder"))?;
        let mut depth = 1_usize;
        let mut next = 0;
        while let Some(child) = Self::contained_child(row, next) {
            self.lease.work(1, budget)?;
            next += 1;
            let child_depth = if (child.0 as usize) < base {
                *physical
                    .depths
                    .get(child.0 as usize)
                    .ok_or_else(|| error("original publication has an absent completed child"))?
            } else {
                let &(height, key, index) = order
                    .get(
                        (child.0 as usize)
                            .checked_sub(base)
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                    )
                    .ok_or_else(|| error("original publication has an absent reserved child"))?;
                let child_node = graph
                    .nodes
                    .get(index)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if child_node.key != key
                    || child_node.id != Some(child)
                    || child_node.depth != height
                {
                    return Err(error(
                        "original publication changed reserved child identity",
                    ));
                }
                height
            };
            depth = depth.max(
                child_depth
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
            );
        }
        if depth != expected.depth || depth > self.limits.containment_depth {
            return Err(error("original publication containment is not closed"));
        }
        Ok(())
    }

    fn original_publish_group(
        &self,
        graph: &SourceStorageOriginalGraphV29,
        order: &[(usize, RowKey, usize)],
        base: usize,
        staged: &mut Vec<SourceStorageOriginalStagedV29>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.lease.work(1, budget)?;
        if order.len() != staged.len() || staged.is_empty() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let _persistent = self.schema_allocation(budget)?;
        let mut physical = self
            .physical
            .try_borrow_mut()
            .map_err(|_| error("original group publication has an outstanding row view"))?;
        if physical.rows.len() != base
            || physical.depths.len() != base
            || argument_sum_v1(&[self.keys.len(), physical.selected_keys.len()])? != base
        {
            return Err(error("original group publication table identity changed"));
        }
        let end = argument_sum_v1(&[base, staged.len()])?;
        if end > self.limits.rows || u32::try_from(end - 1).is_err() {
            return Err(error("original group publication exceeds row policy"));
        }
        let mut edges = physical.edges;
        let mut payload = 0;
        for (offset, ((depth, key, node), row)) in order.iter().zip(staged.iter()).enumerate() {
            self.lease.work(1, budget)?;
            let node_row = graph
                .nodes
                .get(*node)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if row.node != *node
                || node_row.key != *key
                || *depth != node_row.depth
                || node_row.id != Some(row.id)
                || row.id.0 as usize != argument_sum_v1(&[base, offset])?
            {
                return Err(error("original group publication reorders source rows"));
            }
            edges = argument_sum_v1(&[edges, Self::row_edges(&row.row)?])?;
            payload = argument_sum_v1(&[payload, row.backing])?;
            if row.row.size > self.limits.object_bytes || *depth > self.limits.containment_depth {
                return Err(error("original group publication exceeds geometry policy"));
            }
        }
        if edges > self.limits.edges {
            return Err(error("original group publication exceeds edge policy"));
        }
        self.lease
            .persistent
            .get()
            .checked_add(payload)
            .filter(|&bytes| bytes <= self.lease.owned.get())
            .ok_or(ArgumentResourceV1::Accounting)?;
        self.original_grow(&mut physical.rows, staged.len(), budget)?;
        self.original_grow(&mut physical.selected_keys, staged.len(), budget)?;
        self.original_grow(&mut physical.depths, staged.len(), budget)?;
        // Prepay map paths and exact collision buckets before any row is moved.
        // Empty interner buckets are inert, and stay paid on a later refusal.
        for ((_, key, _), row) in order.iter().zip(staged.iter()) {
            self.lease.work(1, budget)?;
            charge_execution_cfg_lookup_v29(physical.original_extensions.len(), budget)?;
            if physical.original_extensions.contains_key(key) {
                return Err(error("original group key is already completed"));
            }
            charge_execution_cfg_lookup_v29(physical.selected.len(), budget)?;
            if !physical.selected.contains_key(&(*key, row.hash)) {
                self.original_map_reserve::<(RowKey, u64), Vec<StorageLayoutIdV1>>(
                    physical.selected.len(),
                    budget,
                )?;
                physical.selected.insert((*key, row.hash), Vec::new());
            }
            charge_execution_cfg_lookup_v29(physical.selected.len(), budget)?;
            let bucket = physical
                .selected
                .get_mut(&(*key, row.hash))
                .ok_or(ArgumentResourceV1::Accounting)?;
            self.original_grow(bucket, 1, budget)?;
        }
        for extra in 0..staged.len() {
            self.original_map_reserve::<RowKey, StorageLayoutIdV1>(
                argument_sum_v1(&[physical.original_extensions.len(), extra])?,
                budget,
            )?;
            charge_execution_cfg_lookup_v29(physical.selected.len(), budget)?;
        }
        self.lease
            .work(argument_product_v1(staged.len(), 7)?, budget)?;
        // No callback, allocation reservation or fallible work follows a move.
        // Each staged payload's prepaid credit changes custody exactly once.
        let SourceStoragePhysicalV29 {
            rows,
            selected,
            selected_keys,
            depths,
            original_extensions,
            ..
        } = &mut *physical;
        for ((depth, key, _), row) in order.iter().zip(staged.drain(..)) {
            let bucket = selected
                .get_mut(&(*key, row.hash))
                .ok_or(ArgumentResourceV1::Accounting)?;
            self.original_promote_payload(row.backing, budget)?;
            rows.push(row.row);
            selected_keys.push(*key);
            depths.push(*depth);
            bucket.push(row.id);
            original_extensions.insert(*key, row.id);
        }
        physical.edges = edges;
        Ok(())
    }

    fn original_stage_group(
        &self,
        graph: &mut SourceStorageOriginalGraphV29,
        order: &[(usize, RowKey, usize)],
        staged: &mut Vec<SourceStorageOriginalStagedV29>,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Error> {
        if !staged.is_empty() || staged.capacity() < order.len() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let base = self
            .physical
            .try_borrow()
            .map_err(|_| error("original closure has an active builder"))?
            .rows
            .len();
        for (offset, &(depth, key, node)) in order.iter().enumerate() {
            self.lease.work(1, budget)?;
            let row = graph
                .nodes
                .get_mut(node)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if row.key != key || row.depth != depth || row.id.is_some() {
                return Err(error("original group reservation changed its source node"));
            }
            row.id = Some(StorageLayoutIdV1(
                u32::try_from(argument_sum_v1(&[base, offset])?)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
            ));
        }
        for &(_, key, node) in order {
            self.lease.work(1, budget)?;
            let row = self.lower_row_resolved(
                key,
                &SourceStorageOriginalResolverV29::Constructing(graph),
                budget,
            )?;
            self.original_validate_staged(graph, order, base, node, &row, budget)?;
            let hash = self.schema_hash(&row, budget)?;
            let backing = Self::row_backing(&row)?;
            let id = graph.nodes[node].id.ok_or(ArgumentResourceV1::Accounting)?;
            staged.push(SourceStorageOriginalStagedV29 {
                node,
                id,
                row,
                hash,
                backing,
            });
        }
        Ok(base)
    }

    fn original_materialize(
        &self,
        graph: &mut SourceStorageOriginalGraphV29,
        components: &SourceStorageOriginalComponentsV29,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        let mut order = self.lease.vector(graph.nodes.len(), budget)?;
        let mut staged = self
            .lease
            .vector::<SourceStorageOriginalStagedV29>(graph.nodes.len(), budget)?;
        for range in &components.ranges {
            self.lease.work(1, budget)?;
            order.clear();
            for &node in components
                .members
                .get(range.clone())
                .ok_or(ArgumentResourceV1::Accounting)?
            {
                self.lease.work(1, budget)?;
                let row = graph
                    .nodes
                    .get(node)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if row.id.is_none() {
                    order.push((row.depth, row.key, node));
                }
            }
            if order.is_empty() {
                continue;
            }
            sort_rows(&mut order, &self.lease, budget)?;
            let (_, key, node) = order[0];
            self.lease.work(graph.nodes[node].edges.len(), budget)?;
            let recursive = order.len() > 1
                || graph.edges[graph.nodes[node].edges.clone()]
                    .iter()
                    .any(|edge| edge.target == node);
            if !recursive {
                let row = self.lower_row_resolved(
                    key,
                    &SourceStorageOriginalResolverV29::Constructing(graph),
                    budget,
                )?;
                let id = self.original_publish_single(key, row, budget)?;
                graph.nodes[node].id = Some(id);
                continue;
            }
            let base = self.original_stage_group(graph, &order, &mut staged, budget)?;
            self.original_publish_group(graph, &order, base, &mut staged, budget)?;
        }
        Ok(())
    }

    pub(super) fn select_original_closure(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        ty: SemanticTypeIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutIdV1, Error> {
        self.check_owner(owner, budget)?;
        if self.lease.schema_allocation.get() {
            return Err(error("original closure cannot nest a physical builder"));
        }
        let before_owned = self.lease.owned.get();
        let before_persistent = self.lease.persistent.get();
        let result = (|| {
            let key = RowKey::ty(ty);
            if let Some(id) = self.original_checked_id(key, budget)? {
                return Ok(id);
            }
            self.lease.reserve(
                argument_sum_v1(&[
                    size_of::<SourceStorageOriginalGraphV29>(),
                    size_of::<SourceStorageOriginalComponentsV29>(),
                    size_of::<SourceStorageOriginalResolverV29<'_>>(),
                    size_of::<Result<StorageLayoutIdV1, Error>>(),
                    4 * size_of::<Vec<usize>>(),
                    size_of::<Vec<u8>>(),
                    size_of::<Vec<bool>>(),
                    size_of::<Vec<(usize, usize)>>(),
                    size_of::<Vec<(usize, usize, usize)>>(),
                    size_of::<Vec<(usize, RowKey, usize)>>(),
                    size_of::<Vec<SourceStorageOriginalStagedV29>>(),
                    16 * size_of::<usize>(),
                ])?,
                budget,
            )?;
            let mut graph = SourceStorageOriginalGraphV29::default();
            let root = graph.discover(self, key, budget)?;
            graph.containment(self, budget)?;
            let components = graph.components(self, budget)?;
            self.original_materialize(&mut graph, &components, budget)?;
            let id = graph
                .nodes
                .get(root)
                .and_then(|node| node.id)
                .ok_or(ArgumentResourceV1::Accounting)?;
            self.check_selected_schema(owner, ty, id, budget)?;
            Ok(id)
        })();
        // All graph/staged values are destroyed before refund. Only actual
        // published rows and table/index backing retain the persistent subset.
        if let Err(error) = &result {
            self.lease.record(error);
        }
        let cleanup = (|| {
            self.lease.custody(budget)?;
            let growth = self
                .lease
                .owned
                .get()
                .checked_sub(before_owned)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let persistent = self
                .lease
                .persistent
                .get()
                .checked_sub(before_persistent)
                .ok_or(ArgumentResourceV1::Accounting)?;
            self.lease.refund(
                growth
                    .checked_sub(persistent)
                    .ok_or(ArgumentResourceV1::Accounting)?,
                budget,
            )
        })();
        match result {
            Ok(id) => {
                cleanup?;
                Ok(id)
            }
            Err(first) => {
                let _ = cleanup;
                Err(first)
            }
        }
    }
}
