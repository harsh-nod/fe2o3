//! Bounded heap composition for the actual private typed-argument Vec.
use super::TypedArgumentListV1;
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};

impl<T> TypedArgumentListV1<T> {
    /// Charges only actual Vec backing and composes each original live element.
    ///
    /// The enclosing owner pays the list header. Collection and per-element
    /// visits precede iteration/callback work, including empty and ZST owners.
    /// The element callback must cover its nested heap with this same counter;
    /// slot headers are already charged. First refusal stops without rollback:
    /// discard the enclosing observation. No constructor/admission cap changes.
    pub(crate) fn charge_retained_heap_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
        mut element: impl FnMut(&T, &mut LogicalStorageCounterV1) -> Result<(), LogicalStorageErrorV1>,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self { arguments } = self;
        counter.vector(arguments)?;
        for value in arguments {
            counter.charge(0, 1)?;
            element(value, counter)?;
        }
        Ok(())
    }
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
    fn original_empty_constructor_keeps_reserved_backing() {
        let values: Vec<u64> = Vec::with_capacity(9);
        let expected = values.capacity() * size_of::<u64>();
        let owner = TypedArgumentListV1::new(values).unwrap();
        let mut c = counter(Some(expected), 1);
        owner
            .charge_retained_heap_v1(&mut c, |_, _| panic!("empty"))
            .unwrap();
        assert_eq!((c.bytes(), c.items()), (expected, 1));
        let mut short = counter(Some(expected - 1), 1);
        assert_eq!(
            owner.charge_retained_heap_v1(&mut short, |_, _| panic!("unpaid")),
            Err(LogicalStorageErrorV1::ByteLimit)
        );
        assert_eq!((short.bytes(), short.items()), (0, 0));
    }

    #[test]
    fn original_elements_and_same_counter_are_composed_once() {
        let mut values = Vec::with_capacity(5);
        let mut text = String::with_capacity(17);
        text.push_str("abc");
        values.push(text);
        let expected = values.capacity() * size_of::<String>() + values[0].capacity();
        let owner = TypedArgumentListV1::new(values).unwrap();
        let mut c = counter(Some(expected), 3);
        let address = &mut c as *mut LogicalStorageCounterV1;
        let mut calls = 0;
        owner
            .charge_retained_heap_v1(&mut c, |value, same| {
                assert!(std::ptr::eq(value, &owner.as_slice()[0]));
                assert_eq!(same as *mut LogicalStorageCounterV1, address);
                calls += 1;
                same.string(value)
            })
            .unwrap();
        assert_eq!((c.bytes(), c.items(), calls), (expected, 3, 1));
    }

    #[test]
    fn collection_and_row_refusal_precede_callbacks_including_zst() {
        let owner = TypedArgumentListV1::new(vec![(); 3]).unwrap();
        let mut c = counter(Some(0), 0);
        assert_eq!(
            owner.charge_retained_heap_v1(&mut c, |_, _| panic!("unpaid collection")),
            Err(LogicalStorageErrorV1::ItemLimit)
        );
        let mut c = counter(Some(0), 2);
        let mut calls = 0;
        assert_eq!(
            owner.charge_retained_heap_v1(&mut c, |_, _| {
                calls += 1;
                Ok(())
            }),
            Err(LogicalStorageErrorV1::ItemLimit)
        );
        assert_eq!((c.bytes(), c.items(), calls), (0, 2, 1));
    }

    #[test]
    fn element_error_and_existing_ledger_overflow_stop_immediately() {
        let owner = TypedArgumentListV1::new(vec![1_u8, 2]).unwrap();
        let mut c = counter(None, 8);
        let mut calls = 0;
        assert_eq!(
            owner.charge_retained_heap_v1(&mut c, |_, _| {
                calls += 1;
                Err(LogicalStorageErrorV1::UnsupportedV11Owner)
            }),
            Err(LogicalStorageErrorV1::UnsupportedV11Owner)
        );
        assert_eq!(calls, 1);
        let mut c = counter(None, usize::MAX);
        c.charge(usize::MAX, 0).unwrap();
        assert_eq!(
            owner.charge_retained_heap_v1(&mut c, |_, _| panic!("overflow")),
            Err(LogicalStorageErrorV1::Arithmetic)
        );
        assert_eq!((c.bytes(), c.items()), (usize::MAX, 0));
    }
}
