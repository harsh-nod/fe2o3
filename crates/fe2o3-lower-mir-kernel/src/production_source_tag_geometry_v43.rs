// Physical tag range and no-op classification. These facts do not establish
// the original tag contract or pointer currentness; both are separate joins.
#[derive(Clone, Copy, Debug)]
struct SourceAddressTagAccessV43 {
    pointer: ValueId,
    access: MemoryAccess,
    written_variant: Option<u32>,
}

fn source_tag_geometry_headers_v43() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<SourceAddressTagAccessV43>(),
        std::mem::size_of::<Option<SourceAddressTagAccessV43>>(),
        std::mem::size_of::<Result<Option<SourceAddressTagAccessV43>, ProductionSemanticKirErrorV1>>(
        ),
        argument_product_v1(3, std::mem::size_of::<Option<(u64, u64)>>())?,
        argument_product_v1(
            3,
            std::mem::size_of::<Result<Option<(u64, u64)>, ProductionSemanticKirErrorV1>>(),
        )?,
        std::mem::size_of::<SourceStaticPointerCellV29>(),
        argument_product_v1(12, std::mem::size_of::<usize>())?,
        argument_product_v1(8, std::mem::size_of::<&()>())?,
    ])
}

fn source_address_tag_access_v43(
    operation: &Operation,
) -> Result<Option<SourceAddressTagAccessV43>, ProductionSemanticKirErrorV1> {
    let (pointer, access, written_variant) = match operation.kind {
        OperationKind::Storage(ScopedObjectOperationV29::ReadDiscriminant { address, access }) => {
            if !matches!(operation.results.as_slice(), [result] if result.ty == Type::Scalar(ScalarType::U128))
            {
                return Err(source_raw_physical_error_v29());
            }
            (address, access, None)
        }
        OperationKind::Storage(ScopedObjectOperationV29::SetDiscriminant {
            address,
            variant,
            access,
        }) => {
            if !operation.results.is_empty() {
                return Err(source_raw_physical_error_v29());
            }
            (address, access, Some(variant))
        }
        _ => return Ok(None),
    };
    Ok(Some(SourceAddressTagAccessV43 {
        pointer,
        access,
        written_variant,
    }))
}

impl SourceAddressMemoryV29<'_> {
    fn object_tag_root_range_v43(
        &self,
        tag: SourceAddressTagAccessV43,
        slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<(u64, u64)>, ProductionSemanticKirErrorV1> {
        use fe2o3_kernel_ir::StorageVariantEncodingV1 as Encoding;
        budget.charge_work(10)?;
        let Type::Pointer(pointer) = self.ty(tag.pointer, budget)? else {
            return Err(source_raw_physical_error_v29());
        };
        let Type::StorageObject(schema) = pointer.pointee.as_ref() else {
            return Err(source_raw_physical_error_v29());
        };
        if self.object_schemas.get(slot).copied().flatten() != Some(*schema) {
            return Err(scoped_object_pending_v29());
        }
        let layout = self
            .object_layouts
            .get(schema.0 as usize)
            .ok_or_else(source_raw_physical_error_v29)?;
        let SourceStaticObjectValueV29::Variants { encoding, count } = layout.value else {
            return Err(source_raw_physical_error_v29());
        };
        if let Some(variant) = tag.written_variant {
            if variant as usize >= count {
                return Err(source_raw_physical_error_v29());
            }
            if matches!(encoding, Encoding::Niche { untagged_variant, .. } if untagged_variant == variant)
            {
                return Ok(None);
            }
            if matches!(encoding, Encoding::Niche { first_niche_variant, last_niche_variant, .. }
                if variant < first_niche_variant || last_niche_variant < variant)
            {
                return Err(source_raw_physical_error_v29());
            }
        }
        let field = encoding.tag();
        let tag_layout = self
            .object_layouts
            .get(field.layout.0 as usize)
            .ok_or_else(source_raw_physical_error_v29)?;
        if !(matches!(tag_layout.value, SourceStaticObjectValueV29::Scalar(scalar)
                if scalar.is_integer() || scalar == ScalarType::Bool)
            || matches!(tag_layout.value, SourceStaticObjectValueV29::Pointer(_)))
            || tag_layout.bytes == 0
        {
            return Err(source_raw_physical_error_v29());
        }
        let end = field
            .offset
            .checked_add(tag_layout.bytes)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if end > layout.bytes {
            return Err(source_raw_physical_error_v29());
        }
        Ok(Some((field.offset, end)))
    }

    fn visit_tag_overwritten_pointer_cells_v43(
        &self,
        tag: SourceAddressTagAccessV43,
        slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl FnMut(
            usize,
            &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if tag.written_variant.is_none() {
            return Ok(());
        }
        let Some((start, end)) = self.object_tag_root_range_v43(tag, slot, budget)? else {
            return Ok(());
        };
        if self
            .pointer_cells
            .get(slot)
            .is_none_or(|cell| cell.is_some())
        {
            return Err(source_raw_physical_error_v29());
        }
        // Summary construction checks pointer widths <=16. A cell beginning
        // farther than 15 bytes before this write cannot overlap it.
        budget.charge_work(argument_product_v1(
            2,
            call_splice_search_work_v1(self.pointer_subcells.len()),
        )?)?;
        let earliest = start.saturating_sub(15);
        let first = self
            .pointer_subcells
            .partition_point(|cell| (cell.slot, cell.offset) < (slot, earliest));
        let limit = self
            .pointer_subcells
            .partition_point(|cell| (cell.slot, cell.offset) < (slot, end));
        let base = self
            .pointer_cell_count
            .checked_sub(self.pointer_subcells.len())
            .ok_or(ArgumentResourceV1::Accounting)?;
        for ordinal in first..limit {
            budget.charge_work(6)?;
            let cell = self.pointer_subcells[ordinal];
            if cell.slot != slot {
                return Err(source_raw_physical_error_v29());
            }
            let layout = self
                .object_layouts
                .get(cell.schema.0 as usize)
                .ok_or_else(source_raw_physical_error_v29)?;
            if !matches!(layout.value, SourceStaticObjectValueV29::Pointer(_))
                || layout.bytes == 0
                || layout.bytes > 16
            {
                return Err(source_raw_physical_error_v29());
            }
            let cell_end = cell
                .offset
                .checked_add(layout.bytes)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if start < cell_end {
                visit(argument_sum_v1(&[base, ordinal])?, budget)?;
            }
        }
        Ok(())
    }

    fn object_tag_range_v43(
        &self,
        tag: SourceAddressTagAccessV43,
        slot: &ScopedSourceSlotV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<(u64, u64)>, ProductionSemanticKirErrorV1> {
        use fe2o3_kernel_ir::StorageVariantEncodingV1 as Encoding;
        budget.charge_work(12)?;
        let Type::Pointer(pointer) = self.ty(tag.pointer, budget)? else {
            return Err(source_raw_physical_error_v29());
        };
        let Type::StorageObject(schema) = pointer.pointee.as_ref() else {
            return Err(source_raw_physical_error_v29());
        };
        let layout = self
            .object_layouts
            .get(schema.0 as usize)
            .ok_or_else(source_raw_physical_error_v29)?;
        let SourceStaticObjectValueV29::Variants { encoding, count } = layout.value else {
            return Err(source_raw_physical_error_v29());
        };
        if let Some(variant) = tag.written_variant {
            if variant as usize >= count {
                return Err(source_raw_physical_error_v29());
            }
            if matches!(encoding, Encoding::Niche { untagged_variant, .. } if untagged_variant == variant)
            {
                return Ok(None);
            }
            if matches!(encoding, Encoding::Niche { first_niche_variant, last_niche_variant, .. }
                if variant < first_niche_variant || last_niche_variant < variant)
            {
                return Err(source_raw_physical_error_v29());
            }
        }
        let location = self.object_location(tag.pointer, budget)?;
        if location.schema != Some(*schema) {
            return Err(source_raw_physical_error_v29());
        }
        let ScopedSlotRepresentationV29::Object {
            bytes, alignment, ..
        } = slot.representation
        else {
            return Err(source_raw_physical_error_v29());
        };
        let field = encoding.tag();
        let tag_layout = self
            .object_layouts
            .get(field.layout.0 as usize)
            .ok_or_else(source_raw_physical_error_v29)?;
        if !(matches!(tag_layout.value, SourceStaticObjectValueV29::Scalar(scalar)
                if scalar.is_integer() || scalar == ScalarType::Bool)
            || matches!(tag_layout.value, SourceStaticObjectValueV29::Pointer(_)))
            || tag_layout.bytes == 0
            || pointer.address_space != AddressSpace::Private
            || tag.access.address_space != pointer.address_space
            || !tag.access.alignment.is_power_of_two()
            || tag.access.volatile
            || tag.written_variant.is_some() && pointer.access != AccessMode::ReadWrite
            || tag.written_variant.is_none() && pointer.access == AccessMode::WriteOnly
        {
            return Err(source_raw_physical_error_v29());
        }
        let local_end = field
            .offset
            .checked_add(tag_layout.bytes)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let start = location
            .offset
            .checked_add(field.offset)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let end = start
            .checked_add(tag_layout.bytes)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let placed = if start == 0 {
            alignment
        } else {
            alignment.min(1_u32 << start.trailing_zeros().min(31))
        };
        if local_end > layout.bytes || end > bytes || tag.access.alignment > placed {
            return Err(source_raw_physical_error_v29());
        }
        Ok(Some((start, end)))
    }
}
