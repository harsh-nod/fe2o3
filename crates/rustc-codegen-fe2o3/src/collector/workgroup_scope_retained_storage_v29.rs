//! Complete owned heap of retained workgroup-scope custody, not its header.
use super::{RetainedWorkgroupScopesV29, ScopeCallableV29, ScopeEventV29};
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};

fn copy_row<T: Copy>() {}

impl RetainedWorkgroupScopesV29 {
    /// The enclosing context header already includes both Vec handles.
    /// Count one scope visit and two actual-capacity allocations. First refusal
    /// stops immediately; discard the enclosing observation on any error.
    pub(crate) fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self { classes, events } = self;
        charge_scope_vectors(classes, events, counter)
    }
}

fn charge_scope_vectors(
    classes: &Vec<ScopeCallableV29>,
    events: &Vec<ScopeEventV29>,
    counter: &mut LogicalStorageCounterV1,
) -> Result<(), LogicalStorageErrorV1> {
    counter.charge(0, 1)?;
    copy_row::<ScopeCallableV29>();
    copy_row::<ScopeEventV29>();
    counter.vector(classes)?;
    counter.vector(events)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::LogicalStorageLimitsV1;
    use std::mem::size_of;

    fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
        LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
            max_bytes: bytes,
            max_items: items,
        })
    }
    #[test]
    fn actual_spare_capacities_count_without_constructing_custody() {
        let mut classes = Vec::with_capacity(17);
        classes.push(ScopeCallableV29::Ordinary);
        let events: Vec<ScopeEventV29> = Vec::with_capacity(11);
        let expected = classes.capacity() * size_of::<ScopeCallableV29>()
            + events.capacity() * size_of::<ScopeEventV29>();
        let mut c = counter(Some(expected), 3);
        charge_scope_vectors(&classes, &events, &mut c).unwrap();
        assert_eq!((c.bytes(), c.items()), (expected, 3));
    }
    #[test]
    fn empty_vectors_still_have_three_bounded_visits() {
        let mut c = counter(Some(0), 3);
        charge_scope_vectors(&vec![], &vec![], &mut c).unwrap();
        assert_eq!((c.bytes(), c.items()), (0, 3));
        let mut short = counter(Some(0), 2);
        assert_eq!(
            charge_scope_vectors(&vec![], &vec![], &mut short),
            Err(LogicalStorageErrorV1::ItemLimit)
        );
        assert_eq!((short.bytes(), short.items()), (0, 2));
    }
    #[test]
    fn zero_items_refuses_before_allocations_and_one_short_bytes_refuses() {
        let classes = Vec::<ScopeCallableV29>::with_capacity(2);
        let events = Vec::<ScopeEventV29>::with_capacity(3);
        let mut zero = counter(None, 0);
        assert_eq!(
            charge_scope_vectors(&classes, &events, &mut zero),
            Err(LogicalStorageErrorV1::ItemLimit)
        );
        assert_eq!((zero.bytes(), zero.items()), (0, 0));
        let expected = classes.capacity() * size_of::<ScopeCallableV29>()
            + events.capacity() * size_of::<ScopeEventV29>();
        let mut short = counter(Some(expected - 1), 3);
        assert_eq!(
            charge_scope_vectors(&classes, &events, &mut short),
            Err(LogicalStorageErrorV1::ByteLimit)
        );
        assert_eq!(
            (short.bytes(), short.items()),
            (classes.capacity() * size_of::<ScopeCallableV29>(), 2)
        );
    }
    #[test]
    fn preexisting_overflow_is_not_reset_or_hidden() {
        let classes = vec![ScopeCallableV29::Ordinary];
        let mut c = counter(None, usize::MAX);
        c.charge(usize::MAX, 4).unwrap();
        assert_eq!(
            charge_scope_vectors(&classes, &vec![], &mut c),
            Err(LogicalStorageErrorV1::Arithmetic)
        );
        assert_eq!((c.bytes(), c.items()), (usize::MAX, 5));
    }
}
