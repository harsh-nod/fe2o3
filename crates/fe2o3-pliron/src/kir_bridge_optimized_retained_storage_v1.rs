//! Actual optimized-bridge correspondence ownership, without graph custody.
use super::{KirBridgeCorrespondenceV1, KirBridgeOptimizedReceiptV1};
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};

fn fixed<T: Copy>(_: &T) {}
fn fixed_rows<T: Copy>(_: &Vec<T>) {}

impl KirBridgeOptimizedReceiptV1 {
    /// Charges the actual correspondence-vector capacity, atomically.
    /// Excludes the receipt's header and root visit, includes one collection
    /// visit. Endpoint digests and every correspondence field are inline Copy.
    /// This does not account for a live graph or authenticate bridge semantics.
    pub fn charge_retained_heap_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self {
            input,
            output,
            correspondence,
        } = self;
        fixed(input);
        fixed(output);
        let rows: &Vec<KirBridgeCorrespondenceV1> = correspondence;
        fixed_rows(rows);
        counter.vector(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::super::KirBridgeDigestV1;
    use super::*;
    use fe2o3_kernel_ir::LogicalStorageLimitsV1 as Limits;
    use std::mem::size_of;

    // Inert capacity-only fixture; genuine constructors are covered by the
    // Policy6 composition controls, not inferred from these zero digests.
    fn fixture(capacity: usize) -> KirBridgeOptimizedReceiptV1 {
        KirBridgeOptimizedReceiptV1 {
            input: KirBridgeDigestV1 {
                digest: [0; 32],
                canonical_bytes: 0,
            },
            output: KirBridgeDigestV1 {
                digest: [0; 32],
                canonical_bytes: 0,
            },
            correspondence: Vec::with_capacity(capacity),
        }
    }

    #[test]
    fn empty_and_spare_bridge_capacity_excludes_header() {
        for capacity in [0, 17] {
            let owner = fixture(capacity);
            let bytes = owner.correspondence.capacity() * size_of::<KirBridgeCorrespondenceV1>();
            let mut counter = Counter::new(Limits {
                max_bytes: Some(bytes),
                max_items: 1,
            });
            owner.charge_retained_heap_storage_v1(&mut counter).unwrap();
            assert_eq!((counter.bytes(), counter.items()), (bytes, 1));
            assert!(owner.correspondence.is_empty());
        }
    }

    #[test]
    fn bridge_refusals_are_exact_and_atomic() {
        let owner = fixture(3);
        let bytes = owner.correspondence.capacity() * size_of::<KirBridgeCorrespondenceV1>();
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
            assert_eq!(
                owner.charge_retained_heap_storage_v1(&mut counter),
                Err(expected)
            );
            assert_eq!((counter.bytes(), counter.items()), (0, 0));
        }
        let mut counter = Counter::new(Limits {
            max_bytes: None,
            max_items: usize::MAX,
        });
        counter.charge(usize::MAX, 0).unwrap();
        assert_eq!(
            owner.charge_retained_heap_storage_v1(&mut counter),
            Err(Error::Arithmetic)
        );
        assert_eq!((counter.bytes(), counter.items()), (usize::MAX, 0));
    }
}
