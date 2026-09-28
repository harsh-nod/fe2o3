// Actual layout-relative equations. These rows do not grant source, lifetime,
// initialization, active-variant or complete memory-effect authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceStaticObjectLocationV29 {
    slot: usize,
    offset: u64,
    schema: Option<fe2o3_kernel_ir::StorageLayoutIdV1>,
}

#[derive(Clone, Copy)]
struct SourceStaticObjectLayoutV29 {
    bytes: u64,
    alignment: u32,
    value: SourceStaticObjectValueV29,
}

#[derive(Clone, Copy)]
enum SourceStaticObjectValueV29 {
    Scalar(ScalarType),
    Pointer(fe2o3_kernel_ir::StoragePointerV1),
    Aggregate,
}

fn source_static_object_layouts_v29(
    layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceStaticObjectLayoutV29>, ProductionSemanticKirErrorV1> {
    let mut rows = emission_vec_v1(layouts.len(), budget)?;
    for row in layouts {
        budget.charge_work(3)?;
        rows.push(SourceStaticObjectLayoutV29 {
            bytes: row.size,
            alignment: row.alignment,
            value: match row.kind {
                fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(scalar) => {
                    SourceStaticObjectValueV29::Scalar(scalar)
                }
                fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer) => {
                    SourceStaticObjectValueV29::Pointer(pointer)
                }
                _ => SourceStaticObjectValueV29::Aggregate,
            },
        });
    }
    Ok(rows)
}

impl SourceAddressMemoryV29<'_> {
    fn object_location(
        &self,
        pointer: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceStaticObjectLocationV29, ProductionSemanticKirErrorV1> {
        if self.projections.is_empty() {
            let slot = self
                .exact(pointer, budget)?
                .ok_or_else(source_raw_physical_error_v29)?;
            let schema = self
                .object_schemas
                .get(slot)
                .copied()
                .flatten()
                .ok_or_else(source_raw_physical_error_v29)?;
            return Ok(SourceStaticObjectLocationV29 {
                slot,
                offset: 0,
                schema: Some(schema),
            });
        }
        match self.locations.get(self.value(pointer, budget)?) {
            Some(origin_worklist_v1::OriginStateV1::Exact(Some(location)))
                if location.schema.is_some() =>
            {
                Ok(*location)
            }
            _ => Err(source_raw_physical_error_v29()),
        }
    }

    fn object_value_range(
        &self,
        pointer: ValueId,
        slot: &ScopedSourceSlotV29,
        value: &Type,
        access: MemoryAccess,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(u64, u64), ProductionSemanticKirErrorV1> {
        let location = self.object_location(pointer, budget)?;
        let schema = location.schema.ok_or_else(source_raw_physical_error_v29)?;
        let layout = self
            .object_layouts
            .get(schema.0 as usize)
            .ok_or_else(source_raw_physical_error_v29)?;
        let Type::Pointer(address) = self.ty(pointer, budget)? else {
            return Err(source_raw_physical_error_v29());
        };
        let ScopedSlotRepresentationV29::Object {
            bytes, alignment, ..
        } = slot.representation
        else {
            return Err(source_raw_physical_error_v29());
        };
        budget.charge_work(8)?;
        let placed = if location.offset == 0 {
            alignment
        } else {
            alignment.min(1_u32 << location.offset.trailing_zeros().min(31))
        };
        let end = location
            .offset
            .checked_add(layout.bytes)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if address.pointee.as_ref() != &Type::StorageObject(schema)
            || access.alignment > placed.min(layout.alignment)
            || end > bytes
        {
            return Err(source_raw_physical_error_v29());
        }
        let valid = match (layout.value, value) {
            (SourceStaticObjectValueV29::Scalar(expected), Type::Scalar(actual)) => {
                expected == *actual
            }
            (SourceStaticObjectValueV29::Pointer(expected), Type::Pointer(actual)) => {
                let (exact_pointee, exact_cell) = if self.pointer_cells[location.slot].is_some() {
                    (
                        actual.pointee.as_ref() == &Type::StorageObject(expected.pointee),
                        location.offset == 0 && self.object_schemas[location.slot] == Some(schema),
                    )
                } else {
                    budget.charge_work(5)?;
                    let pointee = self
                        .object_layouts
                        .get(expected.pointee.0 as usize)
                        .ok_or_else(source_raw_physical_error_v29)?;
                    let exact_pointee = actual.pointee.as_ref()
                        == &Type::StorageObject(expected.pointee)
                        || matches!((pointee.value, actual.pointee.as_ref()),
                            (SourceStaticObjectValueV29::Scalar(left), Type::Scalar(right)) if left == *right);
                    let cell = self
                        .pointer_cell(location.slot, pointer, budget)?
                        .ok_or_else(source_raw_physical_error_v29)?;
                    let base = self
                        .pointer_cell_count
                        .checked_sub(self.pointer_subcells.len())
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let ordinal = cell
                        .checked_sub(base)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let exact_cell = self.pointer_subcells.get(ordinal)
                        == Some(&SourceStaticPointerCellV29 {
                            slot: location.slot,
                            offset: location.offset,
                            schema,
                        });
                    (exact_pointee, exact_cell)
                };
                // Even an unused pointer load must participate in its own
                // content-origin and activation-currentness equations.
                actual.address_space == expected.value_space
                    && actual.access == expected.access
                    && exact_pointee
                    && exact_cell
            }
            _ => false,
        };
        if !valid {
            return Err(source_raw_physical_error_v29());
        }
        Ok((location.offset, end))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceStaticObjectTransferV29 {
    Identity,
    Project {
        parent: fe2o3_kernel_ir::StorageLayoutIdV1,
        child: fe2o3_kernel_ir::StorageLayoutIdV1,
        offset: u64,
        parent_bytes: u64,
        child_bytes: u64,
    },
}

fn source_static_object_transfer_v29(
    layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
    parent: fe2o3_kernel_ir::StorageLayoutIdV1,
    step: ScopedObjectProjectionV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceStaticObjectTransferV29, ProductionSemanticKirErrorV1> {
    source_static_object_transfer_with_index_v29(layouts, parent, step, None, budget)
}

fn source_static_object_transfer_with_index_v29(
    layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
    parent: fe2o3_kernel_ir::StorageLayoutIdV1,
    step: ScopedObjectProjectionV29,
    literal_index: Option<u64>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceStaticObjectTransferV29, ProductionSemanticKirErrorV1> {
    use fe2o3_kernel_ir::StorageLayoutKindV1 as Kind;
    budget.charge_work(7)?;
    let row = layouts
        .get(parent.0 as usize)
        .ok_or_else(source_raw_physical_error_v29)?;
    let (child, offset) = match (step, &row.kind) {
        (ScopedObjectProjectionV29::Field(index), Kind::Record(fields)) => {
            let field = fields
                .get(index as usize)
                .ok_or_else(source_raw_physical_error_v29)?;
            (field.layout, field.offset)
        }
        (
            ScopedObjectProjectionV29::ArrayIndex(_),
            Kind::Array {
                element,
                length,
                stride,
            },
        ) => {
            budget.charge_work(4)?;
            let index = literal_index.ok_or_else(scoped_object_pending_v29)?;
            if index >= *length {
                return Err(source_raw_physical_error_v29());
            }
            (
                *element,
                index
                    .checked_mul(*stride)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
            )
        }
        // Variant views also observe or constrain the current tag. A geometric
        // offset is insufficient until that independent history is connected.
        (
            ScopedObjectProjectionV29::Variant { .. }
            | ScopedObjectProjectionV29::VariantForWrite { .. }
            | ScopedObjectProjectionV29::ArrayIndex(_),
            _,
        ) => {
            return Err(scoped_object_pending_v29());
        }
        _ => return Err(source_raw_physical_error_v29()),
    };
    let child_row = layouts
        .get(child.0 as usize)
        .ok_or_else(source_raw_physical_error_v29)?;
    if !matches!(child_row.kind, Kind::Scalar(_))
        && !(matches!(child_row.kind, Kind::Pointer(_))
            && matches!(step, ScopedObjectProjectionV29::ArrayIndex(_)))
    {
        return Err(scoped_object_pending_v29());
    }
    if offset
        .checked_add(child_row.size)
        .ok_or(ArgumentResourceV1::Arithmetic)?
        > row.size
    {
        return Err(source_raw_physical_error_v29());
    }
    Ok(SourceStaticObjectTransferV29::Project {
        parent,
        child,
        offset,
        parent_bytes: row.size,
        child_bytes: child_row.size,
    })
}

fn source_static_object_apply_v29(
    transfer: SourceStaticObjectTransferV29,
    value: Option<SourceStaticObjectLocationV29>,
    slots: &[ScopedSourceSlotV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceStaticObjectLocationV29>, origin_worklist_v1::OriginWorkErrorV1> {
    use origin_worklist_v1::OriginWorkErrorV1 as Error;
    budget.charge_work(1)?;
    let SourceStaticObjectTransferV29::Project {
        parent,
        child,
        offset,
        parent_bytes,
        child_bytes,
    } = transfer
    else {
        return Ok(value);
    };
    budget.charge_work(5)?;
    let value = value.ok_or(Error::Shape)?;
    let slot = slots.get(value.slot).ok_or(Error::Shape)?;
    let ScopedSlotRepresentationV29::Object { bytes, .. } = slot.representation else {
        return Err(Error::Shape);
    };
    if value.schema != Some(parent)
        || value
            .offset
            .checked_add(parent_bytes)
            .ok_or(ArgumentResourceV1::Arithmetic)?
            > bytes
    {
        return Err(Error::Shape);
    }
    let offset = value
        .offset
        .checked_add(offset)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    if offset
        .checked_add(child_bytes)
        .ok_or(ArgumentResourceV1::Arithmetic)?
        > bytes
    {
        return Err(Error::Shape);
    }
    Ok(Some(SourceStaticObjectLocationV29 {
        slot: value.slot,
        offset,
        schema: Some(child),
    }))
}

fn source_static_object_projections_v29(
    graph: &SourceAddressMemoryV29<'_>,
    layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<(usize, SourceStaticObjectTransferV29)>, ProductionSemanticKirErrorV1> {
    let mut count = 0;
    for (_, block) in &graph.blocks {
        budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
        count = argument_sum_v1(&[
            count,
            block
                .operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Storage(ScopedObjectOperationV29::Project { .. })
                    )
                })
                .count(),
        ])?;
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut transfers = emission_vec_v1(count, budget)?;
    for (_, block) in &graph.blocks {
        budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let OperationKind::Storage(ScopedObjectOperationV29::Project { base, step }) =
                operation.kind
            else {
                continue;
            };
            let [result] = operation.results.as_slice() else {
                return Err(source_raw_physical_error_v29());
            };
            let (Type::Pointer(base), Type::Pointer(output)) =
                (graph.ty(base, budget)?, &result.ty)
            else {
                return Err(source_raw_physical_error_v29());
            };
            let Type::StorageObject(parent) = base.pointee.as_ref() else {
                return Err(source_raw_physical_error_v29());
            };
            budget.charge_work(4)?;
            let rights = base.access == output.access || base.access == AccessMode::ReadWrite;
            if base.address_space != AddressSpace::Private
                || output.address_space != base.address_space
                || !rights
            {
                return Err(source_raw_physical_error_v29());
            }
            let literal = match step {
                ScopedObjectProjectionV29::ArrayIndex(index) => {
                    budget.charge_work(3)?;
                    let previous = ordinal
                        .checked_sub(1)
                        .and_then(|n| block.operations.get(n))
                        .ok_or_else(source_raw_physical_error_v29)?;
                    let OperationKind::Constant(Constant::Index(value)) = previous.kind else {
                        return Err(source_raw_physical_error_v29());
                    };
                    if !matches!(previous.results.as_slice(), [result] if result.id == index && result.ty == Type::INDEX)
                    {
                        return Err(source_raw_physical_error_v29());
                    }
                    Some(value)
                }
                _ => None,
            };
            let transfer = if literal.is_some() {
                source_static_object_transfer_with_index_v29(
                    layouts, *parent, step, literal, budget,
                )?
            } else {
                source_static_object_transfer_v29(layouts, *parent, step, budget)?
            };
            let SourceStaticObjectTransferV29::Project { child, .. } = transfer else {
                unreachable!()
            };
            if output.pointee.as_ref() != &Type::StorageObject(child) {
                return Err(source_raw_physical_error_v29());
            }
            transfers.push((graph.value(result.id, budget)?, transfer));
        }
    }
    call_splice_sort_work_v1(transfers.len(), budget).map_err(source_address_call_error_v29)?;
    transfers.sort_unstable_by_key(|row| row.0);
    budget.charge_work(transfers.len())?;
    if transfers.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err(source_raw_physical_error_v29());
    }
    Ok(transfers)
}
