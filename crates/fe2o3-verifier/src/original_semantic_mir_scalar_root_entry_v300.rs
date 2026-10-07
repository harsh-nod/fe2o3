//! Conservative native entry classification using the actual entry plan.
use super::*;

fn headers() -> usize {
    size_of::<Vec<(usize, u32)>>()
        + 2 * size_of::<Result<Option<Vec<(usize, u32)>>>>()
        + size_of::<std::slice::Iter<'_, Slot>>()
        + size_of::<std::slice::Iter<'_, Option<EntryArgument>>>()
        + size_of::<Argument>()
        + size_of::<EntryArgument>()
        + 6 * size_of::<usize>()
        + 4 * size_of::<&()>()
}

impl SourceFrameEnter<'_, '_, '_> {
    pub(in super::super) fn scalar_root_arguments_v300(
        &self,
        root: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Vec<(usize, u32)>>> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(4)?;
        if self.root != root || self.instance != 0 || self.owners.as_slice() != [self.owner] {
            return Err(mismatch());
        }
        for slot in &self.allocations {
            out.budget.charge_work(1)?;
            if slot.implicit {
                return Ok(None);
            }
        }
        // Classify before allocating: no partial native ABI roster is published.
        for argument in &self.arguments {
            out.budget.charge_work(4)?;
            let Some(EntryArgument::Whole(index)) = argument else {
                return Ok(None);
            };
            let argument = self.fields.get(*index).ok_or_else(mismatch)?;
            if !matches!(argument.class, Class::Scalar(0 | 32))
                || argument.descriptor.is_some()
                || argument.object
            {
                return Ok(None);
            }
        }
        let mut rows = vector(self.arguments.len(), out)?;
        for argument in &self.arguments {
            out.budget.charge_work(3)?;
            let Some(EntryArgument::Whole(index)) = argument else {
                return Err(mismatch());
            };
            let argument = self.fields.get(*index).ok_or_else(mismatch)?;
            let Class::Scalar(bits) = argument.class else {
                return Err(mismatch());
            };
            rows.push((argument.local, bits));
        }
        Ok(Some(rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_root_entry_headers_cover_actual_entry_scan_and_returned_roster() {
        let rows =
            size_of::<Vec<(usize, u32)>>() + 2 * size_of::<Result<Option<Vec<(usize, u32)>>>>();
        let scans = size_of::<std::slice::Iter<'_, Slot>>()
            + size_of::<std::slice::Iter<'_, Option<EntryArgument>>>();
        let selected = size_of::<Argument>() + size_of::<EntryArgument>();
        assert_eq!(
            headers(),
            rows + scans + selected + 6 * size_of::<usize>() + 4 * size_of::<&()>()
        );
    }
}
