//! Public consumer of the original non-authoritative cleanup constructor.
use fe2o3_artifact_transaction::ProducerIdentity;
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1, LogicalStorageLimitsV1};
use std::{mem::size_of, path::Path};

fn charge(
    producer: &ProducerIdentity,
    counter: &mut LogicalStorageCounterV1,
) -> Result<(), LogicalStorageErrorV1> {
    producer.visit_retained_heap_storage_v1(|n, w| {
        counter.charge(
            n.checked_mul(w).ok_or(LogicalStorageErrorV1::Arithmetic)?,
            1,
        )
    })
}

#[test]
fn external_api_composes_original_owner_with_checked_prefix_and_header_once() {
    let producer = ProducerIdentity::from_codegen("kernel", Some(Path::new("source.rs"))).unwrap();
    let mut heap = LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
        max_bytes: None,
        max_items: 3,
    });
    charge(&producer, &mut heap).unwrap();
    let prefix = 17usize;
    let total = prefix + size_of::<ProducerIdentity>() + heap.bytes();
    let run = |max_bytes, max_items| {
        let mut counter = LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
            max_bytes: Some(max_bytes),
            max_items,
        });
        counter.charge(prefix + size_of::<ProducerIdentity>(), 2)?;
        charge(&producer, &mut counter)?;
        Ok::<_, LogicalStorageErrorV1>((counter.bytes(), counter.items()))
    };
    assert_eq!(run(total, 5), Ok((total, 5)));
    assert_eq!(run(total - 1, 5), Err(LogicalStorageErrorV1::ByteLimit));
    assert_eq!(run(total, 4), Err(LogicalStorageErrorV1::ItemLimit));
}
