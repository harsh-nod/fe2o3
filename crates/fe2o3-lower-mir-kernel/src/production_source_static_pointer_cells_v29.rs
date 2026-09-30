// Only observed pointer leaves become memory cells. Address equations exclude
// pointer loads, so stored values cannot manufacture the addresses of these cells.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceStaticPointerCellV29 {
    slot: usize,
    offset: u64,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
}

impl SourceAddressMemoryV29<'_> {
    fn pointer_cell(
        &self,
        slot: usize,
        pointer: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        let whole = self
            .pointer_cells
            .get(slot)
            .ok_or_else(source_raw_physical_error_v29)?;
        if whole.is_some() || self.pointer_subcells.is_empty() {
            return Ok(*whole);
        }
        budget.charge_work(call_splice_search_work_v1(
            self.pointer_subcell_addresses.len(),
        ))?;
        let Ok(row) = self
            .pointer_subcell_addresses
            .binary_search_by_key(&pointer, |row| row.0)
        else {
            return Ok(None);
        };
        let ordinal = self.pointer_subcell_addresses[row].1;
        budget.charge_work(2)?;
        if self
            .pointer_subcells
            .get(ordinal)
            .is_none_or(|cell| cell.slot != slot)
        {
            return Err(source_raw_physical_error_v29());
        }
        Ok(Some(argument_sum_v1(&[
            self.pointer_cell_count
                .checked_sub(self.pointer_subcells.len())
                .ok_or(ArgumentResourceV1::Accounting)?,
            ordinal,
        ])?))
    }

    fn access_pointer_cell(
        &self,
        row: Option<&SourceAddressAccessV29>,
        operation: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        let Some(row) = row else {
            return Ok(None);
        };
        let access = source_address_value_access_v29(operation)?
            .ok_or_else(source_raw_physical_error_v29)?;
        self.pointer_cell(row.slot, access.pointer, budget)
    }

    fn pointer_cell_range(
        &self,
        slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<std::ops::Range<usize>, ProductionSemanticKirErrorV1> {
        if self.pointer_subcells.is_empty() {
            return Ok(0..0);
        }
        budget.charge_work(argument_product_v1(
            2,
            call_splice_search_work_v1(self.pointer_subcells.len()),
        )?)?;
        let first = self
            .pointer_subcells
            .partition_point(|cell| cell.slot < slot);
        let end = self
            .pointer_subcells
            .partition_point(|cell| cell.slot <= slot);
        budget.charge_work(end - first)?;
        let base = self
            .pointer_cell_count
            .checked_sub(self.pointer_subcells.len())
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(argument_sum_v1(&[base, first])?..argument_sum_v1(&[base, end])?)
    }

    fn prepare_pointer_subcells(
        &mut self,
        entry: BlockId,
        slots: &[ScopedSourceSlotV29],
        accesses: &[SourceAddressAccessV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.projections.is_empty() {
            return Ok(());
        }
        budget.charge_work(self.projections.len())?;
        let needed = self.projections.iter().any(|(_, transfer)| {
            matches!(transfer, SourceStaticObjectTransferV29::Project { child, .. }
                if self.object_layouts.get(child.0 as usize).is_some_and(|layout|
                    matches!(layout.value, SourceStaticObjectValueV29::Pointer(_))))
        });
        if !needed {
            return Ok(());
        }
        let floor = budget.storage();
        budget.reserve_storage(std::mem::size_of::<(
            Vec<bool>,
            Vec<(SourceStaticPointerCellV29, ValueId)>,
            Vec<SourceStaticPointerCellV29>,
            Vec<(ValueId, usize)>,
            Vec<origin_worklist_v1::OriginStateV1<Option<SourceStaticObjectLocationV29>>>,
            origin_worklist_v1::OriginWorkV1<
                Option<SourceStaticObjectLocationV29>,
                SourceStaticObjectTransferV29,
            >,
        )>())?;
        let nodes = self.index.values.len();
        let mut eligible = emission_vec_v1(nodes, budget)?;
        budget.charge_work(nodes)?;
        eligible.resize(nodes, false);
        for (_, block) in &self.blocks {
            budget.charge_work(argument_sum_v1(&[
                1,
                block.parameters.len(),
                block.operations.len(),
            ])?)?;
            for parameter in &block.parameters {
                if matches!(parameter.ty, Type::Pointer(_)) {
                    eligible[self.value(parameter.id, budget)?] = true;
                }
            }
            for operation in &block.operations {
                if let [result] = operation.results.as_slice()
                    && matches!(result.ty, Type::Pointer(_))
                    && matches!(
                        operation.kind,
                        OperationKind::Cast {
                            kind: CastKind::RestrictPointerAccess | CastKind::PointerToGeneric,
                            ..
                        } | OperationKind::Select { .. }
                            | OperationKind::GetElementPointer { .. }
                            | OperationKind::Storage(ScopedObjectOperationV29::Project { .. })
                    )
                {
                    if let OperationKind::GetElementPointer { offset, .. } = operation.kind
                        && !self.zero_offsets[self.value(offset, budget)?]
                    {
                        continue;
                    }
                    eligible[self.value(result.id, budget)?] = true;
                }
            }
        }
        let mut edges = 0;
        self.dependencies(accesses, &[], budget, |from, to, _, _| {
            if from < nodes && to < nodes && eligible[to] {
                edges = argument_sum_v1(&[edges, 1])?;
            }
            Ok(())
        })?;
        let mut work = origin_worklist_v1::OriginWorkV1::new(nodes, edges, budget)
            .map_err(source_address_equation_error_v29)?;
        for (ordinal, origin) in self.origins[..nodes].iter().enumerate() {
            budget.charge_work(1)?;
            let seed = match *origin {
                SourceAddressOriginV29::Exact(Some(slot)) => {
                    let schema = match slots
                        .get(slot)
                        .ok_or_else(source_raw_physical_error_v29)?
                        .representation
                    {
                        ScopedSlotRepresentationV29::Object { schema, .. } => Some(schema),
                        ScopedSlotRepresentationV29::ScalarArray(_) => None,
                    };
                    origin_worklist_v1::OriginStateV1::Exact(Some(SourceStaticObjectLocationV29 {
                        slot,
                        offset: 0,
                        schema,
                    }))
                }
                SourceAddressOriginV29::Exact(None) => {
                    origin_worklist_v1::OriginStateV1::Exact(None)
                }
                SourceAddressOriginV29::Pending if eligible[ordinal] => {
                    origin_worklist_v1::OriginStateV1::Pending
                }
                _ => origin_worklist_v1::OriginStateV1::Unknown,
            };
            work.seed_next(seed, budget)
                .map_err(source_address_equation_error_v29)?;
        }
        self.dependencies(accesses, &[], budget, |from, to, transfer, budget| {
            if from < nodes && to < nodes && eligible[to] {
                work.add_transfer(from, to, transfer, budget)
                    .map_err(source_address_equation_error_v29)?;
            }
            Ok(())
        })?;
        let locations = work
            .solve_with(budget, |transfer, value, budget| {
                source_static_object_apply_v29(transfer, value, slots, budget)
            })
            .map_err(source_address_equation_error_v29)?;
        let mut observed = emission_vec_v1(accesses.len(), budget)?;
        for row in accesses {
            budget.charge_work(3)?;
            if self.pointer_cells[row.slot].is_some() {
                continue;
            }
            let operation = self.blocks[self.block(row.block, budget)?]
                .1
                .operations
                .get(row.operation)
                .ok_or_else(source_raw_physical_error_v29)?;
            let access = source_address_value_access_v29(operation)?
                .ok_or_else(source_raw_physical_error_v29)?;
            if !access.object || !matches!(self.ty(access.value, budget)?, Type::Pointer(_)) {
                continue;
            }
            let origin_worklist_v1::OriginStateV1::Exact(Some(location)) =
                locations[self.value(access.pointer, budget)?]
            else {
                return Err(scoped_object_pending_v29());
            };
            let schema = location.schema.ok_or_else(source_raw_physical_error_v29)?;
            if location.slot != row.slot
                || !matches!(
                    self.object_layouts
                        .get(schema.0 as usize)
                        .map(|row| row.value),
                    Some(SourceStaticObjectValueV29::Pointer(_))
                )
            {
                return Err(source_raw_physical_error_v29());
            }
            observed.push((
                SourceStaticPointerCellV29 {
                    slot: row.slot,
                    offset: location.offset,
                    schema,
                },
                access.pointer,
            ));
        }
        call_splice_sort_work_v1(observed.len(), budget).map_err(source_address_call_error_v29)?;
        observed.sort_unstable();
        let mut cells = emission_vec_v1(observed.len(), budget)?;
        let mut addresses = emission_vec_v1(observed.len(), budget)?;
        for (key, pointer) in &observed {
            budget.charge_work(5)?;
            if cells.last() != Some(key) {
                if let Some(previous) = cells.last() {
                    let previous: &SourceStaticPointerCellV29 = previous;
                    let end = previous
                        .offset
                        .checked_add(self.object_layouts[previous.schema.0 as usize].bytes)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    if previous.slot == key.slot && end > key.offset {
                        return Err(source_raw_physical_error_v29());
                    }
                }
                cells.push(*key);
            }
            addresses.push((*pointer, cells.len() - 1));
        }
        call_splice_sort_work_v1(addresses.len(), budget).map_err(source_address_call_error_v29)?;
        addresses.sort_unstable();
        budget.charge_work(addresses.len())?;
        if addresses
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1)
        {
            return Err(source_raw_physical_error_v29());
        }
        addresses.dedup();
        let retained = argument_sum_v1(&[
            argument_product_v1(
                cells.capacity(),
                std::mem::size_of::<SourceStaticPointerCellV29>(),
            )?,
            argument_product_v1(
                addresses.capacity(),
                std::mem::size_of::<(ValueId, usize)>(),
            )?,
        ])?;
        drop((eligible, locations, observed));
        let scratch = budget
            .storage()
            .checked_sub(argument_sum_v1(&[floor, retained])?)
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.release_storage(scratch)?;
        self.pointer_subcells = cells;
        self.pointer_subcell_addresses = addresses;
        if self.pointer_subcells.is_empty() {
            return Ok(());
        }
        self.pointer_cell_count =
            argument_sum_v1(&[self.pointer_cell_count, self.pointer_subcells.len()])?;
        let count = argument_sum_v1(&[
            nodes,
            argument_product_v1(
                argument_product_v1(self.blocks.len(), 2)?,
                self.pointer_cell_count,
            )?,
            1,
        ])?;
        let mut origins = emission_vec_v1(count, budget)?;
        budget.charge_work(count)?;
        origins.extend_from_slice(&self.origins[..nodes]);
        origins.resize(count, SourceAddressOriginV29::Pending);
        origins[count - 1] = SourceAddressOriginV29::Unknown;
        // Use the actual function entry, not sorted block zero.
        let entry = self.block(entry, budget)?;
        budget.charge_work(self.pointer_cell_count)?;
        for cell in 0..self.pointer_cell_count {
            let node = argument_sum_v1(&[
                nodes,
                argument_product_v1(argument_product_v1(entry, 2)?, self.pointer_cell_count)?,
                cell,
            ])?;
            origins[node] = SourceAddressOriginV29::Unknown;
        }
        let old = std::mem::replace(&mut self.origins, origins);
        let bytes = argument_product_v1(
            old.capacity(),
            std::mem::size_of::<SourceAddressOriginV29>(),
        )?;
        drop(old);
        budget.release_storage(bytes)?;
        Ok(())
    }
}
