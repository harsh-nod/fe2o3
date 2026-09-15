//! Reuse only an existing exact raw-path union, never a prefix approximation.
use super::*;

pub(super) fn incoming_is_union(
    previous: Option<&Rc<PartialPaths>>,
    incoming: &Rc<PartialPaths>,
    slots: u64,
    whole: u64,
    budget: &Budget,
) -> Result<bool> {
    // Includes the whole-local filter: no incoming row may survive beneath it.
    if incoming.slots != slots {
        return Ok(false);
    }
    for (slot, old, new) in merged_entries(previous, Some(incoming)) {
        budget.work(1)?;
        if whole & (1u64 << slot) != 0 {
            continue;
        }
        match (old, new) {
            (Some(old), Some(new)) if adds_paths(new, old, budget)? => return Ok(false),
            (Some(_), None) => return Ok(false),
            _ => (),
        }
    }
    Ok(true)
}
