// Tag bytes join the same sparse initialization partition as ordinary payload
// writes. No-op niche updates retain a census row but add no history event.
type SourceAddressHistoryFootprintV43 = (usize, Option<(u64, u64)>, bool);
type SourceAddressHistoryQueryFrameV43<'a> = (
    SourceAddressHistoryFootprintV43,
    Option<SourceAddressTagAccessV43>,
    Option<SourceAddressValueAccessV29>,
    &'a Operation,
    &'a ScopedSourceSlotV29,
    SourceStaticObjectLocationV29,
    (u64, u64),
);

fn source_address_history_footprint_v43(
    graph: &SourceAddressMemoryV29<'_>,
    row: &SourceAddressAccessV29,
    slots: &[ScopedSourceSlotV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<SourceAddressHistoryFootprintV43> {
    let operation = graph.blocks[graph.block(row.block, budget)?]
        .1
        .operations
        .get(row.operation)
        .ok_or_else(scoped_slot_error_v29)?;
    let slot = slots.get(row.slot).ok_or_else(scoped_slot_error_v29)?;
    if let Some(tag) = source_address_tag_access_v43(operation)? {
        if !matches!(
            slot.representation,
            ScopedSlotRepresentationV29::Object { .. }
        ) || graph.object_location(tag.pointer, budget)?.slot != row.slot
        {
            return Err(scoped_slot_error_v29());
        }
        let range = graph.object_tag_range_v43(tag, slot, budget)?;
        if range.is_some_and(|(start, end)| start >= end) {
            return Err(invalid("static tag access has an empty range"));
        }
        return Ok((row.slot, range, tag.written_variant.is_some()));
    }
    let access = source_address_value_access_v29(operation)?.ok_or_else(scoped_slot_error_v29)?;
    let range = match slot.representation {
        ScopedSlotRepresentationV29::Object { .. } => {
            if graph.object_location(access.pointer, budget)?.slot != row.slot {
                return Err(scoped_slot_error_v29());
            }
            graph.object_value_range(
                access.pointer,
                slot,
                graph.ty(access.value, budget)?,
                access.access,
                budget,
            )?
        }
        ScopedSlotRepresentationV29::ScalarArray(scalar)
            if scalar.length == 1 && scalar.bytes == scalar.element.size =>
        {
            (0, scalar.bytes)
        }
        _ => return Err(invalid("static object history needs exact access ranges")),
    };
    if range.0 >= range.1 {
        return Err(invalid("static value access has an empty range"));
    }
    Ok((row.slot, Some(range), access.writing))
}

#[cfg(test)]
mod tag_history_frame_tests_v43 {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn original_tag_history_query_frame_has_an_independent_field_envelope() {
        type Fields<'a> = (
            (usize, Option<(u64, u64)>, bool),
            Option<SourceAddressTagAccessV43>,
            Option<SourceAddressValueAccessV29>,
            &'a Operation,
            &'a ScopedSourceSlotV29,
            SourceStaticObjectLocationV29,
            (u64, u64),
        );
        assert_eq!(
            size_of::<SourceAddressHistoryQueryFrameV43<'_>>(),
            size_of::<Fields<'_>>()
        );
    }
}
