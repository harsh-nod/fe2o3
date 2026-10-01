//! Actual report-vector capacity, independent of stored execution reservations.
use super::{PlironOptimizationPassReportV1, PlironOptimizationReportV1};
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};

fn fixed<T: Copy>(_: &T) {}

fn charge_passes(
    passes: &Vec<PlironOptimizationPassReportV1>,
    counter: &mut Counter,
) -> Result<(), Error> {
    fn fixed_rows<T: Copy>(_: &Vec<T>) {}
    fixed_rows(passes);
    counter.vector(passes)
}

impl PlironOptimizationReportV1 {
    /// Charges only this report's actual pass-vector capacity, atomically.
    /// Its inline header and root visit are excluded; one collection visit is
    /// included. All pass fields and the replay identity are inline Copy data.
    /// Does not execute passes, inspect graph arenas, or alter stored budgets.
    pub fn charge_retained_heap_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self {
            initial_graph_work,
            final_graph_work,
            invalidated_handle_count,
            work_units,
            passes,
            final_graph_identity,
        } = self;
        fixed(initial_graph_work);
        fixed(final_graph_work);
        fixed(invalidated_handle_count);
        fixed(work_units);
        fixed(final_graph_identity);
        charge_passes(passes, counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::LogicalStorageLimitsV1 as Limits;
    use std::mem::size_of;

    #[test]
    fn empty_and_spare_pass_capacity_are_one_heap_only_visit() {
        for capacity in [0, 13] {
            let rows = Vec::with_capacity(capacity);
            let bytes = rows.capacity() * size_of::<PlironOptimizationPassReportV1>();
            let mut counter = Counter::new(Limits {
                max_bytes: Some(bytes),
                max_items: 1,
            });
            charge_passes(&rows, &mut counter).unwrap();
            assert_eq!((counter.bytes(), counter.items()), (bytes, 1));
            assert!(rows.is_empty());
        }
    }

    #[test]
    fn pass_refusals_are_exact_and_atomic() {
        let rows = Vec::with_capacity(3);
        let bytes = rows.capacity() * size_of::<PlironOptimizationPassReportV1>();
        for (limits, expected) in [
            (
                Limits {
                    max_bytes: Some(bytes - 1),
                    max_items: 1,
                },
                Error::ByteLimit,
            ),
            (
                Limits {
                    max_bytes: Some(bytes),
                    max_items: 0,
                },
                Error::ItemLimit,
            ),
        ] {
            let mut counter = Counter::new(limits);
            assert_eq!(charge_passes(&rows, &mut counter), Err(expected));
            assert_eq!((counter.bytes(), counter.items()), (0, 0));
        }
        let mut counter = Counter::new(Limits {
            max_bytes: None,
            max_items: usize::MAX,
        });
        counter.charge(usize::MAX, 0).unwrap();
        assert_eq!(charge_passes(&rows, &mut counter), Err(Error::Arithmetic));
        assert_eq!((counter.bytes(), counter.items()), (usize::MAX, 0));
    }
}
